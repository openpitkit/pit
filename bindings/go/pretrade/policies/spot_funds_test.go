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

package policies_test

import (
	"errors"
	"strings"
	"testing"

	openpit "go.openpit.dev/openpit"
	"go.openpit.dev/openpit/configure"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade/policies"
)

func mustMarketDataService(t *testing.T) *marketdata.Service {
	t.Helper()
	eb := openpit.NewEngineBuilder().FullSync()
	service, err := eb.MarketData(marketdata.InfiniteTTL()).Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	return service
}

func mustAccountGroupID(t *testing.T, id uint32) param.AccountGroupID {
	t.Helper()
	g, err := param.NewAccountGroupIDFromUint32(id)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32(%d) error = %v", id, err)
	}
	return g
}

func TestSpotFundsBuilderLimitOnlyMode(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderWithMarketOrders(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 2000)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderWithMarketOrdersZeroSlippage(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 0)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderWithMarketOrdersMaxSlippageAccepted(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 10_000)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderBookTopWithInstrumentOverrides(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			PricingSource(policies.SpotFundsPricingSourceBookTop).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target:   policies.SpotFundsOverrideTargetInstrument{Instrument: marketdata.NewInstrumentIDFromUint64(1)},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](500)},
				},
				policies.SpotFundsOverrideEntry{
					Target:   policies.SpotFundsOverrideTargetInstrument{Instrument: marketdata.NewInstrumentIDFromUint64(2)},
					Override: policies.SpotFundsOverride{SlippageBps: optional.None[uint16]()},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderOverrideAccountScoped(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	account := param.NewAccountIDFromUint64(42)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target: policies.SpotFundsOverrideTargetInstrumentAccount{
						Instrument: marketdata.NewInstrumentIDFromUint64(1),
						AccountID:  account,
					},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](200)},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderOverrideGroupScoped(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	group := mustAccountGroupID(t, 7)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target: policies.SpotFundsOverrideTargetInstrumentAccountGroup{
						Instrument:     marketdata.NewInstrumentIDFromUint64(1),
						AccountGroupID: group,
					},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](300)},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderOverrideInstrumentOnly(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target:   policies.SpotFundsOverrideTargetInstrument{Instrument: marketdata.NewInstrumentIDFromUint64(1)},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](100)},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderPointerOverrideTargets(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	group := mustAccountGroupID(t, 7)
	entries := []policies.SpotFundsOverrideEntry{
		{
			Target: &policies.SpotFundsOverrideTargetInstrument{
				Instrument: marketdata.NewInstrumentIDFromUint64(1),
			},
			Override: policies.SpotFundsOverride{
				SlippageBps: optional.Some[uint16](100),
			},
		},
		{
			Target: &policies.SpotFundsOverrideTargetInstrumentAccount{
				Instrument: marketdata.NewInstrumentIDFromUint64(2),
				AccountID:  param.NewAccountIDFromUint64(42),
			},
			Override: policies.SpotFundsOverride{
				SlippageBps: optional.Some[uint16](200),
			},
		},
		{
			Target: &policies.SpotFundsOverrideTargetInstrumentAccountGroup{
				Instrument:     marketdata.NewInstrumentIDFromUint64(3),
				AccountGroupID: group,
			},
			Override: policies.SpotFundsOverride{
				SlippageBps: optional.Some[uint16](300),
			},
		},
	}

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(entries...),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderInvalidOverrideTargets(t *testing.T) {
	tests := []struct {
		name   string
		target policies.SpotFundsOverrideTarget
	}{
		{name: "nil interface"},
		{
			name:   "nil instrument pointer",
			target: (*policies.SpotFundsOverrideTargetInstrument)(nil),
		},
		{
			name:   "nil account pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccount)(nil),
		},
		{
			name: "nil account group pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccountGroup)(
				nil,
			),
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := openpit.NewEngineBuilder().NoSync().
				Builtin(policies.BuildSpotFunds().
					PolicyGroupID(0).
					Overrides(policies.SpotFundsOverrideEntry{
						Target: test.target,
					}),
				).Build()
			if err == nil {
				t.Fatal("Build() error = nil, want invalid target error")
			}
			if !strings.Contains(
				err.Error(),
				"spot funds override 0: target is nil",
			) {
				t.Fatalf("Build() error = %q, want indexed nil target error", err)
			}
		})
	}
}

