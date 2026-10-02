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
	"math/big"
	"strings"
	"testing"

	"github.com/shopspring/decimal"
)

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
