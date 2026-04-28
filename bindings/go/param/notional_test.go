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

package param

import (
	"testing"
)

func TestNewNotionalFromStringValid(t *testing.T) {
	t.Parallel()

	notional, err := NewNotionalFromString("10000")
	if err != nil {
		t.Fatalf("NewNotionalFromString failed: %v", err)
	}
	if got := notional.String(); got != "10000" {
		t.Fatalf("String() = %q, want %q", got, "10000")
	}
}

func TestNewNotionalFromStringRejectsNegative(t *testing.T) {
	t.Parallel()

	_, err := NewNotionalFromString("-1")
	if err == nil {
		t.Fatal("expected error for negative notional")
	}
}

func TestNotionalZeroIsZero(t *testing.T) {
	t.Parallel()

	if !NotionalZero.IsZero() {
		t.Fatal("NotionalZero.IsZero() must be true")
	}
}

func TestNotionalFromVolumeRoundtrip(t *testing.T) {
	t.Parallel()

	vol, err := NewVolumeFromString("9999.99")
	if err != nil {
		t.Fatalf("NewVolumeFromString failed: %v", err)
	}

	notional, err := NewNotionalFromVolume(vol)
	if err != nil {
		t.Fatalf("NewNotionalFromVolume failed: %v", err)
	}
	if notional.String() != vol.String() {
		t.Fatalf("notional = %q, want %q", notional.String(), vol.String())
	}

	back, err := notional.Volume()
	if err != nil {
		t.Fatalf("ToVolume failed: %v", err)
	}
	if back.String() != vol.String() {
		t.Fatalf("back = %q, want %q", back.String(), vol.String())
	}
}

func TestNotionalCalculateMarginRequired(t *testing.T) {
	t.Parallel()

	notional, err := NewNotionalFromString("10000")
	if err != nil {
		t.Fatalf("NewNotionalFromString failed: %v", err)
	}
	leverage := NewLeverageFromInt(100)

	margin, err := notional.CalculateMarginRequired(leverage)
	if err != nil {
		t.Fatalf("CalculateMarginRequired failed: %v", err)
	}
	if got := margin.String(); got != "100" {
		t.Fatalf("CalculateMarginRequired() = %q, want %q", got, "100")
	}
}

func TestNotionalCheckedAdd(t *testing.T) {
	t.Parallel()

	a, _ := NewNotionalFromString("1000")
	b, _ := NewNotionalFromString("500")

	result, err := a.CheckedAdd(b)
	if err != nil {
		t.Fatalf("CheckedAdd failed: %v", err)
	}
	if got := result.String(); got != "1500" {
		t.Fatalf("CheckedAdd() = %q, want %q", got, "1500")
	}
}

func TestNotionalCheckedSub(t *testing.T) {
	t.Parallel()

	a, _ := NewNotionalFromString("1000")
	b, _ := NewNotionalFromString("400")

	result, err := a.CheckedSub(b)
	if err != nil {
		t.Fatalf("CheckedSub failed: %v", err)
	}
	if got := result.String(); got != "600" {
		t.Fatalf("CheckedSub() = %q, want %q", got, "600")
	}
}

func TestNotionalCheckedSubRejectsUnderflow(t *testing.T) {
	t.Parallel()

	small, _ := NewNotionalFromString("100")
	large, _ := NewNotionalFromString("200")

	_, err := small.CheckedSub(large)
	if err == nil {
		t.Fatal("expected error for underflow")
	}
}

func TestNotionalCompare(t *testing.T) {
	t.Parallel()

	small, _ := NewNotionalFromString("100")
	large, _ := NewNotionalFromString("200")

	if small.Compare(large) >= 0 {
		t.Fatal("small.Compare(large) must be negative")
	}
	if large.Compare(small) <= 0 {
		t.Fatal("large.Compare(small) must be positive")
	}
	if small.Compare(small) != 0 { // nolint
		t.Fatal("same.Compare(same) must be 0")
	}
}