func TestSpotFundsConfiguratorPointerOverrideTargets(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 1500)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	group := mustAccountGroupID(t, 7)
	err = engine.Configure().SpotFunds(
		policies.SpotFundsPolicyName,
		optional.None[uint16](),
		optional.None[policies.SpotFundsPricingSource](),
		[]policies.SpotFundsOverrideEntry{
			{
				Target: &policies.SpotFundsOverrideTargetInstrument{
					Instrument: marketdata.NewInstrumentIDFromUint64(1),
				},
			},
			{
				Target: &policies.SpotFundsOverrideTargetInstrumentAccount{
					Instrument: marketdata.NewInstrumentIDFromUint64(2),
					AccountID:  param.NewAccountIDFromUint64(42),
				},
			},
			{
				Target: &policies.SpotFundsOverrideTargetInstrumentAccountGroup{
					Instrument:     marketdata.NewInstrumentIDFromUint64(3),
					AccountGroupID: group,
				},
			},
		},
	)
	if err != nil {
		t.Fatalf("Configure().SpotFunds() error = %v", err)
	}
}

func TestSpotFundsConfiguratorInvalidOverrideTargets(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	tests := []struct {
		name   string
		target policies.SpotFundsOverrideTarget
	}{
		{name: "nil interface"},
		{
			name:   "nil instrument pointer",
			target: (*policies.SpotFundsOverrideTargetInstrument)(nil),
		},
		{
			name:   "nil account pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccount)(nil),
		},
		{
			name: "nil account group pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccountGroup)(
				nil,
			),
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := engine.Configure().SpotFunds(
				policies.SpotFundsPolicyName,
				optional.None[uint16](),
				optional.None[policies.SpotFundsPricingSource](),
				[]policies.SpotFundsOverrideEntry{{Target: test.target}},
			)
			if err == nil {
				t.Fatal(
					"Configure().SpotFunds() error = nil, want invalid target error",
				)
			}
			if !strings.Contains(
				err.Error(),
				"configure: spot funds override 0: target is nil",
			) {
				t.Fatalf(
					"Configure().SpotFunds() error = %q, want indexed nil target error",
					err,
				)
			}
		})
	}
}

// TestSpotFundsBuilderOverrideBothAccountAndGroupIsError is no longer
// representable at the type level: the target variants
// SpotFundsOverrideTargetInstrumentAccount and
// SpotFundsOverrideTargetInstrumentAccountGroup are mutually exclusive by
// construction, so this scenario cannot be expressed in Go.
// The C ABI still enforces mutual exclusion, but the Go API prevents it.

func TestSpotFundsLimitModeDefaultIsEnforce(t *testing.T) {
	var zero policies.SpotFundsLimitMode
	if zero != policies.SpotFundsLimitModeEnforce {
		t.Fatalf("zero SpotFundsLimitMode = %v, want Enforce", zero)
	}
}

func TestSpotFundsLimitModeNativeRoundTrip(t *testing.T) {
	tests := []struct {
		name string
		mode policies.SpotFundsLimitMode
		want native.PretradePoliciesSpotFundsLimitMode
	}{
		{"enforce", policies.SpotFundsLimitModeEnforce, native.PretradePoliciesSpotFundsLimitModeEnforce},
		{"track-only", policies.SpotFundsLimitModeTrackOnly, native.PretradePoliciesSpotFundsLimitModeTrackOnly},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := policies.NativeSpotFundsLimitMode(test.mode); got != test.want {
				t.Fatalf("NativeSpotFundsLimitMode(%v) = %v, want %v", test.mode, got, test.want)
			}
			if got := test.mode.Handle(); got != test.want {
				t.Fatalf("%v.Handle() = %v, want %v", test.mode, got, test.want)
			}
		})
	}
}

