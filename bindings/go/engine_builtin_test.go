// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

package openpit

import (
	"errors"
	"fmt"
	"strings"
	"testing"
	"time"

	"go.openpit.dev/openpit/configure"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

func TestBuiltinRateLimitReviewMode(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().Builtin(
		policies.BuildRateLimit().BrokerBarriers(policies.RateLimitBrokerBarrier{
			Mode:  reject.RuleModeReject,
			Limit: policies.RateLimit{MaxOrders: 2, Window: time.Hour},
		}).BrokerBarriers(policies.RateLimitBrokerBarrier{
			Mode:  reject.RuleModeReviewWithoutHold,
			Limit: policies.RateLimit{MaxOrders: 1, Window: time.Hour},
		}),
	).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
	order := rateLimitTestOrder(t, 1)
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, false)
	assertRateLimitOrder(t, engine, order, 1, reject.RuleModeReviewWithoutHold, false)
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, true)
}

func TestBuiltinRateLimitConfirmedSpendsBothThresholdsAndDropsReviewMarks(t *testing.T) {
	for _, mode := range []reject.RuleMode{reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
		t.Run(fmt.Sprint(mode), func(t *testing.T) {
			engine, err := NewEngineBuilder().NoSync().Builtin(
				policies.BuildRateLimit().BrokerBarriers(
					policies.RateLimitBrokerBarrier{
						Mode:  reject.RuleModeReject,
						Limit: policies.RateLimit{MaxOrders: 4, Window: time.Hour},
					},
					policies.RateLimitBrokerBarrier{
						Mode:  mode,
						Limit: policies.RateLimit{MaxOrders: 2, Window: time.Hour},
					},
				),
			).Build()
			if err != nil {
				t.Fatalf("Build() error = %v", err)
			}
			defer engine.Stop()
			order := rateLimitTestOrder(t, 1)
			assertRateLimitOrder(t, engine, order, 0, mode, false)

			confirmed := rateLimitTestOrder(t, 1)
			review := confirmed.EnsureReviewView()
			review.SetConfirmed(true)
			assertRateLimitOrder(t, engine, confirmed, 0, mode, false)

			assertRateLimitOrder(t, engine, order, 1, mode, false)
			assertRateLimitOrder(t, engine, confirmed, 0, mode, false)
			assertRateLimitOrder(t, engine, order, 0, mode, true)
		})
	}
}

func TestBuiltinRateLimitDuplicateKey(t *testing.T) {
	limit := policies.RateLimit{MaxOrders: 0, Window: time.Hour}
	broker := policies.RateLimitBrokerBarrier{Mode: reject.RuleModeReject, Limit: limit}
	asset := policies.RateLimitAssetBarrier{
		Mode: reject.RuleModeReject, Limit: limit, SettlementAsset: builtinTestAsset(t, "USD"),
	}
	for _, test := range []struct {
		name   string
		policy *policies.RateLimitReadyBuilder
	}{
		{"broker", policies.BuildRateLimit().BrokerBarriers(broker, broker)},
		{"asset", policies.BuildRateLimit().AssetBarriers(asset, asset)},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine, err := NewEngineBuilder().NoSync().Builtin(test.policy).Build()
			if engine != nil {
				engine.Stop()
			}
			if err == nil || !strings.Contains(err.Error(), "Reject") {
				t.Fatalf("Build() error = %v, want duplicate naming Reject", err)
			}
		})
	}
	reviewAsset := asset
	reviewAsset.Mode = reject.RuleModeReviewWithoutHold
	engine, err := NewEngineBuilder().NoSync().Builtin(
		policies.BuildRateLimit().AssetBarriers(asset, reviewAsset),
	).Build()
	if err != nil {
		t.Fatalf("different modes Build() error = %v", err)
	}
	defer engine.Stop()
	// A valid broker update must not leak through a duplicate on another axis.
	err = engine.Configure().RateLimit(policies.RateLimitPolicyName,
		[]policies.RateLimitBrokerBarrier{broker},
		[]policies.RateLimitAssetBarrier{reviewAsset, reviewAsset}, nil, nil)
	var configErr *configure.Error
	if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
		t.Fatalf("RateLimit() error = %v, want Validation", err)
	}
	// The old USD reject barrier is still present, with no broker barrier.
	order := orderSizeTestOrder(t, 1, "EUR", "1")
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, false)
	assertRateLimitOrder(t, engine, rateLimitTestOrder(t, 1), 0, reject.RuleModeReviewWithoutHold, true)
	err = engine.Configure().RateLimit(policies.RateLimitPolicyName,
		[]policies.RateLimitBrokerBarrier{broker, broker}, nil, nil, nil)
	if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
		t.Fatalf("duplicate brokers RateLimit() error = %v, want Validation", err)
	}
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, false)
}

func TestBuiltinRateLimitZeroModeIsReject(t *testing.T) {
	limit := policies.RateLimit{MaxOrders: 1, Window: time.Hour}
	asset := builtinTestAsset(t, "USD")
	account := param.NewAccountIDFromUint64(1)
	for _, test := range []struct {
		name   string
		policy *policies.RateLimitReadyBuilder
	}{
		{"broker", policies.BuildRateLimit().BrokerBarriers(
			policies.RateLimitBrokerBarrier{Limit: limit},
			policies.RateLimitBrokerBarrier{Limit: limit, Mode: reject.RuleModeReject},
		)},
		{"asset", policies.BuildRateLimit().AssetBarriers(
			policies.RateLimitAssetBarrier{Limit: limit, SettlementAsset: asset},
			policies.RateLimitAssetBarrier{Limit: limit, SettlementAsset: asset, Mode: reject.RuleModeReject},
		)},
		{"account", policies.BuildRateLimit().AccountBarriers(
			policies.RateLimitAccountBarrier{Limit: limit, AccountID: account},
			policies.RateLimitAccountBarrier{Limit: limit, AccountID: account, Mode: reject.RuleModeReject},
		)},
		{"account-asset", policies.BuildRateLimit().AccountAssetBarriers(
			policies.RateLimitAccountAssetBarrier{Limit: limit, AccountID: account, SettlementAsset: asset},
			policies.RateLimitAccountAssetBarrier{Limit: limit, AccountID: account, SettlementAsset: asset, Mode: reject.RuleModeReject},
		)},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine, err := NewEngineBuilder().NoSync().Builtin(test.policy).Build()
			if engine != nil {
				engine.Stop()
			}
			if err == nil || !strings.Contains(err.Error(), "duplicate") || !strings.Contains(err.Error(), "Reject") {
				t.Fatalf("Build() error = %v, want duplicate Reject for zero mode", err)
			}
		})
	}
}

func TestConfigureRateLimitZeroModeIsRejectAndDuplicateIsAtomic(t *testing.T) {
	limit := policies.RateLimit{MaxOrders: 100, Window: time.Hour}
	asset := builtinTestAsset(t, "USD")
	account := param.NewAccountIDFromUint64(1)
	for _, test := range []struct {
		name          string
		brokers       []policies.RateLimitBrokerBarrier
		assets        []policies.RateLimitAssetBarrier
		accounts      []policies.RateLimitAccountBarrier
		accountAssets []policies.RateLimitAccountAssetBarrier
	}{
		{name: "broker", brokers: []policies.RateLimitBrokerBarrier{
			{Limit: limit}, {Limit: limit, Mode: reject.RuleModeReject},
		}},
		{name: "asset", assets: []policies.RateLimitAssetBarrier{
			{Limit: limit, SettlementAsset: asset},
			{Limit: limit, SettlementAsset: asset, Mode: reject.RuleModeReject},
		}},
		{name: "account", accounts: []policies.RateLimitAccountBarrier{
			{Limit: limit, AccountID: account},
			{Limit: limit, AccountID: account, Mode: reject.RuleModeReject},
		}},
		{name: "account-asset", accountAssets: []policies.RateLimitAccountAssetBarrier{
			{Limit: limit, AccountID: account, SettlementAsset: asset},
			{Limit: limit, AccountID: account, SettlementAsset: asset, Mode: reject.RuleModeReject},
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine, err := NewEngineBuilder().NoSync().Builtin(
				policies.BuildRateLimit().BrokerBarriers(policies.RateLimitBrokerBarrier{
					Limit: policies.RateLimit{MaxOrders: 1, Window: time.Hour},
				}),
			).Build()
			if err != nil {
				t.Fatalf("Build() error = %v", err)
			}
			defer engine.Stop()
			order := rateLimitTestOrder(t, 1)
			assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReject, false)
			brokers := test.brokers
			if brokers == nil {
				brokers = []policies.RateLimitBrokerBarrier{{Limit: limit, Mode: reject.RuleModeReject}}
			}
			err = engine.Configure().RateLimit(policies.RateLimitPolicyName,
				brokers, test.assets, test.accounts, test.accountAssets)
			var configErr *configure.Error
			if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation ||
				!strings.Contains(configErr.Message, "duplicate") || !strings.Contains(configErr.Message, "Reject") {
				t.Fatalf("RateLimit() error = %v, want Validation naming duplicate Reject for zero mode", err)
			}
			assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReject, true)
		})
	}
}

