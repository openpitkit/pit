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

//------------------------------------------------------------------------------
// Pnl

func CreateParamPnl(value ParamDecimal) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl(value, &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_pnl failed")
	}
	return result, nil
}

func CreateParamPnlFromStr(v string) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl_from_str(importString(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_pnl_from_str failed")
	}
	return result, nil
}

func CreateParamPnlFromF64(v float64) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_pnl_from_f64 failed")
	}
	return result, nil
}

func CreateParamPnlFromI64(v int64) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_pnl_from_i64 failed")
	}
	return result, nil
}

func CreateParamPnlFromU64(v uint64) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_pnl_from_u64 failed")
	}
	return result, nil
}

func CreateParamPnlFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_pnl_from_str_rounded failed")
	}
	return result, nil
}

func CreateParamPnlFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_pnl_from_f64_rounded failed")
	}
	return result, nil
}

func CreateParamPnlFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPnl, error) {
	var result ParamPnl
	var outError SharedString
	if !C.pit_create_param_pnl_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_pnl_from_decimal_rounded failed")
	}
	return result, nil
}

func ParamPnlGetDecimal(value ParamPnl) ParamDecimal {
	// Keep direct field access for Go hot paths: this is a zero-cost read of the
	// transparent wrapper and avoids extra cgo call overhead.
	return value._0
}

func ParamPnlToF64(value ParamPnl) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_pnl_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_pnl_to_f64 failed")
	}
	return float64(out), nil
}

func ParamPnlIsZero(value ParamPnl) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_pnl_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError, "pit_param_pnl_is_zero failed")
	}
	return bool(out), nil
}

func ParamPnlCompare(lhs ParamPnl, rhs ParamPnl) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_pnl_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_pnl_compare failed")
	}
	return int(out), nil
}

func ParamPnlToString(value ParamPnl) (string, error) {
	var outError SharedString
	handle := C.pit_param_pnl_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_pnl_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamPnlCheckedAdd(lhs ParamPnl, rhs ParamPnl) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_add failed")
	}
	return out, nil
}

func ParamPnlCheckedSub(lhs ParamPnl, rhs ParamPnl) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_sub failed")
	}
	return out, nil
}

func ParamPnlCheckedNeg(value ParamPnl) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_neg(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_neg failed")
	}
	return out, nil
}

func ParamPnlCheckedMulI64(value ParamPnl, scalar int64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_mul_i64(value, C.int64_t(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamPnlCheckedMulU64(value ParamPnl, scalar uint64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_mul_u64(value, C.uint64_t(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamPnlCheckedMulF64(value ParamPnl, scalar float64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_mul_f64(value, C.double(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamPnlCheckedDivI64(value ParamPnl, divisor int64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_div_i64(value, C.int64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_div_i64 failed")
	}
	return out, nil
}

func ParamPnlCheckedDivU64(value ParamPnl, divisor uint64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_div_u64(value, C.uint64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_div_u64 failed")
	}
	return out, nil
}

func ParamPnlCheckedDivF64(value ParamPnl, divisor float64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_div_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_div_f64 failed")
	}
	return out, nil
}

func ParamPnlCheckedRemI64(value ParamPnl, divisor int64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_rem_i64(value, C.int64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamPnlCheckedRemU64(value ParamPnl, divisor uint64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_rem_u64(value, C.uint64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamPnlCheckedRemF64(value ParamPnl, divisor float64) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_checked_rem_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamPnlToCashFlow(value ParamPnl) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_pnl_to_cash_flow(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_to_cash_flow failed")
	}
	return out, nil
}

func ParamPnlToPositionSize(value ParamPnl) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_pnl_to_position_size(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_to_position_size failed")
	}
	return out, nil
}

func ParamPnlFromFee(fee ParamFee) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_pnl_from_fee(fee, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_pnl_from_fee failed")
	}
	return out, nil
}

func ParamPnlOptionalIsSet(value ParamPnlOptional) bool {
	return bool(value.is_set)
}

func ParamPnlOptionalGet(value ParamPnlOptional) ParamPnl {
	return value.value
}

//------------------------------------------------------------------------------
// Price

func CreateParamPrice(value ParamDecimal) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price(value, &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_price failed")
	}
	return result, nil
}

func CreateParamPriceFromStr(v string) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price_from_str(importString(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_price_from_str failed")
	}
	return result, nil
}

func CreateParamPriceFromF64(v float64) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_price_from_f64 failed")
	}
	return result, nil
}

func CreateParamPriceFromI64(v int64) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_price_from_i64 failed")
	}
	return result, nil
}

func CreateParamPriceFromU64(v uint64) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_price_from_u64 failed")
	}
	return result, nil
}

func CreateParamPriceFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_price_from_str_rounded failed")
	}
	return result, nil
}

func CreateParamPriceFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_price_from_f64_rounded failed")
	}
	return result, nil
}

func CreateParamPriceFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPrice, error) {
	var result ParamPrice
	var outError SharedString
	if !C.pit_create_param_price_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(
				outError,
				"pit_create_param_price_from_decimal_rounded failed",
			)
	}
	return result, nil
}

func ParamPriceGetDecimal(value ParamPrice) ParamDecimal {
	// Keep direct field access for Go hot paths: this is a zero-cost read of the
	// transparent wrapper and avoids extra cgo call overhead.
	return value._0
}

func ParamPriceToF64(value ParamPrice) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_price_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_price_to_f64 failed")
	}
	return float64(out), nil
}

func ParamPriceIsZero(value ParamPrice) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_price_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError, "pit_param_price_is_zero failed")
	}
	return bool(out), nil
}

func ParamPriceCompare(lhs ParamPrice, rhs ParamPrice) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_price_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_price_compare failed")
	}
	return int(out), nil
}

func ParamPriceToString(value ParamPrice) (string, error) {
	var outError SharedString
	handle := C.pit_param_price_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_price_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamPriceCheckedAdd(lhs ParamPrice, rhs ParamPrice) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_add failed")
	}
	return out, nil
}

func ParamPriceCheckedSub(lhs ParamPrice, rhs ParamPrice) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_sub failed")
	}
	return out, nil
}

func ParamPriceCheckedNeg(value ParamPrice) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_neg(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_neg failed")
	}
	return out, nil
}

func ParamPriceCheckedMulI64(value ParamPrice, scalar int64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_mul_i64(value, C.int64_t(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamPriceCheckedMulU64(value ParamPrice, scalar uint64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_mul_u64(value, C.uint64_t(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamPriceCheckedMulF64(value ParamPrice, scalar float64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_mul_f64(value, C.double(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamPriceCheckedDivI64(value ParamPrice, divisor int64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_div_i64(value, C.int64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_div_i64 failed")
	}
	return out, nil
}

func ParamPriceCheckedDivU64(value ParamPrice, divisor uint64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_div_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_div_u64 failed")
	}
	return out, nil
}

func ParamPriceCheckedDivF64(value ParamPrice, divisor float64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_div_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_div_f64 failed")
	}
	return out, nil
}

func ParamPriceCheckedRemI64(value ParamPrice, divisor int64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_rem_i64(value, C.int64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamPriceCheckedRemU64(value ParamPrice, divisor uint64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_rem_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamPriceCheckedRemF64(value ParamPrice, divisor float64) (ParamPrice, error) {
	var out ParamPrice
	var outError SharedString
	if !C.pit_param_price_checked_rem_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamPriceCalculateVolume(price ParamPrice, quantity ParamQuantity) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_price_calculate_volume(price, quantity, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_price_calculate_volume failed")
	}
	return out, nil
}

func ParamPriceOptionalIsSet(value ParamPriceOptional) bool {
	return bool(value.is_set)
}

func ParamPriceOptionalGet(value ParamPriceOptional) ParamPrice {
	return value.value
}

//------------------------------------------------------------------------------
// Quantity

func CreateParamQuantity(value ParamDecimal) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity(value, &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_quantity failed")
	}
	return result, nil
}

func CreateParamQuantityFromStr(v string) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity_from_str(importString(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_quantity_from_str failed")
	}
	return result, nil
}

func CreateParamQuantityFromF64(v float64) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_quantity_from_f64 failed")
	}
	return result, nil
}

func CreateParamQuantityFromI64(v int64) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_quantity_from_i64 failed")
	}
	return result, nil
}

func CreateParamQuantityFromU64(v uint64) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_quantity_from_u64 failed")
	}
	return result, nil
}

func CreateParamQuantityFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_quantity_from_str_rounded failed")
	}
	return result, nil
}

func CreateParamQuantityFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_quantity_from_f64_rounded failed")
	}
	return result, nil
}

func CreateParamQuantityFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamQuantity, error) {
	var result ParamQuantity
	var outError SharedString
	if !C.pit_create_param_quantity_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(
				outError,
				"pit_create_param_quantity_from_decimal_rounded failed",
			)
	}
	return result, nil
}

func ParamQuantityGetDecimal(value ParamQuantity) ParamDecimal {
	// Keep direct field access for Go hot paths: this is a zero-cost read of the
	// transparent wrapper and avoids extra cgo call overhead.
	return value._0
}

func ParamQuantityToF64(value ParamQuantity) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_quantity_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_quantity_to_f64 failed")
	}
	return float64(out), nil
}

func ParamQuantityIsZero(value ParamQuantity) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_quantity_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError, "pit_param_quantity_is_zero failed")
	}
	return bool(out), nil
}

func ParamQuantityCompare(lhs ParamQuantity, rhs ParamQuantity) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_quantity_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_quantity_compare failed")
	}
	return int(out), nil
}

