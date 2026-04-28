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
	"errors"
	"math"
	"testing"
	"unsafe"

	"go.openpit.dev/openpit/internal/native"
)

const positionSizeCanonicalValue = "54.321"

func TestPositionSizeFromString(t *testing.T) {
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
		{name: "invalid", input: "position-size", wantError: true},
	}

	for _, tt := range tests { //nolint:copyloopvar
		tt := tt //nolint:copyloopvar
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			value, err := NewPositionSizeFromString(tt.input)
			if tt.wantError {
				if err == nil {
					t.Fatalf("NewPositionSizeFromString(%q) error = nil, want non-nil", tt.input)
				}
				assertErrorContains(t, err, "invalid format")
				return
			}
			if err != nil {
				t.Fatalf("NewPositionSizeFromString(%q) error = %v", tt.input, err)
			}
			if got := value.String(); got != tt.want {
				t.Fatalf("String() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestPositionSizeFromIntAndUint(t *testing.T) {
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

			value, err := NewPositionSizeFromInt(tt.input)
			if err != nil {
				t.Fatalf("NewPositionSizeFromInt(%d) error = %v", tt.input, err)
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

			value, err := NewPositionSizeFromUint(tt.input)
			if err != nil {
				t.Fatalf("NewPositionSizeFromUint(%d) error = %v", tt.input, err)
			}
			if got := value.String(); got != tt.want {
				t.Fatalf("String() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestPositionSizeFromFloat(t *testing.T) {
	t.Parallel()

	value, err := NewPositionSizeFromFloat(12.5)
	if err != nil {
		t.Fatalf("NewPositionSizeFromFloat(12.5) error = %v", err)
	}
	if got := value.String(); got != "12.5" {
		t.Fatalf("String() = %q, want %q", got, "12.5")
	}

	for _, input := range []float64{math.NaN(), math.Inf(1), math.Inf(-1)} { //nolint:copyloopvar
		input := input //nolint:copyloopvar
		t.Run("invalid", func(t *testing.T) {
			t.Parallel()

			_, gotErr := NewPositionSizeFromFloat(input)
			if gotErr == nil {
				t.Fatalf("NewPositionSizeFromFloat(%v) error = nil, want non-nil", input)
			}
		})
	}
}

func TestPositionSizeFromDecimal(t *testing.T) {
	t.Parallel()

	source := mustDecimal(t, positionSizeCanonicalValue)
	value, err := NewPositionSizeFromDecimal(source)
	if err != nil {
		t.Fatalf("NewPositionSizeFromDecimal(%q) error = %v", source.String(), err)
	}
	if got := value.String(); got != positionSizeCanonicalValue {
		t.Fatalf("String() = %q, want %q", got, positionSizeCanonicalValue)
	}
}

func TestPositionSizeRoundedConstructors(t *testing.T) {
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
		call func(strategy RoundingStrategy) (PositionSize, error)
	}{
		{
			name: "string",
			call: func(strategy RoundingStrategy) (PositionSize, error) {
				return NewPositionSizeFromStringRounded("1.25", 1, strategy)
			},
		},
		{
			name: "float",
			call: func(strategy RoundingStrategy) (PositionSize, error) {
				return NewPositionSizeFromFloatRounded(1.25, 1, strategy)
			},
		},
		{
			name: "decimal",
			call: func(strategy RoundingStrategy) (PositionSize, error) {
				return NewPositionSizeFromDecimalRounded(decimalInput, 1, strategy)
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

func TestNewPositionSizeOptionFromHandle(t *testing.T) {
	t.Parallel()

	unset := NewPositionSizeOptionFromHandle(native.ParamPositionSizeOptional{})
	if unset.IsSet() {
		t.Fatal("unset native position size should map to empty option")
	}

	expected := mustPositionSizeValue(t, positionSizeCanonicalValue)
	set := NewPositionSizeOptionFromHandle(makeNativePositionSizeOptional(expected))
	got, ok := set.Get()
	if !ok {
		t.Fatal("set native position size should map to present option")
	}
	if !got.Equal(expected) {
		t.Fatalf("option value = %v, want %v", got, expected)
	}
}

func TestPositionSizeConstructorsFromRelatedTypes(t *testing.T) {
	t.Parallel()

	pnl := mustPnlValue(t, "1.25")
	fromPnl := newPositionSizeOrPanic(NewPositionSizeFromPnl(pnl))
	if got := fromPnl.String(); got != "1.25" {
		t.Fatalf("NewPositionSizeFromPnl() = %q, want %q", got, "1.25")
	}

	fee := newFeeOrPanic(NewFeeFromString("1.25"))
	fromFee := newPositionSizeOrPanic(NewPositionSizeFromFee(fee))
	if got := fromFee.String(); got != "-1.25" {
		t.Fatalf("NewPositionSizeFromFee() = %q, want %q", got, "-1.25")
	}

	quantity := mustQuantityForPosition(t, "2")
	long := newPositionSizeOrPanic(NewPositionSizeFromQuantityAndSide(quantity, SideBuy))
	if got := long.String(); got != "2" {
		t.Fatalf("NewPositionSizeFromQuantityAndSide(buy) = %q, want %q", got, "2")
	}

	short := newPositionSizeOrPanic(NewPositionSizeFromQuantityAndSide(quantity, SideSell))
	if got := short.String(); got != "-2" {
		t.Fatalf("NewPositionSizeFromQuantityAndSide(sell) = %q, want %q", got, "-2")
	}
}

func TestPositionSizeAccessors(t *testing.T) {
	t.Parallel()

	value := mustPositionSizeValue(t, positionSizeCanonicalValue)
	wantDecimal := mustDecimal(t, positionSizeCanonicalValue)
	if got := value.Decimal(); !got.Equal(wantDecimal) {
		t.Fatalf("Decimal() = %s, want %s", got.String(), wantDecimal.String())
	}

	roundTrip := NewPositionSizeFromHandle(value.Handle())
	if !roundTrip.Equal(value) {
		t.Fatalf("NewPositionSizeFromHandle(Handle()) = %v, want %v", roundTrip, value)
	}

	if got := value.String(); got != positionSizeCanonicalValue {
		t.Fatalf("String() = %q, want %q", got, positionSizeCanonicalValue)
	}

	const epsilon = 1e-9
	wantFloat := 54.321
	if got := value.Float(); math.Abs(got-wantFloat) > epsilon {
		t.Fatalf("Float() = %.12f, want %.12f (eps=%g)", got, wantFloat, epsilon)
	}
}

func TestPositionSizeIsZeroEqualCompare(t *testing.T) {
	t.Parallel()

	if !mustPositionSizeValue(t, "0").IsZero() {
		t.Fatal("IsZero() = false, want true for zero")
	}
	if mustPositionSizeValue(t, "1").IsZero() {
		t.Fatal("IsZero() = true, want false for non-zero")
	}

	one := mustPositionSizeValue(t, "1.0")
	oneAlt := mustPositionSizeValue(t, "1.00")
	if !one.Equal(oneAlt) {
		t.Fatal("Equal() must treat 1.0 and 1.00 as equal")
	}

	a := mustPositionSizeValue(t, "-2")
	b := mustPositionSizeValue(t, "0")
	c := mustPositionSizeValue(t, "3")

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

func TestPositionSizeOpenAndCloseQuantity(t *testing.T) {
	t.Parallel()

	short := mustPositionSizeValue(t, "-0.5")
	openQtyShort, openSideShort := short.OpenQuantity()
	if got := openQtyShort.String(); got != "0.5" {
		t.Fatalf("OpenQuantity() qty for short = %q, want %q", got, "0.5")
	}
	if openSideShort != SideSell {
		t.Fatalf("OpenQuantity() side for short = %v, want %v", openSideShort, SideSell)
	}

	closeQtyShort, closeSideShort := short.CloseQuantity()
	if got := closeQtyShort.String(); got != "0.5" {
		t.Fatalf("CloseQuantity() qty for short = %q, want %q", got, "0.5")
	}
	sideShort, ok := closeSideShort.Get()
	if !ok {
		t.Fatal("CloseQuantity() side for short = none, want buy")
	}
	if sideShort != SideBuy {
		t.Fatalf("CloseQuantity() side for short = %v, want %v", sideShort, SideBuy)
	}

	long := mustPositionSizeValue(t, "0.5")
	openQtyLong, openSideLong := long.OpenQuantity()
	if got := openQtyLong.String(); got != "0.5" {
		t.Fatalf("OpenQuantity() qty for long = %q, want %q", got, "0.5")
	}
	if openSideLong != SideBuy {
		t.Fatalf("OpenQuantity() side for long = %v, want %v", openSideLong, SideBuy)
	}

	closeQtyLong, closeSideLong := long.CloseQuantity()
	if got := closeQtyLong.String(); got != "0.5" {
		t.Fatalf("CloseQuantity() qty for long = %q, want %q", got, "0.5")
	}
	sideLong, ok := closeSideLong.Get()
	if !ok {
		t.Fatal("CloseQuantity() side for long = none, want sell")
	}
	if sideLong != SideSell {
		t.Fatalf("CloseQuantity() side for long = %v, want %v", sideLong, SideSell)
	}

	zero := mustPositionSizeValue(t, "0")
	openQtyZero, openSideZero := zero.OpenQuantity()
	if got := openQtyZero.String(); got != "0" {
		t.Fatalf("OpenQuantity() qty for zero = %q, want %q", got, "0")
	}
	if openSideZero != SideBuy {
		t.Fatalf("OpenQuantity() side for zero = %v, want %v", openSideZero, SideBuy)
	}

	closeQtyZero, closeSideZero := zero.CloseQuantity()
	if got := closeQtyZero.String(); got != "0" {
		t.Fatalf("CloseQuantity() qty for zero = %q, want %q", got, "0")
	}
	if closeSideZero.IsSet() {
		t.Fatal("CloseQuantity() side for zero should be empty")
	}
}

func TestPositionSizeCheckedAddQuantity(t *testing.T) {
	t.Parallel()

	start := mustPositionSizeValue(t, "1.5")
	quantity := mustQuantityForPosition(t, "0.5")

	buy, err := start.CheckedAddQuantity(quantity, SideBuy)
	if err != nil {
		t.Fatalf("CheckedAddQuantity(buy) error = %v", err)
	}
	if got := buy.String(); got != "2.0" {
		t.Fatalf("CheckedAddQuantity(buy) = %q, want %q", got, "2.0")
	}

	sell, err := start.CheckedAddQuantity(quantity, SideSell)
	if err != nil {
		t.Fatalf("CheckedAddQuantity(sell) error = %v", err)
	}
	if got := sell.String(); got != "1.0" {
		t.Fatalf("CheckedAddQuantity(sell) = %q, want %q", got, "1.0")
	}

	flip, err := mustPositionSizeValue(t, "1").CheckedAddQuantity(
		mustQuantityForPosition(t, "2"),
		SideSell,
	)
	if err != nil {
		t.Fatalf("CheckedAddQuantity(flip) error = %v", err)
	}
	if got := flip.String(); got != "-1" {
		t.Fatalf("CheckedAddQuantity(flip) = %q, want %q", got, "-1")
	}

	max := mustPositionSizeValue(t, decimalMaxValue)
	if _, err := max.CheckedAddQuantity(mustQuantityForPosition(t, "1"), SideBuy); err == nil {
		t.Fatal("CheckedAddQuantity(overflow) error = nil, want overflow error")
	}
}

func TestPositionSizePanicHelpers(t *testing.T) {
	t.Parallel()

	value := newPositionSizeOrPanic(NewPositionSizeFromString("1"))
	if got := value.String(); got != "1" {
		t.Fatalf("newPositionSizeOrPanic() = %q, want %q", got, "1")
	}

	assertPanicContains(t, "invalid format", func() {
		_ = newPositionSizeOrPanic(NewPositionSizeFromString("invalid-position-size"))
	})

	quantity := mustQuantityForPosition(t, "2")
	gotQuantity, gotSide := newPositionSizeQuantitySideOrPanic(quantity.Handle(), SideBuy.Handle(), nil)
	if NewQuantityFromHandle(gotQuantity).String() != "2" {
		t.Fatalf("newPositionSizeQuantitySideOrPanic() qty = %q, want %q", NewQuantityFromHandle(gotQuantity).String(), "2")
	}
	if gotSide != SideBuy.Handle() {
		t.Fatalf("newPositionSizeQuantitySideOrPanic() side = %v, want %v", gotSide, SideBuy.Handle())
	}

	assertPanicContains(t, "boom", func() {
		_, _ = newPositionSizeQuantitySideOrPanic(
			native.ParamQuantity{},
			native.ParamSideNotSet,
			errors.New("boom"),
		)
	})
}

func makeNativePositionSizeOptional(value PositionSize) native.ParamPositionSizeOptional {
	type positionSizeOptionalLayout struct {
		Value native.ParamPositionSize
		IsSet bool
	}

	layout := positionSizeOptionalLayout{Value: value.Handle(), IsSet: true}
	return *(*native.ParamPositionSizeOptional)(unsafe.Pointer(&layout))
}

func mustPositionSizeValue(t *testing.T, source string) PositionSize {
	t.Helper()

	value, err := NewPositionSizeFromString(source)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", source, err)
	}
	return value
}

func mustQuantityForPosition(t *testing.T, source string) Quantity {
	t.Helper()

	value, err := NewQuantityFromString(source)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", source, err)
	}
	return value
}
