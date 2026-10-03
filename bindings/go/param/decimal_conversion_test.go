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

package param

import (
	"errors"
	"fmt"
	"math/big"
	"testing"

	"github.com/shopspring/decimal"
)

func TestDecimalConversionCoreMantissaLimit(t *testing.T) {
	t.Parallel()

	_, err := NewQuantityFromDecimal(decimal.New(1, 30))
	if err == nil || errors.Is(err, ErrOverflow) {
		t.Fatalf("error = %v, want a core error without ErrOverflow", err)
	}
}

func TestDecimalConversionCoefficientBoundaries(t *testing.T) {
	t.Parallel()

	constructors := []struct {
		name   string
		signed bool
		call   func(decimal.Decimal) (decimal.Decimal, error)
	}{
		{
			name: "NewQuantityFromDecimal",
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewQuantityFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name: "NewQuantityFromDecimalRounded",
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewQuantityFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewPositionSizeFromDecimal",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewPositionSizeFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewPositionSizeFromDecimalRounded",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewPositionSizeFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewCashFlowFromDecimal",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewCashFlowFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewCashFlowFromDecimalRounded",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewCashFlowFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewPriceFromDecimal",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewPriceFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewPriceFromDecimalRounded",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewPriceFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewFeeFromDecimal",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewFeeFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewFeeFromDecimalRounded",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewFeeFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewPnlFromDecimal",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewPnlFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name:   "NewPnlFromDecimalRounded",
			signed: true,
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewPnlFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name: "NewNotionalFromDecimal",
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewNotionalFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name: "NewNotionalFromDecimalRounded",
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewNotionalFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name: "NewVolumeFromDecimal",
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewVolumeFromDecimal(source)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
		{
			name: "NewVolumeFromDecimalRounded",
			call: func(source decimal.Decimal) (decimal.Decimal, error) {
				value, err := NewVolumeFromDecimalRounded(source, 4, RoundingStrategyMidpointNearestEven)
				if err != nil {
					return decimal.Decimal{}, err
				}
				return value.Decimal(), nil
			},
		},
	}

	tests := []struct {
		name         string
		coefficient  string
		exponent     int32
		wantOverflow bool
	}{
		{name: "two_to_127", coefficient: "170141183460469231731687303715884105728", wantOverflow: true},
		{name: "positive_exponent", coefficient: "7", exponent: 3},
		{name: "negative_positive_exponent", coefficient: "-7", exponent: 3},
		{name: "two_to_64_plus_five", coefficient: "18446744073709551621"},
		{name: "two_to_64_plus_five", coefficient: "18446744073709551621", exponent: -4},
		{name: "negative_two_to_64_plus_five", coefficient: "-18446744073709551621"},
		{name: "negative_two_to_64_plus_five", coefficient: "-18446744073709551621", exponent: -4},
	}

	for _, constructor := range constructors {
		t.Run(constructor.name, func(t *testing.T) {
			for _, tt := range tests {
				coefficient, ok := new(big.Int).SetString(tt.coefficient, 10)
				if !ok {
					t.Fatalf("invalid test coefficient %q", tt.coefficient)
				}
				if coefficient.Sign() < 0 && !constructor.signed {
					continue
				}
				t.Run(fmt.Sprintf("%s/exponent_%d", tt.name, tt.exponent), func(t *testing.T) {
					source := decimal.NewFromBigInt(coefficient, tt.exponent)
					got, err := constructor.call(source)
					if tt.wantOverflow {
						if !errors.Is(err, ErrOverflow) {
							t.Fatalf("error = %v, want ErrOverflow", err)
						}
						return
					}
					if err != nil {
						t.Fatalf("%s(%s) error = %v", constructor.name, source, err)
					}
					if !got.Equal(source) {
						t.Fatalf("Decimal() = %s, want %s", got, source)
					}
				})
			}
		})
	}
}