func ParamQuantityToString(value ParamQuantity) (string, error) {
	var outError SharedString
	handle := C.pit_param_quantity_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_quantity_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamQuantityCheckedAdd(lhs ParamQuantity, rhs ParamQuantity) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_add failed")
	}
	return out, nil
}

func ParamQuantityCheckedSub(lhs ParamQuantity, rhs ParamQuantity) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_sub failed")
	}
	return out, nil
}

func ParamQuantityCheckedMulI64(value ParamQuantity, scalar int64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_mul_i64(
		value,
		C.int64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedMulU64(value ParamQuantity, scalar uint64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_mul_u64(
		value,
		C.uint64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedMulF64(value ParamQuantity, scalar float64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_mul_f64(
		value,
		C.double(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedDivI64(value ParamQuantity, divisor int64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_div_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_div_i64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedDivU64(value ParamQuantity, divisor uint64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_div_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_div_u64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedDivF64(value ParamQuantity, divisor float64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_div_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_div_f64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedRemI64(value ParamQuantity, divisor int64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_rem_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedRemU64(value ParamQuantity, divisor uint64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_rem_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamQuantityCheckedRemF64(value ParamQuantity, divisor float64) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_quantity_checked_rem_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamQuantityCalculateVolume(quantity ParamQuantity, price ParamPrice) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_quantity_calculate_volume(quantity, price, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_calculate_volume failed")
	}
	return out, nil
}

func ParamQuantityOptionalIsSet(value ParamQuantityOptional) bool {
	return bool(value.is_set)
}

func ParamQuantityOptionalGet(value ParamQuantityOptional) ParamQuantity {
	return value.value
}

//------------------------------------------------------------------------------
// Volume

func CreateParamVolume(value ParamDecimal) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume(value, &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_volume failed")
	}
	return result, nil
}

func CreateParamVolumeFromStr(v string) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume_from_str(importString(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_volume_from_str failed")
	}
	return result, nil
}

func CreateParamVolumeFromF64(v float64) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_volume_from_f64 failed")
	}
	return result, nil
}

func CreateParamVolumeFromI64(v int64) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_volume_from_i64 failed")
	}
	return result, nil
}

func CreateParamVolumeFromU64(v uint64) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_volume_from_u64 failed")
	}
	return result, nil
}

func CreateParamVolumeFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_volume_from_str_rounded failed")
	}
	return result, nil
}

func CreateParamVolumeFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_volume_from_f64_rounded failed")
	}
	return result, nil
}

func CreateParamVolumeFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamVolume, error) {
	var result ParamVolume
	var outError SharedString
	if !C.pit_create_param_volume_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(
				outError,
				"pit_create_param_volume_from_decimal_rounded failed",
			)
	}
	return result, nil
}

func ParamVolumeGetDecimal(value ParamVolume) ParamDecimal {
	// Keep direct field access for Go hot paths: this is a zero-cost read of the
	// transparent wrapper and avoids extra cgo call overhead.
	return value._0
}

func ParamVolumeToF64(value ParamVolume) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_volume_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_volume_to_f64 failed")
	}
	return float64(out), nil
}

func ParamVolumeIsZero(value ParamVolume) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_volume_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError, "pit_param_volume_is_zero failed")
	}
	return bool(out), nil
}

func ParamVolumeCompare(lhs ParamVolume, rhs ParamVolume) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_volume_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_volume_compare failed")
	}
	return int(out), nil
}

func ParamVolumeToString(value ParamVolume) (string, error) {
	var outError SharedString
	handle := C.pit_param_volume_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_volume_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamVolumeCheckedAdd(lhs ParamVolume, rhs ParamVolume) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_add failed")
	}
	return out, nil
}

func ParamVolumeCheckedSub(lhs ParamVolume, rhs ParamVolume) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_sub failed")
	}
	return out, nil
}