func TestBuiltinRateLimitInvalidMode(t *testing.T) {
	for _, mode := range []reject.RuleMode{255} {
		t.Run(fmt.Sprint(mode), func(t *testing.T) {
			limit := policies.RateLimit{MaxOrders: 1, Window: time.Hour}
			brokers := []policies.RateLimitBrokerBarrier{{Mode: mode, Limit: limit}}
			assets := []policies.RateLimitAssetBarrier{{Mode: mode, Limit: limit, SettlementAsset: builtinTestAsset(t, "USD")}}
			accounts := []policies.RateLimitAccountBarrier{{Mode: mode, Limit: limit, AccountID: param.NewAccountIDFromUint64(1)}}
			accountAssets := []policies.RateLimitAccountAssetBarrier{{Mode: mode, Limit: limit, AccountID: param.NewAccountIDFromUint64(1), SettlementAsset: builtinTestAsset(t, "USD")}}
			for _, axis := range []string{"broker", "asset", "account", "account-asset"} {
				t.Run(axis, func(t *testing.T) {
					var bs []policies.RateLimitBrokerBarrier
					var as []policies.RateLimitAssetBarrier
					var ac []policies.RateLimitAccountBarrier
					var aa []policies.RateLimitAccountAssetBarrier
					switch axis {
					case "broker":
						bs = brokers
					case "asset":
						as = assets
					case "account":
						ac = accounts
					case "account-asset":
						aa = accountAssets
					}
					engine, err := NewEngineBuilder().NoSync().Builtin(
						policies.BuildRateLimit().BrokerBarriers(bs...).AssetBarriers(as...).AccountBarriers(ac...).AccountAssetBarriers(aa...),
					).Build()
					if engine != nil {
						engine.Stop()
					}
					message := fmt.Sprintf("%s[0] has invalid mode %d", strings.ReplaceAll(axis, "-", "_"), mode)
					if err == nil || !strings.Contains(err.Error(), message) {
						t.Errorf("Build() error = %v, want %q", err, message)
					}
					engine, err = NewEngineBuilder().NoSync().Builtin(policies.BuildRateLimit().BrokerBarriers()).Build()
					if err != nil {
						t.Fatalf("empty Build() error = %v", err)
					}
					defer engine.Stop()
					err = engine.Configure().RateLimit(policies.RateLimitPolicyName, bs, as, ac, aa)
					var configErr *configure.Error
					if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
						t.Fatalf("RateLimit() error = %v, want Validation", err)
					}
					if !strings.Contains(configErr.Message, message) {
						t.Errorf("RateLimit() error = %v, want %q", err, message)
					}
				})
			}
		})
	}
}

func TestConfigureRateLimitBrokerModes(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().Builtin(policies.BuildRateLimit().BrokerBarriers()).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
	barriers := []policies.RateLimitBrokerBarrier{
		{Mode: reject.RuleModeReject, Limit: policies.RateLimit{MaxOrders: 2, Window: time.Hour}},
		{Mode: reject.RuleModeReviewWithoutHold, Limit: policies.RateLimit{MaxOrders: 1, Window: time.Hour}},
	}
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName, barriers, nil, nil, nil); err != nil {
		t.Fatalf("RateLimit(set) error = %v", err)
	}
	order := rateLimitTestOrder(t, 1)
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, false)
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName, nil, nil, nil, nil); err != nil {
		t.Fatalf("RateLimit(nil) error = %v", err)
	}
	assertRateLimitOrder(t, engine, order, 1, reject.RuleModeReviewWithoutHold, false)
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, true)
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName, []policies.RateLimitBrokerBarrier{}, nil, nil, nil); err != nil {
		t.Fatalf("RateLimit(clear) error = %v", err)
	}
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, false)
	// Removing the reject mode must remove it while retaining the review counter.
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName, barriers, nil, nil, nil); err != nil {
		t.Fatalf("RateLimit(reset) error = %v", err)
	}
	assertRateLimitOrder(t, engine, order, 0, reject.RuleModeReviewWithoutHold, false)
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName, barriers[1:], nil, nil, nil); err != nil {
		t.Fatalf("RateLimit(replace) error = %v", err)
	}
	assertRateLimitOrder(t, engine, order, 1, reject.RuleModeReviewWithoutHold, false)
	assertRateLimitOrder(t, engine, order, 1, reject.RuleModeReviewWithoutHold, false)
}

func assertRateLimitOrder(t *testing.T, engine *Engine, order model.Order, wantReviews int, wantMode reject.RuleMode, wantReject bool) {
	t.Helper()
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if request != nil {
		defer request.Close()
	}
	if wantReject {
		if request != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeRateLimitExceeded || rejects[0].Mode != reject.RuleModeReject {
			t.Fatalf("StartPreTrade() rejects = %v, want RateLimitExceeded Reject", rejects)
		}
		return
	}
	if request == nil || len(rejects) != 0 {
		t.Fatalf("StartPreTrade() rejects = %v, want accepted request", rejects)
	}
	reviews, err := request.Reviews()
	if err != nil || len(reviews) != wantReviews {
		t.Fatalf("Reviews() = %v, %v, want %d marks", reviews, err, wantReviews)
	}
	for _, mark := range reviews {
		if mark.Code != reject.CodeRateLimitExceeded || mark.Mode != wantMode {
			t.Fatalf("review = %v, want RateLimitExceeded %v", mark, wantMode)
		}
	}
}

func TestBuiltinRateLimitBrokerAxisHappyAndReject(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 1, Window: 60 * time.Second},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, 1))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(rateLimitTestOrder(t, 1))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("second StartPreTrade() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeRateLimitExceeded,
		)
	}
	if rejects[0].Reason != "rate limit exceeded: broker barrier" {
		t.Fatalf(
			"reject reason = %q, want %q",
			rejects[0].Reason, "rate limit exceeded: broker barrier",
		)
	}
}

func TestBuiltinRateLimitAssetAxisHappyAndReject(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			AssetBarriers(policies.RateLimitAssetBarrier{
				Mode:            reject.RuleModeReject,
				Limit:           policies.RateLimit{MaxOrders: 1, Window: 60 * time.Second},
				SettlementAsset: usd,
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, 1))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(rateLimitTestOrder(t, 1))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("second StartPreTrade() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeRateLimitExceeded,
		)
	}
}

func TestBuiltinRateLimitAccountAxisHappyAndReject(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			AccountBarriers(policies.RateLimitAccountBarrier{
				Mode:      reject.RuleModeReject,
				Limit:     policies.RateLimit{MaxOrders: 1, Window: 60 * time.Second},
				AccountID: param.NewAccountIDFromUint64(1001),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("second StartPreTrade() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeRateLimitExceeded,
		)
	}

	request2, rejects2, err := engine.StartPreTrade(rateLimitTestOrder(t, 9999))
	if err != nil {
		t.Fatalf("other account StartPreTrade() error = %v", err)
	}
	if len(rejects2) != 0 {
		t.Fatalf("other account rejects = %v, want none", rejects2)
	}
	request2.Close()
}

func TestBuiltinRateLimitAccountAssetAxisHappyAndReject(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			AccountAssetBarriers(policies.RateLimitAccountAssetBarrier{
				Mode:            reject.RuleModeReject,
				Limit:           policies.RateLimit{MaxOrders: 1, Window: 60 * time.Second},
				AccountID:       param.NewAccountIDFromUint64(1001),
				SettlementAsset: usd,
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("second StartPreTrade() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeRateLimitExceeded,
		)
	}

	request2, rejects2, err := engine.StartPreTrade(rateLimitTestOrder(t, 9999))
	if err != nil {
		t.Fatalf("other account StartPreTrade() error = %v", err)
	}
	if len(rejects2) != 0 {
		t.Fatalf("other account rejects = %v, want none", rejects2)
	}
	request2.Close()
}

func TestBuiltinRateLimitCombinedAxesHappyAndReject(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 1, Window: 60 * time.Second},
			}).
			AssetBarriers(policies.RateLimitAssetBarrier{
				Mode:            reject.RuleModeReject,
				Limit:           policies.RateLimit{MaxOrders: 5, Window: 60 * time.Second},
				SettlementAsset: usd,
			}).
			AccountBarriers(policies.RateLimitAccountBarrier{
				Mode:      reject.RuleModeReject,
				Limit:     policies.RateLimit{MaxOrders: 5, Window: 60 * time.Second},
				AccountID: param.NewAccountIDFromUint64(1001),
			}).
			AccountAssetBarriers(policies.RateLimitAccountAssetBarrier{
				Mode:            reject.RuleModeReject,
				Limit:           policies.RateLimit{MaxOrders: 5, Window: 60 * time.Second},
				AccountID:       param.NewAccountIDFromUint64(1001),
				SettlementAsset: usd,
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("second StartPreTrade() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeRateLimitExceeded,
		)
	}
}

