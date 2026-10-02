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
	"math/big"

	"github.com/shopspring/decimal"
)

const decimalMantissaBits = 64 // width of each native mantissa half

// NewDecimalFromNative constructs a decimal from a native decimal.
func NewDecimalFromNative(source ParamDecimal) decimal.Decimal {
	mantissa := big.NewInt(int64(source.mantissa_hi))
	mantissa.Lsh(mantissa, decimalMantissaBits)
	mantissa.Add(mantissa, new(big.Int).SetUint64(uint64(source.mantissa_lo)))
	return decimal.NewFromBigInt(mantissa, -int32(source.scale))
}

// NewNativeDecimalFromDecimal transfers a shopspring decimal's full coefficient
// into the native decimal's signed 128-bit mantissa.
//
// Coefficients outside signed 128-bit range return an error wrapping ErrOverflow.
// The core rejects mantissas wider than 96 bits or scales above 28 with its own
// error.
func NewNativeDecimalFromDecimal(source decimal.Decimal) (ParamDecimal, error) {
	coefficient := source.Coefficient()
	hi := new(big.Int).Rsh(coefficient, decimalMantissaBits)
	if !hi.IsInt64() {
		return ParamDecimal{}, fmt.Errorf(
			"%w: decimal coefficient %s does not fit a 128-bit mantissa",
			ErrOverflow, coefficient,
		)
	}
	lo := new(big.Int).And(coefficient, new(big.Int).SetUint64(^uint64(0)))
	return ParamDecimal{
		mantissa_lo: C.int64_t(lo.Uint64()),
		mantissa_hi: C.int64_t(hi.Int64()),
		scale:       C.int32_t(-source.Exponent()),
	}, nil
}