func ParamVolumeCheckedMulI64(value ParamVolume, scalar int64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_mul_i64(value, C.int64_t(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedMulU64(value ParamVolume, scalar uint64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_mul_u64(
		value,
		C.uint64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedMulF64(value ParamVolume, scalar float64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_mul_f64(value, C.double(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedDivI64(value ParamVolume, divisor int64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_div_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_div_i64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedDivU64(value ParamVolume, divisor uint64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_div_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_div_u64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedDivF64(value ParamVolume, divisor float64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_div_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_div_f64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedRemI64(value ParamVolume, divisor int64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_rem_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedRemU64(value ParamVolume, divisor uint64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_rem_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamVolumeCheckedRemF64(value ParamVolume, divisor float64) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_checked_rem_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamVolumeCalculateQuantity(volume ParamVolume, price ParamPrice) (ParamQuantity, error) {
	var out ParamQuantity
	var outError SharedString
	if !C.pit_param_volume_calculate_quantity(volume, price, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_volume_calculate_quantity failed")
	}
	return out, nil
}

func ParamVolumeToCashFlowInflow(value ParamVolume) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_volume_to_cash_flow_inflow(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError,
			"pit_param_volume_to_cash_flow_inflow failed",
		)
	}
	return out, nil
}

func ParamVolumeToCashFlowOutflow(value ParamVolume) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_volume_to_cash_flow_outflow(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError,
			"pit_param_volume_to_cash_flow_outflow failed",
		)
	}
	return out, nil
}

func ParamVolumeOptionalIsSet(value ParamVolumeOptional) bool {
	return bool(value.is_set)
}

func ParamVolumeOptionalGet(value ParamVolumeOptional) ParamVolume {
	return value.value
}

//------------------------------------------------------------------------------
// Notional

func CreateParamNotional(value ParamDecimal) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional(value, &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_notional failed")
	}
	return result, nil
}

func CreateParamNotionalFromStr(v string) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional_from_str(importString(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_notional_from_str failed")
	}
	return result, nil
}

func CreateParamNotionalFromF64(v float64) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_notional_from_f64 failed")
	}
	return result, nil
}

func CreateParamNotionalFromI64(v int64) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_notional_from_i64 failed")
	}
	return result, nil
}

func CreateParamNotionalFromU64(v uint64) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_notional_from_u64 failed")
	}
	return result, nil
}

func CreateParamNotionalFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_notional_from_str_rounded failed")
	}
	return result, nil
}

func CreateParamNotionalFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_notional_from_f64_rounded failed")
	}
	return result, nil
}

func CreateParamNotionalFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamNotional, error) {
	var result ParamNotional
	var outError SharedString
	if !C.pit_create_param_notional_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(
				outError,
				"pit_create_param_notional_from_decimal_rounded failed",
			)
	}
	return result, nil
}

func ParamNotionalGetDecimal(value ParamNotional) ParamDecimal {
	return C.pit_param_notional_get_decimal(value)
}

func ParamNotionalToF64(value ParamNotional) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_notional_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_notional_to_f64 failed")
	}
	return float64(out), nil
}

func ParamNotionalIsZero(value ParamNotional) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_notional_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError, "pit_param_notional_is_zero failed")
	}
	return bool(out), nil
}

func ParamNotionalCompare(lhs ParamNotional, rhs ParamNotional) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_notional_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_notional_compare failed")
	}
	return int(out), nil
}

func ParamNotionalToString(value ParamNotional) (string, error) {
	var outError SharedString
	handle := C.pit_param_notional_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_notional_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamNotionalCheckedAdd(lhs ParamNotional, rhs ParamNotional) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_notional_checked_add failed")
	}
	return out, nil
}

func ParamNotionalCheckedSub(lhs ParamNotional, rhs ParamNotional) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_notional_checked_sub failed")
	}
	return out, nil
}

func ParamNotionalCheckedMulI64(value ParamNotional, scalar int64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_mul_i64(
		value,
		C.int64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedMulU64(value ParamNotional, scalar uint64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_mul_u64(
		value,
		C.uint64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedMulF64(value ParamNotional, scalar float64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_mul_f64(
		value,
		C.double(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedDivI64(value ParamNotional, divisor int64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_div_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_div_i64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedDivU64(value ParamNotional, divisor uint64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_div_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_div_u64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedDivF64(value ParamNotional, divisor float64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_div_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_div_f64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedRemI64(value ParamNotional, divisor int64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_rem_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedRemU64(value ParamNotional, divisor uint64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_rem_u64(value, C.uint64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_notional_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamNotionalCheckedRemF64(value ParamNotional, divisor float64) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_checked_rem_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamNotionalFromVolume(volume ParamVolume) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_from_volume(volume, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_notional_from_volume failed")
	}
	return out, nil
}

func ParamNotionalToVolume(notional ParamNotional) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_notional_to_volume(notional, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_notional_to_volume failed")
	}
	return out, nil
}

func ParamNotionalCalculateMarginRequired(
	notional ParamNotional,
	leverage ParamLeverage,
) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_notional_calculate_margin_required(
		notional,
		leverage,
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(
				outError,
				"pit_param_notional_calculate_margin_required failed",
			)
	}
	return out, nil
}

func ParamNotionalOptionalIsSet(value ParamNotionalOptional) bool {
	return bool(value.is_set)
}

func ParamNotionalOptionalGet(value ParamNotionalOptional) ParamNotional {
	return value.value
}

func ParamPriceCalculateNotional(price ParamPrice, quantity ParamQuantity) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_price_calculate_notional(price, quantity, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_price_calculate_notional failed")
	}
	return out, nil
}

func ParamQuantityCalculateNotional(
	quantity ParamQuantity,
	price ParamPrice,
) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_quantity_calculate_notional(quantity, price, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_quantity_calculate_notional failed")
	}
	return out, nil
}

func ParamVolumeFromNotional(notional ParamNotional) (ParamVolume, error) {
	var out ParamVolume
	var outError SharedString
	if !C.pit_param_volume_from_notional(notional, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_volume_from_notional failed")
	}
	return out, nil
}

//------------------------------------------------------------------------------
// CashFlow

func CreateParamCashFlow(value ParamDecimal) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow(value, &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_cash_flow failed")
	}
	return result, nil
}

func CreateParamCashFlowFromStr(v string) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow_from_str(importString(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_cash_flow_from_str failed")
	}
	return result, nil
}

func CreateParamCashFlowFromF64(v float64) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_cash_flow_from_f64 failed")
	}
	return result, nil
}

func CreateParamCashFlowFromI64(v int64) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_cash_flow_from_i64 failed")
	}
	return result, nil
}

func CreateParamCashFlowFromU64(v uint64) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_cash_flow_from_u64 failed")
	}
	return result, nil
}

func CreateParamCashFlowFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(
				outError,
				"pit_create_param_cash_flow_from_str_rounded failed",
			)
	}
	return result, nil
}

func CreateParamCashFlowFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(
				outError,
				"pit_create_param_cash_flow_from_f64_rounded failed",
			)
	}
	return result, nil
}

func CreateParamCashFlowFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamCashFlow, error) {
	var result ParamCashFlow
	var outError SharedString
	if !C.pit_create_param_cash_flow_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError,
			"pit_create_param_cash_flow_from_decimal_rounded failed",
		)
	}
	return result, nil
}

func ParamCashFlowGetDecimal(value ParamCashFlow) ParamDecimal {
	// Keep direct field access for Go hot paths: this is a zero-cost read of the
	// transparent wrapper and avoids extra cgo call overhead.
	return value._0
}

func ParamCashFlowToF64(value ParamCashFlow) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_cash_flow_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_to_f64 failed")
	}
	return float64(out), nil
}

func ParamCashFlowIsZero(value ParamCashFlow) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_cash_flow_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_is_zero failed")
	}
	return bool(out), nil
}

func ParamCashFlowCompare(lhs ParamCashFlow, rhs ParamCashFlow) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_cash_flow_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_compare failed")
	}
	return int(out), nil
}

func ParamCashFlowToString(value ParamCashFlow) (string, error) {
	var outError SharedString
	handle := C.pit_param_cash_flow_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_cash_flow_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamCashFlowCheckedAdd(lhs ParamCashFlow, rhs ParamCashFlow) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_add failed")
	}
	return out, nil
}

func ParamCashFlowCheckedSub(lhs ParamCashFlow, rhs ParamCashFlow) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_sub failed")
	}
	return out, nil
}

func ParamCashFlowCheckedNeg(value ParamCashFlow) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_neg(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_neg failed")
	}
	return out, nil
}

func ParamCashFlowCheckedMulI64(value ParamCashFlow, scalar int64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_mul_i64(
		value,
		C.int64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedMulU64(value ParamCashFlow, scalar uint64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_mul_u64(
		value,
		C.uint64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedMulF64(value ParamCashFlow, scalar float64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_mul_f64(
		value,
		C.double(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedDivI64(value ParamCashFlow, divisor int64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_div_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_div_i64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedDivU64(value ParamCashFlow, divisor uint64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_div_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_div_u64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedDivF64(value ParamCashFlow, divisor float64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_div_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_div_f64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedRemI64(value ParamCashFlow, divisor int64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_rem_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedRemU64(value ParamCashFlow, divisor uint64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_rem_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamCashFlowCheckedRemF64(value ParamCashFlow, divisor float64) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_checked_rem_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_cash_flow_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamCashFlowFromPnl(value ParamPnl) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_from_pnl(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_from_pnl failed")
	}
	return out, nil
}

func ParamCashFlowFromFee(value ParamFee) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_from_fee(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_cash_flow_from_fee failed")
	}
	return out, nil
}

func ParamCashFlowFromVolumeInflow(value ParamVolume) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_from_volume_inflow(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(
			outError,
			"pit_param_cash_flow_from_volume_inflow failed",
		)
	}
	return out, nil
}

func ParamCashFlowFromVolumeOutflow(value ParamVolume) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_cash_flow_from_volume_outflow(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(
			outError,
			"pit_param_cash_flow_from_volume_outflow failed",
		)
	}
	return out, nil
}

func ParamCashFlowOptionalIsSet(value ParamCashFlowOptional) bool {
	return bool(value.is_set)
}

func ParamCashFlowOptionalGet(value ParamCashFlowOptional) ParamCashFlow {
	return value.value
}

//------------------------------------------------------------------------------
// PositionSize

func CreateParamPositionSize(value ParamDecimal) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size(value, &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_position_size failed")
	}
	return result, nil
}

func CreateParamPositionSizeFromStr(v string) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size_from_str(
		importString(v),
		&result,
		C.PitOutError(&outError),
	) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_position_size_from_str failed")
	}
	return result, nil
}

func CreateParamPositionSizeFromF64(v float64) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_position_size_from_f64 failed")
	}
	return result, nil
}

func CreateParamPositionSizeFromI64(v int64) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_position_size_from_i64 failed")
	}
	return result, nil
}

func CreateParamPositionSizeFromU64(v uint64) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_position_size_from_u64 failed")
	}
	return result, nil
}

func CreateParamPositionSizeFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError,
			"pit_create_param_position_size_from_str_rounded failed",
		)
	}
	return result, nil
}

func CreateParamPositionSizeFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError,
			"pit_create_param_position_size_from_f64_rounded failed",
		)
	}
	return result, nil
}

func CreateParamPositionSizeFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamPositionSize, error) {
	var result ParamPositionSize
	var outError SharedString
	if !C.pit_create_param_position_size_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError,
			"pit_create_param_position_size_from_decimal_rounded failed",
		)
	}
	return result, nil
}

func ParamPositionSizeGetDecimal(value ParamPositionSize) ParamDecimal {
	// Keep direct field access for Go hot paths: this is a zero-cost read of the
	// transparent wrapper and avoids extra cgo call overhead.
	return value._0
}

func ParamPositionSizeToF64(value ParamPositionSize) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_position_size_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_position_size_to_f64 failed")
	}
	return float64(out), nil
}

func ParamPositionSizeIsZero(value ParamPositionSize) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_position_size_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError,
			"pit_param_position_size_is_zero failed",
		)
	}
	return bool(out), nil
}

func ParamPositionSizeCompare(
	lhs ParamPositionSize,
	rhs ParamPositionSize,
) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_position_size_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_position_size_compare failed")
	}
	return int(out), nil
}