func TestSpotFundsLimitModeOptionPinsAndClears(t *testing.T) {
	mode, has := policies.NativeSpotFundsLimitModeOption(
		optional.Some(policies.SpotFundsLimitModeEnforce),
	)
	if !has {
		t.Fatal("Some(Enforce): hasMode = false, want true")
	}
	if mode != native.PretradePoliciesSpotFundsLimitModeEnforce {
		t.Fatalf("Some(Enforce): mode = %v, want Enforce", mode)
	}

	mode, has = policies.NativeSpotFundsLimitModeOption(
		optional.Some(policies.SpotFundsLimitModeTrackOnly),
	)
	if !has {
		t.Fatal("Some(TrackOnly): hasMode = false, want true")
	}
	if mode != native.PretradePoliciesSpotFundsLimitModeTrackOnly {
		t.Fatalf("Some(TrackOnly): mode = %v, want TrackOnly", mode)
	}

	_, has = policies.NativeSpotFundsLimitModeOption(
		optional.None[policies.SpotFundsLimitMode](),
	)
	if has {
		t.Fatal("None: hasMode = true, want false")
	}
}

func TestSpotFundsConfiguratorRejectsInvalidLimitMode(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	invalidMode := policies.SpotFundsLimitMode(99)

	assertConfigureValidationError(
		t,
		"SpotFundsGlobalLimitMode(invalid)",
		engine.Configure().SpotFundsGlobalLimitMode(
			policies.SpotFundsPolicyName,
			invalidMode,
		),
	)

	assertConfigureValidationError(
		t,
		"SpotFundsAccountLimitMode(Some(invalid))",
		engine.Configure().SpotFundsAccountLimitMode(
			policies.SpotFundsPolicyName,
			param.NewAccountIDFromUint64(77002),
			optional.Some(invalidMode),
		),
	)

	assertConfigureValidationError(
		t,
		"SpotFundsAccountGroupLimitMode(Some(invalid))",
		engine.Configure().SpotFundsAccountGroupLimitMode(
			policies.SpotFundsPolicyName,
			mustAccountGroupID(t, 43),
			optional.Some(invalidMode),
		),
	)
}

func assertConfigureValidationError(t *testing.T, label string, err error) {
	t.Helper()
	if err == nil {
		t.Fatalf("%s error = nil, want validation error", label)
	}

	var configErr *configure.Error
	if !errors.As(err, &configErr) {
		t.Fatalf("%s error = %T, want *configure.Error", label, err)
	}
	if configErr.Kind != configure.ErrorKindValidation {
		t.Fatalf("%s kind = %v, want Validation", label, configErr.Kind)
	}
}

func TestSpotFundsLimitModeString(t *testing.T) {
	if got := policies.SpotFundsLimitModeEnforce.String(); got != "enforce" {
		t.Fatalf("Enforce.String() = %q, want %q", got, "enforce")
	}
	if got := policies.SpotFundsLimitModeTrackOnly.String(); got != "track-only" {
		t.Fatalf("TrackOnly.String() = %q, want %q", got, "track-only")
	}
}

