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

	"go.openpit.dev/openpit/internal/native"
)

func TestAdjustmentAmountDelta(t *testing.T) {
	t.Parallel()

	value := newPositionSizeOrPanic(NewPositionSizeFromString("-3.5"))
	amount := NewDeltaAdjustmentAmount(value)

	if !amount.IsDelta() {
		t.Fatal("expected delta amount")
	}
	if amount.IsAbsolute() {
		t.Fatal("unexpected absolute amount")
	}
	if got := amount.MustDelta(); !got.Equal(value) {
		t.Fatalf("MustDelta() = %v, want %v", got, value)
	}
	if got := amount.String(); got != "delta: -3.5" {
		t.Fatalf("String() = %q, want %q", got, "delta: -3.5")
	}

	var chosenDelta, chosenAbsolute bool
	amount.Choose(
		func(pos PositionSize) {
			chosenDelta = pos.Equal(value)
		},
		func(PositionSize) {
			chosenAbsolute = true
		},
	)
	if !chosenDelta {
		t.Fatal("Choose() did not call delta branch")
	}
	if chosenAbsolute {
		t.Fatal("Choose() called absolute branch unexpectedly")
	}

	nativeAmount := amount.Native()
	roundTrip := NewAdjustmentAmountFromNative(nativeAmount)
	got, ok := roundTrip.Get()
	if !ok {
		t.Fatal("round-trip amount should be present")
	}
	if !got.IsDelta() {
		t.Fatal("round-trip amount should remain delta")
	}
}

func TestAdjustmentAmountAbsolute(t *testing.T) {
	t.Parallel()

	value := newPositionSizeOrPanic(NewPositionSizeFromString("12"))
	amount := NewAbsoluteAdjustmentAmount(value)

	if !amount.IsAbsolute() {
		t.Fatal("expected absolute amount")
	}
	if amount.IsDelta() {
		t.Fatal("unexpected delta amount")
	}
	if got := amount.MustAbsolute(); !got.Equal(value) {
		t.Fatalf("MustAbsolute() = %v, want %v", got, value)
	}
	if got := amount.String(); got != "absolute: 12" {
		t.Fatalf("String() = %q, want %q", got, "sz: 12")
	}
}

func TestAdjustmentAmountFromNativeUnset(t *testing.T) {
	t.Parallel()

	if got := NewAdjustmentAmountFromNative(native.ParamAdjustmentAmount{}); got.IsSet() {
		t.Fatal("unset native value should map to empty option")
	}
}

func TestAdjustmentAmountMustWrongVariantPanics(t *testing.T) {
	t.Parallel()

	amount := NewDeltaAdjustmentAmount(newPositionSizeOrPanic(NewPositionSizeFromString("1")))

	defer func() {
		if recover() == nil {
			t.Fatal("MustAbsolute() should panic for delta amount")
		}
	}()

	_ = amount.MustAbsolute()
}