func ParamPositionSizeToString(value ParamPositionSize) (string, error) {
	var outError SharedString
	handle := C.pit_param_position_size_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_position_size_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamPositionSizeCheckedAdd(
	lhs ParamPositionSize,
	rhs ParamPositionSize,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_add failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedSub(
	lhs ParamPositionSize,
	rhs ParamPositionSize,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_sub failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedNeg(value ParamPositionSize) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_neg(value, &out, C.PitOutError(&outError)) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_neg failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedMulI64(
	value ParamPositionSize,
	scalar int64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_mul_i64(
		value,
		C.int64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedMulU64(
	value ParamPositionSize,
	scalar uint64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_mul_u64(
		value,
		C.uint64_t(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedMulF64(
	value ParamPositionSize,
	scalar float64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_mul_f64(
		value,
		C.double(scalar),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedDivI64(
	value ParamPositionSize,
	divisor int64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_div_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_div_i64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedDivU64(
	value ParamPositionSize,
	divisor uint64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_div_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_div_u64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedDivF64(
	value ParamPositionSize,
	divisor float64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_div_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_div_f64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedRemI64(
	value ParamPositionSize,
	divisor int64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_rem_i64(
		value,
		C.int64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedRemU64(
	value ParamPositionSize,
	divisor uint64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_rem_u64(
		value,
		C.uint64_t(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamPositionSizeCheckedRemF64(
	value ParamPositionSize,
	divisor float64,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_rem_f64(
		value,
		C.double(divisor),
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(outError, "pit_param_position_size_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamPositionSizeFromPnl(value ParamPnl) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_from_pnl(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_position_size_from_pnl failed")
	}
	return out, nil
}

func ParamPositionSizeFromFee(value ParamFee) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_from_fee(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_position_size_from_fee failed")
	}
	return out, nil
}

func ParamPositionSizeFromQuantityAndSide(
	quantity ParamQuantity,
	side ParamSide,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_from_quantity_and_side(
		quantity,
		side,
		&out,
		C.PitOutError(&outError),
	) {
		return out, consumeSharedStringAsParamError(outError,
			"pit_param_position_size_from_quantity_and_side failed",
		)
	}
	return out, nil
}

func ParamPositionSizeToOpenQuantity(
	value ParamPositionSize,
) (ParamQuantity, ParamSide, error) {
	var quantity ParamQuantity
	var side ParamSide
	var outError SharedString
	if !C.pit_param_position_size_to_open_quantity(
		value,
		&quantity,
		&side,
		C.PitOutError(&outError),
	) {
		return quantity, side, consumeSharedStringAsParamError(outError,
			"pit_param_position_size_to_open_quantity failed",
		)
	}
	return quantity, side, nil
}

func ParamPositionSizeToCloseQuantity(
	value ParamPositionSize,
) (ParamQuantity, ParamSide, error) {
	var quantity ParamQuantity
	var side ParamSide
	var outError SharedString
	if !C.pit_param_position_size_to_close_quantity(
		value,
		&quantity,
		&side,
		C.PitOutError(&outError),
	) {
		return quantity,
			side,
			consumeSharedStringAsParamError(
				outError,
				"pit_param_position_size_to_close_quantity failed",
			)
	}
	return quantity, side, nil
}

func ParamPositionSizeCheckedAddQuantity(
	value ParamPositionSize,
	quantity ParamQuantity,
	side ParamSide,
) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_position_size_checked_add_quantity(
		value,
		quantity,
		side,
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(
				outError,
				"pit_param_position_size_checked_add_quantity failed",
			)
	}
	return out, nil
}

func ParamPositionSizeOptionalIsSet(value ParamPositionSizeOptional) bool {
	return bool(value.is_set)
}

func ParamPositionSizeOptionalGet(value ParamPositionSizeOptional) ParamPositionSize {
	return value.value
}

//------------------------------------------------------------------------------
// Fee

func CreateParamFee(value ParamDecimal) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee(value, &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_fee failed")
	}
	return result, nil
}

func CreateParamFeeFromStr(v string) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee_from_str(importString(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_fee_from_str failed")
	}
	return result, nil
}

func CreateParamFeeFromF64(v float64) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee_from_f64(C.double(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_fee_from_f64 failed")
	}
	return result, nil
}

func CreateParamFeeFromI64(v int64) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee_from_i64(C.int64_t(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_fee_from_i64 failed")
	}
	return result, nil
}

func CreateParamFeeFromU64(v uint64) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee_from_u64(C.uint64_t(v), &result, C.PitOutError(&outError)) {
		return result, consumeSharedStringAsParamError(outError, "pit_create_param_fee_from_u64 failed")
	}
	return result, nil
}

func CreateParamFeeFromStrRounded(
	v string,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee_from_str_rounded(
		importString(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_fee_from_str_rounded failed")
	}
	return result, nil
}

func CreateParamFeeFromF64Rounded(
	v float64,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee_from_f64_rounded(
		C.double(v),
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_fee_from_f64_rounded failed")
	}
	return result, nil
}

func CreateParamFeeFromDecimalRounded(
	v ParamDecimal,
	scale uint32,
	strategy ParamRoundingStrategy,
) (ParamFee, error) {
	var result ParamFee
	var outError SharedString
	if !C.pit_create_param_fee_from_decimal_rounded(
		v,
		C.uint32_t(scale),
		C.uint8_t(strategy),
		&result,
		C.PitOutError(&outError)) {
		return result,
			consumeSharedStringAsParamError(outError, "pit_create_param_fee_from_decimal_rounded failed")
	}
	return result, nil
}

func ParamFeeGetDecimal(value ParamFee) ParamDecimal {
	// Keep direct field access for Go hot paths: this is a zero-cost read of the
	// transparent wrapper and avoids extra cgo call overhead.
	return value._0
}

func ParamFeeToF64(value ParamFee) (float64, error) {
	var out C.double
	var outError SharedString
	if !C.pit_param_fee_to_f64(value, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_fee_to_f64 failed")
	}
	return float64(out), nil
}

func ParamFeeIsZero(value ParamFee) (bool, error) {
	var out C.bool
	var outError SharedString
	if !C.pit_param_fee_is_zero(value, &out, C.PitOutError(&outError)) {
		return false, consumeSharedStringAsParamError(outError, "pit_param_fee_is_zero failed")
	}
	return bool(out), nil
}

func ParamFeeCompare(lhs ParamFee, rhs ParamFee) (int, error) {
	var out C.int8_t
	var outError SharedString
	if !C.pit_param_fee_compare(lhs, rhs, &out, C.PitOutError(&outError)) {
		return 0, consumeSharedStringAsParamError(outError, "pit_param_fee_compare failed")
	}
	return int(out), nil
}

func ParamFeeToString(value ParamFee) (string, error) {
	var outError SharedString
	handle := C.pit_param_fee_to_string(value, C.PitOutError(&outError))
	if handle == nil {
		return "", consumeSharedStringAsParamError(outError, "pit_param_fee_to_string failed")
	}
	return consumeSharedString(handle), nil
}

func ParamFeeCheckedAdd(lhs ParamFee, rhs ParamFee) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_add(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_add failed")
	}
	return out, nil
}

func ParamFeeCheckedSub(lhs ParamFee, rhs ParamFee) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_sub(lhs, rhs, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_sub failed")
	}
	return out, nil
}

func ParamFeeCheckedNeg(value ParamFee) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_neg(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_neg failed")
	}
	return out, nil
}

func ParamFeeCheckedMulI64(value ParamFee, scalar int64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_mul_i64(value, C.int64_t(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_mul_i64 failed")
	}
	return out, nil
}

func ParamFeeCheckedMulU64(value ParamFee, scalar uint64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_mul_u64(value, C.uint64_t(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_mul_u64 failed")
	}
	return out, nil
}

func ParamFeeCheckedMulF64(value ParamFee, scalar float64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_mul_f64(value, C.double(scalar), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_mul_f64 failed")
	}
	return out, nil
}

func ParamFeeCheckedDivI64(value ParamFee, divisor int64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_div_i64(value, C.int64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_div_i64 failed")
	}
	return out, nil
}

func ParamFeeCheckedDivU64(value ParamFee, divisor uint64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_div_u64(value, C.uint64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_div_u64 failed")
	}
	return out, nil
}

func ParamFeeCheckedDivF64(value ParamFee, divisor float64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_div_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_div_f64 failed")
	}
	return out, nil
}

func ParamFeeCheckedRemI64(value ParamFee, divisor int64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_rem_i64(value, C.int64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_rem_i64 failed")
	}
	return out, nil
}

func ParamFeeCheckedRemU64(value ParamFee, divisor uint64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_rem_u64(value, C.uint64_t(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_rem_u64 failed")
	}
	return out, nil
}

func ParamFeeCheckedRemF64(value ParamFee, divisor float64) (ParamFee, error) {
	var out ParamFee
	var outError SharedString
	if !C.pit_param_fee_checked_rem_f64(value, C.double(divisor), &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_checked_rem_f64 failed")
	}
	return out, nil
}

func ParamFeeToPnl(value ParamFee) (ParamPnl, error) {
	var out ParamPnl
	var outError SharedString
	if !C.pit_param_fee_to_pnl(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_to_pnl failed")
	}
	return out, nil
}

func ParamFeeToPositionSize(value ParamFee) (ParamPositionSize, error) {
	var out ParamPositionSize
	var outError SharedString
	if !C.pit_param_fee_to_position_size(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_to_position_size failed")
	}
	return out, nil
}

func ParamFeeToCashFlow(value ParamFee) (ParamCashFlow, error) {
	var out ParamCashFlow
	var outError SharedString
	if !C.pit_param_fee_to_cash_flow(value, &out, C.PitOutError(&outError)) {
		return out, consumeSharedStringAsParamError(outError, "pit_param_fee_to_cash_flow failed")
	}
	return out, nil
}

func ParamFeeOptionalIsSet(value ParamFeeOptional) bool {
	return bool(value.is_set)
}

func ParamFeeOptionalGet(value ParamFeeOptional) ParamFee {
	return value.value
}

//------------------------------------------------------------------------------
// AccountID

func CreateParamAccountIDFromU64(value uint64) ParamAccountID {
	return C.pit_create_param_account_id_from_u64(C.uint64_t(value))
}

func CreateParamAccountIDFromStr(value string) ParamAccountID {
	return C.pit_create_param_account_id_from_str(importString(value))
}

func ParamAccountIDOptionalIsSet(value ParamAccountIDOptional) bool {
	return bool(value.is_set)
}

func ParamAccountIDOptionalGet(value ParamAccountIDOptional) ParamAccountID {
	return value.value
}

//------------------------------------------------------------------------------
// TradeAmount

func CreateParamTradeAmount(kind ParamTradeAmountKind, value ParamDecimal) ParamTradeAmount {
	return ParamTradeAmount{
		kind:  kind,
		value: value,
	}
}

func ParamTradeAmountGetKind(amount ParamTradeAmount) ParamTradeAmountKind {
	return amount.kind
}

func ParamTradeAmountGetValue(amount ParamTradeAmount) ParamDecimal {
	return amount.value
}

//------------------------------------------------------------------------------
// AdjustmentAmount

func CreateParamAdjustmentAmount(
	kind ParamAdjustmentAmountKind,
	value ParamPositionSize,
) ParamAdjustmentAmount {
	return ParamAdjustmentAmount{kind: kind, value: value}
}

func ParamAdjustmentAmountGetKind(amount ParamAdjustmentAmount) ParamAdjustmentAmountKind {
	return amount.kind
}

func ParamAdjustmentAmountGetValue(amount ParamAdjustmentAmount) ParamPositionSize {
	return amount.value
}

//------------------------------------------------------------------------------
// Leverage

func ParamLeverageCalculateMarginRequired(
	leverage ParamLeverage,
	notional ParamNotional,
) (ParamNotional, error) {
	var out ParamNotional
	var outError SharedString
	if !C.pit_param_leverage_calculate_margin_required(
		leverage,
		notional,
		&out,
		C.PitOutError(&outError),
	) {
		return out,
			consumeSharedStringAsParamError(
				outError,
				"pit_param_leverage_calculate_margin_required failed",
			)
	}
	return out, nil
}

//------------------------------------------------------------------------------
