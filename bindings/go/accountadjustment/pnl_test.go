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

package accountadjustment

import (
	"errors"
	"testing"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
)

func TestNewPnlHaltedOutcomeAcceptsEveryHaltReason(t *testing.T) {
	t.Parallel()

	reasons := []model.PnlHaltReason{
		model.PnlHaltReasonMissingFx,
		model.PnlHaltReasonMissingAccountCurrency,
		model.PnlHaltReasonMissingInitialPnl,
		model.PnlHaltReasonMissingCostBasis,
		model.PnlHaltReasonArithmeticOverflow,
	}
	for _, reason := range reasons {
		outcome, err := NewPnlHaltedOutcome(reason)
		if err != nil {
			t.Fatalf("NewPnlHaltedOutcome(%d) error = %v", reason, err)
		}
		if !outcome.IsHalted() {
			t.Fatalf("NewPnlHaltedOutcome(%d).IsHalted() = false", reason)
		}
		if got := reason.String(); got == "" {
			t.Fatalf("PnlHaltReason(%d).String() is empty", reason)
		}
		got, ok := outcome.HaltReason()
		if !ok || got != reason {
			t.Fatalf("HaltReason() = (%d, %v), want (%d, true)", got, ok, reason)
		}
		if _, ok := outcome.Amount(); ok {
			t.Fatalf("NewPnlHaltedOutcome(%d).Amount() is authoritative", reason)
		}
	}
}

func TestNewPnlHaltedOutcomeRejectsNoneAndUnknown(t *testing.T) {
	t.Parallel()

	for _, reason := range []model.PnlHaltReason{0, 255} {
		_, err := NewPnlHaltedOutcome(reason)
		if !errors.Is(err, model.ErrInvalidPnlHaltReason) {
			t.Fatalf("NewPnlHaltedOutcome(%d) error = %v", reason, err)
		}
		want := "PnlHaltReason(0)"
		if reason == 255 {
			want = "PnlHaltReason(255)"
		}
		if got := reason.String(); got != want {
			t.Fatalf("PnlHaltReason(%d).String() = %q, want %q", reason, got, want)
		}
	}
}

func TestAccountPnlOutcomeEmbedsUnambiguousPnlResult(t *testing.T) {
	t.Parallel()

	delta, err := param.NewPnlFromString("1.25")
	if err != nil {
		t.Fatalf("NewPnlFromString(delta) error = %v", err)
	}
	absolute, err := param.NewPnlFromString("7.5")
	if err != nil {
		t.Fatalf("NewPnlFromString(absolute) error = %v", err)
	}
	account := param.NewAccountIDFromUint64(99224416)
	outcome := NewAccountPnlOutcome(
		7,
		account,
		PnlOutcomeAmount{Delta: delta, Absolute: absolute},
	)

	if outcome.PolicyGroupID != 7 || outcome.AccountID != account {
		t.Fatalf("account outcome identity = (%d, %v)", outcome.PolicyGroupID, outcome.AccountID)
	}
	got, ok := outcome.Amount()
	if !ok || !got.Delta.Equal(delta) || !got.Absolute.Equal(absolute) {
		t.Fatalf("Amount() = (%v, %v), want authoritative amount", got, ok)
	}
	if _, ok := outcome.HaltReason(); ok {
		t.Fatal("HaltReason() is set for an authoritative outcome")
	}
}