func TestBuiltinRateLimitWithFullSyncDoesNotPanic(t *testing.T) {
	engine, err := NewEngineBuilder().FullSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 10, Window: 60 * time.Second},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, 1))
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()
}

func TestBuiltinRateLimitZeroWindowReturnsError(t *testing.T) {
	_, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 1, Window: 0},
			}),
		).Build()
	if err == nil {
		t.Fatal("expected error for zero window, got nil")
	}
}

func TestBuiltinRateLimitNegativeWindowReturnsError(t *testing.T) {
	_, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 1, Window: -time.Second},
			}),
		).Build()
	if err == nil {
		t.Fatal("expected error for negative window, got nil")
	}
}

func TestBuiltinRateLimitSubMicrosecondWindowAccepted(t *testing.T) {
	_, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 1, Window: 100 * time.Nanosecond},
			}),
		).Build()
	if err != nil {
		t.Fatalf("sub-microsecond window must be accepted, got error: %v", err)
	}
}

func TestConfigureRateLimitRejectsNegativeWindow(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 10, Window: time.Minute},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	err = engine.Configure().RateLimit(
		policies.RateLimitPolicyName,
		[]policies.RateLimitBrokerBarrier{{
			Mode:  reject.RuleModeReject,
			Limit: policies.RateLimit{MaxOrders: 1, Window: -time.Second},
		}},
		nil,
		nil,
		nil,
	)
	if err == nil {
		t.Fatal("Configure().RateLimit() error = nil, want negative window error")
	}
	var configErr *configure.Error
	if !errors.As(err, &configErr) {
		t.Fatalf("Configure().RateLimit() error = %T, want *configure.Error", err)
	}
	if configErr.Kind != configure.ErrorKindValidation {
		t.Fatalf("Configure().RateLimit() error kind = %v, want Validation", configErr.Kind)
	}
}

func TestConfigureRateLimitClearsBrokerBarriers(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(1001)
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 1, Window: time.Minute},
			}).
			AccountBarriers(policies.RateLimitAccountBarrier{
				Mode:      reject.RuleModeReject,
				AccountID: accountID,
				Limit:     policies.RateLimit{MaxOrders: 10, Window: time.Minute},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 || rejects[0].Reason != "rate limit exceeded: broker barrier" {
		t.Fatalf("second StartPreTrade() rejects = %v, want broker reject", rejects)
	}

	err = engine.Configure().RateLimit(
		policies.RateLimitPolicyName,
		[]policies.RateLimitBrokerBarrier{},
		nil,
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf("Configure().RateLimit() error = %v", err)
	}

	request, rejects, err = engine.StartPreTrade(rateLimitTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("third StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("third StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()
}

func TestEmptyPolicyLifecycleRateLimit(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().AssetBarriers()).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	check := func(t *testing.T, order model.Order, wantReject bool) {
		t.Helper()
		request, rejects, err := engine.StartPreTrade(order)
		if err != nil {
			t.Fatalf("StartPreTrade() error = %v", err)
		}
		if request != nil {
			request.Close()
		}
		if wantReject {
			if len(rejects) != 1 || rejects[0].Code != reject.CodeRateLimitExceeded || request != nil {
				t.Fatalf("StartPreTrade() rejects = %v, want rate-limit reject", rejects)
			}
		} else if len(rejects) != 0 || request == nil {
			t.Fatalf("StartPreTrade() request = %v, rejects = %v, want accepted", request, rejects)
		}
	}
	check(t, model.NewOrder(), false)
	limit := policies.RateLimit{MaxOrders: 1, Window: time.Hour}
	for _, test := range []struct {
		name          string
		accountID     uint64
		settlement    string
		broker        []policies.RateLimitBrokerBarrier
		assets        []policies.RateLimitAssetBarrier
		accounts      []policies.RateLimitAccountBarrier
		accountAssets []policies.RateLimitAccountAssetBarrier
	}{
		{name: "broker", accountID: 1001, settlement: "USD",
			broker: []policies.RateLimitBrokerBarrier{{Mode: reject.RuleModeReject, Limit: limit}}},
		{name: "asset", accountID: 1002, settlement: "EUR",
			assets: []policies.RateLimitAssetBarrier{{Mode: reject.RuleModeReject, Limit: limit, SettlementAsset: builtinTestAsset(t, "EUR")}}},
		{name: "account", accountID: 1003, settlement: "GBP",
			accounts: []policies.RateLimitAccountBarrier{{Mode: reject.RuleModeReject, Limit: limit, AccountID: param.NewAccountIDFromUint64(1003)}}},
		{name: "account-asset", accountID: 1004, settlement: "JPY",
			accountAssets: []policies.RateLimitAccountAssetBarrier{{Mode: reject.RuleModeReject, Limit: limit, AccountID: param.NewAccountIDFromUint64(1004), SettlementAsset: builtinTestAsset(t, "JPY")}}},
	} {
		t.Run(test.name, func(t *testing.T) {
			order := orderSizeTestOrder(t, test.accountID, test.settlement, "1")
			set := func() {
				t.Helper()
				if err := engine.Configure().RateLimit(policies.RateLimitPolicyName,
					test.broker, test.assets, test.accounts, test.accountAssets); err != nil {
					t.Fatalf("Configure().RateLimit(set) error = %v", err)
				}
			}
			clearAxis := func() {
				t.Helper()
				var broker []policies.RateLimitBrokerBarrier
				var assets []policies.RateLimitAssetBarrier
				var accounts []policies.RateLimitAccountBarrier
				var accountAssets []policies.RateLimitAccountAssetBarrier
				if test.broker != nil {
					broker = []policies.RateLimitBrokerBarrier{}
				}
				if test.assets != nil {
					assets = []policies.RateLimitAssetBarrier{}
				}
				if test.accounts != nil {
					accounts = []policies.RateLimitAccountBarrier{}
				}
				if test.accountAssets != nil {
					accountAssets = []policies.RateLimitAccountAssetBarrier{}
				}
				if err := engine.Configure().RateLimit(policies.RateLimitPolicyName,
					broker, assets, accounts, accountAssets); err != nil {
					t.Fatalf("Configure().RateLimit(clear) error = %v", err)
				}
			}
			set()
			check(t, order, false)
			check(t, order, true)
			if err := engine.Configure().RateLimit(policies.RateLimitPolicyName, nil, nil, nil, nil); err != nil {
				t.Fatalf("Configure().RateLimit(untouched) error = %v", err)
			}
			check(t, order, true)
			clearAxis()
			check(t, order, false)
			check(t, order, false)
			set()
			if test.accounts == nil && test.accountAssets == nil {
				check(t, order, false)
			}
			check(t, order, true)
			clearAxis()
			check(t, model.NewOrder(), false)
		})
	}

	order := orderSizeTestOrder(t, 1005, "CHF", "1")
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName,
		[]policies.RateLimitBrokerBarrier{{Mode: reject.RuleModeReject, Limit: limit}},
		[]policies.RateLimitAssetBarrier{{Mode: reject.RuleModeReject, Limit: limit, SettlementAsset: builtinTestAsset(t, "CHF")}}, nil, nil); err != nil {
		t.Fatalf("Configure().RateLimit(two axes) error = %v", err)
	}
	check(t, order, false)
	check(t, order, true)
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName,
		[]policies.RateLimitBrokerBarrier{}, nil, nil, nil); err != nil {
		t.Fatalf("Configure().RateLimit(clear broker) error = %v", err)
	}
	check(t, order, true)
	if err := engine.Configure().RateLimit(policies.RateLimitPolicyName,
		nil, []policies.RateLimitAssetBarrier{}, nil, nil); err != nil {
		t.Fatalf("Configure().RateLimit(clear asset) error = %v", err)
	}
	check(t, order, false)
	check(t, order, false)
}

