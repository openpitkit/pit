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
	"math"
	"testing"
	"unsafe"

	"go.openpit.dev/openpit/internal/native"
)

const pnlCanonicalValue = "98.76"

func TestPnlFromString(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name      string
		input     string
		want      string
		wantError bool
	}{
		{name: "integer", input: "42", want: "42"},
		{name: "decimal", input: "42.5", want: "42.5"},
		{name: "positive-sign", input: "+42.5", want: "42.5"},
		{name: "negative-sign", input: "-42.5", want: "-42.5"},
		{name: "whitespace", input: " 42.5", wantError: true},
		{name: "invalid", input: "pnl", wantError: true},
	}

	for _, tt := range tests { //nolint:copyloopvar
		tt := tt //nolint:copyloopvar
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			value, err := NewPnlFromString(tt.input)
			if tt.wantError {
				if err == nil {
					t.Fatalf("NewPnlFromString(%q) error = nil, want non-nil", tt.input)
				}
				assertErrorContains(t, err, "invalid format")
				return
			}
			if err != nil {
				t.Fatalf("NewPnlFromString(%q) error = %v", tt.input, err)
			}
			if got := value.String(); got != tt.want {
				t.Fatalf("String() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestPnlFromIntAndUint(t *testing.T) {
	t.Parallel()

	intTests := []struct {
		name  string
		input int64
		want  string
	}{
		{name: "negative", input: -5, want: "-5"},
		{name: "zero", input: 0, want: "0"},
		{name: "positive", input: 7, want: "7"},
	}
	for _, tt := range intTests { //nolint:copyloopvar
		tt := tt //nolint:copyloopvar
		t.Run("int-"+tt.name, func(t *testing.T) {
			t.Parallel()

			value, err := NewPnlFromInt(tt.input)
			if err != nil {
				t.Fatalf("NewPnlFromInt(%d) error = %v", tt.input, err)
			}
			if got := value.String(); got != tt.want {
				t.Fatalf("String() = %q, want %q", got, tt.want)
			}
		})
	}

	uintTests := []struct {
		name  string
		input uint64
		want  string
	}{
		{name: "zero", input: 0, want: "0"},
		{name: "positive", input: 7, want: "7"},
	}
	for _, tt := range uintTests { //nolint:copyloopvar
		tt := tt //nolint:copyloopvar
		t.Run("uint-"+tt.name, func(t *testing.T) {
			t.Parallel()

			value, err := NewPnlFromUint(tt.input)
			if err != nil {
				t.Fatalf("NewPnlFromUint(%d) error = %v", tt.input, err)
			}
			if got := value.String(); got != tt.want {
				t.Fatalf("String() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestPnlFromFloat(t *testing.T) {
	t.Parallel()

	value, err := NewPnlFromFloat(12.5)
	if err != nil {
		t.Fatalf("NewPnlFromFloat(12.5) error = %v", err)
	}
	if got := value.String(); got != "12.5" {
		t.Fatalf("String() = %q, want %q", got, "12.5")
	}

	for _, input := range []float64{math.NaN(), math.Inf(1), math.Inf(-1)} { //nolint:copyloopvar
		input := input //nolint:copyloopvar
		t.Run("invalid", func(t *testing.T) {
			t.Parallel()

			_, gotErr := NewPnlFromFloat(input)
			if gotErr == nil {
				t.Fatalf("NewPnlFromFloat(%v) error = nil, want non-nil", input)
			}
		})
	}
}

func TestPnlFromDecimal(t *testing.T) {
	t.Parallel()

	source := mustDecimal(t, pnlCanonicalValue)
	value, err := NewPnlFromDecimal(source)
	if err != nil {
		t.Fatalf("NewPnlFromDecimal(%q) error = %v", source.String(), err)
	}
	if got := value.String(); got != pnlCanonicalValue {
		t.Fatalf("String() = %q, want %q", got, pnlCanonicalValue)
	}
}

func TestPnlRoundedConstructors(t *testing.T) {
	t.Parallel()

	type roundedCase struct {
		name     string
		strategy RoundingStrategy
		want     string
	}
	strategyCases := []roundedCase{
		{
			name:     "nearest-even",
			strategy: RoundingStrategyMidpointNearestEven,
			want:     "1.2",
		},
		{
			name:     "away-from-zero",
			strategy: RoundingStrategyMidpointAwayFromZero,
			want:     "1.3",
		},
		{name: "up", strategy: RoundingStrategyUp, want: "1.3"},
		{name: "down", strategy: RoundingStrategyDown, want: "1.2"},
		{name: "default", strategy: RoundingStrategyDefault, want: "1.2"},
		{name: "banker", strategy: RoundingStrategyBanker, want: "1.2"},
		{
			name:     "conservative-profit",
			strategy: RoundingStrategyConservativeProfit,
			want:     "1.2",
		},
		{
			name:     "conservative-loss",
			strategy: RoundingStrategyConservativeLoss,
			want:     "1.2",
		},
	}

	decimalInput := mustDecimal(t, "1.25")
	constructors := []struct {
		name string
		call func(strategy RoundingStrategy) (Pnl, error)
	}{
		{
			name: "string",
			call: func(strategy RoundingStrategy) (Pnl, error) {
				return NewPnlFromStringRounded("1.25", 1, strategy)
			},
		},
		{
			name: "float",
			call: func(strategy RoundingStrategy) (Pnl, error) {
				return NewPnlFromFloatRounded(1.25, 1, strategy)
			},
		},
		{
			name: "decimal",
			call: func(strategy RoundingStrategy) (Pnl, error) {
				return NewPnlFromDecimalRounded(decimalInput, 1, strategy)
			},
		},
	}

	for _, ctor := range constructors { //nolint:copyloopvar
		ctor := ctor //nolint:copyloopvar
		t.Run(ctor.name, func(t *testing.T) {
			t.Parallel()

			for _, tc := range strategyCases { //nolint:copyloopvar
				tc := tc //nolint:copyloopvar
				t.Run(tc.name, func(t *testing.T) {
					t.Parallel()

					value, err := ctor.call(tc.strategy)
					if err != nil {
						t.Fatalf("constructor error = %v", err)
					}
					if got := value.String(); got != tc.want {
						t.Fatalf("String() = %q, want %q", got, tc.want)
					}
				})
			}
		})
	}
}

func TestNewPnlOptionFromHandle(t *testing.T) {
	t.Parallel()

	unset := NewPnlOptionFromHandle(native.ParamPnlOptional{})
	if unset.IsSet() {
		t.Fatal("unset native pnl should map to empty option")
	}

	expected := mustPnlValue(t, pnlCanonicalValue)
	set := NewPnlOptionFromHandle(makeNativePnlOptional(expected))
	got, ok := set.Get()
	if !ok {
		t.Fatal("set native pnl should map to present option")
	}
	if !got.Equal(expected) {
		t.Fatalf("option value = %v, want %v", got, expected)
	}
}

func TestNewPnlFromFee(t *testing.T) {
	t.Parallel()

	fee := newFeeOrPanic(NewFeeFromString("1.25"))
	value := newPnlOrPanic(NewPnlFromFee(fee))
	if got := value.String(); got != "-1.25" {
		t.Fatalf("NewPnlFromFee() = %q, want %q", got, "-1.25")
	}
}

func TestPnlAccessors(t *testing.T) {
	t.Parallel()

	value := mustPnlValue(t, pnlCanonicalValue)
	wantDecimal := mustDecimal(t, pnlCanonicalValue)
	if got := value.Decimal(); !got.Equal(wantDecimal) {
		t.Fatalf("Decimal() = %s, want %s", got.String(), wantDecimal.String())
	}

	roundTrip := NewPnlFromHandle(value.Handle())
	if !roundTrip.Equal(value) {
		t.Fatalf("NewPnlFromHandle(Handle()) = %v, want %v", roundTrip, value)
	}

	if got := value.String(); got != pnlCanonicalValue {
		t.Fatalf("String() = %q, want %q", got, pnlCanonicalValue)
	}

	const epsilon = 1e-9
	wantFloat := 98.76
	if got := value.Float(); math.Abs(got-wantFloat) > epsilon {
		t.Fatalf("Float() = %.12f, want %.12f (eps=%g)", got, wantFloat, epsilon)
	}
}

func TestPnlIsZeroEqualCompare(t *testing.T) {
	t.Parallel()

	if !mustPnlValue(t, "0").IsZero() {
		t.Fatal("IsZero() = false, want true for zero")
	}
	if mustPnlValue(t, "1").IsZero() {
		t.Fatal("IsZero() = true, want false for non-zero")
	}

	one := mustPnlValue(t, "1.0")
	oneAlt := mustPnlValue(t, "1.00")
	if !one.Equal(oneAlt) {
		t.Fatal("Equal() must treat 1.0 and 1.00 as equal")
	}

	a := mustPnlValue(t, "-2")
	b := mustPnlValue(t, "0")
	c := mustPnlValue(t, "3")

	if a.Compare(a) != 0 { //nolint:gocritic
		t.Fatalf("reflexive compare = %d, want 0", a.Compare(a)) //nolint:gocritic
	}
	if !(a.Compare(b) < 0 && b.Compare(c) < 0 && a.Compare(c) < 0) {
		t.Fatal("transitive compare contract violated")
	}
	if a.Compare(b) != -b.Compare(a) {
		t.Fatal("antisymmetric compare contract violated")
	}
}

func TestPnlCheckedArithmeticHappyPaths(t *testing.T) {
	t.Parallel()

	value := mustPnlValue(t, "12.5")
	zero := mustPnlValue(t, "0")

	added, err := value.CheckedAdd(zero)
	if err != nil {
		t.Fatalf("CheckedAdd() error = %v", err)
	}
	if !added.Equal(value) {
		t.Fatalf("CheckedAdd() = %v, want %v", added, value)
	}

	subtracted, err := value.CheckedSub(zero)
	if err != nil {
		t.Fatalf("CheckedSub() error = %v", err)
	}
	if !subtracted.Equal(value) {
		t.Fatalf("CheckedSub() = %v, want %v", subtracted, value)
	}

	negated, err := value.CheckedNeg()
	if err != nil {
		t.Fatalf("CheckedNeg() error = %v", err)
	}
	if got := negated.String(); got != "-12.5" {
		t.Fatalf("CheckedNeg() = %q, want %q", got, "-12.5")
	}

	mulInt, err := value.CheckedMulInt(1)
	if err != nil {
		t.Fatalf("CheckedMulInt() error = %v", err)
	}
	if !mulInt.Equal(value) {
		t.Fatalf("CheckedMulInt() = %v, want %v", mulInt, value)
	}

	mulUint, err := value.CheckedMulUint(1)
	if err != nil {
		t.Fatalf("CheckedMulUint() error = %v", err)
	}
	if !mulUint.Equal(value) {
		t.Fatalf("CheckedMulUint() = %v, want %v", mulUint, value)
	}

	mulFloat, err := value.CheckedMulFloat(1.0)
	if err != nil {
		t.Fatalf("CheckedMulFloat() error = %v", err)
	}
	if !mulFloat.Equal(value) {
		t.Fatalf("CheckedMulFloat() = %v, want %v", mulFloat, value)
	}

	divInt, err := value.CheckedDivInt(1)
	if err != nil {
		t.Fatalf("CheckedDivInt() error = %v", err)
	}
	if !divInt.Equal(value) {
		t.Fatalf("CheckedDivInt() = %v, want %v", divInt, value)
	}

	divUint, err := value.CheckedDivUint(1)
	if err != nil {
		t.Fatalf("CheckedDivUint() error = %v", err)
	}
	if !divUint.Equal(value) {
		t.Fatalf("CheckedDivUint() = %v, want %v", divUint, value)
	}

	divFloat, err := value.CheckedDivFloat(1.0)
	if err != nil {
		t.Fatalf("CheckedDivFloat() error = %v", err)
	}
	if !divFloat.Equal(value) {
		t.Fatalf("CheckedDivFloat() = %v, want %v", divFloat, value)
	}

	remainderBase := mustPnlValue(t, "5")
	remInt, err := remainderBase.CheckedRemInt(2)
	if err != nil {
		t.Fatalf("CheckedRemInt() error = %v", err)
	}
	if got := remInt.String(); got != "1" {
		t.Fatalf("CheckedRemInt() = %q, want %q", got, "1")
	}

	remUint, err := remainderBase.CheckedRemUint(2)
	if err != nil {
		t.Fatalf("CheckedRemUint() error = %v", err)
	}
	if got := remUint.String(); got != "1" {
		t.Fatalf("CheckedRemUint() = %q, want %q", got, "1")
	}

	remFloat, err := remainderBase.CheckedRemFloat(2.0)
	if err != nil {
		t.Fatalf("CheckedRemFloat() error = %v", err)
	}
	if got := remFloat.String(); got != "1" {
		t.Fatalf("CheckedRemFloat() = %q, want %q", got, "1")
	}
}

func TestPnlCheckedArithmeticErrors(t *testing.T) {
	t.Parallel()

	max := mustPnlValue(t, decimalMaxValue)
	min := mustPnlValue(t, decimalMinValue)
	one := mustPnlValue(t, "1")

	if _, err := max.CheckedAdd(one); err == nil {
		t.Fatal("CheckedAdd() error = nil, want overflow error")
	}
	if _, err := min.CheckedSub(one); err == nil {
		t.Fatal("CheckedSub() error = nil, want overflow error")
	}
	if _, err := max.CheckedMulInt(2); err == nil {
		t.Fatal("CheckedMulInt() error = nil, want overflow error")
	}
	if _, err := max.CheckedMulUint(2); err == nil {
		t.Fatal("CheckedMulUint() error = nil, want overflow error")
	}
	if _, err := one.CheckedMulFloat(math.NaN()); err == nil {
		t.Fatal("CheckedMulFloat() error = nil, want invalid-float error")
	}

	if _, err := one.CheckedDivInt(0); err == nil {
		t.Fatal("CheckedDivInt() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedDivUint(0); err == nil {
		t.Fatal("CheckedDivUint() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedDivFloat(0); err == nil {
		t.Fatal("CheckedDivFloat() error = nil, want divide-by-zero error")
	}

	if _, err := one.CheckedRemInt(0); err == nil {
		t.Fatal("CheckedRemInt() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedRemUint(0); err == nil {
		t.Fatal("CheckedRemUint() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedRemFloat(0); err == nil {
		t.Fatal("CheckedRemFloat() error = nil, want divide-by-zero error")
	}
}

func TestPnlToCashFlowAndPositionSize(t *testing.T) {
	t.Parallel()

	value := mustPnlValue(t, "2.5")
	cashFlow := newCashFlowOrPanic(value.CashFlow())
	if got := cashFlow.String(); got != "2.5" {
		t.Fatalf("CashFlow() = %q, want %q", got, "2.5")
	}

	positionSize := newPositionSizeOrPanic(value.PositionSize())
	if got := positionSize.String(); got != "2.5" {
		t.Fatalf("PositionSize() = %q, want %q", got, "2.5")
	}
}

func TestNewPnlOrPanic(t *testing.T) {
	t.Parallel()

	value := newPnlOrPanic(NewPnlFromString("1"))
	if got := value.String(); got != "1" {
		t.Fatalf("newPnlOrPanic() = %q, want %q", got, "1")
	}

	assertPanicContains(t, "invalid format", func() {
		_ = newPnlOrPanic(NewPnlFromString("invalid-pnl"))
	})
}

func makeNativePnlOptional(value Pnl) native.ParamPnlOptional {
	type pnlOptionalLayout struct {
		Value native.ParamPnl
		IsSet bool
	}

	layout := pnlOptionalLayout{Value: value.Handle(), IsSet: true}
	return *(*native.ParamPnlOptional)(unsafe.Pointer(&layout))
}

func mustPnlValue(t *testing.T, source string) Pnl {
	t.Helper()

	value, err := NewPnlFromString(source)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", source, err)
	}
	return value
}
