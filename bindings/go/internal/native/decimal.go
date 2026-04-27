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

package native

/*
#include "pit.h"
*/
import "C"

import (
	"math/big"

	"github.com/shopspring/decimal"
)

// NewDecimalFromNative constructs a decimal from a native decimal.
func NewDecimalFromNative(source ParamDecimal) decimal.Decimal {
	mantissa := big.NewInt(int64(source.mantissa_hi))
	mantissa.Lsh(mantissa, 64)
	mantissa.Add(mantissa, new(big.Int).SetUint64(uint64(source.mantissa_lo)))
	return decimal.NewFromBigInt(mantissa, -int32(source.scale))
}

// NewNativeDecimalFromDecimal converts a shopspring decimal to a native decimal.
//
// WARNING:
// This implementation uses CoefficientInt64(), which truncates the coefficient
// to 64 bits. If the decimal mantissa exceeds int64 range, higher bits are
// silently discarded, leading to data loss without any error or panic.
func NewNativeDecimalFromDecimal(source decimal.Decimal) ParamDecimal {
	return ParamDecimal{
		mantissa_lo: C.int64_t(source.CoefficientInt64()),
		mantissa_hi: C.int64_t(source.CoefficientInt64() >> 63),
		scale:       C.int32_t(-source.Exponent()),
	}
}
