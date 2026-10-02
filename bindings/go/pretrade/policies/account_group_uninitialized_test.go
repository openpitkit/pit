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
	"testing"

	openpit "go.openpit.dev/openpit"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade/policies"
)

// Handle 0 of an AccountGroupID legally names param.DefaultAccountGroup, the
// global currency tier every unassigned account inherits, so the engine cannot
// tell an unset group from a deliberate one. These tests pin that the group
// currency setters reject the unset value before it reaches the engine, and
// that param.DefaultAccountGroup keeps addressing the global tier. The effective
// currency is read back through the spot-funds P&L barrier currency check.

// newEURAccountBarrierEngine arms an EUR account barrier for every account, so
// an account's effective currency shows up as a currency-mismatch block once
// it is anything but EUR.
func newEURAccountBarrierEngine(
	t *testing.T,
	accounts ...param.AccountID,
) *openpit.Engine {
	t.Helper()
	barriers := make([]policies.SpotFundsPnlBoundsAccountBarrier, 0, len(accounts))
	for _, account := range accounts {
		barriers = append(barriers, policies.SpotFundsPnlBoundsAccountBarrier{
			AccountID: account,
			Barrier: policies.SpotFundsPnlBoundsBarrier{
				Currency:   mustAsset(t, "EUR"),
				LowerBound: optional.Some(mustPnl(t, "-100")),
			},
		})
	}
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().AccountBarriers(barriers...)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	return engine
}

// assertEffectiveCurrencyUSD checks whether account resolves to a USD effective
// currency, read back through the EUR account barrier: USD yields an exact
// mismatch block, no currency yields none.
func assertEffectiveCurrencyUSD(
	t *testing.T,
	engine *openpit.Engine,
	account param.AccountID,
	want bool,
) {
	t.Helper()
	result, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "0")),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	if !want {
		if len(result.AccountBlocks) != 0 {
			t.Fatalf("AccountBlocks = %v, want none for an account without currency", result.AccountBlocks)
		}
		return
	}
	if len(result.AccountBlocks) != 1 ||
		result.AccountBlocks[0].Details != "account currency USD, barrier currency EUR" {
		t.Fatalf("AccountBlocks = %v, want one USD/EUR mismatch block", result.AccountBlocks)
	}
}

func TestSetGroupCurrencyRejectsUninitializedAccountGroup(t *testing.T) {
	afterRejected := param.NewAccountIDFromUint64(86001)
	afterDefault := param.NewAccountIDFromUint64(86002)
	engine := newEURAccountBarrierEngine(t, afterRejected, afterDefault)
	usd := mustAsset(t, "USD")

	err := engine.Accounts().SetGroupCurrency(param.AccountGroupID{}, usd)
	if !errors.Is(err, param.ErrUninitializedAccountGroupID) {
		t.Fatalf("SetGroupCurrency() error = %v, want ErrUninitializedAccountGroupID", err)
	}
	assertEffectiveCurrencyUSD(t, engine, afterRejected, false)

	if err := engine.Accounts().SetGroupCurrency(param.DefaultAccountGroup, usd); err != nil {
		t.Fatalf("SetGroupCurrency(DefaultAccountGroup) error = %v", err)
	}
	assertEffectiveCurrencyUSD(t, engine, afterDefault, true)
}

func TestClearGroupCurrencyRejectsUninitializedAccountGroup(t *testing.T) {
	afterRejected := param.NewAccountIDFromUint64(86011)
	afterDefault := param.NewAccountIDFromUint64(86012)
	engine := newEURAccountBarrierEngine(t, afterRejected, afterDefault)

	if err := engine.Accounts().SetGroupCurrency(
		param.DefaultAccountGroup,
		mustAsset(t, "USD"),
	); err != nil {
		t.Fatalf("SetGroupCurrency(DefaultAccountGroup) error = %v", err)
	}

	err := engine.Accounts().ClearGroupCurrency(param.AccountGroupID{})
	if !errors.Is(err, param.ErrUninitializedAccountGroupID) {
		t.Fatalf("ClearGroupCurrency() error = %v, want ErrUninitializedAccountGroupID", err)
	}
	assertEffectiveCurrencyUSD(t, engine, afterRejected, true)

	if err := engine.Accounts().ClearGroupCurrency(param.DefaultAccountGroup); err != nil {
		t.Fatalf("ClearGroupCurrency(DefaultAccountGroup) error = %v", err)
	}
	assertEffectiveCurrencyUSD(t, engine, afterDefault, false)
}
