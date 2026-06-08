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

package generator

import (
	"testing"

	"github.com/shopspring/decimal"
)

func dec(s string) decimal.Decimal { return decimal.RequireFromString(s) }

// TestBuyChargeAndSettlement walks the contract §2 happy path for a Buy:
// seed USD, reserve (held grows, available shrinks by q*p), then full-fill
// (held drops, underlying credited by q). Mirrors the engine integration test
// buy_limit_full_fill_reduces_settlement_and_credits_underlying.
func TestBuyChargeAndSettlement(t *testing.T) {
	t.Parallel()
	l := newLedger()

	// Seed 10000 USD (Absolute).
	if r := l.applyFunding("a", "USD", fundingAbsolute, dec("10000")); r.rejected {
		t.Fatalf("seed rejected: %v", r.reason)
	}

	// Buy 10 AAPL @ 200 -> charge 2000 USD.
	res := l.preTrade("a", SideBuy, "AAPL", "USD", dec("10"), dec("200"))
	if !res.accepted() {
		t.Fatalf("buy rejected: %v", res.reason)
	}
	if res.chargeAsset != "USD" || !res.chargeAmount.Equal(dec("2000")) {
		t.Fatalf("charge = %s %s, want USD 2000", res.chargeAsset, res.chargeAmount)
	}
	assertBal(t, l, "USD", "8000", "2000")

	// Full fill: held(USD) -= 2000; available(AAPL) += 10.
	sr, err := l.settleFullFill("a", SideBuy, "AAPL", "USD", dec("10"), dec("200"))
	if err != nil {
		t.Fatalf("settle: %v", err)
	}
	if !sr.creditAmount.Equal(dec("10")) {
		t.Fatalf("credit = %s, want 10", sr.creditAmount)
	}
	assertBal(t, l, "USD", "8000", "0")
	assertBal(t, l, "AAPL", "10", "0")
}

// TestSellChargeAndSettlement walks the Sell path: charge underlying q, then
// full-fill credits settlement q*p.
func TestSellChargeAndSettlement(t *testing.T) {
	t.Parallel()
	l := newLedger()

	// Give the account 10 AAPL available (Absolute on the underlying).
	l.applyFunding("a", "AAPL", fundingAbsolute, dec("10"))

	res := l.preTrade("a", SideSell, "AAPL", "USD", dec("4"), dec("150"))
	if !res.accepted() {
		t.Fatalf("sell rejected: %v", res.reason)
	}
	if res.chargeAsset != "AAPL" || !res.chargeAmount.Equal(dec("4")) {
		t.Fatalf("charge = %s %s, want AAPL 4", res.chargeAsset, res.chargeAmount)
	}
	assertBal(t, l, "AAPL", "6", "4")

	sr, err := l.settleFullFill("a", SideSell, "AAPL", "USD", dec("4"), dec("150"))
	if err != nil {
		t.Fatalf("settle: %v", err)
	}
	if !sr.creditAmount.Equal(dec("600")) { // 4*150
		t.Fatalf("credit = %s, want 600", sr.creditAmount)
	}
	assertBal(t, l, "AAPL", "6", "0")
	assertBal(t, l, "USD", "600", "0")
}

// TestInsufficientFunds confirms a charge above available is rejected and the
// slot is untouched.
func TestInsufficientFunds(t *testing.T) {
	t.Parallel()
	l := newLedger()
	l.applyFunding("a", "USD", fundingAbsolute, dec("100"))

	res := l.preTrade("a", SideBuy, "AAPL", "USD", dec("1"), dec("150")) // charge 150 > 100
	if res.accepted() {
		t.Fatal("expected reject")
	}
	if res.reason != RejectInsufficientFunds {
		t.Fatalf("reason = %q, want InsufficientFunds", res.reason)
	}
	assertBal(t, l, "USD", "100", "0") // unchanged
}

// TestMissingRecordRejectsAsInsufficientFunds mirrors the real engine: a buy on
// an unseeded settlement asset rejects with InsufficientFunds (available reads
// as zero), NOT a distinct not-configured reject.
func TestMissingRecordRejectsAsInsufficientFunds(t *testing.T) {
	t.Parallel()
	l := newLedger()

	res := l.preTrade("a", SideBuy, "AAPL", "USD", dec("1"), dec("1"))
	if res.accepted() || res.reason != RejectInsufficientFunds {
		t.Fatalf("got accepted=%v reason=%q, want reject InsufficientFunds", res.accepted(), res.reason)
	}
}

// TestPruneWhenZero confirms a slot that reaches zero on both legs is removed,
// matching the engine's remove_if_zero (so missing == all-zero).
func TestPruneWhenZero(t *testing.T) {
	t.Parallel()
	l := newLedger()
	l.applyFunding("a", "USD", fundingAbsolute, dec("200"))

	// Reserve the entire balance: available 0, held 200.
	l.preTrade("a", SideBuy, "AAPL", "USD", dec("1"), dec("200"))
	assertBal(t, l, "USD", "0", "200")

	// Settle it: held(USD) -> 0, available(USD) untouched (0). USD slot now all
	// zero and must be pruned.
	if _, err := l.settleFullFill("a", SideBuy, "AAPL", "USD", dec("1"), dec("200")); err != nil {
		t.Fatalf("settle: %v", err)
	}
	if _, ok := l.get("a", "USD"); ok {
		t.Fatal("zero USD slot should have been pruned")
	}
}

// TestFundingDeltaSemantics covers contract §2.4 Delta rules (engine-faithful):
// missing+Delta creates a zero slot and applies the delta; present+Delta adds;
// Delta that drives available negative is accepted (no non-negative guard).
func TestFundingDeltaSemantics(t *testing.T) {
	t.Parallel()
	l := newLedger()

	// Missing + Delta -> accepted; slot created with available = 0 + delta.
	if r := l.applyFunding("a", "USD", fundingDelta, dec("100")); r.rejected {
		t.Fatalf("missing+Delta: unexpected reject reason=%q", r.reason)
	}
	assertBal(t, l, "USD", "100", "0")

	if r := l.applyFunding("a", "USD", fundingDelta, dec("50")); r.rejected {
		t.Fatalf("present+Delta rejected: %v", r.reason)
	}
	assertBal(t, l, "USD", "150", "0")

	// Delta that drives available negative is accepted (engine allows it).
	if r := l.applyFunding("a", "USD", fundingDelta, dec("-1000")); r.rejected {
		t.Fatalf("negative-result Delta: unexpected reject reason=%q", r.reason)
	}
	assertBal(t, l, "USD", "-850", "0") // 150 + (-1000) = -850
}

// TestSettlementUnderflowIsError ensures settling without a matching held
// reservation surfaces an error rather than driving held negative.
func TestSettlementUnderflowIsError(t *testing.T) {
	t.Parallel()
	l := newLedger()
	if _, err := l.settleFullFill("a", SideBuy, "AAPL", "USD", dec("1"), dec("1")); err == nil {
		t.Fatal("expected settlement underflow error")
	}
}

func assertBal(t *testing.T, l *ledger, asset, wantAvail, wantHeld string) {
	t.Helper()
	const account = "a"
	h, _ := l.get(account, asset)
	if !h.available.Equal(dec(wantAvail)) {
		t.Fatalf("%s/%s available = %s, want %s", account, asset, h.available, wantAvail)
	}
	if !h.held.Equal(dec(wantHeld)) {
		t.Fatalf("%s/%s held = %s, want %s", account, asset, h.held, wantHeld)
	}
}
