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

func TestBuiltinRateLimitBrokerAxisHappyAndReject(t *testing.T) {
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarrier(policies.RateLimitBrokerBarrier{
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
			BrokerBarrier(policies.RateLimitBrokerBarrier{
				Limit: policies.RateLimit{MaxOrders: 1, Window: 60 * time.Second},
			}).
			AssetBarriers(policies.RateLimitAssetBarrier{
				Limit:           policies.RateLimit{MaxOrders: 5, Window: 60 * time.Second},
				SettlementAsset: usd,
			}).
			AccountBarriers(policies.RateLimitAccountBarrier{
				Limit:     policies.RateLimit{MaxOrders: 5, Window: 60 * time.Second},
				AccountID: param.NewAccountIDFromUint64(1001),
			}).
			AccountAssetBarriers(policies.RateLimitAccountAssetBarrier{
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
			BrokerBarrier(policies.RateLimitBrokerBarrier{
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
			BrokerBarrier(policies.RateLimitBrokerBarrier{
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
			BrokerBarrier(policies.RateLimitBrokerBarrier{
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
			BrokerBarrier(policies.RateLimitBrokerBarrier{
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
			BrokerBarrier(policies.RateLimitBrokerBarrier{
				Limit: policies.RateLimit{MaxOrders: 10, Window: time.Minute},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	err = engine.Configure().RateLimit(
		policies.RateLimitPolicyName,
		&policies.RateLimitBrokerBarrier{
			Limit: policies.RateLimit{MaxOrders: 1, Window: -time.Second},
		},
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

func TestConfigureRateLimitUpdateClearsBrokerBarrier(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(1001)
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarrier(policies.RateLimitBrokerBarrier{
				Limit: policies.RateLimit{MaxOrders: 1, Window: time.Minute},
			}).
			AccountBarriers(policies.RateLimitAccountBarrier{
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

	err = engine.Configure().RateLimitUpdate(
		policies.RateLimitPolicyName,
		optional.Some[*policies.RateLimitBrokerBarrier](nil),
		nil,
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf("Configure().RateLimitUpdate() error = %v", err)
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

// hugeOrderSizeLimit is a broker barrier large enough not to restrict any
// order in tests that focus on asset- or account-level barriers.
func hugeOrderSizeLimit(t *testing.T) policies.OrderSizeBrokerBarrier {
	t.Helper()
	return policies.OrderSizeBrokerBarrier{
		Limit: policies.OrderSizeLimit{
			MaxQuantity: orderSizeTestQty(t, "1000000"),
			MaxNotional: orderSizeTestVol(t, "1000000000"),
		},
	}
}

func TestConfigureOrderSizeLimitUpdateClearsBrokerBarrier(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(policies.OrderSizeBrokerBarrier{
				Limit: policies.OrderSizeLimit{
					MaxQuantity: orderSizeTestQty(t, "1"),
					MaxNotional: orderSizeTestVol(t, "1000000"),
				},
			}).
			AssetBarriers(policies.OrderSizeAssetBarrier{
				SettlementAsset: usd,
				Limit: policies.OrderSizeLimit{
					MaxQuantity: orderSizeTestQty(t, "10"),
					MaxNotional: orderSizeTestVol(t, "1000000"),
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

func TestBuiltinOrderSizeLimitAccountAssetOverridesAssetBaseline(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	acct := param.NewAccountIDFromUint64(1001)

	// Asset baseline: max qty 10. Account+asset override: max qty 5.
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(hugeOrderSizeLimit(t)).
			AssetBarriers(policies.OrderSizeAssetBarrier{
				SettlementAsset: usd,
				Limit: policies.OrderSizeLimit{
					MaxQuantity: orderSizeTestQty(t, "10"),
					MaxNotional: orderSizeTestVol(t, "10000"),
				},
			}).
			AccountAssetBarriers(policies.OrderSizeAccountAssetBarrier{
				AccountID:       acct,
				SettlementAsset: usd,
				Limit: policies.OrderSizeLimit{
					MaxQuantity: orderSizeTestQty(t, "5"),
					MaxNotional: orderSizeTestVol(t, "10000"),
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

func TestBuiltinOrderSizeLimitUnknownSettlementPasses(t *testing.T) {
	usd := builtinTestAsset(t, "USD")

	// Only USD asset barrier configured; EUR is unknown and must pass.
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			BrokerBarrier(hugeOrderSizeLimit(t)).
			AssetBarriers(policies.OrderSizeAssetBarrier{
				SettlementAsset: usd,
				Limit: policies.OrderSizeLimit{
					MaxQuantity: orderSizeTestQty(t, "1"),
					MaxNotional: orderSizeTestVol(t, "1000"),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// EUR settlement: no asset barrier, must pass.
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

	// USD settlement, qty 2 > maxQty 1: must be rejected on qty.
	_, rejects, err = engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "2"),
	)
	if err != nil {
		t.Fatalf("USD order StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("USD order reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOrderQtyExceedsLimit {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeOrderQtyExceedsLimit,
		)
	}
}

func TestBuiltinOrderSizeLimitAssetOnlyBuildsAndRejects(t *testing.T) {
	usd := builtinTestAsset(t, "USD")
	maxQty := orderSizeTestQty(t, "10")
	maxNotional := orderSizeTestVol(t, "1000")

	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildOrderSizeLimit().
			AssetBarriers(policies.OrderSizeAssetBarrier{
				Limit: policies.OrderSizeLimit{
					MaxQuantity: maxQty,
					MaxNotional: maxNotional,
				},
				SettlementAsset: usd,
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v (asset-only must work)", err)
	}
	defer engine.Stop()

	// Order with qty 15 > maxQty 10: expected reject.
	_, rejects, err := engine.StartPreTrade(
		orderSizeTestOrder(t, 1, "USD", "15"),
	)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if len(rejects) == 0 {
		t.Fatal("expected reject for oversized order")
	}
	if rejects[0].Code != reject.CodeOrderExceedsLimit {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeOrderExceedsLimit,
		)
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
			BrokerBarrier(policies.RateLimitBrokerBarrier{
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