// hugeOrderSizeLimit is a broker barrier large enough not to restrict any
// order in tests that focus on asset- or account-level barriers.
func hugeOrderSizeLimit(t *testing.T) policies.OrderSizeBrokerBarrier {
	t.Helper()
	return policies.OrderSizeBrokerBarrier{
		Limit: policies.OrderSizeLimit{
			MaxQuantity: optional.Some(orderSizeTestQty(t, "1000000")),
			MaxNotional: optional.Some(orderSizeTestVol(t, "1000000000")),
		},
	}
}

func TestConfigureOrderSizeLimitUpdateClearsBrokerBarrier(t *testing.T) {
	aapl := builtinTestAsset(t, "AAPL")
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(policies.OrderSizeBrokerBarrier{
				Limit: policies.OrderSizeLimit{
					MaxQuantity: optional.Some(orderSizeTestQty(t, "1")),
				},
			}).
			AssetBarriers(policies.OrderSizeAssetBarrier{
				Asset: aapl,
				Limit: policies.OrderSizeLimit{
					MaxQuantity: optional.Some(orderSizeTestQty(t, "10")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	_, rejects, err := engine.StartPreTrade(orderSizeTestOrder(t, 1001, "USD", "2"))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) == 0 || rejects[0].Code != reject.CodeOrderQtyExceedsLimit {
		t.Fatalf("first StartPreTrade() rejects = %v, want broker qty reject", rejects)
	}

	err = engine.Configure().OrderSizeLimitUpdate(
		policies.OrderSizeLimitPolicyName,
		optional.Some[*policies.OrderSizeBrokerBarrier](nil),
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf("Configure().OrderSizeLimitUpdate() error = %v", err)
	}

	request, rejects, err := engine.StartPreTrade(orderSizeTestOrder(t, 1001, "USD", "2"))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("second StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()
}

func TestEmptyPolicyLifecycleOrderSizeLimit(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().AssetBarriers()).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	check := func(t *testing.T, order model.Order, wantCode reject.Code) {
		t.Helper()
		request, rejects, err := engine.StartPreTrade(order)
		if err != nil {
			t.Fatalf("StartPreTrade() error = %v", err)
		}
		if request != nil {
			request.Close()
		}
		if wantCode != 0 {
			if len(rejects) != 1 || rejects[0].Code != wantCode || request != nil {
				t.Fatalf("StartPreTrade() rejects = %v, want code %v", rejects, wantCode)
			}
		} else if len(rejects) != 0 || request == nil {
			t.Fatalf("StartPreTrade() request = %v, rejects = %v, want accepted", request, rejects)
		}
	}
	check(t, model.NewOrder(), 0)
	qtyLimit := policies.OrderSizeLimit{MaxQuantity: optional.Some(orderSizeTestQty(t, "1"))}
	notionalLimit := policies.OrderSizeLimit{MaxNotional: optional.Some(orderSizeTestVol(t, "1"))}
	for _, test := range []struct {
		name          string
		accountID     uint64
		settlement    string
		code          reject.Code
		broker        *policies.OrderSizeBrokerBarrier
		assets        []policies.OrderSizeAssetBarrier
		accountAssets []policies.OrderSizeAccountAssetBarrier
	}{
		{name: "broker", accountID: 2001, settlement: "USD", code: reject.CodeOrderQtyExceedsLimit,
			broker: &policies.OrderSizeBrokerBarrier{Limit: qtyLimit}},
		{name: "asset", accountID: 2002, settlement: "EUR", code: reject.CodeOrderNotionalExceedsLimit,
			assets: []policies.OrderSizeAssetBarrier{{Limit: notionalLimit, Asset: builtinTestAsset(t, "EUR")}}},
		{name: "account-asset", accountID: 2003, settlement: "GBP", code: reject.CodeOrderNotionalExceedsLimit,
			accountAssets: []policies.OrderSizeAccountAssetBarrier{{Limit: notionalLimit, AccountID: param.NewAccountIDFromUint64(2003), Asset: builtinTestAsset(t, "GBP")}}},
	} {
		t.Run(test.name, func(t *testing.T) {
			order := orderSizeTestOrder(t, test.accountID, test.settlement, "2")
			set := func() {
				t.Helper()
				if err := engine.Configure().OrderSizeLimitUpdate(policies.OrderSizeLimitPolicyName,
					optional.From(test.broker, test.broker != nil), test.assets, test.accountAssets); err != nil {
					t.Fatalf("Configure().OrderSizeLimitUpdate(set) error = %v", err)
				}
			}
			clearAxis := func() {
				t.Helper()
				broker := optional.None[*policies.OrderSizeBrokerBarrier]()
				var assets []policies.OrderSizeAssetBarrier
				var accountAssets []policies.OrderSizeAccountAssetBarrier
				if test.broker != nil {
					broker = optional.Some[*policies.OrderSizeBrokerBarrier](nil)
				}
				if test.assets != nil {
					assets = []policies.OrderSizeAssetBarrier{}
				}
				if test.accountAssets != nil {
					accountAssets = []policies.OrderSizeAccountAssetBarrier{}
				}
				if err := engine.Configure().OrderSizeLimitUpdate(policies.OrderSizeLimitPolicyName,
					broker, assets, accountAssets); err != nil {
					t.Fatalf("Configure().OrderSizeLimitUpdate(clear) error = %v", err)
				}
			}
			set()
			check(t, order, test.code)
			if err := engine.Configure().OrderSizeLimit(policies.OrderSizeLimitPolicyName, nil, nil, nil); err != nil {
				t.Fatalf("Configure().OrderSizeLimit(untouched) error = %v", err)
			}
			check(t, order, test.code)
			clearAxis()
			check(t, order, 0)
			check(t, order, 0)
			set()
			check(t, order, test.code)
			clearAxis()
			check(t, model.NewOrder(), 0)
		})
	}

	order := orderSizeTestOrder(t, 2004, "CHF", "2")
	if err := engine.Configure().OrderSizeLimit(policies.OrderSizeLimitPolicyName,
		&policies.OrderSizeBrokerBarrier{Limit: qtyLimit},
		[]policies.OrderSizeAssetBarrier{{Limit: notionalLimit, Asset: builtinTestAsset(t, "CHF")}}, nil); err != nil {
		t.Fatalf("Configure().OrderSizeLimit(two axes) error = %v", err)
	}
	check(t, order, reject.CodeOrderNotionalExceedsLimit)
	if err := engine.Configure().OrderSizeLimitUpdate(policies.OrderSizeLimitPolicyName,
		optional.Some[*policies.OrderSizeBrokerBarrier](nil), nil, nil); err != nil {
		t.Fatalf("Configure().OrderSizeLimitUpdate(clear broker) error = %v", err)
	}
	check(t, order, reject.CodeOrderNotionalExceedsLimit)
	if err := engine.Configure().OrderSizeLimitUpdate(policies.OrderSizeLimitPolicyName,
		optional.None[*policies.OrderSizeBrokerBarrier](), []policies.OrderSizeAssetBarrier{}, nil); err != nil {
		t.Fatalf("Configure().OrderSizeLimitUpdate(clear asset) error = %v", err)
	}
	check(t, order, 0)
	check(t, order, 0)
}

func TestConfigureOrderSizeLimitPreservesOptionalNotionalCap(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(policies.OrderSizeBrokerBarrier{
				Limit: policies.OrderSizeLimit{
					MaxQuantity: optional.Some(orderSizeTestQty(t, "100")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	err = engine.Configure().OrderSizeLimit(
		policies.OrderSizeLimitPolicyName,
		&policies.OrderSizeBrokerBarrier{
			Limit: policies.OrderSizeLimit{
				MaxNotional: optional.Some(orderSizeTestVol(t, "1000")),
			},
		},
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf("Configure().OrderSizeLimit() error = %v", err)
	}

	request, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1001, "USD", "10"),
	)
	if err != nil {
		t.Fatalf("boundary StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("boundary rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(
		orderSizeTestOrder(t, 1001, "USD", "11"),
	)
	if err != nil {
		t.Fatalf("above-cap StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeOrderNotionalExceedsLimit {
		t.Fatalf("above-cap rejects = %v, want notional reject", rejects)
	}

	err = engine.Configure().OrderSizeLimit(
		policies.OrderSizeLimitPolicyName,
		&policies.OrderSizeBrokerBarrier{
			Limit: policies.OrderSizeLimit{
				MaxNotional: optional.Some(orderSizeTestVol(t, "0")),
			},
		},
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf("zero-cap Configure().OrderSizeLimit() error = %v", err)
	}

	_, rejects, err = engine.StartPreTrade(
		orderSizeTestOrder(t, 1001, "USD", "1"),
	)
	if err != nil {
		t.Fatalf("zero-cap StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeOrderNotionalExceedsLimit {
		t.Fatalf("zero-cap rejects = %v, want notional reject", rejects)
	}
}

func TestConfigureOrderSizeLimitReportsCoreValidationErrors(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(policies.OrderSizeBrokerBarrier{
				Limit: policies.OrderSizeLimit{
					MaxQuantity: optional.Some(orderSizeTestQty(t, "100")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	err = engine.Configure().OrderSizeLimit(
		policies.OrderSizeLimitPolicyName,
		&policies.OrderSizeBrokerBarrier{},
		nil,
		nil,
	)
	var configErr *configure.Error
	wantMessage := "policy OrderSizeLimitPolicy rejected the update: " +
		"at least one of max_quantity or max_notional must be configured"
	if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation ||
		configErr.Message != wantMessage {
		t.Fatalf("capless Configure().OrderSizeLimit() error = %v", err)
	}

	aapl := builtinTestAsset(t, "AAPL")
	limit := policies.OrderSizeLimit{
		MaxQuantity: optional.Some(orderSizeTestQty(t, "10")),
	}
	err = engine.Configure().OrderSizeLimit(
		policies.OrderSizeLimitPolicyName,
		nil,
		[]policies.OrderSizeAssetBarrier{
			{Limit: limit, Asset: aapl},
			{Limit: limit, Asset: aapl},
		},
		nil,
	)
	wantMessage = "policy OrderSizeLimitPolicy rejected the update: " +
		"duplicate asset barrier for asset AAPL"
	if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation ||
		configErr.Message != wantMessage {
		t.Fatalf("duplicate Configure().OrderSizeLimit() error = %v", err)
	}
}

func TestBuiltinOrderSizeLimitAccountAssetOverridesAssetBaseline(t *testing.T) {
	aapl := builtinTestAsset(t, "AAPL")
	acct := param.NewAccountIDFromUint64(1001)

	// Asset baseline: max qty 10. Account+asset override: max qty 5.
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(hugeOrderSizeLimit(t)).
			AssetBarriers(policies.OrderSizeAssetBarrier{
				Asset: aapl,
				Limit: policies.OrderSizeLimit{
					MaxQuantity: optional.Some(orderSizeTestQty(t, "10")),
				},
			}).
			AccountAssetBarriers(policies.OrderSizeAccountAssetBarrier{
				AccountID: acct,
				Asset:     aapl,
				Limit: policies.OrderSizeLimit{
					MaxQuantity: optional.Some(orderSizeTestQty(t, "5")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// account 1001, qty 8: rejected (account+asset barrier max 5).
	request, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1001, "USD", "8"),
	)
	if err != nil {
		t.Fatalf("acct 1001 qty 8 StartPreTrade() error = %v", err)
	}
	if request != nil {
		request.Close()
		t.Fatal("acct 1001 qty 8: request != nil, want nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("acct 1001 qty 8: reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOrderQtyExceedsLimit {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeOrderQtyExceedsLimit,
		)
	}

	// account 9999, qty 8: passes (asset baseline max 10).
	request2, rejects2, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 9999, "USD", "8"),
	)
	if err != nil {
		t.Fatalf("acct 9999 qty 8 StartPreTrade() error = %v", err)
	}
	if len(rejects2) != 0 {
		t.Fatalf("acct 9999 qty 8: rejects = %v, want none", rejects2)
	}
	request2.Close()
}

func TestBuiltinOrderSizeLimitSettlementAssetControlsNotional(t *testing.T) {
	usd := builtinTestAsset(t, "USD")

	// A USD barrier does not constrain quantity, but it does constrain notional.
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(hugeOrderSizeLimit(t)).
			AssetBarriers(policies.OrderSizeAssetBarrier{
				Asset: usd,
				Limit: policies.OrderSizeLimit{
					MaxNotional: optional.Some(orderSizeTestVol(t, "100")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// EUR settlement: the USD barrier matches neither metric and must pass.
	request, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "EUR", "100"),
	)
	if err != nil {
		t.Fatalf("EUR order StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("EUR order rejects = %v, want none", rejects)
	}
	request.Close()

	// USD settlement: qty 2 is allowed, but notional 200 exceeds 100.
	_, rejects, err = engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "2"),
	)
	if err != nil {
		t.Fatalf("USD order StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("USD order reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOrderNotionalExceedsLimit {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeOrderNotionalExceedsLimit,
		)
	}
}

func TestBuiltinOrderSizeLimitCombinedRejectCode(t *testing.T) {
	aapl := builtinTestAsset(t, "AAPL")
	usd := builtinTestAsset(t, "USD")
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			AssetBarriers(
				policies.OrderSizeAssetBarrier{
					Asset: aapl,
					Limit: policies.OrderSizeLimit{
						MaxQuantity: optional.Some(orderSizeTestQty(t, "10")),
					},
				},
				policies.OrderSizeAssetBarrier{
					Asset: usd,
					Limit: policies.OrderSizeLimit{
						MaxNotional: optional.Some(orderSizeTestVol(t, "1000")),
					},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	_, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "15"),
	)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOrderExceedsLimit {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeOrderExceedsLimit,
		)
	}
}

func TestBuiltinOrderSizeLimitQuantityOnlyBoundaries(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(policies.OrderSizeBrokerBarrier{
				Limit: policies.OrderSizeLimit{
					MaxQuantity: optional.Some(orderSizeTestQty(t, "10")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	_, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "11"),
	)
	if err != nil {
		t.Fatalf("above-cap StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeOrderQtyExceedsLimit {
		t.Fatalf("above-cap rejects = %v, want quantity reject", rejects)
	}

	request, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "10"),
	)
	if err != nil {
		t.Fatalf("boundary StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("boundary rejects = %v, want none", rejects)
	}
	request.Close()
}

func TestBuiltinOrderSizeLimitNotionalOnlyBoundaries(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(policies.OrderSizeBrokerBarrier{
				Limit: policies.OrderSizeLimit{
					MaxNotional: optional.Some(orderSizeTestVol(t, "1000")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	_, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "11"),
	)
	if err != nil {
		t.Fatalf("above-cap StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeOrderNotionalExceedsLimit {
		t.Fatalf("above-cap rejects = %v, want notional reject", rejects)
	}

	request, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "10"),
	)
	if err != nil {
		t.Fatalf("boundary StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("boundary rejects = %v, want none", rejects)
	}
	request.Close()
}

func TestBuiltinOrderSizeLimitExplicitZeroCapsReject(t *testing.T) {
	tests := []struct {
		name     string
		asset    string
		limit    policies.OrderSizeLimit
		wantCode reject.Code
	}{
		{
			name:  "quantity",
			asset: "AAPL",
			limit: policies.OrderSizeLimit{
				MaxQuantity: optional.Some(orderSizeTestQty(t, "0")),
			},
			wantCode: reject.CodeOrderQtyExceedsLimit,
		},
		{
			name:  "notional",
			asset: "USD",
			limit: policies.OrderSizeLimit{
				MaxNotional: optional.Some(orderSizeTestVol(t, "0")),
			},
			wantCode: reject.CodeOrderNotionalExceedsLimit,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			engine, err := NewEngineBuilder().NoSync().
				Builtin(policies.BuildOrderSizeLimit().
					AssetBarriers(policies.OrderSizeAssetBarrier{
						Limit: test.limit,
						Asset: builtinTestAsset(t, test.asset),
					}),
				).Build()
			if err != nil {
				t.Fatalf("Build() error = %v", err)
			}
			defer engine.Stop()

			_, rejects, err := engine.StartPreTrade(
				orderSizeTestOrder(t, 1, "USD", "1"),
			)
			if err != nil {
				t.Fatalf("StartPreTrade() error = %v", err)
			}
			if len(rejects) != 1 || rejects[0].Code != test.wantCode {
				t.Fatalf("rejects = %v, want code %v", rejects, test.wantCode)
			}
		})
	}
}

func TestBuiltinOrderSizeLimitRejectsCaplessLimit(t *testing.T) {
	_, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(policies.OrderSizeBrokerBarrier{}),
		).Build()
	if err == nil || !strings.Contains(
		err.Error(),
		"at least one of max_quantity or max_notional must be configured",
	) {
		t.Fatalf("Build() error = %v, want capless-limit error", err)
	}
}

func TestBuiltinOrderSizeLimitRejectsDuplicateAssetKey(t *testing.T) {
	aapl := builtinTestAsset(t, "AAPL")
	limit := policies.OrderSizeLimit{
		MaxQuantity: optional.Some(orderSizeTestQty(t, "10")),
	}
	_, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			AssetBarriers(
				policies.OrderSizeAssetBarrier{Limit: limit, Asset: aapl},
				policies.OrderSizeAssetBarrier{Limit: limit, Asset: aapl},
			),
		).Build()
	if err == nil || !strings.Contains(
		err.Error(),
		"duplicate asset barrier for asset AAPL",
	) {
		t.Fatalf("Build() error = %v, want duplicate-asset error", err)
	}
}

func TestBuiltinPnlBoundsKillSwitchBuildValidation(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	rejectBarrier := policies.PnlBoundsBrokerBarrier{
		SettlementAsset: usd,
		Mode:            reject.RuleModeReject,
		LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
	}
	reviewBarrier := rejectBarrier
	reviewBarrier.Mode = reject.RuleModeReviewWithoutHold
	unknownMode := rejectBarrier
	unknownMode.Mode = reject.RuleMode(255)
	accountBarrier := func(barrier policies.PnlBoundsBrokerBarrier, seed string) policies.PnlBoundsAccountAssetBarrier {
		return policies.PnlBoundsAccountAssetBarrier{
			Barrier:    barrier,
			AccountID:  param.NewAccountIDFromUint64(1),
			InitialPnl: pnlTestPnl(t, seed),
		}
	}
	for _, test := range []struct {
		name     string
		brokers  []policies.PnlBoundsBrokerBarrier
		accounts []policies.PnlBoundsAccountAssetBarrier
		wantErr  string
	}{
		{name: "broker unknown mode", brokers: []policies.PnlBoundsBrokerBarrier{unknownMode}, wantErr: "mode"},
		{name: "account unknown mode", accounts: []policies.PnlBoundsAccountAssetBarrier{accountBarrier(unknownMode, "0")}, wantErr: "mode"},
		{name: "duplicate broker", brokers: []policies.PnlBoundsBrokerBarrier{rejectBarrier, rejectBarrier}, wantErr: "duplicate"},
		{name: "different broker modes", brokers: []policies.PnlBoundsBrokerBarrier{rejectBarrier, reviewBarrier}},
		{name: "duplicate account", accounts: []policies.PnlBoundsAccountAssetBarrier{accountBarrier(rejectBarrier, "0"), accountBarrier(rejectBarrier, "0")}, wantErr: "duplicate"},
		{name: "conflicting seeds", accounts: []policies.PnlBoundsAccountAssetBarrier{accountBarrier(rejectBarrier, "0"), accountBarrier(reviewBarrier, "1")}, wantErr: "initial"},
		{name: "equal seeds", accounts: []policies.PnlBoundsAccountAssetBarrier{accountBarrier(rejectBarrier, "0"), accountBarrier(reviewBarrier, "0")}},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine, err := NewEngineBuilder().NoSync().Builtin(
				policies.BuildPnlBoundsKillSwitch().BrokerBarriers(test.brokers...).
					AccountBarriers(test.accounts...),
			).Build()
			if err == nil {
				defer engine.Stop()
			}
			if test.wantErr != "" {
				if err == nil || !strings.Contains(strings.ToLower(err.Error()), test.wantErr) {
					t.Fatalf("Build() error = %v, want %q", err, test.wantErr)
				}
			} else if err != nil {
				t.Fatalf("Build() error = %v, want success", err)
			}
		})
	}
}

func TestBuiltinPnlBoundsKillSwitchPreTradeModes(t *testing.T) {
	for _, mode := range []reject.RuleMode{
		reject.RuleModeReject,
		reject.RuleModeReviewWithHold,
		reject.RuleModeReviewWithoutHold,
	} {
		for _, axis := range []string{"broker", "account"} {
			for _, source := range []string{"builder", "configurator"} {
				t.Run(fmt.Sprintf("%d/%s/%s", mode, axis, source), func(t *testing.T) {
					barrier := policies.PnlBoundsBrokerBarrier{
						SettlementAsset: builtinTestAsset(t, "USD"),
						Mode:            mode,
						LowerBound:      optional.Some(pnlTestPnl(t, "100")),
					}
					initial := barrier
					if source == "configurator" {
						initial.Mode = reject.RuleModeReject
						initial.LowerBound = optional.Some(pnlTestPnl(t, "-100"))
					}
					builder := policies.BuildPnlBoundsKillSwitch().BrokerBarriers()
					if axis == "broker" {
						builder.BrokerBarriers(initial)
					} else {
						builder.AccountBarriers(policies.PnlBoundsAccountAssetBarrier{
							Barrier:    initial,
							AccountID:  param.NewAccountIDFromUint64(1),
							InitialPnl: pnlTestPnl(t, "0"),
						})
					}
					engine, err := NewEngineBuilder().NoSync().Builtin(builder).Build()
					if err != nil {
						t.Fatalf("Build() error = %v", err)
					}
					defer engine.Stop()
					if source == "configurator" {
						var brokers []policies.PnlBoundsBrokerBarrier
						var accounts []policies.PnlBoundsAccountAssetBarrierUpdate
						if axis == "broker" {
							brokers = []policies.PnlBoundsBrokerBarrier{barrier}
						} else {
							accounts = []policies.PnlBoundsAccountAssetBarrierUpdate{{
								Barrier:   barrier,
								AccountID: param.NewAccountIDFromUint64(1),
							}}
						}
						if err := engine.Configure().PnlBoundsKillSwitch(
							policies.PnlBoundsKillSwitchPolicyName, brokers, accounts,
						); err != nil {
							t.Fatalf("PnlBoundsKillSwitch() error = %v", err)
						}
					}
					for attempt := 0; attempt < 2; attempt++ {
						request, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
						if err != nil {
							t.Fatalf("StartPreTrade() error = %v", err)
						}
						if mode == reject.RuleModeReject {
							wantCode := reject.CodePnlKillSwitchTriggered
							if attempt > 0 {
								wantCode = reject.CodeAccountBlocked
							}
							if len(rejects) != 1 || rejects[0].Code != wantCode {
								t.Fatalf("attempt %d rejects = %v, want code %v", attempt, rejects, wantCode)
							}
							continue
						}
						if len(rejects) != 0 {
							t.Fatalf("attempt %d rejects = %v, want no reject or account block", attempt, rejects)
						}
						reviews, err := request.Reviews()
						request.Close()
						if err != nil || len(reviews) != 1 || reviews[0].Mode != mode ||
							reviews[0].Code != reject.CodePnlKillSwitchTriggered {
							t.Fatalf("Reviews() = %v, %v, want one kill switch mark with mode %v", reviews, err, mode)
						}
					}
					if mode == reject.RuleModeReject {
						// Recover P&L to distinguish a persistent block from a new breach.
						if err := engine.Configure().SetAccountPnl(
							policies.PnlBoundsKillSwitchPolicyName,
							param.NewAccountIDFromUint64(1), barrier.SettlementAsset,
							pnlTestPnl(t, "200"),
						); err != nil {
							t.Fatalf("SetAccountPnl() error = %v", err)
						}
						_, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
						if err != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeAccountBlocked {
							t.Fatalf("recovered account rejects = %v, %v, want persistent block", rejects, err)
						}
					}
				})
			}
		}
	}
}

func TestBuiltinPnlBoundsKillSwitchExecutionReportModes(t *testing.T) {
	for _, mode := range []reject.RuleMode{
		reject.RuleModeReject,
		reject.RuleModeReviewWithHold,
		reject.RuleModeReviewWithoutHold,
	} {
		for _, axis := range []string{"broker", "account"} {
			t.Run(fmt.Sprintf("%d/%s", mode, axis), func(t *testing.T) {
				barrier := policies.PnlBoundsBrokerBarrier{
					SettlementAsset: builtinTestAsset(t, "USD"),
					Mode:            mode,
					LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
				}
				builder := policies.BuildPnlBoundsKillSwitch().BrokerBarriers()
				if axis == "broker" {
					builder.BrokerBarriers(barrier)
				} else {
					builder.AccountBarriers(policies.PnlBoundsAccountAssetBarrier{
						Barrier:    barrier,
						AccountID:  param.NewAccountIDFromUint64(1),
						InitialPnl: pnlTestPnl(t, "0"),
					})
				}
				engine, err := NewEngineBuilder().NoSync().Builtin(builder).Build()
				if err != nil {
					t.Fatalf("Build() error = %v", err)
				}
				defer engine.Stop()
				result, err := engine.ApplyExecutionReport(pnlTestReport(t, 1, "AAPL", "USD", "-200"))
				if err != nil {
					t.Fatalf("ApplyExecutionReport() error = %v", err)
				}
				wantBlocks := 0
				if mode == reject.RuleModeReject {
					wantBlocks = 1
				}
				if len(result.AccountBlocks) != wantBlocks {
					t.Fatalf("AccountBlocks = %v, want %d", result.AccountBlocks, wantBlocks)
				}
				if err := engine.Configure().SetAccountPnl(
					policies.PnlBoundsKillSwitchPolicyName,
					param.NewAccountIDFromUint64(1), barrier.SettlementAsset,
					pnlTestPnl(t, "0"),
				); err != nil {
					t.Fatalf("SetAccountPnl() error = %v", err)
				}
				request, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
				if err != nil || len(rejects) != wantBlocks {
					t.Fatalf("rejects after P&L recovery = %v, %v, want %d", rejects, err, wantBlocks)
				}
				if mode == reject.RuleModeReject && rejects[0].Code != reject.CodePnlKillSwitchTriggered {
					t.Fatalf("rejects after P&L recovery = %v, want latched kill-switch cause", rejects)
				}
				if request != nil {
					request.Close()
				}
			})
		}
	}
}

func TestBuiltinPnlBoundsKillSwitchReviewEvaluationFailures(t *testing.T) {
	for _, mode := range []reject.RuleMode{
		reject.RuleModeReviewWithHold,
		reject.RuleModeReviewWithoutHold,
	} {
		t.Run(fmt.Sprint(mode), func(t *testing.T) {
			newEngine := func(t *testing.T) *Engine {
				t.Helper()
				builder := policies.BuildPnlBoundsKillSwitch().BrokerBarriers(policies.PnlBoundsBrokerBarrier{
					SettlementAsset: builtinTestAsset(t, "USD"),
					Mode:            mode,
					LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
				})
				engine, err := NewEngineBuilder().NoSync().Builtin(builder).Build()
				if err != nil {
					t.Fatalf("Build() error = %v", err)
				}
				t.Cleanup(engine.Stop)
				return engine
			}

			t.Run("missing instrument", func(t *testing.T) {
				engine := newEngine(t)
				order := pnlTestOrder(t, 1)
				operation := order.EnsureOperationView()
				operation.UnsetInstrument()
				request, rejects, err := engine.StartPreTrade(order)
				if err != nil {
					t.Fatalf("StartPreTrade() error = %v", err)
				}
				if request != nil {
					request.Close()
					t.Fatal("StartPreTrade() returned a request, want a hard reject")
				}
				if len(rejects) != 1 || rejects[0].Code != reject.CodeMissingRequiredField ||
					rejects[0].Mode != reject.RuleModeReject || rejects[0].Policy != policies.PnlBoundsKillSwitchPolicyName {
					t.Fatalf("StartPreTrade() rejects = %v, want kill-switch MissingRequiredField reject", rejects)
				}
			})

			t.Run("missing fee", func(t *testing.T) {
				engine := newEngine(t)
				report := pnlTestReport(t, 1, "AAPL", "USD", "-200")
				impact := report.EnsureFinancialImpactView()
				impact.UnsetFee()
				result, err := engine.ApplyExecutionReport(report)
				if err != nil {
					t.Fatalf("ApplyExecutionReport() error = %v", err)
				}
				if len(result.AccountBlocks) != 1 || result.AccountBlocks[0].Code != reject.CodeMissingRequiredField {
					t.Fatalf("AccountBlocks = %v, want one MissingRequiredField block", result.AccountBlocks)
				}
				request, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
				if request != nil {
					request.Close()
				}
				if err != nil || request != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeMissingRequiredField {
					t.Fatalf("post-report rejects = %v, error = %v, want the recorded MissingRequiredField block", rejects, err)
				}
			})
		})
	}
}

func TestConfigurePnlBoundsKillSwitchValidationIsAtomic(t *testing.T) {
	for _, axis := range []string{"broker", "account"} {
		for _, invalid := range []string{"unknown mode", "duplicate"} {
			t.Run(axis+"/"+invalid, func(t *testing.T) {
				barrier := policies.PnlBoundsBrokerBarrier{
					SettlementAsset: builtinTestAsset(t, "USD"),
					Mode:            reject.RuleModeReject,
					LowerBound:      optional.Some(pnlTestPnl(t, "100")),
				}
				engine, err := NewEngineBuilder().NoSync().Builtin(
					policies.BuildPnlBoundsKillSwitch().BrokerBarriers(barrier),
				).Build()
				if err != nil {
					t.Fatalf("Build() error = %v", err)
				}
				defer engine.Stop()
				barrier.LowerBound = optional.Some(pnlTestPnl(t, "-100"))
				if invalid == "unknown mode" {
					barrier.Mode = reject.RuleMode(255)
				}
				brokers := []policies.PnlBoundsBrokerBarrier{barrier}
				var accounts []policies.PnlBoundsAccountAssetBarrierUpdate
				if axis == "account" {
					brokers = []policies.PnlBoundsBrokerBarrier{{
						SettlementAsset: builtinTestAsset(t, "USD"),
						Mode:            reject.RuleModeReject,
						LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
					}}
					accounts = []policies.PnlBoundsAccountAssetBarrierUpdate{{
						Barrier:   barrier,
						AccountID: param.NewAccountIDFromUint64(1),
					}}
				}
				if invalid == "duplicate" {
					if axis == "broker" {
						brokers = append(brokers, barrier)
					} else {
						duplicate := accounts[0]
						duplicate.Barrier = barrier
						accounts = append(accounts, duplicate)
					}
				}
				err = engine.Configure().PnlBoundsKillSwitch(
					policies.PnlBoundsKillSwitchPolicyName, brokers, accounts,
				)
				var configErr *configure.Error
				if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
					t.Fatalf("PnlBoundsKillSwitch() error = %v, want *configure.Error Validation", err)
				}
				// The Go native wrapper sends 4 for out-of-range modes.
				wantMessage := axis + "[0] mode is invalid: 4"
				if invalid == "duplicate" {
					wantMessage = "duplicate " + axis + " barrier"
				}
				if !strings.Contains(configErr.Message, wantMessage) {
					t.Fatalf("validation message = %q, want %q", configErr.Message, wantMessage)
				}
				_, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
				if err != nil || len(rejects) != 1 || rejects[0].Code != reject.CodePnlKillSwitchTriggered {
					t.Fatalf("after invalid update rejects = %v, %v, want previous barrier's reject", rejects, err)
				}
			})
		}
	}
}

func TestBuiltinPnlBoundsKillSwitchBrokerOnlyTriggersAndBlocksAccount(t *testing.T) {
	usd := builtinTestAsset(t, "USD")

	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildPnlBoundsKillSwitch().
			BrokerBarriers(policies.PnlBoundsBrokerBarrier{
				SettlementAsset: usd,
				LowerBound:      optional.Some(pnlTestPnl(t, "-500")),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	result, err := engine.ApplyExecutionReport(
		pnlTestReport(t, 1, "AAPL", "USD", "-600"),
	)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) == 0 || result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("AccountBlocks = %v, want kill switch block", result.AccountBlocks)
	}
	// Reason/details must survive list destruction: the block string views
	// borrow the native list's memory, so they must be copied before the list is
	// freed. A use-after-free here surfaces as garbage rather than these exact
	// strings. The account identifier must never leak into the block details.
	block := result.AccountBlocks[0]
	if block.Reason != "pnl kill switch triggered" {
		t.Fatalf("block reason = %q, want %q", block.Reason, "pnl kill switch triggered")
	}
	if !strings.Contains(block.Details, "settlement asset USD") ||
		strings.Contains(block.Details, "account 1") {
		t.Fatalf("block details = %q, want redacted realized-pnl summary", block.Details)
	}

	_, rejects, err = engine.StartPreTrade(pnlTestOrder(t, 1))
	if err != nil {
		t.Fatalf("post-kill StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("post-kill reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodePnlKillSwitchTriggered,
		)
	}
}

func TestBuiltinPnlBoundsKillSwitchAccountBarrierIndependentOfOtherAccounts(
	t *testing.T,
) {
	usd := builtinTestAsset(t, "USD")

	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildPnlBoundsKillSwitch().
			AccountBarriers(policies.PnlBoundsAccountAssetBarrier{
				Barrier: policies.PnlBoundsBrokerBarrier{
					SettlementAsset: usd,
					LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
				},
				AccountID:  param.NewAccountIDFromUint64(1001),
				InitialPnl: pnlTestPnl(t, "0"),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	result, err := engine.ApplyExecutionReport(
		pnlTestReport(t, 1001, "AAPL", "USD", "-200"),
	)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) == 0 || result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("AccountBlocks = %v, want kill switch block for account 1001", result.AccountBlocks)
	}

	_, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("acct 1001 StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("acct 1001 reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodePnlKillSwitchTriggered,
		)
	}

	request, rejects, err := engine.StartPreTrade(
		pnlTestOrder(t, 9999),
	)
	if err != nil {
		t.Fatalf("acct 9999 StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("acct 9999 rejects = %v, want none", rejects)
	}
	request.Close()
}

func TestBuiltinPnlBoundsKillSwitchRuntimeAccountBarrierPreservesPnl(
	t *testing.T,
) {
	usd := builtinTestAsset(t, "USD")
	accountID := param.NewAccountIDFromUint64(1001)

	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildPnlBoundsKillSwitch().
			AccountBarriers(policies.PnlBoundsAccountAssetBarrier{
				Barrier: policies.PnlBoundsBrokerBarrier{
					SettlementAsset: usd,
					LowerBound:      optional.Some(pnlTestPnl(t, "-1000")),
				},
				AccountID:  accountID,
				InitialPnl: pnlTestPnl(t, "-50"),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	err = engine.Configure().PnlBoundsKillSwitch(
		policies.PnlBoundsKillSwitchPolicyName,
		nil,
		[]policies.PnlBoundsAccountAssetBarrierUpdate{
			{
				Barrier: policies.PnlBoundsBrokerBarrier{
					SettlementAsset: usd,
					LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
				},
				AccountID: accountID,
			},
		},
	)
	if err != nil {
		t.Fatalf("Configure().PnlBoundsKillSwitch() error = %v", err)
	}

	result, err := engine.ApplyExecutionReport(
		pnlTestReport(t, 1001, "AAPL", "USD", "-60"),
	)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) == 0 ||
		result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"AccountBlocks = %v, want preserved P&L to trigger kill switch",
			result.AccountBlocks,
		)
	}
}

func TestBuiltinSetAccountPnlForcesBreachAndRejectsNextOrder(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	account := param.NewAccountIDFromUint64(1001)

	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildPnlBoundsKillSwitch().
			BrokerBarriers(policies.PnlBoundsBrokerBarrier{
				SettlementAsset: usd,
				LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// With no P&L history the order passes against the lower bound of -100.
	request, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	// Force the accumulator below the lower bound; this trips the kill switch.
	err = engine.Configure().SetAccountPnl(
		policies.PnlBoundsKillSwitchPolicyName,
		account,
		usd,
		pnlTestPnl(t, "-150"),
	)
	if err != nil {
		t.Fatalf("Configure().SetAccountPnl() error = %v", err)
	}

	_, rejects, err = engine.StartPreTrade(pnlTestOrder(t, 1001))
	if err != nil {
		t.Fatalf("post-force StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("post-force reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodePnlKillSwitchTriggered,
		)
	}
}

func TestBuiltinSetAccountPnlUnknownPolicyReturnsUnknown(t *testing.T) {
	usd := builtinTestAsset(t, "USD")

	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildPnlBoundsKillSwitch().
			BrokerBarriers(policies.PnlBoundsBrokerBarrier{
				SettlementAsset: usd,
				LowerBound:      optional.Some(pnlTestPnl(t, "-100")),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	err = engine.Configure().SetAccountPnl(
		"NoSuchPolicy",
		param.NewAccountIDFromUint64(1001),
		usd,
		pnlTestPnl(t, "-150"),
	)

	var configErr *configure.Error
	if !errors.As(err, &configErr) {
		t.Fatalf("SetAccountPnl(unknown) error = %v, want *configure.Error", err)
	}
	if configErr.Kind != configure.ErrorKindUnknown {
		t.Fatalf("error kind = %v, want %v", configErr.Kind, configure.ErrorKindUnknown)
	}
}

func TestBuiltinSetAccountPnlWrongPolicyTypeReturnsTypeMismatch(t *testing.T) {
	usd := builtinTestAsset(t, "USD")

	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarriers(policies.RateLimitBrokerBarrier{
				Mode:  reject.RuleModeReject,
				Limit: policies.RateLimit{MaxOrders: 5, Window: 60 * time.Second},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	err = engine.Configure().SetAccountPnl(
		policies.RateLimitPolicyName,
		param.NewAccountIDFromUint64(1001),
		usd,
		pnlTestPnl(t, "-150"),
	)

	var configErr *configure.Error
	if !errors.As(err, &configErr) {
		t.Fatalf("SetAccountPnl(wrong type) error = %v, want *configure.Error", err)
	}
	if configErr.Kind != configure.ErrorKindTypeMismatch {
		t.Fatalf("error kind = %v, want %v", configErr.Kind, configure.ErrorKindTypeMismatch)
	}
}

func TestBuiltinPnlBoundsKillSwitchBrokerBarrierRejectViaCheckPreTradeStart(
	t *testing.T,
) {
	usd := builtinTestAsset(t, "USD")
	// Lower bound > 0 means zero P&L is already below the lower bound.
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildPnlBoundsKillSwitch().
			BrokerBarriers(policies.PnlBoundsBrokerBarrier{
				SettlementAsset: usd,
				LowerBound:      optional.Some(pnlTestPnl(t, "100")),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	_, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodePnlKillSwitchTriggered,
		)
	}
	if rejects[0].Reason != "pnl kill switch triggered: broker barrier" {
		t.Fatalf(
			"reject reason = %q, want %q",
			rejects[0].Reason, "pnl kill switch triggered: broker barrier",
		)
	}
}

func builtinTestAsset(t *testing.T, symbol string) param.Asset {
	t.Helper()
	asset, err := param.NewAsset(symbol)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", symbol, err)
	}
	return asset
}

func rateLimitTestOrder(
	t *testing.T,
	accountID uint64,
) model.Order {
	t.Helper()
	underlying := builtinTestAsset(t, "AAPL")
	settlement := builtinTestAsset(t, "USD")
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(underlying, settlement))
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	op.SetSide(param.SideBuy)
	qty, err := param.NewQuantityFromString("1")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	price, err := param.NewPriceFromString("100")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	op.SetPrice(price)
	return order
}

func orderSizeTestQty(t *testing.T, s string) param.Quantity {
	t.Helper()
	v, err := param.NewQuantityFromString(s)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", s, err)
	}
	return v
}

func orderSizeTestVol(t *testing.T, s string) param.Volume {
	t.Helper()
	v, err := param.NewVolumeFromString(s)
	if err != nil {
		t.Fatalf("NewVolumeFromString(%q) error = %v", s, err)
	}
	return v
}

func orderSizeTestOrder(
	t *testing.T,
	accountID uint64,
	settlement, quantity string,
) model.Order {
	t.Helper()
	u := builtinTestAsset(t, "AAPL")
	s := builtinTestAsset(t, settlement)
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(u, s))
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	op.SetSide(param.SideBuy)
	qty, err := param.NewQuantityFromString(quantity)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", quantity, err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	p, err := param.NewPriceFromString("100")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	op.SetPrice(p)
	return order
}

func pnlTestPnl(t *testing.T, s string) param.Pnl {
	t.Helper()
	v, err := param.NewPnlFromString(s)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", s, err)
	}
	return v
}

func pnlTestOrder(
	t *testing.T,
	accountID uint64,
) model.Order {
	t.Helper()
	u := builtinTestAsset(t, "AAPL")
	s := builtinTestAsset(t, "USD")
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(u, s))
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	op.SetSide(param.SideBuy)
	qty, err := param.NewQuantityFromString("1")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	price, err := param.NewPriceFromString("100")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	op.SetPrice(price)
	return order
}

func pnlTestReport(
	t *testing.T,
	accountID uint64,
	underlying, settlement, pnlStr string,
) model.ExecutionReport {
	t.Helper()
	u := builtinTestAsset(t, underlying)
	s := builtinTestAsset(t, settlement)
	report := model.NewExecutionReport()
	op := model.NewExecutionReportOperation()
	op.SetInstrument(param.NewInstrument(u, s))
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	op.SetSide(param.SideBuy)
	report.SetOperation(op)
	pnl := pnlTestPnl(t, pnlStr)
	fee, err := param.NewFeeFromInt64(0)
	if err != nil {
		t.Fatalf("NewFeeFromInt64() error = %v", err)
	}
	impact := model.NewExecutionReportFinancialImpact()
	impact.SetPnl(pnl)
	impact.SetFee(fee)
	report.SetFinancialImpact(impact)
	return report
}
