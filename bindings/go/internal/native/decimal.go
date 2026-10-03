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

/*
#include "openpit.h"
*/
import "C"

import (
	"fmt"
	"math"
	"math/big"

	"github.com/shopspring/decimal"
)

const (
	decimalMantissaBits             = 64 // width of each native mantissa half
	decimalRadix                    = 10
	decimalMantissaOverflowExponent = 39
)

// NewDecimalFromNative constructs a decimal from a native decimal.
func NewDecimalFromNative(source ParamDecimal) decimal.Decimal {
	mantissa := big.NewInt(int64(source.mantissa_hi))
	mantissa.Lsh(mantissa, decimalMantissaBits)
	mantissa.Add(mantissa, new(big.Int).SetUint64(uint64(source.mantissa_lo)))
	return decimal.NewFromBigInt(mantissa, -int32(source.scale))
}

// NewNativeDecimalFromDecimal transfers a shopspring decimal exactly into a
// native decimal, folding a positive exponent into its signed 128-bit mantissa
// at scale zero. Nonpositive exponents preserve the coefficient and scale.
//
// A mantissa outside signed 128-bit range or a scale that does not fit int32
// returns an error wrapping ErrOverflow. Nonzero coefficients with exponents
// of 39 or more overflow without computing a power of ten; zero coefficients
// with positive exponents produce zero at scale zero. The core rejects
// mantissas wider than 96 bits or scales above 28 with its own error.
func NewNativeDecimalFromDecimal(source decimal.Decimal) (ParamDecimal, error) {
	coefficient := source.Coefficient()
	exponent := source.Exponent()
	if exponent == math.MinInt32 {
		return ParamDecimal{}, fmt.Errorf(
			"%w: decimal coefficient %s with exponent %d has a scale that does not fit int32",
			ErrOverflow, coefficient, exponent,
		)
	}
	mantissaOverflowError := func() error {
		value := coefficient.String()
		if exponent > 0 {
			value = fmt.Sprintf("%s with exponent %d", coefficient, exponent)
		}
		return fmt.Errorf(
			"%w: decimal coefficient %s does not fit a 128-bit mantissa",
			ErrOverflow, value,
		)
	}
	scale := -exponent
	mantissa := coefficient
	if exponent > 0 {
		scale = 0
		if coefficient.Sign() != 0 {
			// 10^39 exceeds signed 128-bit range even for coefficient 1.
			if exponent >= decimalMantissaOverflowExponent {
				return ParamDecimal{}, mantissaOverflowError()
			}
			power := new(big.Int).Exp(big.NewInt(decimalRadix), big.NewInt(int64(exponent)), nil)
			mantissa = new(big.Int).Mul(coefficient, power)
		}
	}
	hi := new(big.Int).Rsh(mantissa, decimalMantissaBits)
	if !hi.IsInt64() {
		return ParamDecimal{}, mantissaOverflowError()
	}
	lo := new(big.Int).And(mantissa, new(big.Int).SetUint64(^uint64(0)))
	return ParamDecimal{
		mantissa_lo: C.int64_t(lo.Uint64()),
		mantissa_hi: C.int64_t(hi.Int64()),
		scale:       C.int32_t(scale),
	}, nil
}
