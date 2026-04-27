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

func TestLeverageZeroValueUsesNotSetSentinel(t *testing.T) {
	t.Parallel()

	var leverage Leverage
	if leverage.Native() != native.ParamLeverageNotSet {
		t.Fatalf("zero leverage native = %d, want %d", leverage.Native(), native.ParamLeverageNotSet)
	}
	if leverage.IsSet() {
		t.Fatal("zero leverage should not be set")
	}
	if leverage.String() != "0" {
		t.Fatalf("zero leverage string = %q, want %q", leverage.String(), "0")
	}
}

func TestNewLeverageFromNativeRoundTrip(t *testing.T) {
	t.Parallel()

	leverage := NewLeverageFromNative(1005)
	if leverage.Native() != 1005 {
		t.Fatalf("Native() = %d, want %d", leverage.Native(), 1005)
	}
	if leverage.Raw() != 1005 {
		t.Fatalf("Raw() = %d, want %d", leverage.Raw(), 1005)
	}
	if got := leverage.Value(); got != 100.5 {
		t.Fatalf("Value() = %v, want %v", got, float32(100.5))
	}
}

func TestLeverageConstantsAreSourcedFromNative(t *testing.T) {
	t.Parallel()

	if LeverageScale != native.ParamLeverageScale {
		t.Fatalf("LeverageScale = %d, want %d", LeverageScale, native.ParamLeverageScale)
	}
	if LeverageMin != uint16(native.ParamLeverageMin) {
		t.Fatalf("LeverageMin = %d, want %d", LeverageMin, uint16(native.ParamLeverageMin))
	}
	if LeverageMax != uint16(native.ParamLeverageMax) {
		t.Fatalf("LeverageMax = %d, want %d", LeverageMax, uint16(native.ParamLeverageMax))
	}
	if LeverageStep != float32(native.ParamLeverageStep) {
		t.Fatalf("LeverageStep = %v, want %v", LeverageStep, float32(native.ParamLeverageStep))
	}
}

func TestNewLeverageOptionFromNative(t *testing.T) {
	t.Parallel()

	none := NewLeverageOptionFromNative(native.ParamLeverageNotSet)
	if none.IsSet() {
		t.Fatal("not-set native leverage should map to empty option")
	}

	some := NewLeverageOptionFromNative(11)
	value, ok := some.Get()
	if !ok {
		t.Fatal("set native leverage should map to present option")
	}
	if value.Native() != 11 {
		t.Fatalf("option value native = %d, want %d", value.Native(), 11)
	}
}

func TestNewLeverageFromIntEncodesFixedPoint(t *testing.T) {
	t.Parallel()

	leverage := NewLeverageFromInt(100)
	if leverage.Native() != 1000 {
		t.Fatalf("Native() = %d, want %d", leverage.Native(), 1000)
	}
	if got := leverage.String(); got != "100" {
		t.Fatalf("String() = %q, want %q", got, "100")
	}
}

func TestNewLeverageFromFloat32EncodesFixedPoint(t *testing.T) {
	t.Parallel()

	leverage := NewLeverageFromFloat32(100.5)
	if leverage.Native() != 1005 {
		t.Fatalf("Native() = %d, want %d", leverage.Native(), 1005)
	}
	if got := leverage.String(); got != "100.5" {
		t.Fatalf("String() = %q, want %q", got, "100.5")
	}
}

func TestLeverageCalculateMarginRequired(t *testing.T) {
	t.Parallel()

	leverage := NewLeverageFromFloat32(100)
	notional := newNotionalOrPanic(NewNotionalFromInt(1000))
	margin, err := leverage.CalculateMarginRequired(notional)
	if err != nil {
		t.Fatalf("CalculateMarginRequired() error = %v", err)
	}
	if margin.String() != "10" {
		t.Fatalf("CalculateMarginRequired() = %v, want %v", margin.String(), "10")
	}
}
