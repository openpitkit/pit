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

package native

import (
	"errors"
	"fmt"
	"math"
	"math/big"
	"strings"
	"testing"
	"time"

	"github.com/shopspring/decimal"
)

func TestNativeDecimalExponentBoundaries(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name         string
		coefficient  string
		exponent     int32
		wantMantissa string
		wantScale    int32
		wantOverflow bool
	}{
		{name: "positive", coefficient: "1", exponent: 3, wantMantissa: "1000"},
		{name: "negative", coefficient: "-7", exponent: 2, wantMantissa: "-700"},
		{name: "zero", coefficient: "0", exponent: 5, wantMantissa: "0"},
		{name: "zero_max_exponent", coefficient: "0", exponent: math.MaxInt32, wantMantissa: "0"},
		{name: "exponent_38", coefficient: "1", exponent: 38, wantMantissa: "100000000000000000000000000000000000000"},
		{name: "exponent_39", coefficient: "1", exponent: 39, wantOverflow: true},
		// Max int128 is not divisible by 10; these products straddle it.
		{name: "below_max_int128", coefficient: "17014118346046923173168730371588410572", exponent: 1, wantMantissa: "170141183460469231731687303715884105720"},
		{name: "above_max_int128", coefficient: "17014118346046923173168730371588410573", exponent: 1, wantOverflow: true},
		{name: "above_min_int128", coefficient: "-17014118346046923173168730371588410572", exponent: 1, wantMantissa: "-170141183460469231731687303715884105720"},
		{name: "below_min_int128", coefficient: "-17014118346046923173168730371588410573", exponent: 1, wantOverflow: true},
		{name: "max_exponent", coefficient: "1", exponent: math.MaxInt32, wantOverflow: true},
		{name: "min_exponent", coefficient: "1", exponent: math.MinInt32, wantOverflow: true},
		{name: "zero_min_exponent", coefficient: "0", exponent: math.MinInt32, wantOverflow: true},
		{name: "max_scale", coefficient: "1", exponent: math.MinInt32 + 1, wantMantissa: "1", wantScale: math.MaxInt32},
		{name: "trailing_zeros", coefficient: "1000", exponent: -4, wantMantissa: "1000", wantScale: 4},
		{name: "zero_negative_exponent", coefficient: "0", exponent: -5, wantMantissa: "0", wantScale: 5},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			coefficient, ok := new(big.Int).SetString(tt.coefficient, 10)
			if !ok {
				t.Fatalf("invalid test coefficient %q", tt.coefficient)
			}
			source := decimal.NewFromBigInt(coefficient, tt.exponent)
			var nativeDecimal ParamDecimal
			var err error
			done := make(chan struct{})
			go func() {
				nativeDecimal, err = NewNativeDecimalFromDecimal(source)
				close(done)
			}()
			select {
			case <-done:
			case <-time.After(time.Second):
				t.Fatal("conversion did not return within 1s")
			}
			if tt.wantOverflow {
				if !errors.Is(err, ErrOverflow) {
					t.Fatalf("error = %v, want ErrOverflow", err)
				}
				if !strings.Contains(err.Error(), tt.coefficient) {
					t.Fatalf("error = %q, want coefficient %s", err, tt.coefficient)
				}
				if want := fmt.Sprintf("exponent %d", tt.exponent); !strings.Contains(err.Error(), want) {
					t.Fatalf("error = %q, want %s", err, want)
				}
				if tt.exponent == math.MinInt32 && !strings.Contains(err.Error(), "scale that does not fit int32") {
					t.Fatalf("error = %q, want scale that does not fit int32", err)
				}
				return
			}
			if err != nil {
				t.Fatalf("NewNativeDecimalFromDecimal(coefficient %s, exponent %d) error = %v", tt.coefficient, tt.exponent, err)
			}
			converted := NewDecimalFromNative(nativeDecimal)
			if got := converted.Coefficient().String(); got != tt.wantMantissa {
				t.Fatalf("native mantissa = %s, want %s", got, tt.wantMantissa)
			}
			if got := -converted.Exponent(); got != tt.wantScale {
				t.Fatalf("native scale = %d, want %d", got, tt.wantScale)
			}
		})
	}
}

func TestNativeDecimalCoefficientBoundaries(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name         string
		coefficient  string
		wantOverflow bool
	}{
		{name: "zero", coefficient: "0"},
		{name: "one", coefficient: "1"},
		{name: "negative_one", coefficient: "-1"},
		{name: "max_int64", coefficient: "9223372036854775807"},
		{name: "min_int64", coefficient: "-9223372036854775808"},
		{name: "max_int64_plus_one", coefficient: "9223372036854775808"},
		{name: "min_int64_minus_one", coefficient: "-9223372036854775809"},
		{name: "two_to_64_plus_five", coefficient: "18446744073709551621"},
		{name: "max_96_bit", coefficient: "79228162514264337593543950335"},
		{name: "negative_max_96_bit", coefficient: "-79228162514264337593543950335"},
		{name: "max_int128", coefficient: "170141183460469231731687303715884105727"},
		{name: "min_int128", coefficient: "-170141183460469231731687303715884105728"},
		{name: "max_int128_plus_one", coefficient: "170141183460469231731687303715884105728", wantOverflow: true},
		{name: "min_int128_minus_one", coefficient: "-170141183460469231731687303715884105729", wantOverflow: true},
	}

	for _, tt := range tests {
		for _, exponent := range []int32{0, -4} {
			t.Run(fmt.Sprintf("%s/exponent_%d", tt.name, exponent), func(t *testing.T) {
				coefficient, ok := new(big.Int).SetString(tt.coefficient, 10)
				if !ok {
					t.Fatalf("invalid test coefficient %q", tt.coefficient)
				}
				source := decimal.NewFromBigInt(coefficient, exponent)
				nativeDecimal, err := NewNativeDecimalFromDecimal(source)
				if tt.wantOverflow {
					if !errors.Is(err, ErrOverflow) {
						t.Fatalf("error = %v, want ErrOverflow", err)
					}
					if !strings.Contains(err.Error(), tt.coefficient) {
						t.Fatalf("error = %q, want coefficient %s", err, tt.coefficient)
					}
					return
				}
				if err != nil {
					t.Fatalf("NewNativeDecimalFromDecimal(%s) error = %v", source, err)
				}
				if got := NewDecimalFromNative(nativeDecimal); !got.Equal(source) {
					t.Fatalf("round trip = %s, want %s", got, source)
				}
				if got := int32(nativeDecimal.scale); got != -exponent {
					t.Fatalf("scale = %d, want %d", got, -exponent)
				}
			})
		}
	}
}