func TestSpotFundsBuilderGroupID(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().PolicyGroupID(7)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

// TestSpotFundsConfiguratorGlobalLimitModeRoundTrip exercises the dlsym
// dispatch path for SpotFundsGlobalLimitMode against a live engine: sets
// TrackOnly then restores Enforce.
func TestSpotFundsConfiguratorGlobalLimitModeRoundTrip(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	if err := engine.Configure().SpotFundsGlobalLimitMode(
		policies.SpotFundsPolicyName,
		policies.SpotFundsLimitModeTrackOnly,
	); err != nil {
		t.Fatalf("SpotFundsGlobalLimitMode(TrackOnly) error = %v", err)
	}
	if err := engine.Configure().SpotFundsGlobalLimitMode(
		policies.SpotFundsPolicyName,
		policies.SpotFundsLimitModeEnforce,
	); err != nil {
		t.Fatalf("SpotFundsGlobalLimitMode(Enforce) error = %v", err)
	}
}

// TestSpotFundsConfiguratorAccountLimitModeRoundTrip exercises the dlsym
// dispatch path for SpotFundsAccountLimitMode: pins an account to TrackOnly,
// then clears the override.
func TestSpotFundsConfiguratorAccountLimitModeRoundTrip(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(77001)

	if err := engine.Configure().SpotFundsAccountLimitMode(
		policies.SpotFundsPolicyName,
		accountID,
		optional.Some(policies.SpotFundsLimitModeTrackOnly),
	); err != nil {
		t.Fatalf("SpotFundsAccountLimitMode(Some(TrackOnly)) error = %v", err)
	}
	if err := engine.Configure().SpotFundsAccountLimitMode(
		policies.SpotFundsPolicyName,
		accountID,
		optional.None[policies.SpotFundsLimitMode](),
	); err != nil {
		t.Fatalf("SpotFundsAccountLimitMode(None) error = %v", err)
	}
}

// TestSpotFundsConfiguratorAccountGroupLimitModeRoundTrip exercises the dlsym
// dispatch path for SpotFundsAccountGroupLimitMode: pins a group to TrackOnly,
// then clears the override.
func TestSpotFundsConfiguratorAccountGroupLimitModeRoundTrip(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	groupID := mustAccountGroupID(t, 42)

	if err := engine.Configure().SpotFundsAccountGroupLimitMode(
		policies.SpotFundsPolicyName,
		groupID,
		optional.Some(policies.SpotFundsLimitModeTrackOnly),
	); err != nil {
		t.Fatalf("SpotFundsAccountGroupLimitMode(Some(TrackOnly)) error = %v", err)
	}
	if err := engine.Configure().SpotFundsAccountGroupLimitMode(
		policies.SpotFundsPolicyName,
		groupID,
		optional.None[policies.SpotFundsLimitMode](),
	); err != nil {
		t.Fatalf("SpotFundsAccountGroupLimitMode(None) error = %v", err)
	}
}

// TestSpotFundsFullEngineWithLocalMDServiceIsRejected verifies that a
// Full-sync engine builder rejects a no-sync market-data service
// with a descriptive mismatch error.
func TestSpotFundsFullEngineWithLocalMDServiceIsRejected(t *testing.T) {
	// Build a no-sync MD service: derive from a NoSync engine builder and do NOT
	// call FullSync on the MD builder.
	noSyncEB := openpit.NewEngineBuilder().NoSync()
	localService, err := noSyncEB.MarketData(marketdata.InfiniteTTL()).Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer localService.Close()

	// A Full-sync engine must reject the no-sync MD service.
	_, buildErr := openpit.NewEngineBuilder().FullSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(localService, 100)).
		Build()
	if buildErr == nil {
		t.Fatal("expected Build() to fail for Full engine + no-sync MD service, but it succeeded")
	}
}

// TestSpotFundsLocalEngineWithFullMDServiceIsAccepted verifies that a
// no-sync engine builder accepts a Full-sync market-data service.
func TestSpotFundsLocalEngineWithFullMDServiceIsAccepted(t *testing.T) {
	// Build a Full MD service: derive from a NoSync engine builder then upgrade
	// the MD builder to FullSync before building.
	noSyncEB := openpit.NewEngineBuilder().NoSync()
	fullService, err := noSyncEB.MarketData(marketdata.InfiniteTTL()).FullSync().Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer fullService.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(fullService, 100)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}
