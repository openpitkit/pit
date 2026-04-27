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
	"fmt"
	"math"
	"strings"
	"testing"
	"unsafe"

	"github.com/openpitkit/pit-go/internal/native"
	"github.com/shopspring/decimal"
)

const (
	cashFlowCanonicalValue = "123.45"
	decimalMaxValue        = "79228162514264337593543950335"
	decimalMinValue        = "-79228162514264337593543950335"
)

func TestCashFlowFromString(t *testing.T) {
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
		{name: "invalid", input: "cash-flow", wantError: true},
	}

	for _, tt := range tests {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			value, err := NewCashFlowFromString(tt.input)
			if tt.wantError {
				if err == nil {
					t.Fatalf("NewCashFlowFromString(%q) error = nil, want non-nil", tt.input)
				}
				assertErrorContains(t, err, "invalid format")
				return
			}
			if err != nil {
				t.Fatalf("NewCashFlowFromString(%q) error = %v", tt.input, err)
			}
			if got := value.String(); got != tt.want {
				t.Fatalf("String() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCashFlowFromIntAndUint(t *testing.T) {
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
	for _, tt := range intTests {
		tt := tt
		t.Run("int-"+tt.name, func(t *testing.T) {
			t.Parallel()

			value, err := NewCashFlowFromInt(tt.input)
			if err != nil {
				t.Fatalf("NewCashFlowFromInt(%d) error = %v", tt.input, err)
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
	for _, tt := range uintTests {
		tt := tt
		t.Run("uint-"+tt.name, func(t *testing.T) {
			t.Parallel()

			value, err := NewCashFlowFromUint(tt.input)
			if err != nil {
				t.Fatalf("NewCashFlowFromUint(%d) error = %v", tt.input, err)
			}
			if got := value.String(); got != tt.want {
				t.Fatalf("String() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCashFlowFromFloat(t *testing.T) {
	t.Parallel()

	value, err := NewCashFlowFromFloat(12.5)
	if err != nil {
		t.Fatalf("NewCashFlowFromFloat(12.5) error = %v", err)
	}
	if got := value.String(); got != "12.5" {
		t.Fatalf("String() = %q, want %q", got, "12.5")
	}

	for _, input := range []float64{math.NaN(), math.Inf(1), math.Inf(-1)} {
		input := input
		t.Run(fmt.Sprintf("invalid-%v", input), func(t *testing.T) {
			t.Parallel()

			_, gotErr := NewCashFlowFromFloat(input)
			if gotErr == nil {
				t.Fatalf("NewCashFlowFromFloat(%v) error = nil, want non-nil", input)
			}
		})
	}
}

func TestCashFlowFromDecimal(t *testing.T) {
	t.Parallel()

	source := mustDecimal(t, cashFlowCanonicalValue)
	value, err := NewCashFlowFromDecimal(source)
	if err != nil {
		t.Fatalf("NewCashFlowFromDecimal(%q) error = %v", source.String(), err)
	}
	if got := value.String(); got != cashFlowCanonicalValue {
		t.Fatalf("String() = %q, want %q", got, cashFlowCanonicalValue)
	}
}

func TestCashFlowRoundedConstructors(t *testing.T) {
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
		call func(strategy RoundingStrategy) (CashFlow, error)
	}{
		{
			name: "string",
			call: func(strategy RoundingStrategy) (CashFlow, error) {
				return NewCashFlowFromStringRounded("1.25", 1, strategy)
			},
		},
		{
			name: "float",
			call: func(strategy RoundingStrategy) (CashFlow, error) {
				return NewCashFlowFromFloatRounded(1.25, 1, strategy)
			},
		},
		{
			name: "decimal",
			call: func(strategy RoundingStrategy) (CashFlow, error) {
				return NewCashFlowFromDecimalRounded(decimalInput, 1, strategy)
			},
		},
	}

	for _, ctor := range constructors {
		ctor := ctor
		t.Run(ctor.name, func(t *testing.T) {
			t.Parallel()

			for _, tc := range strategyCases {
				tc := tc
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

func TestNewCashFlowOptionFromNative(t *testing.T) {
	t.Parallel()

	unset := NewCashFlowOptionFromNative(native.ParamCashFlowOptional{})
	if unset.IsSet() {
		t.Fatal("unset native cash flow should map to empty option")
	}

	expected := mustCashFlow(t, cashFlowCanonicalValue)
	set := NewCashFlowOptionFromNative(makeNativeCashFlowOptional(expected))
	got, ok := set.Get()
	if !ok {
		t.Fatal("set native cash flow should map to present option")
	}
	if !got.Equal(expected) {
		t.Fatalf("option value = %v, want %v", got, expected)
	}
}

func TestCashFlowConstructorsFromRelatedTypes(t *testing.T) {
	t.Parallel()

	pnl := mustPnlForCashFlow(t, "1.25")
	fromPnl := NewCashFlowFromPnl(pnl)
	if got := fromPnl.String(); got != "1.25" {
		t.Fatalf("NewCashFlowFromPnl() = %q, want %q", got, "1.25")
	}

	fee := newFeeOrPanic(NewFeeFromString("1.25"))
	fromFee := NewCashFlowFromFee(fee)
	if got := fromFee.String(); got != "-1.25" {
		t.Fatalf("NewCashFlowFromFee() = %q, want %q", got, "-1.25")
	}

	volume := newVolumeOrPanic(NewVolumeFromString("1.25"))
	fromInflow := NewCashFlowFromVolumeInflow(volume)
	if got := fromInflow.String(); got != "1.25" {
		t.Fatalf("NewCashFlowFromVolumeInflow() = %q, want %q", got, "1.25")
	}

	fromOutflow := NewCashFlowFromVolumeOutflow(volume)
	if got := fromOutflow.String(); got != "-1.25" {
		t.Fatalf("NewCashFlowFromVolumeOutflow() = %q, want %q", got, "-1.25")
	}
}

func TestCashFlowAccessors(t *testing.T) {
	t.Parallel()

	value := mustCashFlow(t, cashFlowCanonicalValue)
	wantDecimal := mustDecimal(t, cashFlowCanonicalValue)
	if got := value.Decimal(); !got.Equal(wantDecimal) {
		t.Fatalf("Decimal() = %s, want %s", got.String(), wantDecimal.String())
	}

	roundTrip := NewCashFlowFromNative(value.Native())
	if !roundTrip.Equal(value) {
		t.Fatalf("NewCashFlowFromNative(Native()) = %v, want %v", roundTrip, value)
	}

	if got := value.String(); got != cashFlowCanonicalValue {
		t.Fatalf("String() = %q, want %q", got, cashFlowCanonicalValue)
	}

	const epsilon = 1e-9
	wantFloat := 123.45
	if got := value.Float(); math.Abs(got-wantFloat) > epsilon {
		t.Fatalf("Float() = %.12f, want %.12f (eps=%g)", got, wantFloat, epsilon)
	}
}

func TestCashFlowIsZeroEqualCompare(t *testing.T) {
	t.Parallel()

	if !mustCashFlow(t, "0").IsZero() {
		t.Fatal("IsZero() = false, want true for zero")
	}
	if mustCashFlow(t, "1").IsZero() {
		t.Fatal("IsZero() = true, want false for non-zero")
	}

	one := mustCashFlow(t, "1.0")
	oneAlt := mustCashFlow(t, "1.00")
	if !one.Equal(oneAlt) {
		t.Fatal("Equal() must treat 1.0 and 1.00 as equal")
	}

	a := mustCashFlow(t, "-2")
	b := mustCashFlow(t, "0")
	c := mustCashFlow(t, "3")

	if a.Compare(a) != 0 {
		t.Fatalf("reflexive compare = %d, want 0", a.Compare(a))
	}
	if !(a.Compare(b) < 0 && b.Compare(c) < 0 && a.Compare(c) < 0) {
		t.Fatal("transitive compare contract violated")
	}
	if a.Compare(b) != -b.Compare(a) {
		t.Fatal("antisymmetric compare contract violated")
	}
}

func TestCashFlowCheckedArithmeticHappyPaths(t *testing.T) {
	t.Parallel()

	value := mustCashFlow(t, "12.5")
	zero := mustCashFlow(t, "0")

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

	remainderBase := mustCashFlow(t, "5")
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

func TestCashFlowCheckedArithmeticErrors(t *testing.T) {
	t.Parallel()

	max := mustCashFlow(t, decimalMaxValue)
	min := mustCashFlow(t, decimalMinValue)
	one := mustCashFlow(t, "1")

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

func TestNewCashFlowOrPanic(t *testing.T) {
	t.Parallel()

	value := newCashFlowOrPanic(NewCashFlowFromString("1"))
	if got := value.String(); got != "1" {
		t.Fatalf("newCashFlowOrPanic() = %q, want %q", got, "1")
	}

	assertPanicContains(t, "invalid format", func() {
		_ = newCashFlowOrPanic(NewCashFlowFromString("invalid-cash-flow"))
	})
}

func makeNativeCashFlowOptional(value CashFlow) native.ParamCashFlowOptional {
	type cashFlowOptionalLayout struct {
		Value native.ParamCashFlow
		IsSet bool
	}

	layout := cashFlowOptionalLayout{Value: value.Native(), IsSet: true}
	return *(*native.ParamCashFlowOptional)(unsafe.Pointer(&layout))
}

func assertPanicContains(t *testing.T, want string, fn func()) {
	t.Helper()

	defer func() {
		recovered := recover()
		if recovered == nil {
			t.Fatalf("panic = nil, want non-nil panic containing %q", want)
		}
		if !strings.Contains(fmt.Sprint(recovered), want) {
			t.Fatalf("panic = %q, want to contain %q", fmt.Sprint(recovered), want)
		}
	}()

	fn()
}

func assertErrorContains(t *testing.T, err error, want string) {
	t.Helper()

	if err == nil {
		t.Fatalf("error = nil, want error containing %q", want)
	}
	if !strings.Contains(err.Error(), want) {
		t.Fatalf("error = %q, want to contain %q", err.Error(), want)
	}
}

func mustCashFlow(t *testing.T, source string) CashFlow {
	t.Helper()

	value, err := NewCashFlowFromString(source)
	if err != nil {
		t.Fatalf("NewCashFlowFromString(%q) error = %v", source, err)
	}
	return value
}

func mustPnlForCashFlow(t *testing.T, source string) Pnl {
	t.Helper()

	value, err := NewPnlFromString(source)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", source, err)
	}
	return value
}

func mustDecimal(t *testing.T, source string) decimal.Decimal {
	t.Helper()

	value, err := decimal.NewFromString(source)
	if err != nil {
		t.Fatalf("decimal.NewFromString(%q) error = %v", source, err)
	}
	return value
}
