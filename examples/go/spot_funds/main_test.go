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
// Please see https://github.com/openpitkit and the OWNERS file for details.

package main

import (
	"testing"

	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/reject"
)

// TestSpotFundsReservationFlow drives the same shared helpers main() uses and
// asserts the three outcomes that make the example a lesson: the first buy is
// accepted (reserving funds), the second identical buy is rejected with
// InsufficientFunds (those funds are held), and the fill - carrying the first
// reservation's lock - settles without an account block.
func TestSpotFundsReservationFlow(t *testing.T) {
	account := param.NewAccountIDFromUint64(scenarioAccount)

	engine, err := buildEngine()
	if err != nil {
		t.Fatalf("buildEngine: %v", err)
	}
	defer engine.Stop()

	if err := seedFunds(engine, account, scenarioSeedFunds); err != nil {
		t.Fatalf("seedFunds: %v", err)
	}

	// Buy #1 must be accepted and yield a non-empty lock to carry to the fill.
	buy1, err := buildOrder(account)
	if err != nil {
		t.Fatalf("buildOrder buy #1: %v", err)
	}
	lock1, rejects, err := placeOrder(engine, buy1)
	if err != nil {
		t.Fatalf("placeOrder buy #1: %v", err)
	}
	if rejects != nil {
		t.Fatalf("buy #1 rejected: %s", describe(rejects))
	}
	if len(lock1) == 0 {
		t.Fatal("buy #1 accepted but produced no pre-trade lock")
	}

	// Buy #2 must be rejected with InsufficientFunds: 60000 is held by buy #1,
	// only 40000 is available, and the order needs 60000.
	buy2, err := buildOrder(account)
	if err != nil {
		t.Fatalf("buildOrder buy #2: %v", err)
	}
	lock2, rejects, err := placeOrder(engine, buy2)
	if err != nil {
		t.Fatalf("placeOrder buy #2: %v", err)
	}
	if lock2 != nil {
		t.Fatal("buy #2 was accepted; expected an InsufficientFunds reject")
	}
	if !containsCode(rejects, reject.CodeInsufficientFunds) {
		t.Fatalf("buy #2 reject codes = %v, want InsufficientFunds", rejects)
	}

	// The fill carries buy #1's lock, so SpotFunds settles that reservation;
	// a successful settlement produces no account block.
	fill, err := buildFillReport(account, lock1)
	if err != nil {
		t.Fatalf("buildFillReport: %v", err)
	}
	result, err := applyFill(engine, fill)
	if err != nil {
		t.Fatalf("applyFill: %v", err)
	}
	if len(result.AccountBlocks) > 0 {
		t.Fatalf("fill produced %d account block(s), want 0",
			len(result.AccountBlocks))
	}

	// After switching to track-only, an identical buy that needs 60000 against
	// the 40000 now available is accepted instead of rejected: the policy
	// records the overshoot rather than gating on insufficient funds.
	if err := enableTrackOnly(engine); err != nil {
		t.Fatalf("enableTrackOnly: %v", err)
	}
	buy3, err := buildOrder(account)
	if err != nil {
		t.Fatalf("buildOrder buy #3: %v", err)
	}
	lock3, rejects, err := placeOrder(engine, buy3)
	if err != nil {
		t.Fatalf("placeOrder buy #3: %v", err)
	}
	if lock3 == nil {
		t.Fatalf("buy #3 rejected in track-only: %s", describe(rejects))
	}
}
