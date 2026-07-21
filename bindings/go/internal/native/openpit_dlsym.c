/*
 * Copyright The Pit Project Owners. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Please see https://openpit.dev and the OWNERS file for details.
 *
 * Generated file. Do not edit manually.
 */

#include "openpit.h"

#ifdef _WIN32
#include <windows.h>
static void *openpit_dlsym(void *handle, const char *name) {
    return (void *)(uintptr_t)GetProcAddress((HMODULE)handle, name);
}
#else
#include <dlfcn.h>
static void *openpit_dlsym(void *handle, const char *name) {
    return dlsym(handle, name);
}
#endif

/* Function pointers resolved via openpit_dlsym after the runtime is loaded. */
static bool (*_fn_openpit_create_param_pnl)(OpenPitParamDecimal, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_pnl_get_decimal)(OpenPitParamPnl) = NULL;
static bool (*_fn_openpit_create_param_price)(OpenPitParamDecimal, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_price_get_decimal)(OpenPitParamPrice) = NULL;
static bool (*_fn_openpit_create_param_quantity)(OpenPitParamDecimal, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_quantity_get_decimal)(OpenPitParamQuantity) = NULL;
static bool (*_fn_openpit_create_param_volume)(OpenPitParamDecimal, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_volume_get_decimal)(OpenPitParamVolume) = NULL;
static bool (*_fn_openpit_create_param_cash_flow)(OpenPitParamDecimal, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_cash_flow_get_decimal)(OpenPitParamCashFlow) = NULL;
static bool (*_fn_openpit_create_param_position_size)(OpenPitParamDecimal, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_position_size_get_decimal)(OpenPitParamPositionSize) = NULL;
static bool (*_fn_openpit_create_param_fee)(OpenPitParamDecimal, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_fee_get_decimal)(OpenPitParamFee) = NULL;
static bool (*_fn_openpit_create_param_notional)(OpenPitParamDecimal, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static OpenPitParamDecimal (*_fn_openpit_param_notional_get_decimal)(OpenPitParamNotional) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_string)(OpenPitStringView, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_f64)(double, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_int64)(int64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_uint64)(uint64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_to_f64)(OpenPitParamPnl, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_is_zero)(OpenPitParamPnl, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_compare)(OpenPitParamPnl, OpenPitParamPnl, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_pnl_to_string)(OpenPitParamPnl, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_add)(OpenPitParamPnl, OpenPitParamPnl, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_sub)(OpenPitParamPnl, OpenPitParamPnl, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_mul_i64)(OpenPitParamPnl, int64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_mul_u64)(OpenPitParamPnl, uint64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_mul_f64)(OpenPitParamPnl, double, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_div_i64)(OpenPitParamPnl, int64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_div_u64)(OpenPitParamPnl, uint64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_div_f64)(OpenPitParamPnl, double, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_rem_i64)(OpenPitParamPnl, int64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_rem_u64)(OpenPitParamPnl, uint64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_rem_f64)(OpenPitParamPnl, double, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_checked_neg)(OpenPitParamPnl, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_string)(OpenPitStringView, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_f64)(double, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_int64)(int64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_uint64)(uint64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_to_f64)(OpenPitParamPrice, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_is_zero)(OpenPitParamPrice, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_compare)(OpenPitParamPrice, OpenPitParamPrice, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_price_to_string)(OpenPitParamPrice, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_add)(OpenPitParamPrice, OpenPitParamPrice, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_sub)(OpenPitParamPrice, OpenPitParamPrice, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_mul_i64)(OpenPitParamPrice, int64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_mul_u64)(OpenPitParamPrice, uint64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_mul_f64)(OpenPitParamPrice, double, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_div_i64)(OpenPitParamPrice, int64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_div_u64)(OpenPitParamPrice, uint64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_div_f64)(OpenPitParamPrice, double, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_rem_i64)(OpenPitParamPrice, int64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_rem_u64)(OpenPitParamPrice, uint64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_rem_f64)(OpenPitParamPrice, double, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_checked_neg)(OpenPitParamPrice, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_string)(OpenPitStringView, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_f64)(double, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_int64)(int64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_uint64)(uint64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_to_f64)(OpenPitParamQuantity, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_is_zero)(OpenPitParamQuantity, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_compare)(OpenPitParamQuantity, OpenPitParamQuantity, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_quantity_to_string)(OpenPitParamQuantity, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_add)(OpenPitParamQuantity, OpenPitParamQuantity, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_sub)(OpenPitParamQuantity, OpenPitParamQuantity, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_mul_i64)(OpenPitParamQuantity, int64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_mul_u64)(OpenPitParamQuantity, uint64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_mul_f64)(OpenPitParamQuantity, double, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_div_i64)(OpenPitParamQuantity, int64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_div_u64)(OpenPitParamQuantity, uint64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_div_f64)(OpenPitParamQuantity, double, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_rem_i64)(OpenPitParamQuantity, int64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_rem_u64)(OpenPitParamQuantity, uint64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_checked_rem_f64)(OpenPitParamQuantity, double, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_string)(OpenPitStringView, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_f64)(double, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_int64)(int64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_uint64)(uint64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_to_f64)(OpenPitParamVolume, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_is_zero)(OpenPitParamVolume, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_compare)(OpenPitParamVolume, OpenPitParamVolume, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_volume_to_string)(OpenPitParamVolume, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_add)(OpenPitParamVolume, OpenPitParamVolume, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_sub)(OpenPitParamVolume, OpenPitParamVolume, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_mul_i64)(OpenPitParamVolume, int64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_mul_u64)(OpenPitParamVolume, uint64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_mul_f64)(OpenPitParamVolume, double, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_div_i64)(OpenPitParamVolume, int64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_div_u64)(OpenPitParamVolume, uint64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_div_f64)(OpenPitParamVolume, double, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_rem_i64)(OpenPitParamVolume, int64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_rem_u64)(OpenPitParamVolume, uint64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_checked_rem_f64)(OpenPitParamVolume, double, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_string)(OpenPitStringView, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_f64)(double, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_int64)(int64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_uint64)(uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_to_f64)(OpenPitParamCashFlow, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_is_zero)(OpenPitParamCashFlow, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_compare)(OpenPitParamCashFlow, OpenPitParamCashFlow, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_cash_flow_to_string)(OpenPitParamCashFlow, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_add)(OpenPitParamCashFlow, OpenPitParamCashFlow, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_sub)(OpenPitParamCashFlow, OpenPitParamCashFlow, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_mul_i64)(OpenPitParamCashFlow, int64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_mul_u64)(OpenPitParamCashFlow, uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_mul_f64)(OpenPitParamCashFlow, double, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_div_i64)(OpenPitParamCashFlow, int64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_div_u64)(OpenPitParamCashFlow, uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_div_f64)(OpenPitParamCashFlow, double, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_rem_i64)(OpenPitParamCashFlow, int64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_rem_u64)(OpenPitParamCashFlow, uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_rem_f64)(OpenPitParamCashFlow, double, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_checked_neg)(OpenPitParamCashFlow, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_string)(OpenPitStringView, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_f64)(double, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_int64)(int64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_uint64)(uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_to_f64)(OpenPitParamPositionSize, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_is_zero)(OpenPitParamPositionSize, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_compare)(OpenPitParamPositionSize, OpenPitParamPositionSize, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_position_size_to_string)(OpenPitParamPositionSize, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_add)(OpenPitParamPositionSize, OpenPitParamPositionSize, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_sub)(OpenPitParamPositionSize, OpenPitParamPositionSize, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_mul_i64)(OpenPitParamPositionSize, int64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_mul_u64)(OpenPitParamPositionSize, uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_mul_f64)(OpenPitParamPositionSize, double, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_div_i64)(OpenPitParamPositionSize, int64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_div_u64)(OpenPitParamPositionSize, uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_div_f64)(OpenPitParamPositionSize, double, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_rem_i64)(OpenPitParamPositionSize, int64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_rem_u64)(OpenPitParamPositionSize, uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_rem_f64)(OpenPitParamPositionSize, double, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_neg)(OpenPitParamPositionSize, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_string)(OpenPitStringView, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_f64)(double, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_int64)(int64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_uint64)(uint64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_to_f64)(OpenPitParamFee, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_is_zero)(OpenPitParamFee, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_compare)(OpenPitParamFee, OpenPitParamFee, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_fee_to_string)(OpenPitParamFee, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_add)(OpenPitParamFee, OpenPitParamFee, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_sub)(OpenPitParamFee, OpenPitParamFee, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_mul_i64)(OpenPitParamFee, int64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_mul_u64)(OpenPitParamFee, uint64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_mul_f64)(OpenPitParamFee, double, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_div_i64)(OpenPitParamFee, int64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_div_u64)(OpenPitParamFee, uint64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_div_f64)(OpenPitParamFee, double, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_rem_i64)(OpenPitParamFee, int64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_rem_u64)(OpenPitParamFee, uint64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_rem_f64)(OpenPitParamFee, double, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_checked_neg)(OpenPitParamFee, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_string)(OpenPitStringView, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_f64)(double, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_int64)(int64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_uint64)(uint64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_string_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_f64_rounded)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_decimal_rounded)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_to_f64)(OpenPitParamNotional, double *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_is_zero)(OpenPitParamNotional, bool *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_compare)(OpenPitParamNotional, OpenPitParamNotional, int8_t *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_notional_to_string)(OpenPitParamNotional, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_add)(OpenPitParamNotional, OpenPitParamNotional, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_sub)(OpenPitParamNotional, OpenPitParamNotional, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_mul_i64)(OpenPitParamNotional, int64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_mul_u64)(OpenPitParamNotional, uint64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_mul_f64)(OpenPitParamNotional, double, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_div_i64)(OpenPitParamNotional, int64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_div_u64)(OpenPitParamNotional, uint64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_div_f64)(OpenPitParamNotional, double, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_rem_i64)(OpenPitParamNotional, int64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_rem_u64)(OpenPitParamNotional, uint64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_checked_rem_f64)(OpenPitParamNotional, double, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_leverage_calculate_margin_required)(OpenPitParamLeverage, OpenPitParamNotional, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_calculate_volume)(OpenPitParamPrice, OpenPitParamQuantity, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_calculate_position_size)(OpenPitParamPrice, OpenPitParamQuantity, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_calculate_volume)(OpenPitParamQuantity, OpenPitParamPrice, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_calculate_quantity)(OpenPitParamVolume, OpenPitParamPrice, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_to_cash_flow)(OpenPitParamPnl, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_to_position_size)(OpenPitParamPnl, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_to_position_size)(OpenPitParamQuantity, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_to_position_size)(OpenPitParamVolume, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_from_fee)(OpenPitParamFee, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_from_pnl)(OpenPitParamPnl, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_from_fee)(OpenPitParamFee, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_from_volume_inflow)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_cash_flow_from_volume_outflow)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_to_pnl)(OpenPitParamFee, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_to_position_size)(OpenPitParamFee, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_fee_to_cash_flow)(OpenPitParamFee, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_to_cash_flow_inflow)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_to_cash_flow_outflow)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_from_pnl)(OpenPitParamPnl, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_from_fee)(OpenPitParamFee, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_from_quantity_and_side)(OpenPitParamQuantity, OpenPitParamSide, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_to_open_quantity)(OpenPitParamPositionSize, OpenPitParamQuantity *, OpenPitParamSide *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_to_close_quantity)(OpenPitParamPositionSize, OpenPitParamQuantity *, OpenPitParamSide *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_position_size_checked_add_quantity)(OpenPitParamPositionSize, OpenPitParamQuantity, OpenPitParamSide, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_price_calculate_notional)(OpenPitParamPrice, OpenPitParamQuantity, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_quantity_calculate_notional)(OpenPitParamQuantity, OpenPitParamPrice, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_from_volume)(OpenPitParamVolume, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_to_volume)(OpenPitParamNotional, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_notional_calculate_margin_required)(OpenPitParamNotional, OpenPitParamLeverage, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_from_notional)(OpenPitParamNotional, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static OpenPitParamAccountId (*_fn_openpit_create_param_account_id_from_uint64)(uint64_t) = NULL;
static bool (*_fn_openpit_create_param_account_id_from_string)(OpenPitStringView, OpenPitParamAccountId *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_create_param_asset_from_string)(OpenPitStringView, OpenPitOutParamError) = NULL;
static void (*_fn_openpit_destroy_param_asset)(OpenPitSharedString *) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_side_to_string)(OpenPitParamSide, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_position_side_to_string)(OpenPitParamPositionSide, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_position_effect_to_string)(OpenPitParamPositionEffect, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_position_mode_to_string)(OpenPitParamPositionMode, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_account_id_to_string)(OpenPitParamAccountId) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_trade_amount_to_string)(OpenPitParamTradeAmount, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_param_adjustment_amount_to_string)(OpenPitParamAdjustmentAmount, OpenPitOutParamError) = NULL;
static OpenPitPretradeRejectList * (*_fn_openpit_pretrade_create_reject_list)(size_t) = NULL;
static void (*_fn_openpit_pretrade_destroy_reject_list)(OpenPitPretradeRejectList *) = NULL;
static bool (*_fn_openpit_pretrade_reject_list_push)(OpenPitPretradeRejectList *, OpenPitPretradeReject) = NULL;
static size_t (*_fn_openpit_pretrade_reject_list_len)(const OpenPitPretradeRejectList *) = NULL;
static bool (*_fn_openpit_pretrade_reject_list_get)(const OpenPitPretradeRejectList *, size_t, OpenPitPretradeReject *) = NULL;
static OpenPitPretradeAccountBlockList * (*_fn_openpit_pretrade_create_account_block_list)(size_t) = NULL;
static void (*_fn_openpit_pretrade_destroy_account_block_list)(OpenPitPretradeAccountBlockList *) = NULL;
static void (*_fn_openpit_pretrade_account_block_list_push)(OpenPitPretradeAccountBlockList *, OpenPitPretradeAccountBlock) = NULL;
static size_t (*_fn_openpit_pretrade_account_block_list_len)(const OpenPitPretradeAccountBlockList *) = NULL;
static bool (*_fn_openpit_pretrade_account_block_list_get)(const OpenPitPretradeAccountBlockList *, size_t, OpenPitPretradeAccountBlock *) = NULL;
static void (*_fn_openpit_destroy_param_error)(OpenPitParamError *) = NULL;
static OpenPitEngineBuilder * (*_fn_openpit_create_engine_builder)(uint8_t, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_engine_builder)(OpenPitEngineBuilder *) = NULL;
static OpenPitEngine * (*_fn_openpit_engine_builder_build)(OpenPitEngineBuilder *, OpenPitEngineBuildError **, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_engine_build_error)(OpenPitEngineBuildError *) = NULL;
static OpenPitEngineBuildErrorCode (*_fn_openpit_engine_build_error_get_code)(const OpenPitEngineBuildError *) = NULL;
static OpenPitStringView (*_fn_openpit_engine_build_error_get_policy_name)(const OpenPitEngineBuildError *) = NULL;
static uint16_t (*_fn_openpit_engine_build_error_get_policy_group_id)(const OpenPitEngineBuildError *) = NULL;
static void (*_fn_openpit_destroy_engine)(OpenPitEngine *) = NULL;
static OpenPitPretradeStatus (*_fn_openpit_engine_start_pre_trade)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeRequest **, OpenPitPretradeRejectList **, OpenPitOutError) = NULL;
static OpenPitPretradeStatus (*_fn_openpit_engine_execute_pre_trade)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_execute_pre_trade_drop_copy)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeReservation **, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_start_pre_trade_dry_run)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeDryRunReport **, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_execute_pre_trade_dry_run)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeDryRunReport **, OpenPitOutError) = NULL;
static OpenPitPretradeStatus (*_fn_openpit_pretrade_pre_trade_request_execute)(OpenPitPretradePreTradeRequest *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_request)(OpenPitPretradePreTradeRequest *) = NULL;
static void (*_fn_openpit_pretrade_pre_trade_reservation_commit)(OpenPitPretradePreTradeReservation *) = NULL;
static void (*_fn_openpit_pretrade_pre_trade_reservation_rollback)(OpenPitPretradePreTradeReservation *) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_pretrade_pre_trade_reservation_get_lock)(const OpenPitPretradePreTradeReservation *) = NULL;
static OpenPitAccountAdjustmentOutcomeList * (*_fn_openpit_pretrade_pre_trade_reservation_get_account_adjustments)(const OpenPitPretradePreTradeReservation *) = NULL;
static OpenPitPretradeAccountBlockList * (*_fn_openpit_pretrade_pre_trade_reservation_get_account_block)(const OpenPitPretradePreTradeReservation *) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_reservation)(OpenPitPretradePreTradeReservation *) = NULL;
static bool (*_fn_openpit_pretrade_pre_trade_dry_run_report_is_pass)(const OpenPitPretradePreTradeDryRunReport *) = NULL;
static OpenPitPretradeRejectList * (*_fn_openpit_pretrade_pre_trade_dry_run_report_get_rejects)(const OpenPitPretradePreTradeDryRunReport *) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_pretrade_pre_trade_dry_run_report_get_lock)(const OpenPitPretradePreTradeDryRunReport *) = NULL;
static OpenPitAccountAdjustmentOutcomeList * (*_fn_openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments)(const OpenPitPretradePreTradeDryRunReport *) = NULL;
static OpenPitPretradeAccountBlockList * (*_fn_openpit_pretrade_pre_trade_dry_run_report_get_account_block)(const OpenPitPretradePreTradeDryRunReport *) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_dry_run_report)(OpenPitPretradePreTradeDryRunReport *) = NULL;
static bool (*_fn_openpit_engine_apply_execution_report)(OpenPitEngine *, const OpenPitExecutionReport *, OpenPitPostTradeResult **, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_post_trade_result)(OpenPitPostTradeResult *) = NULL;
static const OpenPitPretradeAccountBlockList * (*_fn_openpit_post_trade_result_get_account_blocks)(const OpenPitPostTradeResult *) = NULL;
static const OpenPitAccountAdjustmentOutcomeList * (*_fn_openpit_post_trade_result_get_account_adjustments)(const OpenPitPostTradeResult *) = NULL;
static const OpenPitAccountPnlOutcomeList * (*_fn_openpit_post_trade_result_get_account_pnls)(const OpenPitPostTradeResult *) = NULL;
static void (*_fn_openpit_destroy_account_adjustment_batch_error)(OpenPitAccountAdjustmentBatchError *) = NULL;
static size_t (*_fn_openpit_account_adjustment_batch_error_get_failed_adjustment_index)(const OpenPitAccountAdjustmentBatchError *) = NULL;
static const OpenPitPretradeRejectList * (*_fn_openpit_account_adjustment_batch_error_get_rejects)(const OpenPitAccountAdjustmentBatchError *) = NULL;
static OpenPitAccountAdjustmentApplyStatus (*_fn_openpit_engine_apply_account_adjustment)(OpenPitEngine *, OpenPitParamAccountId, const OpenPitAccountAdjustment *, size_t, OpenPitAccountAdjustmentBatchError **, OpenPitAccountAdjustmentOutcomeList **, OpenPitPretradeAccountBlockList **, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_account_group_error)(OpenPitAccountGroupError *) = NULL;
static OpenPitStringView (*_fn_openpit_account_group_error_get_message)(const OpenPitAccountGroupError *) = NULL;
static OpenPitParamAccountId (*_fn_openpit_account_group_error_get_account)(const OpenPitAccountGroupError *) = NULL;
static bool (*_fn_openpit_account_group_error_get_current_group)(const OpenPitAccountGroupError *, OpenPitParamAccountGroupId *) = NULL;
static bool (*_fn_openpit_engine_register_account_group)(OpenPitEngine *, const OpenPitParamAccountId *, size_t, OpenPitParamAccountGroupId, OpenPitAccountGroupError **, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_unregister_account_group)(OpenPitEngine *, const OpenPitParamAccountId *, size_t, OpenPitParamAccountGroupId, OpenPitAccountGroupError **, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_account_group)(const OpenPitEngine *, OpenPitParamAccountId, OpenPitParamAccountGroupId *) = NULL;
static bool (*_fn_openpit_engine_set_account_currency)(OpenPitEngine *, OpenPitParamAccountId, OpenPitStringView, OpenPitOutError) = NULL;
static void (*_fn_openpit_engine_clear_account_currency)(OpenPitEngine *, OpenPitParamAccountId) = NULL;
static bool (*_fn_openpit_engine_set_account_group_currency)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitStringView, OpenPitOutError) = NULL;
static void (*_fn_openpit_engine_clear_account_group_currency)(OpenPitEngine *, OpenPitParamAccountGroupId) = NULL;
static void (*_fn_openpit_destroy_account_block_error)(OpenPitAccountBlockError *) = NULL;
static OpenPitStringView (*_fn_openpit_account_block_error_get_message)(const OpenPitAccountBlockError *) = NULL;
static OpenPitAccountBlockErrorKind (*_fn_openpit_account_block_error_get_kind)(const OpenPitAccountBlockError *) = NULL;
static bool (*_fn_openpit_account_block_error_get_account)(const OpenPitAccountBlockError *, OpenPitParamAccountId *) = NULL;
static bool (*_fn_openpit_account_block_error_get_group)(const OpenPitAccountBlockError *, OpenPitParamAccountGroupId *) = NULL;
static void (*_fn_openpit_destroy_configure_error)(OpenPitConfigureError *) = NULL;
static OpenPitStringView (*_fn_openpit_configure_error_get_message)(const OpenPitConfigureError *) = NULL;
static OpenPitConfigureErrorKind (*_fn_openpit_configure_error_get_kind)(const OpenPitConfigureError *) = NULL;
static void (*_fn_openpit_engine_block_account)(OpenPitEngine *, OpenPitParamAccountId, OpenPitStringView) = NULL;
static void (*_fn_openpit_engine_unblock_account)(OpenPitEngine *, OpenPitParamAccountId) = NULL;
static bool (*_fn_openpit_engine_replace_account_block_reason)(OpenPitEngine *, OpenPitParamAccountId, OpenPitStringView, OpenPitAccountBlockError **) = NULL;
static bool (*_fn_openpit_engine_block_account_group)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitStringView, OpenPitAccountBlockError **) = NULL;
static bool (*_fn_openpit_engine_unblock_account_group)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitAccountBlockError **) = NULL;
static bool (*_fn_openpit_engine_replace_account_group_block_reason)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitStringView, OpenPitAccountBlockError **) = NULL;
static OpenPitPretradePreTradePolicy * (*_fn_openpit_create_pretrade_custom_pre_trade_policy)(OpenPitStringView, uint16_t, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyApplyExecutionReportFn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn, OpenPitPretradePreTradePolicyFreeUserDataFn, void *, OpenPitOutError) = NULL;
static OpenPitPretradePreTradePolicy * (*_fn_openpit_create_pretrade_custom_pre_trade_policy_with_dry_run)(OpenPitStringView, uint16_t, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyApplyExecutionReportFn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn, OpenPitPretradePreTradePolicyFreeUserDataFn, void *, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_policy)(OpenPitPretradePreTradePolicy *) = NULL;
static OpenPitStringView (*_fn_openpit_pretrade_pre_trade_policy_get_name)(const OpenPitPretradePreTradePolicy *) = NULL;
static bool (*_fn_openpit_engine_builder_add_pre_trade_policy)(OpenPitEngineBuilder *, OpenPitPretradePreTradePolicy *, OpenPitOutError) = NULL;
static bool (*_fn_openpit_mutations_push)(OpenPitMutations *, OpenPitMutationFn, OpenPitMutationFn, void *, OpenPitMutationFreeFn, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_order_size_limit_policy)(OpenPitEngineBuilder *, uint16_t, const OpenPitPretradePoliciesOrderSizeBrokerBarrier *, const OpenPitPretradePoliciesOrderSizeAssetBarrier *, size_t, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_configure_order_size_limit)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesOrderSizeBrokerBarrier *, bool, const OpenPitPretradePoliciesOrderSizeAssetBarrier *, size_t, bool, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier *, size_t, bool, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_order_validation_policy)(OpenPitEngineBuilder *, uint16_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy)(OpenPitEngineBuilder *, uint16_t, const OpenPitPretradePoliciesPnlBoundsBarrier *, size_t, const OpenPitPretradePoliciesPnlBoundsAccountBarrier *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_configure_pnl_bounds_killswitch)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesPnlBoundsBarrier *, size_t, bool, const OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate *, size_t, bool, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountId, OpenPitStringView, OpenPitParamPnl, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_rate_limit_policy)(OpenPitEngineBuilder *, uint16_t, const OpenPitPretradePoliciesRateLimitBrokerBarrier *, const OpenPitPretradePoliciesRateLimitAssetBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_configure_rate_limit)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesRateLimitBrokerBarrier *, bool, const OpenPitPretradePoliciesRateLimitAssetBarrier *, size_t, bool, const OpenPitPretradePoliciesRateLimitAccountBarrier *, size_t, bool, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier *, size_t, bool, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_spot_funds_policy)(OpenPitEngineBuilder *, const OpenPitMarketDataService *, const uint16_t *, uint8_t, const OpenPitPretradePoliciesSpotFundsOverride *, size_t, uint16_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy)(OpenPitEngineBuilder *, const OpenPitMarketDataService *, uint16_t, const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier *, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier *, size_t, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_configure_spot_funds)(OpenPitEngine *, OpenPitStringView, uint16_t, bool, uint8_t, bool, const OpenPitPretradePoliciesSpotFundsOverride *, size_t, bool, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_configure_spot_funds_pnl_bounds_killswitch)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier *, bool, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier *, size_t, bool, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier *, size_t, bool, OpenPitConfigureError **) = NULL;
static OpenPitPretradeAccountBlockList * (*_fn_openpit_engine_configure_spot_funds_set_account_pnl)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountId, OpenPitPnlState, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_configure_spot_funds_global_limit_mode)(OpenPitEngine *, OpenPitStringView, OpenPitPretradePoliciesSpotFundsLimitMode, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_configure_spot_funds_account_limit_mode)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountId, OpenPitPretradePoliciesSpotFundsLimitMode, bool, OpenPitConfigureError **) = NULL;
static bool (*_fn_openpit_engine_configure_spot_funds_account_group_limit_mode)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountGroupId, OpenPitPretradePoliciesSpotFundsLimitMode, bool, OpenPitConfigureError **) = NULL;
static OpenPitStringView (*_fn_openpit_get_runtime_version)(void) = NULL;
static OpenPitStringView (*_fn_openpit_get_runtime_build_profile)(void) = NULL;
static void (*_fn_openpit_account_control_block)(const OpenPitAccountControl *, OpenPitPretradeAccountBlock) = NULL;
static OpenPitAccountControl * (*_fn_openpit_account_control_clone)(const OpenPitAccountControl *) = NULL;
static void (*_fn_openpit_destroy_account_control)(OpenPitAccountControl *) = NULL;
static OpenPitAccountControl * (*_fn_openpit_pretrade_context_get_account_control)(const OpenPitPretradeContext *) = NULL;
static OpenPitAccountControl * (*_fn_openpit_account_adjustment_context_get_account_control)(const OpenPitAccountAdjustmentContext *) = NULL;
static bool (*_fn_openpit_pretrade_context_get_account_group)(const OpenPitPretradeContext *, OpenPitParamAccountGroupId *) = NULL;
static bool (*_fn_openpit_account_adjustment_context_get_account_group)(const OpenPitAccountAdjustmentContext *, OpenPitParamAccountGroupId *) = NULL;
static bool (*_fn_openpit_post_trade_context_get_account_group)(const OpenPitPostTradeContext *, OpenPitParamAccountGroupId *) = NULL;
static bool (*_fn_openpit_create_param_account_group_id_from_uint32)(uint32_t, OpenPitParamAccountGroupId *, OpenPitOutError) = NULL;
static bool (*_fn_openpit_create_param_account_group_id_from_string)(OpenPitStringView, OpenPitParamAccountGroupId *, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_account_adjustment_outcome_list)(OpenPitAccountAdjustmentOutcomeList *) = NULL;
static size_t (*_fn_openpit_account_adjustment_outcome_list_len)(const OpenPitAccountAdjustmentOutcomeList *) = NULL;
static bool (*_fn_openpit_account_adjustment_outcome_list_get)(const OpenPitAccountAdjustmentOutcomeList *, size_t, OpenPitAccountAdjustmentOutcome *) = NULL;
static size_t (*_fn_openpit_account_pnl_outcome_list_len)(const OpenPitAccountPnlOutcomeList *) = NULL;
static bool (*_fn_openpit_account_pnl_outcome_list_get)(const OpenPitAccountPnlOutcomeList *, size_t, OpenPitAccountPnlOutcome *) = NULL;
static bool (*_fn_openpit_pretrade_pre_trade_result_push_lock_price)(OpenPitPretradePreTradeResult *, OpenPitParamPrice, OpenPitOutError) = NULL;
static bool (*_fn_openpit_pretrade_pre_trade_result_push_account_adjustment)(OpenPitPretradePreTradeResult *, OpenPitAccountOutcomeEntry, OpenPitOutError) = NULL;
static bool (*_fn_openpit_pretrade_post_trade_adjustment_list_push)(OpenPitPostTradeAdjustmentList *, uint16_t, OpenPitAccountOutcomeEntry, OpenPitOutError) = NULL;
static bool (*_fn_openpit_pretrade_post_trade_account_pnl_list_push)(OpenPitPostTradeAccountPnlList *, OpenPitAccountPnlOutcome, OpenPitOutError) = NULL;
static bool (*_fn_openpit_pretrade_account_adjustment_result_push_account_outcome)(OpenPitPretradeAccountAdjustmentResult *, OpenPitAccountOutcomeEntry, OpenPitOutError) = NULL;
static void (*_fn_openpit_pretrade_account_adjustment_result_push_account_block)(OpenPitPretradeAccountAdjustmentResult *, OpenPitPretradeAccountBlock) = NULL;
static void (*_fn_openpit_destroy_shared_bytes)(OpenPitSharedBytes *) = NULL;
static OpenPitBytesView (*_fn_openpit_shared_bytes_view)(const OpenPitSharedBytes *) = NULL;
static OpenPitMarketDataQuote (*_fn_openpit_create_marketdata_quote)(void) = NULL;
static OpenPitMarketDataQuoteTtl (*_fn_openpit_create_marketdata_quote_ttl_infinite)(void) = NULL;
static OpenPitMarketDataQuoteTtl (*_fn_openpit_create_marketdata_quote_ttl_within)(uint64_t, uint32_t) = NULL;
static OpenPitMarketDataService * (*_fn_openpit_create_marketdata_service)(uint8_t, OpenPitMarketDataQuoteTtl, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_marketdata_service)(OpenPitMarketDataService *) = NULL;
static OpenPitMarketDataService * (*_fn_openpit_marketdata_service_clone)(const OpenPitMarketDataService *) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_register)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId *, OpenPitOutError) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_register_with_ttl)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataQuoteTtl, OpenPitMarketDataInstrumentId *, OpenPitOutError) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_register_with_id)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId, OpenPitMarketDataInstrumentId *, OpenPitOutError) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_register_with_id_and_ttl)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuoteTtl, OpenPitMarketDataInstrumentId *, OpenPitOutError) = NULL;
static void (*_fn_openpit_marketdata_service_set_account_ttl)(const OpenPitMarketDataService *, OpenPitParamAccountId, OpenPitMarketDataQuoteTtl) = NULL;
static void (*_fn_openpit_marketdata_service_clear_account_ttl)(const OpenPitMarketDataService *, OpenPitParamAccountId) = NULL;
static void (*_fn_openpit_marketdata_service_set_account_group_ttl)(const OpenPitMarketDataService *, OpenPitParamAccountGroupId, OpenPitMarketDataQuoteTtl) = NULL;
static void (*_fn_openpit_marketdata_service_clear_account_group_ttl)(const OpenPitMarketDataService *, OpenPitParamAccountGroupId) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_set_instrument_ttl)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuoteTtl) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_clear_instrument_ttl)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_set_instrument_account_ttl)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountId, OpenPitMarketDataQuoteTtl) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_clear_instrument_account_ttl)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountId) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_set_instrument_account_group_ttl)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountGroupId, OpenPitMarketDataQuoteTtl) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_clear_instrument_account_group_ttl)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountGroupId) = NULL;
static void (*_fn_openpit_marketdata_service_clear)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_push)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, OpenPitOutError) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_push_patch)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, OpenPitOutError) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_push_for)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, const OpenPitParamAccountId *, size_t, const OpenPitParamAccountGroupId *, size_t, OpenPitOutError) = NULL;
static OpenPitMarketDataRegisterStatus (*_fn_openpit_marketdata_service_push_for_patch)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, const OpenPitParamAccountId *, size_t, const OpenPitParamAccountGroupId *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_marketdata_service_push_by_instrument)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataQuote, OpenPitMarketDataInstrumentId *, OpenPitOutError) = NULL;
static bool (*_fn_openpit_marketdata_service_push_by_instrument_patch)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataQuote, OpenPitMarketDataInstrumentId *, OpenPitOutError) = NULL;
static OpenPitMarketDataGetStatus (*_fn_openpit_marketdata_service_get)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountId, OpenPitMarketDataAccountGroupResolver, void *, OpenPitMarketDataQuoteResolution, OpenPitMarketDataQuote *) = NULL;
static bool (*_fn_openpit_marketdata_service_resolve)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId *) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_create_pretrade_pre_trade_lock)(void) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_lock)(OpenPitPretradePreTradeLock *) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_pretrade_pre_trade_lock_clone)(const OpenPitPretradePreTradeLock *) = NULL;
static size_t (*_fn_openpit_pretrade_pre_trade_lock_len)(const OpenPitPretradePreTradeLock *) = NULL;
static bool (*_fn_openpit_pretrade_pre_trade_lock_is_empty)(const OpenPitPretradePreTradeLock *) = NULL;
static bool (*_fn_openpit_pretrade_pre_trade_lock_push)(OpenPitPretradePreTradeLock *, uint16_t, OpenPitParamPrice, OpenPitOutError) = NULL;
static bool (*_fn_openpit_pretrade_pre_trade_lock_push_many)(OpenPitPretradePreTradeLock *, const OpenPitPretradePreTradeLockEntry *, size_t, OpenPitOutError) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_create_pretrade_pre_trade_lock_from_entries)(const OpenPitPretradePreTradeLockEntry *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_pretrade_pre_trade_lock_merge)(OpenPitPretradePreTradeLock *, const OpenPitPretradePreTradeLock *, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_lock_prices)(OpenPitPretradePreTradeLockPrices *) = NULL;
static OpenPitPretradePreTradeLockPricesView (*_fn_openpit_pretrade_pre_trade_lock_prices_view)(const OpenPitPretradePreTradeLockPrices *) = NULL;
static OpenPitPretradePreTradeLockPricesStatus (*_fn_openpit_pretrade_pre_trade_lock_prices_of)(const OpenPitPretradePreTradeLock *, uint16_t, OpenPitParamPrice *, OpenPitPretradePreTradeLockPrices **, OpenPitOutError) = NULL;
static OpenPitPretradePreTradeLockEntries * (*_fn_openpit_pretrade_pre_trade_lock_entries)(const OpenPitPretradePreTradeLock *) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_lock_entries)(OpenPitPretradePreTradeLockEntries *) = NULL;
static OpenPitPretradePreTradeLockEntriesView (*_fn_openpit_pretrade_pre_trade_lock_entries_view)(const OpenPitPretradePreTradeLockEntries *) = NULL;
static OpenPitSharedBytes * (*_fn_openpit_pretrade_pre_trade_lock_to_msgpack)(const OpenPitPretradePreTradeLock *, OpenPitOutError) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_create_pretrade_pre_trade_lock_from_msgpack)(const uint8_t *, size_t, OpenPitOutError) = NULL;
static OpenPitSharedString * (*_fn_openpit_pretrade_pre_trade_lock_to_json)(const OpenPitPretradePreTradeLock *, OpenPitOutError) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_create_pretrade_pre_trade_lock_from_json)(const uint8_t *, size_t, OpenPitOutError) = NULL;
static OpenPitSharedBytes * (*_fn_openpit_pretrade_pre_trade_lock_to_cbor)(const OpenPitPretradePreTradeLock *, OpenPitOutError) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_create_pretrade_pre_trade_lock_from_cbor)(const uint8_t *, size_t, OpenPitOutError) = NULL;
static OpenPitSharedBytes * (*_fn_openpit_pretrade_pre_trade_lock_to_raw)(const OpenPitPretradePreTradeLock *) = NULL;
static OpenPitPretradePreTradeLock * (*_fn_openpit_create_pretrade_pre_trade_lock_from_raw)(const uint8_t *, size_t, OpenPitOutError) = NULL;
static OpenPitReferenceBook * (*_fn_openpit_create_reference_book)(void) = NULL;
static void (*_fn_openpit_destroy_reference_book)(OpenPitReferenceBook *) = NULL;
static OpenPitReferenceBookRegisterStatus (*_fn_openpit_reference_book_register)(OpenPitReferenceBook *, const OpenPitInstrument *, OpenPitInstrumentId *, OpenPitOutError) = NULL;
static OpenPitReferenceBookRegisterStatus (*_fn_openpit_reference_book_register_with_id)(OpenPitReferenceBook *, const OpenPitInstrument *, OpenPitInstrumentId, OpenPitInstrumentId *, OpenPitOutError) = NULL;
static bool (*_fn_openpit_reference_book_resolve)(const OpenPitReferenceBook *, const OpenPitInstrument *, OpenPitInstrumentId *) = NULL;
static OpenPitReferenceBookStatus (*_fn_openpit_reference_book_set_settlement_scheme)(OpenPitReferenceBook *, OpenPitInstrumentId, OpenPitSettlementScheme, OpenPitOutError) = NULL;
static OpenPitReferenceBookStatus (*_fn_openpit_reference_book_clear_settlement_scheme)(OpenPitReferenceBook *, OpenPitInstrumentId, OpenPitOutError) = NULL;
static OpenPitReferenceBookStatus (*_fn_openpit_reference_book_get_settlement_scheme)(const OpenPitReferenceBook *, OpenPitInstrumentId, OpenPitSettlementScheme *, bool *, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_shared_string)(OpenPitSharedString *) = NULL;
static OpenPitStringView (*_fn_openpit_shared_string_view)(const OpenPitSharedString *) = NULL;

/*
 * Resolves every function pointer by name from the given runtime handle.
 * Returns NULL on success; on failure returns the name of the first symbol
 * that could not be resolved (the pointer references a static string
 * literal that lives for the lifetime of the process).
 */
const char *openpit_native_init(void *handle) {
    _fn_openpit_create_param_pnl = (bool (*)(OpenPitParamDecimal, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl");
    if (_fn_openpit_create_param_pnl == NULL) return "openpit_create_param_pnl";
    _fn_openpit_param_pnl_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamPnl))openpit_dlsym(handle, "openpit_param_pnl_get_decimal");
    if (_fn_openpit_param_pnl_get_decimal == NULL) return "openpit_param_pnl_get_decimal";
    _fn_openpit_create_param_price = (bool (*)(OpenPitParamDecimal, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price");
    if (_fn_openpit_create_param_price == NULL) return "openpit_create_param_price";
    _fn_openpit_param_price_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamPrice))openpit_dlsym(handle, "openpit_param_price_get_decimal");
    if (_fn_openpit_param_price_get_decimal == NULL) return "openpit_param_price_get_decimal";
    _fn_openpit_create_param_quantity = (bool (*)(OpenPitParamDecimal, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity");
    if (_fn_openpit_create_param_quantity == NULL) return "openpit_create_param_quantity";
    _fn_openpit_param_quantity_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamQuantity))openpit_dlsym(handle, "openpit_param_quantity_get_decimal");
    if (_fn_openpit_param_quantity_get_decimal == NULL) return "openpit_param_quantity_get_decimal";
    _fn_openpit_create_param_volume = (bool (*)(OpenPitParamDecimal, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume");
    if (_fn_openpit_create_param_volume == NULL) return "openpit_create_param_volume";
    _fn_openpit_param_volume_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamVolume))openpit_dlsym(handle, "openpit_param_volume_get_decimal");
    if (_fn_openpit_param_volume_get_decimal == NULL) return "openpit_param_volume_get_decimal";
    _fn_openpit_create_param_cash_flow = (bool (*)(OpenPitParamDecimal, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow");
    if (_fn_openpit_create_param_cash_flow == NULL) return "openpit_create_param_cash_flow";
    _fn_openpit_param_cash_flow_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamCashFlow))openpit_dlsym(handle, "openpit_param_cash_flow_get_decimal");
    if (_fn_openpit_param_cash_flow_get_decimal == NULL) return "openpit_param_cash_flow_get_decimal";
    _fn_openpit_create_param_position_size = (bool (*)(OpenPitParamDecimal, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size");
    if (_fn_openpit_create_param_position_size == NULL) return "openpit_create_param_position_size";
    _fn_openpit_param_position_size_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamPositionSize))openpit_dlsym(handle, "openpit_param_position_size_get_decimal");
    if (_fn_openpit_param_position_size_get_decimal == NULL) return "openpit_param_position_size_get_decimal";
    _fn_openpit_create_param_fee = (bool (*)(OpenPitParamDecimal, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee");
    if (_fn_openpit_create_param_fee == NULL) return "openpit_create_param_fee";
    _fn_openpit_param_fee_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamFee))openpit_dlsym(handle, "openpit_param_fee_get_decimal");
    if (_fn_openpit_param_fee_get_decimal == NULL) return "openpit_param_fee_get_decimal";
    _fn_openpit_create_param_notional = (bool (*)(OpenPitParamDecimal, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional");
    if (_fn_openpit_create_param_notional == NULL) return "openpit_create_param_notional";
    _fn_openpit_param_notional_get_decimal = (OpenPitParamDecimal (*)(OpenPitParamNotional))openpit_dlsym(handle, "openpit_param_notional_get_decimal");
    if (_fn_openpit_param_notional_get_decimal == NULL) return "openpit_param_notional_get_decimal";
    _fn_openpit_create_param_pnl_from_string = (bool (*)(OpenPitStringView, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_string");
    if (_fn_openpit_create_param_pnl_from_string == NULL) return "openpit_create_param_pnl_from_string";
    _fn_openpit_create_param_pnl_from_f64 = (bool (*)(double, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_f64");
    if (_fn_openpit_create_param_pnl_from_f64 == NULL) return "openpit_create_param_pnl_from_f64";
    _fn_openpit_create_param_pnl_from_int64 = (bool (*)(int64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_int64");
    if (_fn_openpit_create_param_pnl_from_int64 == NULL) return "openpit_create_param_pnl_from_int64";
    _fn_openpit_create_param_pnl_from_uint64 = (bool (*)(uint64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_uint64");
    if (_fn_openpit_create_param_pnl_from_uint64 == NULL) return "openpit_create_param_pnl_from_uint64";
    _fn_openpit_create_param_pnl_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_string_rounded");
    if (_fn_openpit_create_param_pnl_from_string_rounded == NULL) return "openpit_create_param_pnl_from_string_rounded";
    _fn_openpit_create_param_pnl_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_f64_rounded");
    if (_fn_openpit_create_param_pnl_from_f64_rounded == NULL) return "openpit_create_param_pnl_from_f64_rounded";
    _fn_openpit_create_param_pnl_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_decimal_rounded");
    if (_fn_openpit_create_param_pnl_from_decimal_rounded == NULL) return "openpit_create_param_pnl_from_decimal_rounded";
    _fn_openpit_param_pnl_to_f64 = (bool (*)(OpenPitParamPnl, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_to_f64");
    if (_fn_openpit_param_pnl_to_f64 == NULL) return "openpit_param_pnl_to_f64";
    _fn_openpit_param_pnl_is_zero = (bool (*)(OpenPitParamPnl, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_is_zero");
    if (_fn_openpit_param_pnl_is_zero == NULL) return "openpit_param_pnl_is_zero";
    _fn_openpit_param_pnl_compare = (bool (*)(OpenPitParamPnl, OpenPitParamPnl, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_compare");
    if (_fn_openpit_param_pnl_compare == NULL) return "openpit_param_pnl_compare";
    _fn_openpit_param_pnl_to_string = (OpenPitSharedString * (*)(OpenPitParamPnl, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_to_string");
    if (_fn_openpit_param_pnl_to_string == NULL) return "openpit_param_pnl_to_string";
    _fn_openpit_param_pnl_checked_add = (bool (*)(OpenPitParamPnl, OpenPitParamPnl, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_add");
    if (_fn_openpit_param_pnl_checked_add == NULL) return "openpit_param_pnl_checked_add";
    _fn_openpit_param_pnl_checked_sub = (bool (*)(OpenPitParamPnl, OpenPitParamPnl, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_sub");
    if (_fn_openpit_param_pnl_checked_sub == NULL) return "openpit_param_pnl_checked_sub";
    _fn_openpit_param_pnl_checked_mul_i64 = (bool (*)(OpenPitParamPnl, int64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_mul_i64");
    if (_fn_openpit_param_pnl_checked_mul_i64 == NULL) return "openpit_param_pnl_checked_mul_i64";
    _fn_openpit_param_pnl_checked_mul_u64 = (bool (*)(OpenPitParamPnl, uint64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_mul_u64");
    if (_fn_openpit_param_pnl_checked_mul_u64 == NULL) return "openpit_param_pnl_checked_mul_u64";
    _fn_openpit_param_pnl_checked_mul_f64 = (bool (*)(OpenPitParamPnl, double, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_mul_f64");
    if (_fn_openpit_param_pnl_checked_mul_f64 == NULL) return "openpit_param_pnl_checked_mul_f64";
    _fn_openpit_param_pnl_checked_div_i64 = (bool (*)(OpenPitParamPnl, int64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_div_i64");
    if (_fn_openpit_param_pnl_checked_div_i64 == NULL) return "openpit_param_pnl_checked_div_i64";
    _fn_openpit_param_pnl_checked_div_u64 = (bool (*)(OpenPitParamPnl, uint64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_div_u64");
    if (_fn_openpit_param_pnl_checked_div_u64 == NULL) return "openpit_param_pnl_checked_div_u64";
    _fn_openpit_param_pnl_checked_div_f64 = (bool (*)(OpenPitParamPnl, double, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_div_f64");
    if (_fn_openpit_param_pnl_checked_div_f64 == NULL) return "openpit_param_pnl_checked_div_f64";
    _fn_openpit_param_pnl_checked_rem_i64 = (bool (*)(OpenPitParamPnl, int64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_rem_i64");
    if (_fn_openpit_param_pnl_checked_rem_i64 == NULL) return "openpit_param_pnl_checked_rem_i64";
    _fn_openpit_param_pnl_checked_rem_u64 = (bool (*)(OpenPitParamPnl, uint64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_rem_u64");
    if (_fn_openpit_param_pnl_checked_rem_u64 == NULL) return "openpit_param_pnl_checked_rem_u64";
    _fn_openpit_param_pnl_checked_rem_f64 = (bool (*)(OpenPitParamPnl, double, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_rem_f64");
    if (_fn_openpit_param_pnl_checked_rem_f64 == NULL) return "openpit_param_pnl_checked_rem_f64";
    _fn_openpit_param_pnl_checked_neg = (bool (*)(OpenPitParamPnl, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_checked_neg");
    if (_fn_openpit_param_pnl_checked_neg == NULL) return "openpit_param_pnl_checked_neg";
    _fn_openpit_create_param_price_from_string = (bool (*)(OpenPitStringView, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_string");
    if (_fn_openpit_create_param_price_from_string == NULL) return "openpit_create_param_price_from_string";
    _fn_openpit_create_param_price_from_f64 = (bool (*)(double, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_f64");
    if (_fn_openpit_create_param_price_from_f64 == NULL) return "openpit_create_param_price_from_f64";
    _fn_openpit_create_param_price_from_int64 = (bool (*)(int64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_int64");
    if (_fn_openpit_create_param_price_from_int64 == NULL) return "openpit_create_param_price_from_int64";
    _fn_openpit_create_param_price_from_uint64 = (bool (*)(uint64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_uint64");
    if (_fn_openpit_create_param_price_from_uint64 == NULL) return "openpit_create_param_price_from_uint64";
    _fn_openpit_create_param_price_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_string_rounded");
    if (_fn_openpit_create_param_price_from_string_rounded == NULL) return "openpit_create_param_price_from_string_rounded";
    _fn_openpit_create_param_price_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_f64_rounded");
    if (_fn_openpit_create_param_price_from_f64_rounded == NULL) return "openpit_create_param_price_from_f64_rounded";
    _fn_openpit_create_param_price_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_decimal_rounded");
    if (_fn_openpit_create_param_price_from_decimal_rounded == NULL) return "openpit_create_param_price_from_decimal_rounded";
    _fn_openpit_param_price_to_f64 = (bool (*)(OpenPitParamPrice, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_to_f64");
    if (_fn_openpit_param_price_to_f64 == NULL) return "openpit_param_price_to_f64";
    _fn_openpit_param_price_is_zero = (bool (*)(OpenPitParamPrice, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_is_zero");
    if (_fn_openpit_param_price_is_zero == NULL) return "openpit_param_price_is_zero";
    _fn_openpit_param_price_compare = (bool (*)(OpenPitParamPrice, OpenPitParamPrice, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_compare");
    if (_fn_openpit_param_price_compare == NULL) return "openpit_param_price_compare";
    _fn_openpit_param_price_to_string = (OpenPitSharedString * (*)(OpenPitParamPrice, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_to_string");
    if (_fn_openpit_param_price_to_string == NULL) return "openpit_param_price_to_string";
    _fn_openpit_param_price_checked_add = (bool (*)(OpenPitParamPrice, OpenPitParamPrice, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_add");
    if (_fn_openpit_param_price_checked_add == NULL) return "openpit_param_price_checked_add";
    _fn_openpit_param_price_checked_sub = (bool (*)(OpenPitParamPrice, OpenPitParamPrice, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_sub");
    if (_fn_openpit_param_price_checked_sub == NULL) return "openpit_param_price_checked_sub";
    _fn_openpit_param_price_checked_mul_i64 = (bool (*)(OpenPitParamPrice, int64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_mul_i64");
    if (_fn_openpit_param_price_checked_mul_i64 == NULL) return "openpit_param_price_checked_mul_i64";
    _fn_openpit_param_price_checked_mul_u64 = (bool (*)(OpenPitParamPrice, uint64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_mul_u64");
    if (_fn_openpit_param_price_checked_mul_u64 == NULL) return "openpit_param_price_checked_mul_u64";
    _fn_openpit_param_price_checked_mul_f64 = (bool (*)(OpenPitParamPrice, double, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_mul_f64");
    if (_fn_openpit_param_price_checked_mul_f64 == NULL) return "openpit_param_price_checked_mul_f64";
    _fn_openpit_param_price_checked_div_i64 = (bool (*)(OpenPitParamPrice, int64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_div_i64");
    if (_fn_openpit_param_price_checked_div_i64 == NULL) return "openpit_param_price_checked_div_i64";
    _fn_openpit_param_price_checked_div_u64 = (bool (*)(OpenPitParamPrice, uint64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_div_u64");
    if (_fn_openpit_param_price_checked_div_u64 == NULL) return "openpit_param_price_checked_div_u64";
    _fn_openpit_param_price_checked_div_f64 = (bool (*)(OpenPitParamPrice, double, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_div_f64");
    if (_fn_openpit_param_price_checked_div_f64 == NULL) return "openpit_param_price_checked_div_f64";
    _fn_openpit_param_price_checked_rem_i64 = (bool (*)(OpenPitParamPrice, int64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_rem_i64");
    if (_fn_openpit_param_price_checked_rem_i64 == NULL) return "openpit_param_price_checked_rem_i64";
    _fn_openpit_param_price_checked_rem_u64 = (bool (*)(OpenPitParamPrice, uint64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_rem_u64");
    if (_fn_openpit_param_price_checked_rem_u64 == NULL) return "openpit_param_price_checked_rem_u64";
    _fn_openpit_param_price_checked_rem_f64 = (bool (*)(OpenPitParamPrice, double, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_rem_f64");
    if (_fn_openpit_param_price_checked_rem_f64 == NULL) return "openpit_param_price_checked_rem_f64";
    _fn_openpit_param_price_checked_neg = (bool (*)(OpenPitParamPrice, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_checked_neg");
    if (_fn_openpit_param_price_checked_neg == NULL) return "openpit_param_price_checked_neg";
    _fn_openpit_create_param_quantity_from_string = (bool (*)(OpenPitStringView, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_string");
    if (_fn_openpit_create_param_quantity_from_string == NULL) return "openpit_create_param_quantity_from_string";
    _fn_openpit_create_param_quantity_from_f64 = (bool (*)(double, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_f64");
    if (_fn_openpit_create_param_quantity_from_f64 == NULL) return "openpit_create_param_quantity_from_f64";
    _fn_openpit_create_param_quantity_from_int64 = (bool (*)(int64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_int64");
    if (_fn_openpit_create_param_quantity_from_int64 == NULL) return "openpit_create_param_quantity_from_int64";
    _fn_openpit_create_param_quantity_from_uint64 = (bool (*)(uint64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_uint64");
    if (_fn_openpit_create_param_quantity_from_uint64 == NULL) return "openpit_create_param_quantity_from_uint64";
    _fn_openpit_create_param_quantity_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_string_rounded");
    if (_fn_openpit_create_param_quantity_from_string_rounded == NULL) return "openpit_create_param_quantity_from_string_rounded";
    _fn_openpit_create_param_quantity_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_f64_rounded");
    if (_fn_openpit_create_param_quantity_from_f64_rounded == NULL) return "openpit_create_param_quantity_from_f64_rounded";
    _fn_openpit_create_param_quantity_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_decimal_rounded");
    if (_fn_openpit_create_param_quantity_from_decimal_rounded == NULL) return "openpit_create_param_quantity_from_decimal_rounded";
    _fn_openpit_param_quantity_to_f64 = (bool (*)(OpenPitParamQuantity, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_to_f64");
    if (_fn_openpit_param_quantity_to_f64 == NULL) return "openpit_param_quantity_to_f64";
    _fn_openpit_param_quantity_is_zero = (bool (*)(OpenPitParamQuantity, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_is_zero");
    if (_fn_openpit_param_quantity_is_zero == NULL) return "openpit_param_quantity_is_zero";
    _fn_openpit_param_quantity_compare = (bool (*)(OpenPitParamQuantity, OpenPitParamQuantity, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_compare");
    if (_fn_openpit_param_quantity_compare == NULL) return "openpit_param_quantity_compare";
    _fn_openpit_param_quantity_to_string = (OpenPitSharedString * (*)(OpenPitParamQuantity, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_to_string");
    if (_fn_openpit_param_quantity_to_string == NULL) return "openpit_param_quantity_to_string";
    _fn_openpit_param_quantity_checked_add = (bool (*)(OpenPitParamQuantity, OpenPitParamQuantity, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_add");
    if (_fn_openpit_param_quantity_checked_add == NULL) return "openpit_param_quantity_checked_add";
    _fn_openpit_param_quantity_checked_sub = (bool (*)(OpenPitParamQuantity, OpenPitParamQuantity, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_sub");
    if (_fn_openpit_param_quantity_checked_sub == NULL) return "openpit_param_quantity_checked_sub";
    _fn_openpit_param_quantity_checked_mul_i64 = (bool (*)(OpenPitParamQuantity, int64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_mul_i64");
    if (_fn_openpit_param_quantity_checked_mul_i64 == NULL) return "openpit_param_quantity_checked_mul_i64";
    _fn_openpit_param_quantity_checked_mul_u64 = (bool (*)(OpenPitParamQuantity, uint64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_mul_u64");
    if (_fn_openpit_param_quantity_checked_mul_u64 == NULL) return "openpit_param_quantity_checked_mul_u64";
    _fn_openpit_param_quantity_checked_mul_f64 = (bool (*)(OpenPitParamQuantity, double, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_mul_f64");
    if (_fn_openpit_param_quantity_checked_mul_f64 == NULL) return "openpit_param_quantity_checked_mul_f64";
    _fn_openpit_param_quantity_checked_div_i64 = (bool (*)(OpenPitParamQuantity, int64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_div_i64");
    if (_fn_openpit_param_quantity_checked_div_i64 == NULL) return "openpit_param_quantity_checked_div_i64";
    _fn_openpit_param_quantity_checked_div_u64 = (bool (*)(OpenPitParamQuantity, uint64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_div_u64");
    if (_fn_openpit_param_quantity_checked_div_u64 == NULL) return "openpit_param_quantity_checked_div_u64";
    _fn_openpit_param_quantity_checked_div_f64 = (bool (*)(OpenPitParamQuantity, double, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_div_f64");
    if (_fn_openpit_param_quantity_checked_div_f64 == NULL) return "openpit_param_quantity_checked_div_f64";
    _fn_openpit_param_quantity_checked_rem_i64 = (bool (*)(OpenPitParamQuantity, int64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_rem_i64");
    if (_fn_openpit_param_quantity_checked_rem_i64 == NULL) return "openpit_param_quantity_checked_rem_i64";
    _fn_openpit_param_quantity_checked_rem_u64 = (bool (*)(OpenPitParamQuantity, uint64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_rem_u64");
    if (_fn_openpit_param_quantity_checked_rem_u64 == NULL) return "openpit_param_quantity_checked_rem_u64";
    _fn_openpit_param_quantity_checked_rem_f64 = (bool (*)(OpenPitParamQuantity, double, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_checked_rem_f64");
    if (_fn_openpit_param_quantity_checked_rem_f64 == NULL) return "openpit_param_quantity_checked_rem_f64";
    _fn_openpit_create_param_volume_from_string = (bool (*)(OpenPitStringView, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_string");
    if (_fn_openpit_create_param_volume_from_string == NULL) return "openpit_create_param_volume_from_string";
    _fn_openpit_create_param_volume_from_f64 = (bool (*)(double, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_f64");
    if (_fn_openpit_create_param_volume_from_f64 == NULL) return "openpit_create_param_volume_from_f64";
    _fn_openpit_create_param_volume_from_int64 = (bool (*)(int64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_int64");
    if (_fn_openpit_create_param_volume_from_int64 == NULL) return "openpit_create_param_volume_from_int64";
    _fn_openpit_create_param_volume_from_uint64 = (bool (*)(uint64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_uint64");
    if (_fn_openpit_create_param_volume_from_uint64 == NULL) return "openpit_create_param_volume_from_uint64";
    _fn_openpit_create_param_volume_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_string_rounded");
    if (_fn_openpit_create_param_volume_from_string_rounded == NULL) return "openpit_create_param_volume_from_string_rounded";
    _fn_openpit_create_param_volume_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_f64_rounded");
    if (_fn_openpit_create_param_volume_from_f64_rounded == NULL) return "openpit_create_param_volume_from_f64_rounded";
    _fn_openpit_create_param_volume_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_decimal_rounded");
    if (_fn_openpit_create_param_volume_from_decimal_rounded == NULL) return "openpit_create_param_volume_from_decimal_rounded";
    _fn_openpit_param_volume_to_f64 = (bool (*)(OpenPitParamVolume, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_to_f64");
    if (_fn_openpit_param_volume_to_f64 == NULL) return "openpit_param_volume_to_f64";
    _fn_openpit_param_volume_is_zero = (bool (*)(OpenPitParamVolume, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_is_zero");
    if (_fn_openpit_param_volume_is_zero == NULL) return "openpit_param_volume_is_zero";
    _fn_openpit_param_volume_compare = (bool (*)(OpenPitParamVolume, OpenPitParamVolume, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_compare");
    if (_fn_openpit_param_volume_compare == NULL) return "openpit_param_volume_compare";
    _fn_openpit_param_volume_to_string = (OpenPitSharedString * (*)(OpenPitParamVolume, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_to_string");
    if (_fn_openpit_param_volume_to_string == NULL) return "openpit_param_volume_to_string";
    _fn_openpit_param_volume_checked_add = (bool (*)(OpenPitParamVolume, OpenPitParamVolume, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_add");
    if (_fn_openpit_param_volume_checked_add == NULL) return "openpit_param_volume_checked_add";
    _fn_openpit_param_volume_checked_sub = (bool (*)(OpenPitParamVolume, OpenPitParamVolume, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_sub");
    if (_fn_openpit_param_volume_checked_sub == NULL) return "openpit_param_volume_checked_sub";
    _fn_openpit_param_volume_checked_mul_i64 = (bool (*)(OpenPitParamVolume, int64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_mul_i64");
    if (_fn_openpit_param_volume_checked_mul_i64 == NULL) return "openpit_param_volume_checked_mul_i64";
    _fn_openpit_param_volume_checked_mul_u64 = (bool (*)(OpenPitParamVolume, uint64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_mul_u64");
    if (_fn_openpit_param_volume_checked_mul_u64 == NULL) return "openpit_param_volume_checked_mul_u64";
    _fn_openpit_param_volume_checked_mul_f64 = (bool (*)(OpenPitParamVolume, double, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_mul_f64");
    if (_fn_openpit_param_volume_checked_mul_f64 == NULL) return "openpit_param_volume_checked_mul_f64";
    _fn_openpit_param_volume_checked_div_i64 = (bool (*)(OpenPitParamVolume, int64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_div_i64");
    if (_fn_openpit_param_volume_checked_div_i64 == NULL) return "openpit_param_volume_checked_div_i64";
    _fn_openpit_param_volume_checked_div_u64 = (bool (*)(OpenPitParamVolume, uint64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_div_u64");
    if (_fn_openpit_param_volume_checked_div_u64 == NULL) return "openpit_param_volume_checked_div_u64";
    _fn_openpit_param_volume_checked_div_f64 = (bool (*)(OpenPitParamVolume, double, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_div_f64");
    if (_fn_openpit_param_volume_checked_div_f64 == NULL) return "openpit_param_volume_checked_div_f64";
    _fn_openpit_param_volume_checked_rem_i64 = (bool (*)(OpenPitParamVolume, int64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_rem_i64");
    if (_fn_openpit_param_volume_checked_rem_i64 == NULL) return "openpit_param_volume_checked_rem_i64";
    _fn_openpit_param_volume_checked_rem_u64 = (bool (*)(OpenPitParamVolume, uint64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_rem_u64");
    if (_fn_openpit_param_volume_checked_rem_u64 == NULL) return "openpit_param_volume_checked_rem_u64";
    _fn_openpit_param_volume_checked_rem_f64 = (bool (*)(OpenPitParamVolume, double, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_checked_rem_f64");
    if (_fn_openpit_param_volume_checked_rem_f64 == NULL) return "openpit_param_volume_checked_rem_f64";
    _fn_openpit_create_param_cash_flow_from_string = (bool (*)(OpenPitStringView, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_string");
    if (_fn_openpit_create_param_cash_flow_from_string == NULL) return "openpit_create_param_cash_flow_from_string";
    _fn_openpit_create_param_cash_flow_from_f64 = (bool (*)(double, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_f64");
    if (_fn_openpit_create_param_cash_flow_from_f64 == NULL) return "openpit_create_param_cash_flow_from_f64";
    _fn_openpit_create_param_cash_flow_from_int64 = (bool (*)(int64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_int64");
    if (_fn_openpit_create_param_cash_flow_from_int64 == NULL) return "openpit_create_param_cash_flow_from_int64";
    _fn_openpit_create_param_cash_flow_from_uint64 = (bool (*)(uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_uint64");
    if (_fn_openpit_create_param_cash_flow_from_uint64 == NULL) return "openpit_create_param_cash_flow_from_uint64";
    _fn_openpit_create_param_cash_flow_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_string_rounded");
    if (_fn_openpit_create_param_cash_flow_from_string_rounded == NULL) return "openpit_create_param_cash_flow_from_string_rounded";
    _fn_openpit_create_param_cash_flow_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_f64_rounded");
    if (_fn_openpit_create_param_cash_flow_from_f64_rounded == NULL) return "openpit_create_param_cash_flow_from_f64_rounded";
    _fn_openpit_create_param_cash_flow_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_decimal_rounded");
    if (_fn_openpit_create_param_cash_flow_from_decimal_rounded == NULL) return "openpit_create_param_cash_flow_from_decimal_rounded";
    _fn_openpit_param_cash_flow_to_f64 = (bool (*)(OpenPitParamCashFlow, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_to_f64");
    if (_fn_openpit_param_cash_flow_to_f64 == NULL) return "openpit_param_cash_flow_to_f64";
    _fn_openpit_param_cash_flow_is_zero = (bool (*)(OpenPitParamCashFlow, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_is_zero");
    if (_fn_openpit_param_cash_flow_is_zero == NULL) return "openpit_param_cash_flow_is_zero";
    _fn_openpit_param_cash_flow_compare = (bool (*)(OpenPitParamCashFlow, OpenPitParamCashFlow, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_compare");
    if (_fn_openpit_param_cash_flow_compare == NULL) return "openpit_param_cash_flow_compare";
    _fn_openpit_param_cash_flow_to_string = (OpenPitSharedString * (*)(OpenPitParamCashFlow, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_to_string");
    if (_fn_openpit_param_cash_flow_to_string == NULL) return "openpit_param_cash_flow_to_string";
    _fn_openpit_param_cash_flow_checked_add = (bool (*)(OpenPitParamCashFlow, OpenPitParamCashFlow, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_add");
    if (_fn_openpit_param_cash_flow_checked_add == NULL) return "openpit_param_cash_flow_checked_add";
    _fn_openpit_param_cash_flow_checked_sub = (bool (*)(OpenPitParamCashFlow, OpenPitParamCashFlow, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_sub");
    if (_fn_openpit_param_cash_flow_checked_sub == NULL) return "openpit_param_cash_flow_checked_sub";
    _fn_openpit_param_cash_flow_checked_mul_i64 = (bool (*)(OpenPitParamCashFlow, int64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_mul_i64");
    if (_fn_openpit_param_cash_flow_checked_mul_i64 == NULL) return "openpit_param_cash_flow_checked_mul_i64";
    _fn_openpit_param_cash_flow_checked_mul_u64 = (bool (*)(OpenPitParamCashFlow, uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_mul_u64");
    if (_fn_openpit_param_cash_flow_checked_mul_u64 == NULL) return "openpit_param_cash_flow_checked_mul_u64";
    _fn_openpit_param_cash_flow_checked_mul_f64 = (bool (*)(OpenPitParamCashFlow, double, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_mul_f64");
    if (_fn_openpit_param_cash_flow_checked_mul_f64 == NULL) return "openpit_param_cash_flow_checked_mul_f64";
    _fn_openpit_param_cash_flow_checked_div_i64 = (bool (*)(OpenPitParamCashFlow, int64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_div_i64");
    if (_fn_openpit_param_cash_flow_checked_div_i64 == NULL) return "openpit_param_cash_flow_checked_div_i64";
    _fn_openpit_param_cash_flow_checked_div_u64 = (bool (*)(OpenPitParamCashFlow, uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_div_u64");
    if (_fn_openpit_param_cash_flow_checked_div_u64 == NULL) return "openpit_param_cash_flow_checked_div_u64";
    _fn_openpit_param_cash_flow_checked_div_f64 = (bool (*)(OpenPitParamCashFlow, double, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_div_f64");
    if (_fn_openpit_param_cash_flow_checked_div_f64 == NULL) return "openpit_param_cash_flow_checked_div_f64";
    _fn_openpit_param_cash_flow_checked_rem_i64 = (bool (*)(OpenPitParamCashFlow, int64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_rem_i64");
    if (_fn_openpit_param_cash_flow_checked_rem_i64 == NULL) return "openpit_param_cash_flow_checked_rem_i64";
    _fn_openpit_param_cash_flow_checked_rem_u64 = (bool (*)(OpenPitParamCashFlow, uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_rem_u64");
    if (_fn_openpit_param_cash_flow_checked_rem_u64 == NULL) return "openpit_param_cash_flow_checked_rem_u64";
    _fn_openpit_param_cash_flow_checked_rem_f64 = (bool (*)(OpenPitParamCashFlow, double, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_rem_f64");
    if (_fn_openpit_param_cash_flow_checked_rem_f64 == NULL) return "openpit_param_cash_flow_checked_rem_f64";
    _fn_openpit_param_cash_flow_checked_neg = (bool (*)(OpenPitParamCashFlow, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_checked_neg");
    if (_fn_openpit_param_cash_flow_checked_neg == NULL) return "openpit_param_cash_flow_checked_neg";
    _fn_openpit_create_param_position_size_from_string = (bool (*)(OpenPitStringView, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_string");
    if (_fn_openpit_create_param_position_size_from_string == NULL) return "openpit_create_param_position_size_from_string";
    _fn_openpit_create_param_position_size_from_f64 = (bool (*)(double, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_f64");
    if (_fn_openpit_create_param_position_size_from_f64 == NULL) return "openpit_create_param_position_size_from_f64";
    _fn_openpit_create_param_position_size_from_int64 = (bool (*)(int64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_int64");
    if (_fn_openpit_create_param_position_size_from_int64 == NULL) return "openpit_create_param_position_size_from_int64";
    _fn_openpit_create_param_position_size_from_uint64 = (bool (*)(uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_uint64");
    if (_fn_openpit_create_param_position_size_from_uint64 == NULL) return "openpit_create_param_position_size_from_uint64";
    _fn_openpit_create_param_position_size_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_string_rounded");
    if (_fn_openpit_create_param_position_size_from_string_rounded == NULL) return "openpit_create_param_position_size_from_string_rounded";
    _fn_openpit_create_param_position_size_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_f64_rounded");
    if (_fn_openpit_create_param_position_size_from_f64_rounded == NULL) return "openpit_create_param_position_size_from_f64_rounded";
    _fn_openpit_create_param_position_size_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_decimal_rounded");
    if (_fn_openpit_create_param_position_size_from_decimal_rounded == NULL) return "openpit_create_param_position_size_from_decimal_rounded";
    _fn_openpit_param_position_size_to_f64 = (bool (*)(OpenPitParamPositionSize, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_to_f64");
    if (_fn_openpit_param_position_size_to_f64 == NULL) return "openpit_param_position_size_to_f64";
    _fn_openpit_param_position_size_is_zero = (bool (*)(OpenPitParamPositionSize, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_is_zero");
    if (_fn_openpit_param_position_size_is_zero == NULL) return "openpit_param_position_size_is_zero";
    _fn_openpit_param_position_size_compare = (bool (*)(OpenPitParamPositionSize, OpenPitParamPositionSize, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_compare");
    if (_fn_openpit_param_position_size_compare == NULL) return "openpit_param_position_size_compare";
    _fn_openpit_param_position_size_to_string = (OpenPitSharedString * (*)(OpenPitParamPositionSize, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_to_string");
    if (_fn_openpit_param_position_size_to_string == NULL) return "openpit_param_position_size_to_string";
    _fn_openpit_param_position_size_checked_add = (bool (*)(OpenPitParamPositionSize, OpenPitParamPositionSize, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_add");
    if (_fn_openpit_param_position_size_checked_add == NULL) return "openpit_param_position_size_checked_add";
    _fn_openpit_param_position_size_checked_sub = (bool (*)(OpenPitParamPositionSize, OpenPitParamPositionSize, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_sub");
    if (_fn_openpit_param_position_size_checked_sub == NULL) return "openpit_param_position_size_checked_sub";
    _fn_openpit_param_position_size_checked_mul_i64 = (bool (*)(OpenPitParamPositionSize, int64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_mul_i64");
    if (_fn_openpit_param_position_size_checked_mul_i64 == NULL) return "openpit_param_position_size_checked_mul_i64";
    _fn_openpit_param_position_size_checked_mul_u64 = (bool (*)(OpenPitParamPositionSize, uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_mul_u64");
    if (_fn_openpit_param_position_size_checked_mul_u64 == NULL) return "openpit_param_position_size_checked_mul_u64";
    _fn_openpit_param_position_size_checked_mul_f64 = (bool (*)(OpenPitParamPositionSize, double, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_mul_f64");
    if (_fn_openpit_param_position_size_checked_mul_f64 == NULL) return "openpit_param_position_size_checked_mul_f64";
    _fn_openpit_param_position_size_checked_div_i64 = (bool (*)(OpenPitParamPositionSize, int64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_div_i64");
    if (_fn_openpit_param_position_size_checked_div_i64 == NULL) return "openpit_param_position_size_checked_div_i64";
    _fn_openpit_param_position_size_checked_div_u64 = (bool (*)(OpenPitParamPositionSize, uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_div_u64");
    if (_fn_openpit_param_position_size_checked_div_u64 == NULL) return "openpit_param_position_size_checked_div_u64";
    _fn_openpit_param_position_size_checked_div_f64 = (bool (*)(OpenPitParamPositionSize, double, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_div_f64");
    if (_fn_openpit_param_position_size_checked_div_f64 == NULL) return "openpit_param_position_size_checked_div_f64";
    _fn_openpit_param_position_size_checked_rem_i64 = (bool (*)(OpenPitParamPositionSize, int64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_rem_i64");
    if (_fn_openpit_param_position_size_checked_rem_i64 == NULL) return "openpit_param_position_size_checked_rem_i64";
    _fn_openpit_param_position_size_checked_rem_u64 = (bool (*)(OpenPitParamPositionSize, uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_rem_u64");
    if (_fn_openpit_param_position_size_checked_rem_u64 == NULL) return "openpit_param_position_size_checked_rem_u64";
    _fn_openpit_param_position_size_checked_rem_f64 = (bool (*)(OpenPitParamPositionSize, double, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_rem_f64");
    if (_fn_openpit_param_position_size_checked_rem_f64 == NULL) return "openpit_param_position_size_checked_rem_f64";
    _fn_openpit_param_position_size_checked_neg = (bool (*)(OpenPitParamPositionSize, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_neg");
    if (_fn_openpit_param_position_size_checked_neg == NULL) return "openpit_param_position_size_checked_neg";
    _fn_openpit_create_param_fee_from_string = (bool (*)(OpenPitStringView, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_string");
    if (_fn_openpit_create_param_fee_from_string == NULL) return "openpit_create_param_fee_from_string";
    _fn_openpit_create_param_fee_from_f64 = (bool (*)(double, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_f64");
    if (_fn_openpit_create_param_fee_from_f64 == NULL) return "openpit_create_param_fee_from_f64";
    _fn_openpit_create_param_fee_from_int64 = (bool (*)(int64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_int64");
    if (_fn_openpit_create_param_fee_from_int64 == NULL) return "openpit_create_param_fee_from_int64";
    _fn_openpit_create_param_fee_from_uint64 = (bool (*)(uint64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_uint64");
    if (_fn_openpit_create_param_fee_from_uint64 == NULL) return "openpit_create_param_fee_from_uint64";
    _fn_openpit_create_param_fee_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_string_rounded");
    if (_fn_openpit_create_param_fee_from_string_rounded == NULL) return "openpit_create_param_fee_from_string_rounded";
    _fn_openpit_create_param_fee_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_f64_rounded");
    if (_fn_openpit_create_param_fee_from_f64_rounded == NULL) return "openpit_create_param_fee_from_f64_rounded";
    _fn_openpit_create_param_fee_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_decimal_rounded");
    if (_fn_openpit_create_param_fee_from_decimal_rounded == NULL) return "openpit_create_param_fee_from_decimal_rounded";
    _fn_openpit_param_fee_to_f64 = (bool (*)(OpenPitParamFee, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_to_f64");
    if (_fn_openpit_param_fee_to_f64 == NULL) return "openpit_param_fee_to_f64";
    _fn_openpit_param_fee_is_zero = (bool (*)(OpenPitParamFee, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_is_zero");
    if (_fn_openpit_param_fee_is_zero == NULL) return "openpit_param_fee_is_zero";
    _fn_openpit_param_fee_compare = (bool (*)(OpenPitParamFee, OpenPitParamFee, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_compare");
    if (_fn_openpit_param_fee_compare == NULL) return "openpit_param_fee_compare";
    _fn_openpit_param_fee_to_string = (OpenPitSharedString * (*)(OpenPitParamFee, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_to_string");
    if (_fn_openpit_param_fee_to_string == NULL) return "openpit_param_fee_to_string";
    _fn_openpit_param_fee_checked_add = (bool (*)(OpenPitParamFee, OpenPitParamFee, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_add");
    if (_fn_openpit_param_fee_checked_add == NULL) return "openpit_param_fee_checked_add";
    _fn_openpit_param_fee_checked_sub = (bool (*)(OpenPitParamFee, OpenPitParamFee, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_sub");
    if (_fn_openpit_param_fee_checked_sub == NULL) return "openpit_param_fee_checked_sub";
    _fn_openpit_param_fee_checked_mul_i64 = (bool (*)(OpenPitParamFee, int64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_mul_i64");
    if (_fn_openpit_param_fee_checked_mul_i64 == NULL) return "openpit_param_fee_checked_mul_i64";
    _fn_openpit_param_fee_checked_mul_u64 = (bool (*)(OpenPitParamFee, uint64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_mul_u64");
    if (_fn_openpit_param_fee_checked_mul_u64 == NULL) return "openpit_param_fee_checked_mul_u64";
    _fn_openpit_param_fee_checked_mul_f64 = (bool (*)(OpenPitParamFee, double, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_mul_f64");
    if (_fn_openpit_param_fee_checked_mul_f64 == NULL) return "openpit_param_fee_checked_mul_f64";
    _fn_openpit_param_fee_checked_div_i64 = (bool (*)(OpenPitParamFee, int64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_div_i64");
    if (_fn_openpit_param_fee_checked_div_i64 == NULL) return "openpit_param_fee_checked_div_i64";
    _fn_openpit_param_fee_checked_div_u64 = (bool (*)(OpenPitParamFee, uint64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_div_u64");
    if (_fn_openpit_param_fee_checked_div_u64 == NULL) return "openpit_param_fee_checked_div_u64";
    _fn_openpit_param_fee_checked_div_f64 = (bool (*)(OpenPitParamFee, double, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_div_f64");
    if (_fn_openpit_param_fee_checked_div_f64 == NULL) return "openpit_param_fee_checked_div_f64";
    _fn_openpit_param_fee_checked_rem_i64 = (bool (*)(OpenPitParamFee, int64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_rem_i64");
    if (_fn_openpit_param_fee_checked_rem_i64 == NULL) return "openpit_param_fee_checked_rem_i64";
    _fn_openpit_param_fee_checked_rem_u64 = (bool (*)(OpenPitParamFee, uint64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_rem_u64");
    if (_fn_openpit_param_fee_checked_rem_u64 == NULL) return "openpit_param_fee_checked_rem_u64";
    _fn_openpit_param_fee_checked_rem_f64 = (bool (*)(OpenPitParamFee, double, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_rem_f64");
    if (_fn_openpit_param_fee_checked_rem_f64 == NULL) return "openpit_param_fee_checked_rem_f64";
    _fn_openpit_param_fee_checked_neg = (bool (*)(OpenPitParamFee, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_checked_neg");
    if (_fn_openpit_param_fee_checked_neg == NULL) return "openpit_param_fee_checked_neg";
    _fn_openpit_create_param_notional_from_string = (bool (*)(OpenPitStringView, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_string");
    if (_fn_openpit_create_param_notional_from_string == NULL) return "openpit_create_param_notional_from_string";
    _fn_openpit_create_param_notional_from_f64 = (bool (*)(double, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_f64");
    if (_fn_openpit_create_param_notional_from_f64 == NULL) return "openpit_create_param_notional_from_f64";
    _fn_openpit_create_param_notional_from_int64 = (bool (*)(int64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_int64");
    if (_fn_openpit_create_param_notional_from_int64 == NULL) return "openpit_create_param_notional_from_int64";
    _fn_openpit_create_param_notional_from_uint64 = (bool (*)(uint64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_uint64");
    if (_fn_openpit_create_param_notional_from_uint64 == NULL) return "openpit_create_param_notional_from_uint64";
    _fn_openpit_create_param_notional_from_string_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_string_rounded");
    if (_fn_openpit_create_param_notional_from_string_rounded == NULL) return "openpit_create_param_notional_from_string_rounded";
    _fn_openpit_create_param_notional_from_f64_rounded = (bool (*)(double, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_f64_rounded");
    if (_fn_openpit_create_param_notional_from_f64_rounded == NULL) return "openpit_create_param_notional_from_f64_rounded";
    _fn_openpit_create_param_notional_from_decimal_rounded = (bool (*)(OpenPitParamDecimal, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_decimal_rounded");
    if (_fn_openpit_create_param_notional_from_decimal_rounded == NULL) return "openpit_create_param_notional_from_decimal_rounded";
    _fn_openpit_param_notional_to_f64 = (bool (*)(OpenPitParamNotional, double *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_to_f64");
    if (_fn_openpit_param_notional_to_f64 == NULL) return "openpit_param_notional_to_f64";
    _fn_openpit_param_notional_is_zero = (bool (*)(OpenPitParamNotional, bool *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_is_zero");
    if (_fn_openpit_param_notional_is_zero == NULL) return "openpit_param_notional_is_zero";
    _fn_openpit_param_notional_compare = (bool (*)(OpenPitParamNotional, OpenPitParamNotional, int8_t *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_compare");
    if (_fn_openpit_param_notional_compare == NULL) return "openpit_param_notional_compare";
    _fn_openpit_param_notional_to_string = (OpenPitSharedString * (*)(OpenPitParamNotional, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_to_string");
    if (_fn_openpit_param_notional_to_string == NULL) return "openpit_param_notional_to_string";
    _fn_openpit_param_notional_checked_add = (bool (*)(OpenPitParamNotional, OpenPitParamNotional, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_add");
    if (_fn_openpit_param_notional_checked_add == NULL) return "openpit_param_notional_checked_add";
    _fn_openpit_param_notional_checked_sub = (bool (*)(OpenPitParamNotional, OpenPitParamNotional, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_sub");
    if (_fn_openpit_param_notional_checked_sub == NULL) return "openpit_param_notional_checked_sub";
    _fn_openpit_param_notional_checked_mul_i64 = (bool (*)(OpenPitParamNotional, int64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_mul_i64");
    if (_fn_openpit_param_notional_checked_mul_i64 == NULL) return "openpit_param_notional_checked_mul_i64";
    _fn_openpit_param_notional_checked_mul_u64 = (bool (*)(OpenPitParamNotional, uint64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_mul_u64");
    if (_fn_openpit_param_notional_checked_mul_u64 == NULL) return "openpit_param_notional_checked_mul_u64";
    _fn_openpit_param_notional_checked_mul_f64 = (bool (*)(OpenPitParamNotional, double, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_mul_f64");
    if (_fn_openpit_param_notional_checked_mul_f64 == NULL) return "openpit_param_notional_checked_mul_f64";
    _fn_openpit_param_notional_checked_div_i64 = (bool (*)(OpenPitParamNotional, int64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_div_i64");
    if (_fn_openpit_param_notional_checked_div_i64 == NULL) return "openpit_param_notional_checked_div_i64";
    _fn_openpit_param_notional_checked_div_u64 = (bool (*)(OpenPitParamNotional, uint64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_div_u64");
    if (_fn_openpit_param_notional_checked_div_u64 == NULL) return "openpit_param_notional_checked_div_u64";
    _fn_openpit_param_notional_checked_div_f64 = (bool (*)(OpenPitParamNotional, double, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_div_f64");
    if (_fn_openpit_param_notional_checked_div_f64 == NULL) return "openpit_param_notional_checked_div_f64";
    _fn_openpit_param_notional_checked_rem_i64 = (bool (*)(OpenPitParamNotional, int64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_rem_i64");
    if (_fn_openpit_param_notional_checked_rem_i64 == NULL) return "openpit_param_notional_checked_rem_i64";
    _fn_openpit_param_notional_checked_rem_u64 = (bool (*)(OpenPitParamNotional, uint64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_rem_u64");
    if (_fn_openpit_param_notional_checked_rem_u64 == NULL) return "openpit_param_notional_checked_rem_u64";
    _fn_openpit_param_notional_checked_rem_f64 = (bool (*)(OpenPitParamNotional, double, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_checked_rem_f64");
    if (_fn_openpit_param_notional_checked_rem_f64 == NULL) return "openpit_param_notional_checked_rem_f64";
    _fn_openpit_param_leverage_calculate_margin_required = (bool (*)(OpenPitParamLeverage, OpenPitParamNotional, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_leverage_calculate_margin_required");
    if (_fn_openpit_param_leverage_calculate_margin_required == NULL) return "openpit_param_leverage_calculate_margin_required";
    _fn_openpit_param_price_calculate_volume = (bool (*)(OpenPitParamPrice, OpenPitParamQuantity, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_calculate_volume");
    if (_fn_openpit_param_price_calculate_volume == NULL) return "openpit_param_price_calculate_volume";
    _fn_openpit_param_price_calculate_position_size = (bool (*)(OpenPitParamPrice, OpenPitParamQuantity, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_calculate_position_size");
    if (_fn_openpit_param_price_calculate_position_size == NULL) return "openpit_param_price_calculate_position_size";
    _fn_openpit_param_quantity_calculate_volume = (bool (*)(OpenPitParamQuantity, OpenPitParamPrice, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_calculate_volume");
    if (_fn_openpit_param_quantity_calculate_volume == NULL) return "openpit_param_quantity_calculate_volume";
    _fn_openpit_param_volume_calculate_quantity = (bool (*)(OpenPitParamVolume, OpenPitParamPrice, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_calculate_quantity");
    if (_fn_openpit_param_volume_calculate_quantity == NULL) return "openpit_param_volume_calculate_quantity";
    _fn_openpit_param_pnl_to_cash_flow = (bool (*)(OpenPitParamPnl, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_to_cash_flow");
    if (_fn_openpit_param_pnl_to_cash_flow == NULL) return "openpit_param_pnl_to_cash_flow";
    _fn_openpit_param_pnl_to_position_size = (bool (*)(OpenPitParamPnl, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_to_position_size");
    if (_fn_openpit_param_pnl_to_position_size == NULL) return "openpit_param_pnl_to_position_size";
    _fn_openpit_param_quantity_to_position_size = (bool (*)(OpenPitParamQuantity, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_to_position_size");
    if (_fn_openpit_param_quantity_to_position_size == NULL) return "openpit_param_quantity_to_position_size";
    _fn_openpit_param_volume_to_position_size = (bool (*)(OpenPitParamVolume, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_to_position_size");
    if (_fn_openpit_param_volume_to_position_size == NULL) return "openpit_param_volume_to_position_size";
    _fn_openpit_param_pnl_from_fee = (bool (*)(OpenPitParamFee, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_from_fee");
    if (_fn_openpit_param_pnl_from_fee == NULL) return "openpit_param_pnl_from_fee";
    _fn_openpit_param_cash_flow_from_pnl = (bool (*)(OpenPitParamPnl, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_from_pnl");
    if (_fn_openpit_param_cash_flow_from_pnl == NULL) return "openpit_param_cash_flow_from_pnl";
    _fn_openpit_param_cash_flow_from_fee = (bool (*)(OpenPitParamFee, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_from_fee");
    if (_fn_openpit_param_cash_flow_from_fee == NULL) return "openpit_param_cash_flow_from_fee";
    _fn_openpit_param_cash_flow_from_volume_inflow = (bool (*)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_from_volume_inflow");
    if (_fn_openpit_param_cash_flow_from_volume_inflow == NULL) return "openpit_param_cash_flow_from_volume_inflow";
    _fn_openpit_param_cash_flow_from_volume_outflow = (bool (*)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_cash_flow_from_volume_outflow");
    if (_fn_openpit_param_cash_flow_from_volume_outflow == NULL) return "openpit_param_cash_flow_from_volume_outflow";
    _fn_openpit_param_fee_to_pnl = (bool (*)(OpenPitParamFee, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_to_pnl");
    if (_fn_openpit_param_fee_to_pnl == NULL) return "openpit_param_fee_to_pnl";
    _fn_openpit_param_fee_to_position_size = (bool (*)(OpenPitParamFee, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_to_position_size");
    if (_fn_openpit_param_fee_to_position_size == NULL) return "openpit_param_fee_to_position_size";
    _fn_openpit_param_fee_to_cash_flow = (bool (*)(OpenPitParamFee, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_fee_to_cash_flow");
    if (_fn_openpit_param_fee_to_cash_flow == NULL) return "openpit_param_fee_to_cash_flow";
    _fn_openpit_param_volume_to_cash_flow_inflow = (bool (*)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_to_cash_flow_inflow");
    if (_fn_openpit_param_volume_to_cash_flow_inflow == NULL) return "openpit_param_volume_to_cash_flow_inflow";
    _fn_openpit_param_volume_to_cash_flow_outflow = (bool (*)(OpenPitParamVolume, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_to_cash_flow_outflow");
    if (_fn_openpit_param_volume_to_cash_flow_outflow == NULL) return "openpit_param_volume_to_cash_flow_outflow";
    _fn_openpit_param_position_size_from_pnl = (bool (*)(OpenPitParamPnl, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_from_pnl");
    if (_fn_openpit_param_position_size_from_pnl == NULL) return "openpit_param_position_size_from_pnl";
    _fn_openpit_param_position_size_from_fee = (bool (*)(OpenPitParamFee, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_from_fee");
    if (_fn_openpit_param_position_size_from_fee == NULL) return "openpit_param_position_size_from_fee";
    _fn_openpit_param_position_size_from_quantity_and_side = (bool (*)(OpenPitParamQuantity, OpenPitParamSide, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_from_quantity_and_side");
    if (_fn_openpit_param_position_size_from_quantity_and_side == NULL) return "openpit_param_position_size_from_quantity_and_side";
    _fn_openpit_param_position_size_to_open_quantity = (bool (*)(OpenPitParamPositionSize, OpenPitParamQuantity *, OpenPitParamSide *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_to_open_quantity");
    if (_fn_openpit_param_position_size_to_open_quantity == NULL) return "openpit_param_position_size_to_open_quantity";
    _fn_openpit_param_position_size_to_close_quantity = (bool (*)(OpenPitParamPositionSize, OpenPitParamQuantity *, OpenPitParamSide *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_to_close_quantity");
    if (_fn_openpit_param_position_size_to_close_quantity == NULL) return "openpit_param_position_size_to_close_quantity";
    _fn_openpit_param_position_size_checked_add_quantity = (bool (*)(OpenPitParamPositionSize, OpenPitParamQuantity, OpenPitParamSide, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_size_checked_add_quantity");
    if (_fn_openpit_param_position_size_checked_add_quantity == NULL) return "openpit_param_position_size_checked_add_quantity";
    _fn_openpit_param_price_calculate_notional = (bool (*)(OpenPitParamPrice, OpenPitParamQuantity, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_price_calculate_notional");
    if (_fn_openpit_param_price_calculate_notional == NULL) return "openpit_param_price_calculate_notional";
    _fn_openpit_param_quantity_calculate_notional = (bool (*)(OpenPitParamQuantity, OpenPitParamPrice, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_calculate_notional");
    if (_fn_openpit_param_quantity_calculate_notional == NULL) return "openpit_param_quantity_calculate_notional";
    _fn_openpit_param_notional_from_volume = (bool (*)(OpenPitParamVolume, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_from_volume");
    if (_fn_openpit_param_notional_from_volume == NULL) return "openpit_param_notional_from_volume";
    _fn_openpit_param_notional_to_volume = (bool (*)(OpenPitParamNotional, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_to_volume");
    if (_fn_openpit_param_notional_to_volume == NULL) return "openpit_param_notional_to_volume";
    _fn_openpit_param_notional_calculate_margin_required = (bool (*)(OpenPitParamNotional, OpenPitParamLeverage, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_notional_calculate_margin_required");
    if (_fn_openpit_param_notional_calculate_margin_required == NULL) return "openpit_param_notional_calculate_margin_required";
    _fn_openpit_param_volume_from_notional = (bool (*)(OpenPitParamNotional, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_from_notional");
    if (_fn_openpit_param_volume_from_notional == NULL) return "openpit_param_volume_from_notional";
    _fn_openpit_create_param_account_id_from_uint64 = (OpenPitParamAccountId (*)(uint64_t))openpit_dlsym(handle, "openpit_create_param_account_id_from_uint64");
    if (_fn_openpit_create_param_account_id_from_uint64 == NULL) return "openpit_create_param_account_id_from_uint64";
    _fn_openpit_create_param_account_id_from_string = (bool (*)(OpenPitStringView, OpenPitParamAccountId *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_account_id_from_string");
    if (_fn_openpit_create_param_account_id_from_string == NULL) return "openpit_create_param_account_id_from_string";
    _fn_openpit_create_param_asset_from_string = (OpenPitSharedString * (*)(OpenPitStringView, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_asset_from_string");
    if (_fn_openpit_create_param_asset_from_string == NULL) return "openpit_create_param_asset_from_string";
    _fn_openpit_destroy_param_asset = (void (*)(OpenPitSharedString *))openpit_dlsym(handle, "openpit_destroy_param_asset");
    if (_fn_openpit_destroy_param_asset == NULL) return "openpit_destroy_param_asset";
    _fn_openpit_param_side_to_string = (OpenPitSharedString * (*)(OpenPitParamSide, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_side_to_string");
    if (_fn_openpit_param_side_to_string == NULL) return "openpit_param_side_to_string";
    _fn_openpit_param_position_side_to_string = (OpenPitSharedString * (*)(OpenPitParamPositionSide, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_side_to_string");
    if (_fn_openpit_param_position_side_to_string == NULL) return "openpit_param_position_side_to_string";
    _fn_openpit_param_position_effect_to_string = (OpenPitSharedString * (*)(OpenPitParamPositionEffect, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_effect_to_string");
    if (_fn_openpit_param_position_effect_to_string == NULL) return "openpit_param_position_effect_to_string";
    _fn_openpit_param_position_mode_to_string = (OpenPitSharedString * (*)(OpenPitParamPositionMode, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_position_mode_to_string");
    if (_fn_openpit_param_position_mode_to_string == NULL) return "openpit_param_position_mode_to_string";
    _fn_openpit_param_account_id_to_string = (OpenPitSharedString * (*)(OpenPitParamAccountId))openpit_dlsym(handle, "openpit_param_account_id_to_string");
    if (_fn_openpit_param_account_id_to_string == NULL) return "openpit_param_account_id_to_string";
    _fn_openpit_param_trade_amount_to_string = (OpenPitSharedString * (*)(OpenPitParamTradeAmount, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_trade_amount_to_string");
    if (_fn_openpit_param_trade_amount_to_string == NULL) return "openpit_param_trade_amount_to_string";
    _fn_openpit_param_adjustment_amount_to_string = (OpenPitSharedString * (*)(OpenPitParamAdjustmentAmount, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_adjustment_amount_to_string");
    if (_fn_openpit_param_adjustment_amount_to_string == NULL) return "openpit_param_adjustment_amount_to_string";
    _fn_openpit_pretrade_create_reject_list = (OpenPitPretradeRejectList * (*)(size_t))openpit_dlsym(handle, "openpit_pretrade_create_reject_list");
    if (_fn_openpit_pretrade_create_reject_list == NULL) return "openpit_pretrade_create_reject_list";
    _fn_openpit_pretrade_destroy_reject_list = (void (*)(OpenPitPretradeRejectList *))openpit_dlsym(handle, "openpit_pretrade_destroy_reject_list");
    if (_fn_openpit_pretrade_destroy_reject_list == NULL) return "openpit_pretrade_destroy_reject_list";
    _fn_openpit_pretrade_reject_list_push = (bool (*)(OpenPitPretradeRejectList *, OpenPitPretradeReject))openpit_dlsym(handle, "openpit_pretrade_reject_list_push");
    if (_fn_openpit_pretrade_reject_list_push == NULL) return "openpit_pretrade_reject_list_push";
    _fn_openpit_pretrade_reject_list_len = (size_t (*)(const OpenPitPretradeRejectList *))openpit_dlsym(handle, "openpit_pretrade_reject_list_len");
    if (_fn_openpit_pretrade_reject_list_len == NULL) return "openpit_pretrade_reject_list_len";
    _fn_openpit_pretrade_reject_list_get = (bool (*)(const OpenPitPretradeRejectList *, size_t, OpenPitPretradeReject *))openpit_dlsym(handle, "openpit_pretrade_reject_list_get");
    if (_fn_openpit_pretrade_reject_list_get == NULL) return "openpit_pretrade_reject_list_get";
    _fn_openpit_pretrade_create_account_block_list = (OpenPitPretradeAccountBlockList * (*)(size_t))openpit_dlsym(handle, "openpit_pretrade_create_account_block_list");
    if (_fn_openpit_pretrade_create_account_block_list == NULL) return "openpit_pretrade_create_account_block_list";
    _fn_openpit_pretrade_destroy_account_block_list = (void (*)(OpenPitPretradeAccountBlockList *))openpit_dlsym(handle, "openpit_pretrade_destroy_account_block_list");
    if (_fn_openpit_pretrade_destroy_account_block_list == NULL) return "openpit_pretrade_destroy_account_block_list";
    _fn_openpit_pretrade_account_block_list_push = (void (*)(OpenPitPretradeAccountBlockList *, OpenPitPretradeAccountBlock))openpit_dlsym(handle, "openpit_pretrade_account_block_list_push");
    if (_fn_openpit_pretrade_account_block_list_push == NULL) return "openpit_pretrade_account_block_list_push";
    _fn_openpit_pretrade_account_block_list_len = (size_t (*)(const OpenPitPretradeAccountBlockList *))openpit_dlsym(handle, "openpit_pretrade_account_block_list_len");
    if (_fn_openpit_pretrade_account_block_list_len == NULL) return "openpit_pretrade_account_block_list_len";
    _fn_openpit_pretrade_account_block_list_get = (bool (*)(const OpenPitPretradeAccountBlockList *, size_t, OpenPitPretradeAccountBlock *))openpit_dlsym(handle, "openpit_pretrade_account_block_list_get");
    if (_fn_openpit_pretrade_account_block_list_get == NULL) return "openpit_pretrade_account_block_list_get";
    _fn_openpit_destroy_param_error = (void (*)(OpenPitParamError *))openpit_dlsym(handle, "openpit_destroy_param_error");
    if (_fn_openpit_destroy_param_error == NULL) return "openpit_destroy_param_error";
    _fn_openpit_create_engine_builder = (OpenPitEngineBuilder * (*)(uint8_t, OpenPitOutError))openpit_dlsym(handle, "openpit_create_engine_builder");
    if (_fn_openpit_create_engine_builder == NULL) return "openpit_create_engine_builder";
    _fn_openpit_destroy_engine_builder = (void (*)(OpenPitEngineBuilder *))openpit_dlsym(handle, "openpit_destroy_engine_builder");
    if (_fn_openpit_destroy_engine_builder == NULL) return "openpit_destroy_engine_builder";
    _fn_openpit_engine_builder_build = (OpenPitEngine * (*)(OpenPitEngineBuilder *, OpenPitEngineBuildError **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_build");
    if (_fn_openpit_engine_builder_build == NULL) return "openpit_engine_builder_build";
    _fn_openpit_destroy_engine_build_error = (void (*)(OpenPitEngineBuildError *))openpit_dlsym(handle, "openpit_destroy_engine_build_error");
    if (_fn_openpit_destroy_engine_build_error == NULL) return "openpit_destroy_engine_build_error";
    _fn_openpit_engine_build_error_get_code = (OpenPitEngineBuildErrorCode (*)(const OpenPitEngineBuildError *))openpit_dlsym(handle, "openpit_engine_build_error_get_code");
    if (_fn_openpit_engine_build_error_get_code == NULL) return "openpit_engine_build_error_get_code";
    _fn_openpit_engine_build_error_get_policy_name = (OpenPitStringView (*)(const OpenPitEngineBuildError *))openpit_dlsym(handle, "openpit_engine_build_error_get_policy_name");
    if (_fn_openpit_engine_build_error_get_policy_name == NULL) return "openpit_engine_build_error_get_policy_name";
    _fn_openpit_engine_build_error_get_policy_group_id = (uint16_t (*)(const OpenPitEngineBuildError *))openpit_dlsym(handle, "openpit_engine_build_error_get_policy_group_id");
    if (_fn_openpit_engine_build_error_get_policy_group_id == NULL) return "openpit_engine_build_error_get_policy_group_id";
    _fn_openpit_destroy_engine = (void (*)(OpenPitEngine *))openpit_dlsym(handle, "openpit_destroy_engine");
    if (_fn_openpit_destroy_engine == NULL) return "openpit_destroy_engine";
    _fn_openpit_engine_start_pre_trade = (OpenPitPretradeStatus (*)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeRequest **, OpenPitPretradeRejectList **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_start_pre_trade");
    if (_fn_openpit_engine_start_pre_trade == NULL) return "openpit_engine_start_pre_trade";
    _fn_openpit_engine_execute_pre_trade = (OpenPitPretradeStatus (*)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_execute_pre_trade");
    if (_fn_openpit_engine_execute_pre_trade == NULL) return "openpit_engine_execute_pre_trade";
    _fn_openpit_engine_execute_pre_trade_drop_copy = (bool (*)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeReservation **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_execute_pre_trade_drop_copy");
    if (_fn_openpit_engine_execute_pre_trade_drop_copy == NULL) return "openpit_engine_execute_pre_trade_drop_copy";
    _fn_openpit_engine_start_pre_trade_dry_run = (bool (*)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeDryRunReport **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_start_pre_trade_dry_run");
    if (_fn_openpit_engine_start_pre_trade_dry_run == NULL) return "openpit_engine_start_pre_trade_dry_run";
    _fn_openpit_engine_execute_pre_trade_dry_run = (bool (*)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeDryRunReport **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_execute_pre_trade_dry_run");
    if (_fn_openpit_engine_execute_pre_trade_dry_run == NULL) return "openpit_engine_execute_pre_trade_dry_run";
    _fn_openpit_pretrade_pre_trade_request_execute = (OpenPitPretradeStatus (*)(OpenPitPretradePreTradeRequest *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_request_execute");
    if (_fn_openpit_pretrade_pre_trade_request_execute == NULL) return "openpit_pretrade_pre_trade_request_execute";
    _fn_openpit_destroy_pretrade_pre_trade_request = (void (*)(OpenPitPretradePreTradeRequest *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_request");
    if (_fn_openpit_destroy_pretrade_pre_trade_request == NULL) return "openpit_destroy_pretrade_pre_trade_request";
    _fn_openpit_pretrade_pre_trade_reservation_commit = (void (*)(OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_commit");
    if (_fn_openpit_pretrade_pre_trade_reservation_commit == NULL) return "openpit_pretrade_pre_trade_reservation_commit";
    _fn_openpit_pretrade_pre_trade_reservation_rollback = (void (*)(OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_rollback");
    if (_fn_openpit_pretrade_pre_trade_reservation_rollback == NULL) return "openpit_pretrade_pre_trade_reservation_rollback";
    _fn_openpit_pretrade_pre_trade_reservation_get_lock = (OpenPitPretradePreTradeLock * (*)(const OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_get_lock");
    if (_fn_openpit_pretrade_pre_trade_reservation_get_lock == NULL) return "openpit_pretrade_pre_trade_reservation_get_lock";
    _fn_openpit_pretrade_pre_trade_reservation_get_account_adjustments = (OpenPitAccountAdjustmentOutcomeList * (*)(const OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_get_account_adjustments");
    if (_fn_openpit_pretrade_pre_trade_reservation_get_account_adjustments == NULL) return "openpit_pretrade_pre_trade_reservation_get_account_adjustments";
    _fn_openpit_pretrade_pre_trade_reservation_get_account_block = (OpenPitPretradeAccountBlockList * (*)(const OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_get_account_block");
    if (_fn_openpit_pretrade_pre_trade_reservation_get_account_block == NULL) return "openpit_pretrade_pre_trade_reservation_get_account_block";
    _fn_openpit_destroy_pretrade_pre_trade_reservation = (void (*)(OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_reservation");
    if (_fn_openpit_destroy_pretrade_pre_trade_reservation == NULL) return "openpit_destroy_pretrade_pre_trade_reservation";
    _fn_openpit_pretrade_pre_trade_dry_run_report_is_pass = (bool (*)(const OpenPitPretradePreTradeDryRunReport *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_dry_run_report_is_pass");
    if (_fn_openpit_pretrade_pre_trade_dry_run_report_is_pass == NULL) return "openpit_pretrade_pre_trade_dry_run_report_is_pass";
    _fn_openpit_pretrade_pre_trade_dry_run_report_get_rejects = (OpenPitPretradeRejectList * (*)(const OpenPitPretradePreTradeDryRunReport *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_dry_run_report_get_rejects");
    if (_fn_openpit_pretrade_pre_trade_dry_run_report_get_rejects == NULL) return "openpit_pretrade_pre_trade_dry_run_report_get_rejects";
    _fn_openpit_pretrade_pre_trade_dry_run_report_get_lock = (OpenPitPretradePreTradeLock * (*)(const OpenPitPretradePreTradeDryRunReport *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_dry_run_report_get_lock");
    if (_fn_openpit_pretrade_pre_trade_dry_run_report_get_lock == NULL) return "openpit_pretrade_pre_trade_dry_run_report_get_lock";
    _fn_openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments = (OpenPitAccountAdjustmentOutcomeList * (*)(const OpenPitPretradePreTradeDryRunReport *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments");
    if (_fn_openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments == NULL) return "openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments";
    _fn_openpit_pretrade_pre_trade_dry_run_report_get_account_block = (OpenPitPretradeAccountBlockList * (*)(const OpenPitPretradePreTradeDryRunReport *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_dry_run_report_get_account_block");
    if (_fn_openpit_pretrade_pre_trade_dry_run_report_get_account_block == NULL) return "openpit_pretrade_pre_trade_dry_run_report_get_account_block";
    _fn_openpit_destroy_pretrade_pre_trade_dry_run_report = (void (*)(OpenPitPretradePreTradeDryRunReport *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_dry_run_report");
    if (_fn_openpit_destroy_pretrade_pre_trade_dry_run_report == NULL) return "openpit_destroy_pretrade_pre_trade_dry_run_report";
    _fn_openpit_engine_apply_execution_report = (bool (*)(OpenPitEngine *, const OpenPitExecutionReport *, OpenPitPostTradeResult **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_apply_execution_report");
    if (_fn_openpit_engine_apply_execution_report == NULL) return "openpit_engine_apply_execution_report";
    _fn_openpit_destroy_post_trade_result = (void (*)(OpenPitPostTradeResult *))openpit_dlsym(handle, "openpit_destroy_post_trade_result");
    if (_fn_openpit_destroy_post_trade_result == NULL) return "openpit_destroy_post_trade_result";
    _fn_openpit_post_trade_result_get_account_blocks = (const OpenPitPretradeAccountBlockList * (*)(const OpenPitPostTradeResult *))openpit_dlsym(handle, "openpit_post_trade_result_get_account_blocks");
    if (_fn_openpit_post_trade_result_get_account_blocks == NULL) return "openpit_post_trade_result_get_account_blocks";
    _fn_openpit_post_trade_result_get_account_adjustments = (const OpenPitAccountAdjustmentOutcomeList * (*)(const OpenPitPostTradeResult *))openpit_dlsym(handle, "openpit_post_trade_result_get_account_adjustments");
    if (_fn_openpit_post_trade_result_get_account_adjustments == NULL) return "openpit_post_trade_result_get_account_adjustments";
    _fn_openpit_post_trade_result_get_account_pnls = (const OpenPitAccountPnlOutcomeList * (*)(const OpenPitPostTradeResult *))openpit_dlsym(handle, "openpit_post_trade_result_get_account_pnls");
    if (_fn_openpit_post_trade_result_get_account_pnls == NULL) return "openpit_post_trade_result_get_account_pnls";
    _fn_openpit_destroy_account_adjustment_batch_error = (void (*)(OpenPitAccountAdjustmentBatchError *))openpit_dlsym(handle, "openpit_destroy_account_adjustment_batch_error");
    if (_fn_openpit_destroy_account_adjustment_batch_error == NULL) return "openpit_destroy_account_adjustment_batch_error";
    _fn_openpit_account_adjustment_batch_error_get_failed_adjustment_index = (size_t (*)(const OpenPitAccountAdjustmentBatchError *))openpit_dlsym(handle, "openpit_account_adjustment_batch_error_get_failed_adjustment_index");
    if (_fn_openpit_account_adjustment_batch_error_get_failed_adjustment_index == NULL) return "openpit_account_adjustment_batch_error_get_failed_adjustment_index";
    _fn_openpit_account_adjustment_batch_error_get_rejects = (const OpenPitPretradeRejectList * (*)(const OpenPitAccountAdjustmentBatchError *))openpit_dlsym(handle, "openpit_account_adjustment_batch_error_get_rejects");
    if (_fn_openpit_account_adjustment_batch_error_get_rejects == NULL) return "openpit_account_adjustment_batch_error_get_rejects";
    _fn_openpit_engine_apply_account_adjustment = (OpenPitAccountAdjustmentApplyStatus (*)(OpenPitEngine *, OpenPitParamAccountId, const OpenPitAccountAdjustment *, size_t, OpenPitAccountAdjustmentBatchError **, OpenPitAccountAdjustmentOutcomeList **, OpenPitPretradeAccountBlockList **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_apply_account_adjustment");
    if (_fn_openpit_engine_apply_account_adjustment == NULL) return "openpit_engine_apply_account_adjustment";
    _fn_openpit_destroy_account_group_error = (void (*)(OpenPitAccountGroupError *))openpit_dlsym(handle, "openpit_destroy_account_group_error");
    if (_fn_openpit_destroy_account_group_error == NULL) return "openpit_destroy_account_group_error";
    _fn_openpit_account_group_error_get_message = (OpenPitStringView (*)(const OpenPitAccountGroupError *))openpit_dlsym(handle, "openpit_account_group_error_get_message");
    if (_fn_openpit_account_group_error_get_message == NULL) return "openpit_account_group_error_get_message";
    _fn_openpit_account_group_error_get_account = (OpenPitParamAccountId (*)(const OpenPitAccountGroupError *))openpit_dlsym(handle, "openpit_account_group_error_get_account");
    if (_fn_openpit_account_group_error_get_account == NULL) return "openpit_account_group_error_get_account";
    _fn_openpit_account_group_error_get_current_group = (bool (*)(const OpenPitAccountGroupError *, OpenPitParamAccountGroupId *))openpit_dlsym(handle, "openpit_account_group_error_get_current_group");
    if (_fn_openpit_account_group_error_get_current_group == NULL) return "openpit_account_group_error_get_current_group";
    _fn_openpit_engine_register_account_group = (bool (*)(OpenPitEngine *, const OpenPitParamAccountId *, size_t, OpenPitParamAccountGroupId, OpenPitAccountGroupError **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_register_account_group");
    if (_fn_openpit_engine_register_account_group == NULL) return "openpit_engine_register_account_group";
    _fn_openpit_engine_unregister_account_group = (bool (*)(OpenPitEngine *, const OpenPitParamAccountId *, size_t, OpenPitParamAccountGroupId, OpenPitAccountGroupError **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_unregister_account_group");
    if (_fn_openpit_engine_unregister_account_group == NULL) return "openpit_engine_unregister_account_group";
    _fn_openpit_engine_account_group = (bool (*)(const OpenPitEngine *, OpenPitParamAccountId, OpenPitParamAccountGroupId *))openpit_dlsym(handle, "openpit_engine_account_group");
    if (_fn_openpit_engine_account_group == NULL) return "openpit_engine_account_group";
    _fn_openpit_engine_set_account_currency = (bool (*)(OpenPitEngine *, OpenPitParamAccountId, OpenPitStringView, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_set_account_currency");
    if (_fn_openpit_engine_set_account_currency == NULL) return "openpit_engine_set_account_currency";
    _fn_openpit_engine_clear_account_currency = (void (*)(OpenPitEngine *, OpenPitParamAccountId))openpit_dlsym(handle, "openpit_engine_clear_account_currency");
    if (_fn_openpit_engine_clear_account_currency == NULL) return "openpit_engine_clear_account_currency";
    _fn_openpit_engine_set_account_group_currency = (bool (*)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitStringView, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_set_account_group_currency");
    if (_fn_openpit_engine_set_account_group_currency == NULL) return "openpit_engine_set_account_group_currency";
    _fn_openpit_engine_clear_account_group_currency = (void (*)(OpenPitEngine *, OpenPitParamAccountGroupId))openpit_dlsym(handle, "openpit_engine_clear_account_group_currency");
    if (_fn_openpit_engine_clear_account_group_currency == NULL) return "openpit_engine_clear_account_group_currency";
    _fn_openpit_destroy_account_block_error = (void (*)(OpenPitAccountBlockError *))openpit_dlsym(handle, "openpit_destroy_account_block_error");
    if (_fn_openpit_destroy_account_block_error == NULL) return "openpit_destroy_account_block_error";
    _fn_openpit_account_block_error_get_message = (OpenPitStringView (*)(const OpenPitAccountBlockError *))openpit_dlsym(handle, "openpit_account_block_error_get_message");
    if (_fn_openpit_account_block_error_get_message == NULL) return "openpit_account_block_error_get_message";
    _fn_openpit_account_block_error_get_kind = (OpenPitAccountBlockErrorKind (*)(const OpenPitAccountBlockError *))openpit_dlsym(handle, "openpit_account_block_error_get_kind");
    if (_fn_openpit_account_block_error_get_kind == NULL) return "openpit_account_block_error_get_kind";
    _fn_openpit_account_block_error_get_account = (bool (*)(const OpenPitAccountBlockError *, OpenPitParamAccountId *))openpit_dlsym(handle, "openpit_account_block_error_get_account");
    if (_fn_openpit_account_block_error_get_account == NULL) return "openpit_account_block_error_get_account";
    _fn_openpit_account_block_error_get_group = (bool (*)(const OpenPitAccountBlockError *, OpenPitParamAccountGroupId *))openpit_dlsym(handle, "openpit_account_block_error_get_group");
    if (_fn_openpit_account_block_error_get_group == NULL) return "openpit_account_block_error_get_group";
    _fn_openpit_destroy_configure_error = (void (*)(OpenPitConfigureError *))openpit_dlsym(handle, "openpit_destroy_configure_error");
    if (_fn_openpit_destroy_configure_error == NULL) return "openpit_destroy_configure_error";
    _fn_openpit_configure_error_get_message = (OpenPitStringView (*)(const OpenPitConfigureError *))openpit_dlsym(handle, "openpit_configure_error_get_message");
    if (_fn_openpit_configure_error_get_message == NULL) return "openpit_configure_error_get_message";
    _fn_openpit_configure_error_get_kind = (OpenPitConfigureErrorKind (*)(const OpenPitConfigureError *))openpit_dlsym(handle, "openpit_configure_error_get_kind");
    if (_fn_openpit_configure_error_get_kind == NULL) return "openpit_configure_error_get_kind";
    _fn_openpit_engine_block_account = (void (*)(OpenPitEngine *, OpenPitParamAccountId, OpenPitStringView))openpit_dlsym(handle, "openpit_engine_block_account");
    if (_fn_openpit_engine_block_account == NULL) return "openpit_engine_block_account";
    _fn_openpit_engine_unblock_account = (void (*)(OpenPitEngine *, OpenPitParamAccountId))openpit_dlsym(handle, "openpit_engine_unblock_account");
    if (_fn_openpit_engine_unblock_account == NULL) return "openpit_engine_unblock_account";
    _fn_openpit_engine_replace_account_block_reason = (bool (*)(OpenPitEngine *, OpenPitParamAccountId, OpenPitStringView, OpenPitAccountBlockError **))openpit_dlsym(handle, "openpit_engine_replace_account_block_reason");
    if (_fn_openpit_engine_replace_account_block_reason == NULL) return "openpit_engine_replace_account_block_reason";
    _fn_openpit_engine_block_account_group = (bool (*)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitStringView, OpenPitAccountBlockError **))openpit_dlsym(handle, "openpit_engine_block_account_group");
    if (_fn_openpit_engine_block_account_group == NULL) return "openpit_engine_block_account_group";
    _fn_openpit_engine_unblock_account_group = (bool (*)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitAccountBlockError **))openpit_dlsym(handle, "openpit_engine_unblock_account_group");
    if (_fn_openpit_engine_unblock_account_group == NULL) return "openpit_engine_unblock_account_group";
    _fn_openpit_engine_replace_account_group_block_reason = (bool (*)(OpenPitEngine *, OpenPitParamAccountGroupId, OpenPitStringView, OpenPitAccountBlockError **))openpit_dlsym(handle, "openpit_engine_replace_account_group_block_reason");
    if (_fn_openpit_engine_replace_account_group_block_reason == NULL) return "openpit_engine_replace_account_group_block_reason";
    _fn_openpit_create_pretrade_custom_pre_trade_policy = (OpenPitPretradePreTradePolicy * (*)(OpenPitStringView, uint16_t, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyApplyExecutionReportFn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn, OpenPitPretradePreTradePolicyFreeUserDataFn, void *, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_custom_pre_trade_policy");
    if (_fn_openpit_create_pretrade_custom_pre_trade_policy == NULL) return "openpit_create_pretrade_custom_pre_trade_policy";
    _fn_openpit_create_pretrade_custom_pre_trade_policy_with_dry_run = (OpenPitPretradePreTradePolicy * (*)(OpenPitStringView, uint16_t, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyApplyExecutionReportFn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn, OpenPitPretradePreTradePolicyFreeUserDataFn, void *, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_custom_pre_trade_policy_with_dry_run");
    if (_fn_openpit_create_pretrade_custom_pre_trade_policy_with_dry_run == NULL) return "openpit_create_pretrade_custom_pre_trade_policy_with_dry_run";
    _fn_openpit_destroy_pretrade_pre_trade_policy = (void (*)(OpenPitPretradePreTradePolicy *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_policy");
    if (_fn_openpit_destroy_pretrade_pre_trade_policy == NULL) return "openpit_destroy_pretrade_pre_trade_policy";
    _fn_openpit_pretrade_pre_trade_policy_get_name = (OpenPitStringView (*)(const OpenPitPretradePreTradePolicy *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_policy_get_name");
    if (_fn_openpit_pretrade_pre_trade_policy_get_name == NULL) return "openpit_pretrade_pre_trade_policy_get_name";
    _fn_openpit_engine_builder_add_pre_trade_policy = (bool (*)(OpenPitEngineBuilder *, OpenPitPretradePreTradePolicy *, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_pre_trade_policy");
    if (_fn_openpit_engine_builder_add_pre_trade_policy == NULL) return "openpit_engine_builder_add_pre_trade_policy";
    _fn_openpit_mutations_push = (bool (*)(OpenPitMutations *, OpenPitMutationFn, OpenPitMutationFn, void *, OpenPitMutationFreeFn, OpenPitOutError))openpit_dlsym(handle, "openpit_mutations_push");
    if (_fn_openpit_mutations_push == NULL) return "openpit_mutations_push";
    _fn_openpit_engine_builder_add_builtin_order_size_limit_policy = (bool (*)(OpenPitEngineBuilder *, uint16_t, const OpenPitPretradePoliciesOrderSizeBrokerBarrier *, const OpenPitPretradePoliciesOrderSizeAssetBarrier *, size_t, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_order_size_limit_policy");
    if (_fn_openpit_engine_builder_add_builtin_order_size_limit_policy == NULL) return "openpit_engine_builder_add_builtin_order_size_limit_policy";
    _fn_openpit_engine_configure_order_size_limit = (bool (*)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesOrderSizeBrokerBarrier *, bool, const OpenPitPretradePoliciesOrderSizeAssetBarrier *, size_t, bool, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier *, size_t, bool, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_order_size_limit");
    if (_fn_openpit_engine_configure_order_size_limit == NULL) return "openpit_engine_configure_order_size_limit";
    _fn_openpit_engine_builder_add_builtin_order_validation_policy = (bool (*)(OpenPitEngineBuilder *, uint16_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_order_validation_policy");
    if (_fn_openpit_engine_builder_add_builtin_order_validation_policy == NULL) return "openpit_engine_builder_add_builtin_order_validation_policy";
    _fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy = (bool (*)(OpenPitEngineBuilder *, uint16_t, const OpenPitPretradePoliciesPnlBoundsBarrier *, size_t, const OpenPitPretradePoliciesPnlBoundsAccountBarrier *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy");
    if (_fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy == NULL) return "openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy";
    _fn_openpit_engine_configure_pnl_bounds_killswitch = (bool (*)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesPnlBoundsBarrier *, size_t, bool, const OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate *, size_t, bool, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_pnl_bounds_killswitch");
    if (_fn_openpit_engine_configure_pnl_bounds_killswitch == NULL) return "openpit_engine_configure_pnl_bounds_killswitch";
    _fn_openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl = (bool (*)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountId, OpenPitStringView, OpenPitParamPnl, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl");
    if (_fn_openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl == NULL) return "openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl";
    _fn_openpit_engine_builder_add_builtin_rate_limit_policy = (bool (*)(OpenPitEngineBuilder *, uint16_t, const OpenPitPretradePoliciesRateLimitBrokerBarrier *, const OpenPitPretradePoliciesRateLimitAssetBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_rate_limit_policy");
    if (_fn_openpit_engine_builder_add_builtin_rate_limit_policy == NULL) return "openpit_engine_builder_add_builtin_rate_limit_policy";
    _fn_openpit_engine_configure_rate_limit = (bool (*)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesRateLimitBrokerBarrier *, bool, const OpenPitPretradePoliciesRateLimitAssetBarrier *, size_t, bool, const OpenPitPretradePoliciesRateLimitAccountBarrier *, size_t, bool, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier *, size_t, bool, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_rate_limit");
    if (_fn_openpit_engine_configure_rate_limit == NULL) return "openpit_engine_configure_rate_limit";
    _fn_openpit_engine_builder_add_builtin_spot_funds_policy = (bool (*)(OpenPitEngineBuilder *, const OpenPitMarketDataService *, const uint16_t *, uint8_t, const OpenPitPretradePoliciesSpotFundsOverride *, size_t, uint16_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_spot_funds_policy");
    if (_fn_openpit_engine_builder_add_builtin_spot_funds_policy == NULL) return "openpit_engine_builder_add_builtin_spot_funds_policy";
    _fn_openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy = (bool (*)(OpenPitEngineBuilder *, const OpenPitMarketDataService *, uint16_t, const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier *, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier *, size_t, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy");
    if (_fn_openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy == NULL) return "openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy";
    _fn_openpit_engine_configure_spot_funds = (bool (*)(OpenPitEngine *, OpenPitStringView, uint16_t, bool, uint8_t, bool, const OpenPitPretradePoliciesSpotFundsOverride *, size_t, bool, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_spot_funds");
    if (_fn_openpit_engine_configure_spot_funds == NULL) return "openpit_engine_configure_spot_funds";
    _fn_openpit_engine_configure_spot_funds_pnl_bounds_killswitch = (bool (*)(OpenPitEngine *, OpenPitStringView, const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier *, bool, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier *, size_t, bool, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier *, size_t, bool, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_spot_funds_pnl_bounds_killswitch");
    if (_fn_openpit_engine_configure_spot_funds_pnl_bounds_killswitch == NULL) return "openpit_engine_configure_spot_funds_pnl_bounds_killswitch";
    _fn_openpit_engine_configure_spot_funds_set_account_pnl = (OpenPitPretradeAccountBlockList * (*)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountId, OpenPitPnlState, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_spot_funds_set_account_pnl");
    if (_fn_openpit_engine_configure_spot_funds_set_account_pnl == NULL) return "openpit_engine_configure_spot_funds_set_account_pnl";
    _fn_openpit_engine_configure_spot_funds_global_limit_mode = (bool (*)(OpenPitEngine *, OpenPitStringView, OpenPitPretradePoliciesSpotFundsLimitMode, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_spot_funds_global_limit_mode");
    if (_fn_openpit_engine_configure_spot_funds_global_limit_mode == NULL) return "openpit_engine_configure_spot_funds_global_limit_mode";
    _fn_openpit_engine_configure_spot_funds_account_limit_mode = (bool (*)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountId, OpenPitPretradePoliciesSpotFundsLimitMode, bool, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_spot_funds_account_limit_mode");
    if (_fn_openpit_engine_configure_spot_funds_account_limit_mode == NULL) return "openpit_engine_configure_spot_funds_account_limit_mode";
    _fn_openpit_engine_configure_spot_funds_account_group_limit_mode = (bool (*)(OpenPitEngine *, OpenPitStringView, OpenPitParamAccountGroupId, OpenPitPretradePoliciesSpotFundsLimitMode, bool, OpenPitConfigureError **))openpit_dlsym(handle, "openpit_engine_configure_spot_funds_account_group_limit_mode");
    if (_fn_openpit_engine_configure_spot_funds_account_group_limit_mode == NULL) return "openpit_engine_configure_spot_funds_account_group_limit_mode";
    _fn_openpit_get_runtime_version = (OpenPitStringView (*)(void))openpit_dlsym(handle, "openpit_get_runtime_version");
    if (_fn_openpit_get_runtime_version == NULL) return "openpit_get_runtime_version";
    _fn_openpit_get_runtime_build_profile = (OpenPitStringView (*)(void))openpit_dlsym(handle, "openpit_get_runtime_build_profile");
    if (_fn_openpit_get_runtime_build_profile == NULL) return "openpit_get_runtime_build_profile";
    _fn_openpit_account_control_block = (void (*)(const OpenPitAccountControl *, OpenPitPretradeAccountBlock))openpit_dlsym(handle, "openpit_account_control_block");
    if (_fn_openpit_account_control_block == NULL) return "openpit_account_control_block";
    _fn_openpit_account_control_clone = (OpenPitAccountControl * (*)(const OpenPitAccountControl *))openpit_dlsym(handle, "openpit_account_control_clone");
    if (_fn_openpit_account_control_clone == NULL) return "openpit_account_control_clone";
    _fn_openpit_destroy_account_control = (void (*)(OpenPitAccountControl *))openpit_dlsym(handle, "openpit_destroy_account_control");
    if (_fn_openpit_destroy_account_control == NULL) return "openpit_destroy_account_control";
    _fn_openpit_pretrade_context_get_account_control = (OpenPitAccountControl * (*)(const OpenPitPretradeContext *))openpit_dlsym(handle, "openpit_pretrade_context_get_account_control");
    if (_fn_openpit_pretrade_context_get_account_control == NULL) return "openpit_pretrade_context_get_account_control";
    _fn_openpit_account_adjustment_context_get_account_control = (OpenPitAccountControl * (*)(const OpenPitAccountAdjustmentContext *))openpit_dlsym(handle, "openpit_account_adjustment_context_get_account_control");
    if (_fn_openpit_account_adjustment_context_get_account_control == NULL) return "openpit_account_adjustment_context_get_account_control";
    _fn_openpit_pretrade_context_get_account_group = (bool (*)(const OpenPitPretradeContext *, OpenPitParamAccountGroupId *))openpit_dlsym(handle, "openpit_pretrade_context_get_account_group");
    if (_fn_openpit_pretrade_context_get_account_group == NULL) return "openpit_pretrade_context_get_account_group";
    _fn_openpit_account_adjustment_context_get_account_group = (bool (*)(const OpenPitAccountAdjustmentContext *, OpenPitParamAccountGroupId *))openpit_dlsym(handle, "openpit_account_adjustment_context_get_account_group");
    if (_fn_openpit_account_adjustment_context_get_account_group == NULL) return "openpit_account_adjustment_context_get_account_group";
    _fn_openpit_post_trade_context_get_account_group = (bool (*)(const OpenPitPostTradeContext *, OpenPitParamAccountGroupId *))openpit_dlsym(handle, "openpit_post_trade_context_get_account_group");
    if (_fn_openpit_post_trade_context_get_account_group == NULL) return "openpit_post_trade_context_get_account_group";
    _fn_openpit_create_param_account_group_id_from_uint32 = (bool (*)(uint32_t, OpenPitParamAccountGroupId *, OpenPitOutError))openpit_dlsym(handle, "openpit_create_param_account_group_id_from_uint32");
    if (_fn_openpit_create_param_account_group_id_from_uint32 == NULL) return "openpit_create_param_account_group_id_from_uint32";
    _fn_openpit_create_param_account_group_id_from_string = (bool (*)(OpenPitStringView, OpenPitParamAccountGroupId *, OpenPitOutError))openpit_dlsym(handle, "openpit_create_param_account_group_id_from_string");
    if (_fn_openpit_create_param_account_group_id_from_string == NULL) return "openpit_create_param_account_group_id_from_string";
    _fn_openpit_destroy_account_adjustment_outcome_list = (void (*)(OpenPitAccountAdjustmentOutcomeList *))openpit_dlsym(handle, "openpit_destroy_account_adjustment_outcome_list");
    if (_fn_openpit_destroy_account_adjustment_outcome_list == NULL) return "openpit_destroy_account_adjustment_outcome_list";
    _fn_openpit_account_adjustment_outcome_list_len = (size_t (*)(const OpenPitAccountAdjustmentOutcomeList *))openpit_dlsym(handle, "openpit_account_adjustment_outcome_list_len");
    if (_fn_openpit_account_adjustment_outcome_list_len == NULL) return "openpit_account_adjustment_outcome_list_len";
    _fn_openpit_account_adjustment_outcome_list_get = (bool (*)(const OpenPitAccountAdjustmentOutcomeList *, size_t, OpenPitAccountAdjustmentOutcome *))openpit_dlsym(handle, "openpit_account_adjustment_outcome_list_get");
    if (_fn_openpit_account_adjustment_outcome_list_get == NULL) return "openpit_account_adjustment_outcome_list_get";
    _fn_openpit_account_pnl_outcome_list_len = (size_t (*)(const OpenPitAccountPnlOutcomeList *))openpit_dlsym(handle, "openpit_account_pnl_outcome_list_len");
    if (_fn_openpit_account_pnl_outcome_list_len == NULL) return "openpit_account_pnl_outcome_list_len";
    _fn_openpit_account_pnl_outcome_list_get = (bool (*)(const OpenPitAccountPnlOutcomeList *, size_t, OpenPitAccountPnlOutcome *))openpit_dlsym(handle, "openpit_account_pnl_outcome_list_get");
    if (_fn_openpit_account_pnl_outcome_list_get == NULL) return "openpit_account_pnl_outcome_list_get";
    _fn_openpit_pretrade_pre_trade_result_push_lock_price = (bool (*)(OpenPitPretradePreTradeResult *, OpenPitParamPrice, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_result_push_lock_price");
    if (_fn_openpit_pretrade_pre_trade_result_push_lock_price == NULL) return "openpit_pretrade_pre_trade_result_push_lock_price";
    _fn_openpit_pretrade_pre_trade_result_push_account_adjustment = (bool (*)(OpenPitPretradePreTradeResult *, OpenPitAccountOutcomeEntry, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_result_push_account_adjustment");
    if (_fn_openpit_pretrade_pre_trade_result_push_account_adjustment == NULL) return "openpit_pretrade_pre_trade_result_push_account_adjustment";
    _fn_openpit_pretrade_post_trade_adjustment_list_push = (bool (*)(OpenPitPostTradeAdjustmentList *, uint16_t, OpenPitAccountOutcomeEntry, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_post_trade_adjustment_list_push");
    if (_fn_openpit_pretrade_post_trade_adjustment_list_push == NULL) return "openpit_pretrade_post_trade_adjustment_list_push";
    _fn_openpit_pretrade_post_trade_account_pnl_list_push = (bool (*)(OpenPitPostTradeAccountPnlList *, OpenPitAccountPnlOutcome, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_post_trade_account_pnl_list_push");
    if (_fn_openpit_pretrade_post_trade_account_pnl_list_push == NULL) return "openpit_pretrade_post_trade_account_pnl_list_push";
    _fn_openpit_pretrade_account_adjustment_result_push_account_outcome = (bool (*)(OpenPitPretradeAccountAdjustmentResult *, OpenPitAccountOutcomeEntry, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_account_adjustment_result_push_account_outcome");
    if (_fn_openpit_pretrade_account_adjustment_result_push_account_outcome == NULL) return "openpit_pretrade_account_adjustment_result_push_account_outcome";
    _fn_openpit_pretrade_account_adjustment_result_push_account_block = (void (*)(OpenPitPretradeAccountAdjustmentResult *, OpenPitPretradeAccountBlock))openpit_dlsym(handle, "openpit_pretrade_account_adjustment_result_push_account_block");
    if (_fn_openpit_pretrade_account_adjustment_result_push_account_block == NULL) return "openpit_pretrade_account_adjustment_result_push_account_block";
    _fn_openpit_destroy_shared_bytes = (void (*)(OpenPitSharedBytes *))openpit_dlsym(handle, "openpit_destroy_shared_bytes");
    if (_fn_openpit_destroy_shared_bytes == NULL) return "openpit_destroy_shared_bytes";
    _fn_openpit_shared_bytes_view = (OpenPitBytesView (*)(const OpenPitSharedBytes *))openpit_dlsym(handle, "openpit_shared_bytes_view");
    if (_fn_openpit_shared_bytes_view == NULL) return "openpit_shared_bytes_view";
    _fn_openpit_create_marketdata_quote = (OpenPitMarketDataQuote (*)(void))openpit_dlsym(handle, "openpit_create_marketdata_quote");
    if (_fn_openpit_create_marketdata_quote == NULL) return "openpit_create_marketdata_quote";
    _fn_openpit_create_marketdata_quote_ttl_infinite = (OpenPitMarketDataQuoteTtl (*)(void))openpit_dlsym(handle, "openpit_create_marketdata_quote_ttl_infinite");
    if (_fn_openpit_create_marketdata_quote_ttl_infinite == NULL) return "openpit_create_marketdata_quote_ttl_infinite";
    _fn_openpit_create_marketdata_quote_ttl_within = (OpenPitMarketDataQuoteTtl (*)(uint64_t, uint32_t))openpit_dlsym(handle, "openpit_create_marketdata_quote_ttl_within");
    if (_fn_openpit_create_marketdata_quote_ttl_within == NULL) return "openpit_create_marketdata_quote_ttl_within";
    _fn_openpit_create_marketdata_service = (OpenPitMarketDataService * (*)(uint8_t, OpenPitMarketDataQuoteTtl, OpenPitOutError))openpit_dlsym(handle, "openpit_create_marketdata_service");
    if (_fn_openpit_create_marketdata_service == NULL) return "openpit_create_marketdata_service";
    _fn_openpit_destroy_marketdata_service = (void (*)(OpenPitMarketDataService *))openpit_dlsym(handle, "openpit_destroy_marketdata_service");
    if (_fn_openpit_destroy_marketdata_service == NULL) return "openpit_destroy_marketdata_service";
    _fn_openpit_marketdata_service_clone = (OpenPitMarketDataService * (*)(const OpenPitMarketDataService *))openpit_dlsym(handle, "openpit_marketdata_service_clone");
    if (_fn_openpit_marketdata_service_clone == NULL) return "openpit_marketdata_service_clone";
    _fn_openpit_marketdata_service_register = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_register");
    if (_fn_openpit_marketdata_service_register == NULL) return "openpit_marketdata_service_register";
    _fn_openpit_marketdata_service_register_with_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataQuoteTtl, OpenPitMarketDataInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_register_with_ttl");
    if (_fn_openpit_marketdata_service_register_with_ttl == NULL) return "openpit_marketdata_service_register_with_ttl";
    _fn_openpit_marketdata_service_register_with_id = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId, OpenPitMarketDataInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_register_with_id");
    if (_fn_openpit_marketdata_service_register_with_id == NULL) return "openpit_marketdata_service_register_with_id";
    _fn_openpit_marketdata_service_register_with_id_and_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuoteTtl, OpenPitMarketDataInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_register_with_id_and_ttl");
    if (_fn_openpit_marketdata_service_register_with_id_and_ttl == NULL) return "openpit_marketdata_service_register_with_id_and_ttl";
    _fn_openpit_marketdata_service_set_account_ttl = (void (*)(const OpenPitMarketDataService *, OpenPitParamAccountId, OpenPitMarketDataQuoteTtl))openpit_dlsym(handle, "openpit_marketdata_service_set_account_ttl");
    if (_fn_openpit_marketdata_service_set_account_ttl == NULL) return "openpit_marketdata_service_set_account_ttl";
    _fn_openpit_marketdata_service_clear_account_ttl = (void (*)(const OpenPitMarketDataService *, OpenPitParamAccountId))openpit_dlsym(handle, "openpit_marketdata_service_clear_account_ttl");
    if (_fn_openpit_marketdata_service_clear_account_ttl == NULL) return "openpit_marketdata_service_clear_account_ttl";
    _fn_openpit_marketdata_service_set_account_group_ttl = (void (*)(const OpenPitMarketDataService *, OpenPitParamAccountGroupId, OpenPitMarketDataQuoteTtl))openpit_dlsym(handle, "openpit_marketdata_service_set_account_group_ttl");
    if (_fn_openpit_marketdata_service_set_account_group_ttl == NULL) return "openpit_marketdata_service_set_account_group_ttl";
    _fn_openpit_marketdata_service_clear_account_group_ttl = (void (*)(const OpenPitMarketDataService *, OpenPitParamAccountGroupId))openpit_dlsym(handle, "openpit_marketdata_service_clear_account_group_ttl");
    if (_fn_openpit_marketdata_service_clear_account_group_ttl == NULL) return "openpit_marketdata_service_clear_account_group_ttl";
    _fn_openpit_marketdata_service_set_instrument_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuoteTtl))openpit_dlsym(handle, "openpit_marketdata_service_set_instrument_ttl");
    if (_fn_openpit_marketdata_service_set_instrument_ttl == NULL) return "openpit_marketdata_service_set_instrument_ttl";
    _fn_openpit_marketdata_service_clear_instrument_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId))openpit_dlsym(handle, "openpit_marketdata_service_clear_instrument_ttl");
    if (_fn_openpit_marketdata_service_clear_instrument_ttl == NULL) return "openpit_marketdata_service_clear_instrument_ttl";
    _fn_openpit_marketdata_service_set_instrument_account_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountId, OpenPitMarketDataQuoteTtl))openpit_dlsym(handle, "openpit_marketdata_service_set_instrument_account_ttl");
    if (_fn_openpit_marketdata_service_set_instrument_account_ttl == NULL) return "openpit_marketdata_service_set_instrument_account_ttl";
    _fn_openpit_marketdata_service_clear_instrument_account_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountId))openpit_dlsym(handle, "openpit_marketdata_service_clear_instrument_account_ttl");
    if (_fn_openpit_marketdata_service_clear_instrument_account_ttl == NULL) return "openpit_marketdata_service_clear_instrument_account_ttl";
    _fn_openpit_marketdata_service_set_instrument_account_group_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountGroupId, OpenPitMarketDataQuoteTtl))openpit_dlsym(handle, "openpit_marketdata_service_set_instrument_account_group_ttl");
    if (_fn_openpit_marketdata_service_set_instrument_account_group_ttl == NULL) return "openpit_marketdata_service_set_instrument_account_group_ttl";
    _fn_openpit_marketdata_service_clear_instrument_account_group_ttl = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountGroupId))openpit_dlsym(handle, "openpit_marketdata_service_clear_instrument_account_group_ttl");
    if (_fn_openpit_marketdata_service_clear_instrument_account_group_ttl == NULL) return "openpit_marketdata_service_clear_instrument_account_group_ttl";
    _fn_openpit_marketdata_service_clear = (void (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId))openpit_dlsym(handle, "openpit_marketdata_service_clear");
    if (_fn_openpit_marketdata_service_clear == NULL) return "openpit_marketdata_service_clear";
    _fn_openpit_marketdata_service_push = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_push");
    if (_fn_openpit_marketdata_service_push == NULL) return "openpit_marketdata_service_push";
    _fn_openpit_marketdata_service_push_patch = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_push_patch");
    if (_fn_openpit_marketdata_service_push_patch == NULL) return "openpit_marketdata_service_push_patch";
    _fn_openpit_marketdata_service_push_for = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, const OpenPitParamAccountId *, size_t, const OpenPitParamAccountGroupId *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_push_for");
    if (_fn_openpit_marketdata_service_push_for == NULL) return "openpit_marketdata_service_push_for";
    _fn_openpit_marketdata_service_push_for_patch = (OpenPitMarketDataRegisterStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitMarketDataQuote, const OpenPitParamAccountId *, size_t, const OpenPitParamAccountGroupId *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_push_for_patch");
    if (_fn_openpit_marketdata_service_push_for_patch == NULL) return "openpit_marketdata_service_push_for_patch";
    _fn_openpit_marketdata_service_push_by_instrument = (bool (*)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataQuote, OpenPitMarketDataInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_push_by_instrument");
    if (_fn_openpit_marketdata_service_push_by_instrument == NULL) return "openpit_marketdata_service_push_by_instrument";
    _fn_openpit_marketdata_service_push_by_instrument_patch = (bool (*)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataQuote, OpenPitMarketDataInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_marketdata_service_push_by_instrument_patch");
    if (_fn_openpit_marketdata_service_push_by_instrument_patch == NULL) return "openpit_marketdata_service_push_by_instrument_patch";
    _fn_openpit_marketdata_service_get = (OpenPitMarketDataGetStatus (*)(const OpenPitMarketDataService *, OpenPitMarketDataInstrumentId, OpenPitParamAccountId, OpenPitMarketDataAccountGroupResolver, void *, OpenPitMarketDataQuoteResolution, OpenPitMarketDataQuote *))openpit_dlsym(handle, "openpit_marketdata_service_get");
    if (_fn_openpit_marketdata_service_get == NULL) return "openpit_marketdata_service_get";
    _fn_openpit_marketdata_service_resolve = (bool (*)(const OpenPitMarketDataService *, const OpenPitInstrument *, OpenPitMarketDataInstrumentId *))openpit_dlsym(handle, "openpit_marketdata_service_resolve");
    if (_fn_openpit_marketdata_service_resolve == NULL) return "openpit_marketdata_service_resolve";
    _fn_openpit_create_pretrade_pre_trade_lock = (OpenPitPretradePreTradeLock * (*)(void))openpit_dlsym(handle, "openpit_create_pretrade_pre_trade_lock");
    if (_fn_openpit_create_pretrade_pre_trade_lock == NULL) return "openpit_create_pretrade_pre_trade_lock";
    _fn_openpit_destroy_pretrade_pre_trade_lock = (void (*)(OpenPitPretradePreTradeLock *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_lock");
    if (_fn_openpit_destroy_pretrade_pre_trade_lock == NULL) return "openpit_destroy_pretrade_pre_trade_lock";
    _fn_openpit_pretrade_pre_trade_lock_clone = (OpenPitPretradePreTradeLock * (*)(const OpenPitPretradePreTradeLock *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_clone");
    if (_fn_openpit_pretrade_pre_trade_lock_clone == NULL) return "openpit_pretrade_pre_trade_lock_clone";
    _fn_openpit_pretrade_pre_trade_lock_len = (size_t (*)(const OpenPitPretradePreTradeLock *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_len");
    if (_fn_openpit_pretrade_pre_trade_lock_len == NULL) return "openpit_pretrade_pre_trade_lock_len";
    _fn_openpit_pretrade_pre_trade_lock_is_empty = (bool (*)(const OpenPitPretradePreTradeLock *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_is_empty");
    if (_fn_openpit_pretrade_pre_trade_lock_is_empty == NULL) return "openpit_pretrade_pre_trade_lock_is_empty";
    _fn_openpit_pretrade_pre_trade_lock_push = (bool (*)(OpenPitPretradePreTradeLock *, uint16_t, OpenPitParamPrice, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_push");
    if (_fn_openpit_pretrade_pre_trade_lock_push == NULL) return "openpit_pretrade_pre_trade_lock_push";
    _fn_openpit_pretrade_pre_trade_lock_push_many = (bool (*)(OpenPitPretradePreTradeLock *, const OpenPitPretradePreTradeLockEntry *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_push_many");
    if (_fn_openpit_pretrade_pre_trade_lock_push_many == NULL) return "openpit_pretrade_pre_trade_lock_push_many";
    _fn_openpit_create_pretrade_pre_trade_lock_from_entries = (OpenPitPretradePreTradeLock * (*)(const OpenPitPretradePreTradeLockEntry *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_pre_trade_lock_from_entries");
    if (_fn_openpit_create_pretrade_pre_trade_lock_from_entries == NULL) return "openpit_create_pretrade_pre_trade_lock_from_entries";
    _fn_openpit_pretrade_pre_trade_lock_merge = (bool (*)(OpenPitPretradePreTradeLock *, const OpenPitPretradePreTradeLock *, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_merge");
    if (_fn_openpit_pretrade_pre_trade_lock_merge == NULL) return "openpit_pretrade_pre_trade_lock_merge";
    _fn_openpit_destroy_pretrade_pre_trade_lock_prices = (void (*)(OpenPitPretradePreTradeLockPrices *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_lock_prices");
    if (_fn_openpit_destroy_pretrade_pre_trade_lock_prices == NULL) return "openpit_destroy_pretrade_pre_trade_lock_prices";
    _fn_openpit_pretrade_pre_trade_lock_prices_view = (OpenPitPretradePreTradeLockPricesView (*)(const OpenPitPretradePreTradeLockPrices *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_prices_view");
    if (_fn_openpit_pretrade_pre_trade_lock_prices_view == NULL) return "openpit_pretrade_pre_trade_lock_prices_view";
    _fn_openpit_pretrade_pre_trade_lock_prices_of = (OpenPitPretradePreTradeLockPricesStatus (*)(const OpenPitPretradePreTradeLock *, uint16_t, OpenPitParamPrice *, OpenPitPretradePreTradeLockPrices **, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_prices_of");
    if (_fn_openpit_pretrade_pre_trade_lock_prices_of == NULL) return "openpit_pretrade_pre_trade_lock_prices_of";
    _fn_openpit_pretrade_pre_trade_lock_entries = (OpenPitPretradePreTradeLockEntries * (*)(const OpenPitPretradePreTradeLock *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_entries");
    if (_fn_openpit_pretrade_pre_trade_lock_entries == NULL) return "openpit_pretrade_pre_trade_lock_entries";
    _fn_openpit_destroy_pretrade_pre_trade_lock_entries = (void (*)(OpenPitPretradePreTradeLockEntries *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_lock_entries");
    if (_fn_openpit_destroy_pretrade_pre_trade_lock_entries == NULL) return "openpit_destroy_pretrade_pre_trade_lock_entries";
    _fn_openpit_pretrade_pre_trade_lock_entries_view = (OpenPitPretradePreTradeLockEntriesView (*)(const OpenPitPretradePreTradeLockEntries *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_entries_view");
    if (_fn_openpit_pretrade_pre_trade_lock_entries_view == NULL) return "openpit_pretrade_pre_trade_lock_entries_view";
    _fn_openpit_pretrade_pre_trade_lock_to_msgpack = (OpenPitSharedBytes * (*)(const OpenPitPretradePreTradeLock *, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_to_msgpack");
    if (_fn_openpit_pretrade_pre_trade_lock_to_msgpack == NULL) return "openpit_pretrade_pre_trade_lock_to_msgpack";
    _fn_openpit_create_pretrade_pre_trade_lock_from_msgpack = (OpenPitPretradePreTradeLock * (*)(const uint8_t *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_pre_trade_lock_from_msgpack");
    if (_fn_openpit_create_pretrade_pre_trade_lock_from_msgpack == NULL) return "openpit_create_pretrade_pre_trade_lock_from_msgpack";
    _fn_openpit_pretrade_pre_trade_lock_to_json = (OpenPitSharedString * (*)(const OpenPitPretradePreTradeLock *, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_to_json");
    if (_fn_openpit_pretrade_pre_trade_lock_to_json == NULL) return "openpit_pretrade_pre_trade_lock_to_json";
    _fn_openpit_create_pretrade_pre_trade_lock_from_json = (OpenPitPretradePreTradeLock * (*)(const uint8_t *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_pre_trade_lock_from_json");
    if (_fn_openpit_create_pretrade_pre_trade_lock_from_json == NULL) return "openpit_create_pretrade_pre_trade_lock_from_json";
    _fn_openpit_pretrade_pre_trade_lock_to_cbor = (OpenPitSharedBytes * (*)(const OpenPitPretradePreTradeLock *, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_to_cbor");
    if (_fn_openpit_pretrade_pre_trade_lock_to_cbor == NULL) return "openpit_pretrade_pre_trade_lock_to_cbor";
    _fn_openpit_create_pretrade_pre_trade_lock_from_cbor = (OpenPitPretradePreTradeLock * (*)(const uint8_t *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_pre_trade_lock_from_cbor");
    if (_fn_openpit_create_pretrade_pre_trade_lock_from_cbor == NULL) return "openpit_create_pretrade_pre_trade_lock_from_cbor";
    _fn_openpit_pretrade_pre_trade_lock_to_raw = (OpenPitSharedBytes * (*)(const OpenPitPretradePreTradeLock *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_lock_to_raw");
    if (_fn_openpit_pretrade_pre_trade_lock_to_raw == NULL) return "openpit_pretrade_pre_trade_lock_to_raw";
    _fn_openpit_create_pretrade_pre_trade_lock_from_raw = (OpenPitPretradePreTradeLock * (*)(const uint8_t *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_pre_trade_lock_from_raw");
    if (_fn_openpit_create_pretrade_pre_trade_lock_from_raw == NULL) return "openpit_create_pretrade_pre_trade_lock_from_raw";
    _fn_openpit_create_reference_book = (OpenPitReferenceBook * (*)(void))openpit_dlsym(handle, "openpit_create_reference_book");
    if (_fn_openpit_create_reference_book == NULL) return "openpit_create_reference_book";
    _fn_openpit_destroy_reference_book = (void (*)(OpenPitReferenceBook *))openpit_dlsym(handle, "openpit_destroy_reference_book");
    if (_fn_openpit_destroy_reference_book == NULL) return "openpit_destroy_reference_book";
    _fn_openpit_reference_book_register = (OpenPitReferenceBookRegisterStatus (*)(OpenPitReferenceBook *, const OpenPitInstrument *, OpenPitInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_reference_book_register");
    if (_fn_openpit_reference_book_register == NULL) return "openpit_reference_book_register";
    _fn_openpit_reference_book_register_with_id = (OpenPitReferenceBookRegisterStatus (*)(OpenPitReferenceBook *, const OpenPitInstrument *, OpenPitInstrumentId, OpenPitInstrumentId *, OpenPitOutError))openpit_dlsym(handle, "openpit_reference_book_register_with_id");
    if (_fn_openpit_reference_book_register_with_id == NULL) return "openpit_reference_book_register_with_id";
    _fn_openpit_reference_book_resolve = (bool (*)(const OpenPitReferenceBook *, const OpenPitInstrument *, OpenPitInstrumentId *))openpit_dlsym(handle, "openpit_reference_book_resolve");
    if (_fn_openpit_reference_book_resolve == NULL) return "openpit_reference_book_resolve";
    _fn_openpit_reference_book_set_settlement_scheme = (OpenPitReferenceBookStatus (*)(OpenPitReferenceBook *, OpenPitInstrumentId, OpenPitSettlementScheme, OpenPitOutError))openpit_dlsym(handle, "openpit_reference_book_set_settlement_scheme");
    if (_fn_openpit_reference_book_set_settlement_scheme == NULL) return "openpit_reference_book_set_settlement_scheme";
    _fn_openpit_reference_book_clear_settlement_scheme = (OpenPitReferenceBookStatus (*)(OpenPitReferenceBook *, OpenPitInstrumentId, OpenPitOutError))openpit_dlsym(handle, "openpit_reference_book_clear_settlement_scheme");
    if (_fn_openpit_reference_book_clear_settlement_scheme == NULL) return "openpit_reference_book_clear_settlement_scheme";
    _fn_openpit_reference_book_get_settlement_scheme = (OpenPitReferenceBookStatus (*)(const OpenPitReferenceBook *, OpenPitInstrumentId, OpenPitSettlementScheme *, bool *, OpenPitOutError))openpit_dlsym(handle, "openpit_reference_book_get_settlement_scheme");
    if (_fn_openpit_reference_book_get_settlement_scheme == NULL) return "openpit_reference_book_get_settlement_scheme";
    _fn_openpit_destroy_shared_string = (void (*)(OpenPitSharedString *))openpit_dlsym(handle, "openpit_destroy_shared_string");
    if (_fn_openpit_destroy_shared_string == NULL) return "openpit_destroy_shared_string";
    _fn_openpit_shared_string_view = (OpenPitStringView (*)(const OpenPitSharedString *))openpit_dlsym(handle, "openpit_shared_string_view");
    if (_fn_openpit_shared_string_view == NULL) return "openpit_shared_string_view";
    return NULL;
}

bool openpit_create_param_pnl(OpenPitParamDecimal value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl(value, out, out_error);
}

OpenPitParamDecimal openpit_param_pnl_get_decimal(OpenPitParamPnl value) {
    return _fn_openpit_param_pnl_get_decimal(value);
}

bool openpit_create_param_price(OpenPitParamDecimal value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price(value, out, out_error);
}

OpenPitParamDecimal openpit_param_price_get_decimal(OpenPitParamPrice value) {
    return _fn_openpit_param_price_get_decimal(value);
}

bool openpit_create_param_quantity(OpenPitParamDecimal value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity(value, out, out_error);
}

OpenPitParamDecimal openpit_param_quantity_get_decimal(OpenPitParamQuantity value) {
    return _fn_openpit_param_quantity_get_decimal(value);
}

bool openpit_create_param_volume(OpenPitParamDecimal value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume(value, out, out_error);
}

OpenPitParamDecimal openpit_param_volume_get_decimal(OpenPitParamVolume value) {
    return _fn_openpit_param_volume_get_decimal(value);
}

bool openpit_create_param_cash_flow(OpenPitParamDecimal value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow(value, out, out_error);
}

OpenPitParamDecimal openpit_param_cash_flow_get_decimal(OpenPitParamCashFlow value) {
    return _fn_openpit_param_cash_flow_get_decimal(value);
}

bool openpit_create_param_position_size(OpenPitParamDecimal value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size(value, out, out_error);
}

OpenPitParamDecimal openpit_param_position_size_get_decimal(OpenPitParamPositionSize value) {
    return _fn_openpit_param_position_size_get_decimal(value);
}

bool openpit_create_param_fee(OpenPitParamDecimal value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee(value, out, out_error);
}

OpenPitParamDecimal openpit_param_fee_get_decimal(OpenPitParamFee value) {
    return _fn_openpit_param_fee_get_decimal(value);
}

bool openpit_create_param_notional(OpenPitParamDecimal value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional(value, out, out_error);
}

OpenPitParamDecimal openpit_param_notional_get_decimal(OpenPitParamNotional value) {
    return _fn_openpit_param_notional_get_decimal(value);
}

bool openpit_create_param_pnl_from_string(OpenPitStringView value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_string(value, out, out_error);
}

bool openpit_create_param_pnl_from_f64(double value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_f64(value, out, out_error);
}

bool openpit_create_param_pnl_from_int64(int64_t value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_int64(value, out, out_error);
}

bool openpit_create_param_pnl_from_uint64(uint64_t value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_uint64(value, out, out_error);
}

bool openpit_create_param_pnl_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_pnl_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_pnl_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_pnl_to_f64(OpenPitParamPnl value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_to_f64(value, out, out_error);
}

bool openpit_param_pnl_is_zero(OpenPitParamPnl value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_is_zero(value, out, out_error);
}

bool openpit_param_pnl_compare(OpenPitParamPnl lhs, OpenPitParamPnl rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_pnl_to_string(OpenPitParamPnl value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_to_string(value, out_error);
}

bool openpit_param_pnl_checked_add(OpenPitParamPnl lhs, OpenPitParamPnl rhs, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_pnl_checked_sub(OpenPitParamPnl lhs, OpenPitParamPnl rhs, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_pnl_checked_mul_i64(OpenPitParamPnl value, int64_t multiplier, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_pnl_checked_mul_u64(OpenPitParamPnl value, uint64_t multiplier, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_pnl_checked_mul_f64(OpenPitParamPnl value, double multiplier, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_pnl_checked_div_i64(OpenPitParamPnl value, int64_t divisor, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_pnl_checked_div_u64(OpenPitParamPnl value, uint64_t divisor, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_pnl_checked_div_f64(OpenPitParamPnl value, double divisor, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_pnl_checked_rem_i64(OpenPitParamPnl value, int64_t divisor, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_pnl_checked_rem_u64(OpenPitParamPnl value, uint64_t divisor, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_pnl_checked_rem_f64(OpenPitParamPnl value, double divisor, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_param_pnl_checked_neg(OpenPitParamPnl value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_checked_neg(value, out, out_error);
}

bool openpit_create_param_price_from_string(OpenPitStringView value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_string(value, out, out_error);
}

bool openpit_create_param_price_from_f64(double value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_f64(value, out, out_error);
}

bool openpit_create_param_price_from_int64(int64_t value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_int64(value, out, out_error);
}

bool openpit_create_param_price_from_uint64(uint64_t value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_uint64(value, out, out_error);
}

bool openpit_create_param_price_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_price_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_price_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_price_to_f64(OpenPitParamPrice value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_to_f64(value, out, out_error);
}

bool openpit_param_price_is_zero(OpenPitParamPrice value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_is_zero(value, out, out_error);
}

bool openpit_param_price_compare(OpenPitParamPrice lhs, OpenPitParamPrice rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_price_to_string(OpenPitParamPrice value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_to_string(value, out_error);
}

bool openpit_param_price_checked_add(OpenPitParamPrice lhs, OpenPitParamPrice rhs, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_price_checked_sub(OpenPitParamPrice lhs, OpenPitParamPrice rhs, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_price_checked_mul_i64(OpenPitParamPrice value, int64_t multiplier, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_price_checked_mul_u64(OpenPitParamPrice value, uint64_t multiplier, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_price_checked_mul_f64(OpenPitParamPrice value, double multiplier, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_price_checked_div_i64(OpenPitParamPrice value, int64_t divisor, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_price_checked_div_u64(OpenPitParamPrice value, uint64_t divisor, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_price_checked_div_f64(OpenPitParamPrice value, double divisor, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_price_checked_rem_i64(OpenPitParamPrice value, int64_t divisor, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_price_checked_rem_u64(OpenPitParamPrice value, uint64_t divisor, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_price_checked_rem_f64(OpenPitParamPrice value, double divisor, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_param_price_checked_neg(OpenPitParamPrice value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_checked_neg(value, out, out_error);
}

bool openpit_create_param_quantity_from_string(OpenPitStringView value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_string(value, out, out_error);
}

bool openpit_create_param_quantity_from_f64(double value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_f64(value, out, out_error);
}

bool openpit_create_param_quantity_from_int64(int64_t value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_int64(value, out, out_error);
}

bool openpit_create_param_quantity_from_uint64(uint64_t value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_uint64(value, out, out_error);
}

bool openpit_create_param_quantity_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_quantity_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_quantity_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_quantity_to_f64(OpenPitParamQuantity value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_to_f64(value, out, out_error);
}

bool openpit_param_quantity_is_zero(OpenPitParamQuantity value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_is_zero(value, out, out_error);
}

bool openpit_param_quantity_compare(OpenPitParamQuantity lhs, OpenPitParamQuantity rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_quantity_to_string(OpenPitParamQuantity value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_to_string(value, out_error);
}

bool openpit_param_quantity_checked_add(OpenPitParamQuantity lhs, OpenPitParamQuantity rhs, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_quantity_checked_sub(OpenPitParamQuantity lhs, OpenPitParamQuantity rhs, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_quantity_checked_mul_i64(OpenPitParamQuantity value, int64_t multiplier, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_quantity_checked_mul_u64(OpenPitParamQuantity value, uint64_t multiplier, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_quantity_checked_mul_f64(OpenPitParamQuantity value, double multiplier, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_quantity_checked_div_i64(OpenPitParamQuantity value, int64_t divisor, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_quantity_checked_div_u64(OpenPitParamQuantity value, uint64_t divisor, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_quantity_checked_div_f64(OpenPitParamQuantity value, double divisor, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_quantity_checked_rem_i64(OpenPitParamQuantity value, int64_t divisor, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_quantity_checked_rem_u64(OpenPitParamQuantity value, uint64_t divisor, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_quantity_checked_rem_f64(OpenPitParamQuantity value, double divisor, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_create_param_volume_from_string(OpenPitStringView value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_string(value, out, out_error);
}

bool openpit_create_param_volume_from_f64(double value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_f64(value, out, out_error);
}

bool openpit_create_param_volume_from_int64(int64_t value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_int64(value, out, out_error);
}

bool openpit_create_param_volume_from_uint64(uint64_t value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_uint64(value, out, out_error);
}

bool openpit_create_param_volume_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_volume_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_volume_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_volume_to_f64(OpenPitParamVolume value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_to_f64(value, out, out_error);
}

bool openpit_param_volume_is_zero(OpenPitParamVolume value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_is_zero(value, out, out_error);
}

bool openpit_param_volume_compare(OpenPitParamVolume lhs, OpenPitParamVolume rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_volume_to_string(OpenPitParamVolume value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_to_string(value, out_error);
}

bool openpit_param_volume_checked_add(OpenPitParamVolume lhs, OpenPitParamVolume rhs, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_volume_checked_sub(OpenPitParamVolume lhs, OpenPitParamVolume rhs, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_volume_checked_mul_i64(OpenPitParamVolume value, int64_t multiplier, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_volume_checked_mul_u64(OpenPitParamVolume value, uint64_t multiplier, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_volume_checked_mul_f64(OpenPitParamVolume value, double multiplier, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_volume_checked_div_i64(OpenPitParamVolume value, int64_t divisor, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_volume_checked_div_u64(OpenPitParamVolume value, uint64_t divisor, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_volume_checked_div_f64(OpenPitParamVolume value, double divisor, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_volume_checked_rem_i64(OpenPitParamVolume value, int64_t divisor, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_volume_checked_rem_u64(OpenPitParamVolume value, uint64_t divisor, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_volume_checked_rem_f64(OpenPitParamVolume value, double divisor, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_create_param_cash_flow_from_string(OpenPitStringView value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_string(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_f64(double value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_f64(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_int64(int64_t value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_int64(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_uint64(uint64_t value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_uint64(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_cash_flow_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_cash_flow_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_cash_flow_to_f64(OpenPitParamCashFlow value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_to_f64(value, out, out_error);
}

bool openpit_param_cash_flow_is_zero(OpenPitParamCashFlow value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_is_zero(value, out, out_error);
}

bool openpit_param_cash_flow_compare(OpenPitParamCashFlow lhs, OpenPitParamCashFlow rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_cash_flow_to_string(OpenPitParamCashFlow value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_to_string(value, out_error);
}

bool openpit_param_cash_flow_checked_add(OpenPitParamCashFlow lhs, OpenPitParamCashFlow rhs, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_cash_flow_checked_sub(OpenPitParamCashFlow lhs, OpenPitParamCashFlow rhs, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_cash_flow_checked_mul_i64(OpenPitParamCashFlow value, int64_t multiplier, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_cash_flow_checked_mul_u64(OpenPitParamCashFlow value, uint64_t multiplier, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_cash_flow_checked_mul_f64(OpenPitParamCashFlow value, double multiplier, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_cash_flow_checked_div_i64(OpenPitParamCashFlow value, int64_t divisor, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_cash_flow_checked_div_u64(OpenPitParamCashFlow value, uint64_t divisor, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_cash_flow_checked_div_f64(OpenPitParamCashFlow value, double divisor, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_cash_flow_checked_rem_i64(OpenPitParamCashFlow value, int64_t divisor, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_cash_flow_checked_rem_u64(OpenPitParamCashFlow value, uint64_t divisor, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_cash_flow_checked_rem_f64(OpenPitParamCashFlow value, double divisor, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_param_cash_flow_checked_neg(OpenPitParamCashFlow value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_checked_neg(value, out, out_error);
}

bool openpit_create_param_position_size_from_string(OpenPitStringView value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_string(value, out, out_error);
}

bool openpit_create_param_position_size_from_f64(double value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_f64(value, out, out_error);
}

bool openpit_create_param_position_size_from_int64(int64_t value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_int64(value, out, out_error);
}

bool openpit_create_param_position_size_from_uint64(uint64_t value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_uint64(value, out, out_error);
}

bool openpit_create_param_position_size_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_position_size_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_position_size_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_position_size_to_f64(OpenPitParamPositionSize value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_to_f64(value, out, out_error);
}

bool openpit_param_position_size_is_zero(OpenPitParamPositionSize value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_is_zero(value, out, out_error);
}

bool openpit_param_position_size_compare(OpenPitParamPositionSize lhs, OpenPitParamPositionSize rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_position_size_to_string(OpenPitParamPositionSize value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_to_string(value, out_error);
}

bool openpit_param_position_size_checked_add(OpenPitParamPositionSize lhs, OpenPitParamPositionSize rhs, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_position_size_checked_sub(OpenPitParamPositionSize lhs, OpenPitParamPositionSize rhs, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_position_size_checked_mul_i64(OpenPitParamPositionSize value, int64_t multiplier, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_position_size_checked_mul_u64(OpenPitParamPositionSize value, uint64_t multiplier, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_position_size_checked_mul_f64(OpenPitParamPositionSize value, double multiplier, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_position_size_checked_div_i64(OpenPitParamPositionSize value, int64_t divisor, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_position_size_checked_div_u64(OpenPitParamPositionSize value, uint64_t divisor, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_position_size_checked_div_f64(OpenPitParamPositionSize value, double divisor, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_position_size_checked_rem_i64(OpenPitParamPositionSize value, int64_t divisor, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_position_size_checked_rem_u64(OpenPitParamPositionSize value, uint64_t divisor, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_position_size_checked_rem_f64(OpenPitParamPositionSize value, double divisor, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_param_position_size_checked_neg(OpenPitParamPositionSize value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_neg(value, out, out_error);
}

bool openpit_create_param_fee_from_string(OpenPitStringView value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_string(value, out, out_error);
}

bool openpit_create_param_fee_from_f64(double value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_f64(value, out, out_error);
}

bool openpit_create_param_fee_from_int64(int64_t value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_int64(value, out, out_error);
}

bool openpit_create_param_fee_from_uint64(uint64_t value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_uint64(value, out, out_error);
}

bool openpit_create_param_fee_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_fee_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_fee_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_fee_to_f64(OpenPitParamFee value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_to_f64(value, out, out_error);
}

bool openpit_param_fee_is_zero(OpenPitParamFee value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_is_zero(value, out, out_error);
}

bool openpit_param_fee_compare(OpenPitParamFee lhs, OpenPitParamFee rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_fee_to_string(OpenPitParamFee value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_to_string(value, out_error);
}

bool openpit_param_fee_checked_add(OpenPitParamFee lhs, OpenPitParamFee rhs, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_fee_checked_sub(OpenPitParamFee lhs, OpenPitParamFee rhs, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_fee_checked_mul_i64(OpenPitParamFee value, int64_t multiplier, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_fee_checked_mul_u64(OpenPitParamFee value, uint64_t multiplier, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_fee_checked_mul_f64(OpenPitParamFee value, double multiplier, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_fee_checked_div_i64(OpenPitParamFee value, int64_t divisor, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_fee_checked_div_u64(OpenPitParamFee value, uint64_t divisor, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_fee_checked_div_f64(OpenPitParamFee value, double divisor, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_fee_checked_rem_i64(OpenPitParamFee value, int64_t divisor, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_fee_checked_rem_u64(OpenPitParamFee value, uint64_t divisor, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_fee_checked_rem_f64(OpenPitParamFee value, double divisor, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_param_fee_checked_neg(OpenPitParamFee value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_checked_neg(value, out, out_error);
}

bool openpit_create_param_notional_from_string(OpenPitStringView value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_string(value, out, out_error);
}

bool openpit_create_param_notional_from_f64(double value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_f64(value, out, out_error);
}

bool openpit_create_param_notional_from_int64(int64_t value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_int64(value, out, out_error);
}

bool openpit_create_param_notional_from_uint64(uint64_t value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_uint64(value, out, out_error);
}

bool openpit_create_param_notional_from_string_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_string_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_notional_from_f64_rounded(double value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_f64_rounded(value, scale, rounding, out, out_error);
}

bool openpit_create_param_notional_from_decimal_rounded(OpenPitParamDecimal value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_decimal_rounded(value, scale, rounding, out, out_error);
}

bool openpit_param_notional_to_f64(OpenPitParamNotional value, double * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_to_f64(value, out, out_error);
}

bool openpit_param_notional_is_zero(OpenPitParamNotional value, bool * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_is_zero(value, out, out_error);
}

bool openpit_param_notional_compare(OpenPitParamNotional lhs, OpenPitParamNotional rhs, int8_t * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_compare(lhs, rhs, out, out_error);
}

OpenPitSharedString * openpit_param_notional_to_string(OpenPitParamNotional value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_to_string(value, out_error);
}

bool openpit_param_notional_checked_add(OpenPitParamNotional lhs, OpenPitParamNotional rhs, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_add(lhs, rhs, out, out_error);
}

bool openpit_param_notional_checked_sub(OpenPitParamNotional lhs, OpenPitParamNotional rhs, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_sub(lhs, rhs, out, out_error);
}

bool openpit_param_notional_checked_mul_i64(OpenPitParamNotional value, int64_t multiplier, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_mul_i64(value, multiplier, out, out_error);
}

bool openpit_param_notional_checked_mul_u64(OpenPitParamNotional value, uint64_t multiplier, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_mul_u64(value, multiplier, out, out_error);
}

bool openpit_param_notional_checked_mul_f64(OpenPitParamNotional value, double multiplier, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_mul_f64(value, multiplier, out, out_error);
}

bool openpit_param_notional_checked_div_i64(OpenPitParamNotional value, int64_t divisor, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_div_i64(value, divisor, out, out_error);
}

bool openpit_param_notional_checked_div_u64(OpenPitParamNotional value, uint64_t divisor, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_div_u64(value, divisor, out, out_error);
}

bool openpit_param_notional_checked_div_f64(OpenPitParamNotional value, double divisor, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_div_f64(value, divisor, out, out_error);
}

bool openpit_param_notional_checked_rem_i64(OpenPitParamNotional value, int64_t divisor, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_rem_i64(value, divisor, out, out_error);
}

bool openpit_param_notional_checked_rem_u64(OpenPitParamNotional value, uint64_t divisor, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_rem_u64(value, divisor, out, out_error);
}

bool openpit_param_notional_checked_rem_f64(OpenPitParamNotional value, double divisor, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_checked_rem_f64(value, divisor, out, out_error);
}

bool openpit_param_leverage_calculate_margin_required(OpenPitParamLeverage leverage, OpenPitParamNotional notional, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_leverage_calculate_margin_required(leverage, notional, out, out_error);
}

bool openpit_param_price_calculate_volume(OpenPitParamPrice price, OpenPitParamQuantity quantity, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_calculate_volume(price, quantity, out, out_error);
}

bool openpit_param_price_calculate_position_size(OpenPitParamPrice price, OpenPitParamQuantity quantity, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_calculate_position_size(price, quantity, out, out_error);
}

bool openpit_param_quantity_calculate_volume(OpenPitParamQuantity quantity, OpenPitParamPrice price, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_calculate_volume(quantity, price, out, out_error);
}

bool openpit_param_volume_calculate_quantity(OpenPitParamVolume volume, OpenPitParamPrice price, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_calculate_quantity(volume, price, out, out_error);
}

bool openpit_param_pnl_to_cash_flow(OpenPitParamPnl value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_to_cash_flow(value, out, out_error);
}

bool openpit_param_pnl_to_position_size(OpenPitParamPnl value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_to_position_size(value, out, out_error);
}

bool openpit_param_quantity_to_position_size(OpenPitParamQuantity value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_to_position_size(value, out, out_error);
}

bool openpit_param_volume_to_position_size(OpenPitParamVolume value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_to_position_size(value, out, out_error);
}

bool openpit_param_pnl_from_fee(OpenPitParamFee fee, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_pnl_from_fee(fee, out, out_error);
}

bool openpit_param_cash_flow_from_pnl(OpenPitParamPnl pnl, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_from_pnl(pnl, out, out_error);
}

bool openpit_param_cash_flow_from_fee(OpenPitParamFee fee, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_from_fee(fee, out, out_error);
}

bool openpit_param_cash_flow_from_volume_inflow(OpenPitParamVolume volume, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_from_volume_inflow(volume, out, out_error);
}

bool openpit_param_cash_flow_from_volume_outflow(OpenPitParamVolume volume, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_cash_flow_from_volume_outflow(volume, out, out_error);
}

bool openpit_param_fee_to_pnl(OpenPitParamFee fee, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_to_pnl(fee, out, out_error);
}

bool openpit_param_fee_to_position_size(OpenPitParamFee fee, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_to_position_size(fee, out, out_error);
}

bool openpit_param_fee_to_cash_flow(OpenPitParamFee fee, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_fee_to_cash_flow(fee, out, out_error);
}

bool openpit_param_volume_to_cash_flow_inflow(OpenPitParamVolume volume, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_to_cash_flow_inflow(volume, out, out_error);
}

bool openpit_param_volume_to_cash_flow_outflow(OpenPitParamVolume volume, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_to_cash_flow_outflow(volume, out, out_error);
}

bool openpit_param_position_size_from_pnl(OpenPitParamPnl pnl, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_from_pnl(pnl, out, out_error);
}

bool openpit_param_position_size_from_fee(OpenPitParamFee fee, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_from_fee(fee, out, out_error);
}

bool openpit_param_position_size_from_quantity_and_side(OpenPitParamQuantity quantity, OpenPitParamSide side, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_from_quantity_and_side(quantity, side, out, out_error);
}

bool openpit_param_position_size_to_open_quantity(OpenPitParamPositionSize value, OpenPitParamQuantity * out_quantity, OpenPitParamSide * out_side, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_to_open_quantity(value, out_quantity, out_side, out_error);
}

bool openpit_param_position_size_to_close_quantity(OpenPitParamPositionSize value, OpenPitParamQuantity * out_quantity, OpenPitParamSide * out_side, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_to_close_quantity(value, out_quantity, out_side, out_error);
}

bool openpit_param_position_size_checked_add_quantity(OpenPitParamPositionSize value, OpenPitParamQuantity quantity, OpenPitParamSide side, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_size_checked_add_quantity(value, quantity, side, out, out_error);
}

bool openpit_param_price_calculate_notional(OpenPitParamPrice price, OpenPitParamQuantity quantity, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_price_calculate_notional(price, quantity, out, out_error);
}

bool openpit_param_quantity_calculate_notional(OpenPitParamQuantity quantity, OpenPitParamPrice price, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_quantity_calculate_notional(quantity, price, out, out_error);
}

bool openpit_param_notional_from_volume(OpenPitParamVolume volume, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_from_volume(volume, out, out_error);
}

bool openpit_param_notional_to_volume(OpenPitParamNotional notional, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_to_volume(notional, out, out_error);
}

bool openpit_param_notional_calculate_margin_required(OpenPitParamNotional notional, OpenPitParamLeverage leverage, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_notional_calculate_margin_required(notional, leverage, out, out_error);
}

bool openpit_param_volume_from_notional(OpenPitParamNotional notional, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_param_volume_from_notional(notional, out, out_error);
}

OpenPitParamAccountId openpit_create_param_account_id_from_uint64(uint64_t value) {
    return _fn_openpit_create_param_account_id_from_uint64(value);
}

bool openpit_create_param_account_id_from_string(OpenPitStringView value, OpenPitParamAccountId * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_account_id_from_string(value, out, out_error);
}

OpenPitSharedString * openpit_create_param_asset_from_string(OpenPitStringView value, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_asset_from_string(value, out_error);
}

void openpit_destroy_param_asset(OpenPitSharedString * handle) {
    _fn_openpit_destroy_param_asset(handle);
}

OpenPitSharedString * openpit_param_side_to_string(OpenPitParamSide value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_side_to_string(value, out_error);
}

OpenPitSharedString * openpit_param_position_side_to_string(OpenPitParamPositionSide value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_side_to_string(value, out_error);
}

OpenPitSharedString * openpit_param_position_effect_to_string(OpenPitParamPositionEffect value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_effect_to_string(value, out_error);
}

OpenPitSharedString * openpit_param_position_mode_to_string(OpenPitParamPositionMode value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_position_mode_to_string(value, out_error);
}

OpenPitSharedString * openpit_param_account_id_to_string(OpenPitParamAccountId value) {
    return _fn_openpit_param_account_id_to_string(value);
}

OpenPitSharedString * openpit_param_trade_amount_to_string(OpenPitParamTradeAmount value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_trade_amount_to_string(value, out_error);
}

OpenPitSharedString * openpit_param_adjustment_amount_to_string(OpenPitParamAdjustmentAmount value, OpenPitOutParamError out_error) {
    return _fn_openpit_param_adjustment_amount_to_string(value, out_error);
}

OpenPitPretradeRejectList * openpit_pretrade_create_reject_list(size_t reserve) {
    return _fn_openpit_pretrade_create_reject_list(reserve);
}

void openpit_pretrade_destroy_reject_list(OpenPitPretradeRejectList * rejects) {
    _fn_openpit_pretrade_destroy_reject_list(rejects);
}

bool openpit_pretrade_reject_list_push(OpenPitPretradeRejectList * list, OpenPitPretradeReject reject) {
    return _fn_openpit_pretrade_reject_list_push(list, reject);
}

size_t openpit_pretrade_reject_list_len(const OpenPitPretradeRejectList * list) {
    return _fn_openpit_pretrade_reject_list_len(list);
}

bool openpit_pretrade_reject_list_get(const OpenPitPretradeRejectList * list, size_t index, OpenPitPretradeReject * out_reject) {
    return _fn_openpit_pretrade_reject_list_get(list, index, out_reject);
}

OpenPitPretradeAccountBlockList * openpit_pretrade_create_account_block_list(size_t reserve) {
    return _fn_openpit_pretrade_create_account_block_list(reserve);
}

void openpit_pretrade_destroy_account_block_list(OpenPitPretradeAccountBlockList * blocks) {
    _fn_openpit_pretrade_destroy_account_block_list(blocks);
}

void openpit_pretrade_account_block_list_push(OpenPitPretradeAccountBlockList * list, OpenPitPretradeAccountBlock block) {
    _fn_openpit_pretrade_account_block_list_push(list, block);
}

size_t openpit_pretrade_account_block_list_len(const OpenPitPretradeAccountBlockList * list) {
    return _fn_openpit_pretrade_account_block_list_len(list);
}

bool openpit_pretrade_account_block_list_get(const OpenPitPretradeAccountBlockList * list, size_t index, OpenPitPretradeAccountBlock * out_block) {
    return _fn_openpit_pretrade_account_block_list_get(list, index, out_block);
}

void openpit_destroy_param_error(OpenPitParamError * handle) {
    _fn_openpit_destroy_param_error(handle);
}

OpenPitEngineBuilder * openpit_create_engine_builder(uint8_t sync_policy, OpenPitOutError out_error) {
    return _fn_openpit_create_engine_builder(sync_policy, out_error);
}

void openpit_destroy_engine_builder(OpenPitEngineBuilder * builder) {
    _fn_openpit_destroy_engine_builder(builder);
}

OpenPitEngine * openpit_engine_builder_build(OpenPitEngineBuilder * builder, OpenPitEngineBuildError ** out_build_error, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_build(builder, out_build_error, out_error);
}

void openpit_destroy_engine_build_error(OpenPitEngineBuildError * build_error) {
    _fn_openpit_destroy_engine_build_error(build_error);
}

OpenPitEngineBuildErrorCode openpit_engine_build_error_get_code(const OpenPitEngineBuildError * build_error) {
    return _fn_openpit_engine_build_error_get_code(build_error);
}

OpenPitStringView openpit_engine_build_error_get_policy_name(const OpenPitEngineBuildError * build_error) {
    return _fn_openpit_engine_build_error_get_policy_name(build_error);
}

uint16_t openpit_engine_build_error_get_policy_group_id(const OpenPitEngineBuildError * build_error) {
    return _fn_openpit_engine_build_error_get_policy_group_id(build_error);
}

void openpit_destroy_engine(OpenPitEngine * engine) {
    _fn_openpit_destroy_engine(engine);
}

OpenPitPretradeStatus openpit_engine_start_pre_trade(OpenPitEngine * engine, const OpenPitOrder * order, OpenPitPretradePreTradeRequest ** out_request, OpenPitPretradeRejectList ** out_rejects, OpenPitOutError out_error) {
    return _fn_openpit_engine_start_pre_trade(engine, order, out_request, out_rejects, out_error);
}

OpenPitPretradeStatus openpit_engine_execute_pre_trade(OpenPitEngine * engine, const OpenPitOrder * order, OpenPitPretradePreTradeReservation ** out_reservation, OpenPitPretradeRejectList ** out_rejects, OpenPitOutError out_error) {
    return _fn_openpit_engine_execute_pre_trade(engine, order, out_reservation, out_rejects, out_error);
}

bool openpit_engine_execute_pre_trade_drop_copy(OpenPitEngine * engine, const OpenPitOrder * order, OpenPitPretradePreTradeReservation ** out_reservation, OpenPitOutError out_error) {
    return _fn_openpit_engine_execute_pre_trade_drop_copy(engine, order, out_reservation, out_error);
}

bool openpit_engine_start_pre_trade_dry_run(OpenPitEngine * engine, const OpenPitOrder * order, OpenPitPretradePreTradeDryRunReport ** out_report, OpenPitOutError out_error) {
    return _fn_openpit_engine_start_pre_trade_dry_run(engine, order, out_report, out_error);
}

bool openpit_engine_execute_pre_trade_dry_run(OpenPitEngine * engine, const OpenPitOrder * order, OpenPitPretradePreTradeDryRunReport ** out_report, OpenPitOutError out_error) {
    return _fn_openpit_engine_execute_pre_trade_dry_run(engine, order, out_report, out_error);
}

OpenPitPretradeStatus openpit_pretrade_pre_trade_request_execute(OpenPitPretradePreTradeRequest * request, OpenPitPretradePreTradeReservation ** out_reservation, OpenPitPretradeRejectList ** out_rejects, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_request_execute(request, out_reservation, out_rejects, out_error);
}

void openpit_destroy_pretrade_pre_trade_request(OpenPitPretradePreTradeRequest * request) {
    _fn_openpit_destroy_pretrade_pre_trade_request(request);
}

void openpit_pretrade_pre_trade_reservation_commit(OpenPitPretradePreTradeReservation * reservation) {
    _fn_openpit_pretrade_pre_trade_reservation_commit(reservation);
}

void openpit_pretrade_pre_trade_reservation_rollback(OpenPitPretradePreTradeReservation * reservation) {
    _fn_openpit_pretrade_pre_trade_reservation_rollback(reservation);
}

OpenPitPretradePreTradeLock * openpit_pretrade_pre_trade_reservation_get_lock(const OpenPitPretradePreTradeReservation * reservation) {
    return _fn_openpit_pretrade_pre_trade_reservation_get_lock(reservation);
}

OpenPitAccountAdjustmentOutcomeList * openpit_pretrade_pre_trade_reservation_get_account_adjustments(const OpenPitPretradePreTradeReservation * reservation) {
    return _fn_openpit_pretrade_pre_trade_reservation_get_account_adjustments(reservation);
}

OpenPitPretradeAccountBlockList * openpit_pretrade_pre_trade_reservation_get_account_block(const OpenPitPretradePreTradeReservation * reservation) {
    return _fn_openpit_pretrade_pre_trade_reservation_get_account_block(reservation);
}

void openpit_destroy_pretrade_pre_trade_reservation(OpenPitPretradePreTradeReservation * reservation) {
    _fn_openpit_destroy_pretrade_pre_trade_reservation(reservation);
}

bool openpit_pretrade_pre_trade_dry_run_report_is_pass(const OpenPitPretradePreTradeDryRunReport * report) {
    return _fn_openpit_pretrade_pre_trade_dry_run_report_is_pass(report);
}

OpenPitPretradeRejectList * openpit_pretrade_pre_trade_dry_run_report_get_rejects(const OpenPitPretradePreTradeDryRunReport * report) {
    return _fn_openpit_pretrade_pre_trade_dry_run_report_get_rejects(report);
}

OpenPitPretradePreTradeLock * openpit_pretrade_pre_trade_dry_run_report_get_lock(const OpenPitPretradePreTradeDryRunReport * report) {
    return _fn_openpit_pretrade_pre_trade_dry_run_report_get_lock(report);
}

OpenPitAccountAdjustmentOutcomeList * openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(const OpenPitPretradePreTradeDryRunReport * report) {
    return _fn_openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(report);
}

OpenPitPretradeAccountBlockList * openpit_pretrade_pre_trade_dry_run_report_get_account_block(const OpenPitPretradePreTradeDryRunReport * report) {
    return _fn_openpit_pretrade_pre_trade_dry_run_report_get_account_block(report);
}

void openpit_destroy_pretrade_pre_trade_dry_run_report(OpenPitPretradePreTradeDryRunReport * report) {
    _fn_openpit_destroy_pretrade_pre_trade_dry_run_report(report);
}

bool openpit_engine_apply_execution_report(OpenPitEngine * engine, const OpenPitExecutionReport * report, OpenPitPostTradeResult ** out_result, OpenPitOutError out_error) {
    return _fn_openpit_engine_apply_execution_report(engine, report, out_result, out_error);
}

void openpit_destroy_post_trade_result(OpenPitPostTradeResult * result) {
    _fn_openpit_destroy_post_trade_result(result);
}

const OpenPitPretradeAccountBlockList * openpit_post_trade_result_get_account_blocks(const OpenPitPostTradeResult * result) {
    return _fn_openpit_post_trade_result_get_account_blocks(result);
}

const OpenPitAccountAdjustmentOutcomeList * openpit_post_trade_result_get_account_adjustments(const OpenPitPostTradeResult * result) {
    return _fn_openpit_post_trade_result_get_account_adjustments(result);
}

const OpenPitAccountPnlOutcomeList * openpit_post_trade_result_get_account_pnls(const OpenPitPostTradeResult * result) {
    return _fn_openpit_post_trade_result_get_account_pnls(result);
}

void openpit_destroy_account_adjustment_batch_error(OpenPitAccountAdjustmentBatchError * batch_error) {
    _fn_openpit_destroy_account_adjustment_batch_error(batch_error);
}

size_t openpit_account_adjustment_batch_error_get_failed_adjustment_index(const OpenPitAccountAdjustmentBatchError * batch_error) {
    return _fn_openpit_account_adjustment_batch_error_get_failed_adjustment_index(batch_error);
}

const OpenPitPretradeRejectList * openpit_account_adjustment_batch_error_get_rejects(const OpenPitAccountAdjustmentBatchError * batch_error) {
    return _fn_openpit_account_adjustment_batch_error_get_rejects(batch_error);
}

OpenPitAccountAdjustmentApplyStatus openpit_engine_apply_account_adjustment(OpenPitEngine * engine, OpenPitParamAccountId account_id, const OpenPitAccountAdjustment * adjustments, size_t adjustments_len, OpenPitAccountAdjustmentBatchError ** out_reject, OpenPitAccountAdjustmentOutcomeList ** out_outcomes, OpenPitPretradeAccountBlockList ** out_blocks, OpenPitOutError out_error) {
    return _fn_openpit_engine_apply_account_adjustment(engine, account_id, adjustments, adjustments_len, out_reject, out_outcomes, out_blocks, out_error);
}

void openpit_destroy_account_group_error(OpenPitAccountGroupError * err) {
    _fn_openpit_destroy_account_group_error(err);
}

OpenPitStringView openpit_account_group_error_get_message(const OpenPitAccountGroupError * err) {
    return _fn_openpit_account_group_error_get_message(err);
}

OpenPitParamAccountId openpit_account_group_error_get_account(const OpenPitAccountGroupError * err) {
    return _fn_openpit_account_group_error_get_account(err);
}

bool openpit_account_group_error_get_current_group(const OpenPitAccountGroupError * err, OpenPitParamAccountGroupId * out_group) {
    return _fn_openpit_account_group_error_get_current_group(err, out_group);
}

bool openpit_engine_register_account_group(OpenPitEngine * engine, const OpenPitParamAccountId * accounts, size_t accounts_len, OpenPitParamAccountGroupId group, OpenPitAccountGroupError ** out_group_error, OpenPitOutError out_error) {
    return _fn_openpit_engine_register_account_group(engine, accounts, accounts_len, group, out_group_error, out_error);
}

bool openpit_engine_unregister_account_group(OpenPitEngine * engine, const OpenPitParamAccountId * accounts, size_t accounts_len, OpenPitParamAccountGroupId group, OpenPitAccountGroupError ** out_group_error, OpenPitOutError out_error) {
    return _fn_openpit_engine_unregister_account_group(engine, accounts, accounts_len, group, out_group_error, out_error);
}

bool openpit_engine_account_group(const OpenPitEngine * engine, OpenPitParamAccountId account, OpenPitParamAccountGroupId * out_group) {
    return _fn_openpit_engine_account_group(engine, account, out_group);
}

bool openpit_engine_set_account_currency(OpenPitEngine * engine, OpenPitParamAccountId account_id, OpenPitStringView asset, OpenPitOutError out_error) {
    return _fn_openpit_engine_set_account_currency(engine, account_id, asset, out_error);
}

void openpit_engine_clear_account_currency(OpenPitEngine * engine, OpenPitParamAccountId account_id) {
    _fn_openpit_engine_clear_account_currency(engine, account_id);
}

bool openpit_engine_set_account_group_currency(OpenPitEngine * engine, OpenPitParamAccountGroupId group, OpenPitStringView asset, OpenPitOutError out_error) {
    return _fn_openpit_engine_set_account_group_currency(engine, group, asset, out_error);
}

void openpit_engine_clear_account_group_currency(OpenPitEngine * engine, OpenPitParamAccountGroupId group) {
    _fn_openpit_engine_clear_account_group_currency(engine, group);
}

void openpit_destroy_account_block_error(OpenPitAccountBlockError * err) {
    _fn_openpit_destroy_account_block_error(err);
}

OpenPitStringView openpit_account_block_error_get_message(const OpenPitAccountBlockError * err) {
    return _fn_openpit_account_block_error_get_message(err);
}

OpenPitAccountBlockErrorKind openpit_account_block_error_get_kind(const OpenPitAccountBlockError * err) {
    return _fn_openpit_account_block_error_get_kind(err);
}

bool openpit_account_block_error_get_account(const OpenPitAccountBlockError * err, OpenPitParamAccountId * out_account) {
    return _fn_openpit_account_block_error_get_account(err, out_account);
}

bool openpit_account_block_error_get_group(const OpenPitAccountBlockError * err, OpenPitParamAccountGroupId * out_group) {
    return _fn_openpit_account_block_error_get_group(err, out_group);
}

void openpit_destroy_configure_error(OpenPitConfigureError * err) {
    _fn_openpit_destroy_configure_error(err);
}

OpenPitStringView openpit_configure_error_get_message(const OpenPitConfigureError * err) {
    return _fn_openpit_configure_error_get_message(err);
}

OpenPitConfigureErrorKind openpit_configure_error_get_kind(const OpenPitConfigureError * err) {
    return _fn_openpit_configure_error_get_kind(err);
}

void openpit_engine_block_account(OpenPitEngine * engine, OpenPitParamAccountId account_id, OpenPitStringView reason) {
    _fn_openpit_engine_block_account(engine, account_id, reason);
}

void openpit_engine_unblock_account(OpenPitEngine * engine, OpenPitParamAccountId account_id) {
    _fn_openpit_engine_unblock_account(engine, account_id);
}

bool openpit_engine_replace_account_block_reason(OpenPitEngine * engine, OpenPitParamAccountId account_id, OpenPitStringView reason, OpenPitAccountBlockError ** out_error) {
    return _fn_openpit_engine_replace_account_block_reason(engine, account_id, reason, out_error);
}

bool openpit_engine_block_account_group(OpenPitEngine * engine, OpenPitParamAccountGroupId group, OpenPitStringView reason, OpenPitAccountBlockError ** out_error) {
    return _fn_openpit_engine_block_account_group(engine, group, reason, out_error);
}

bool openpit_engine_unblock_account_group(OpenPitEngine * engine, OpenPitParamAccountGroupId group, OpenPitAccountBlockError ** out_error) {
    return _fn_openpit_engine_unblock_account_group(engine, group, out_error);
}

bool openpit_engine_replace_account_group_block_reason(OpenPitEngine * engine, OpenPitParamAccountGroupId group, OpenPitStringView reason, OpenPitAccountBlockError ** out_error) {
    return _fn_openpit_engine_replace_account_group_block_reason(engine, group, reason, out_error);
}

OpenPitPretradePreTradePolicy * openpit_create_pretrade_custom_pre_trade_policy(OpenPitStringView name, uint16_t policy_group_id, OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_fn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_fn, OpenPitPretradePreTradePolicyApplyExecutionReportFn apply_execution_report_fn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn apply_account_adjustment_fn, OpenPitPretradePreTradePolicyFreeUserDataFn free_user_data_fn, void * user_data, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_custom_pre_trade_policy(name, policy_group_id, check_pre_trade_start_fn, perform_pre_trade_check_fn, apply_execution_report_fn, apply_account_adjustment_fn, free_user_data_fn, user_data, out_error);
}

OpenPitPretradePreTradePolicy * openpit_create_pretrade_custom_pre_trade_policy_with_dry_run(OpenPitStringView name, uint16_t policy_group_id, OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_fn, OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_dry_run_fn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_fn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_dry_run_fn, OpenPitPretradePreTradePolicyApplyExecutionReportFn apply_execution_report_fn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn apply_account_adjustment_fn, OpenPitPretradePreTradePolicyFreeUserDataFn free_user_data_fn, void * user_data, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_custom_pre_trade_policy_with_dry_run(name, policy_group_id, check_pre_trade_start_fn, check_pre_trade_start_dry_run_fn, perform_pre_trade_check_fn, perform_pre_trade_check_dry_run_fn, apply_execution_report_fn, apply_account_adjustment_fn, free_user_data_fn, user_data, out_error);
}

void openpit_destroy_pretrade_pre_trade_policy(OpenPitPretradePreTradePolicy * policy) {
    _fn_openpit_destroy_pretrade_pre_trade_policy(policy);
}

OpenPitStringView openpit_pretrade_pre_trade_policy_get_name(const OpenPitPretradePreTradePolicy * policy) {
    return _fn_openpit_pretrade_pre_trade_policy_get_name(policy);
}

bool openpit_engine_builder_add_pre_trade_policy(OpenPitEngineBuilder * builder, OpenPitPretradePreTradePolicy * policy, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_pre_trade_policy(builder, policy, out_error);
}

bool openpit_mutations_push(OpenPitMutations * mutations, OpenPitMutationFn commit_fn, OpenPitMutationFn rollback_fn, void * user_data, OpenPitMutationFreeFn free_fn, OpenPitOutError out_error) {
    return _fn_openpit_mutations_push(mutations, commit_fn, rollback_fn, user_data, free_fn, out_error);
}

bool openpit_engine_builder_add_builtin_order_size_limit_policy(OpenPitEngineBuilder * builder, uint16_t policy_group_id, const OpenPitPretradePoliciesOrderSizeBrokerBarrier * broker, const OpenPitPretradePoliciesOrderSizeAssetBarrier * asset, size_t asset_len, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier * account_asset, size_t account_asset_len, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_order_size_limit_policy(builder, policy_group_id, broker, asset, asset_len, account_asset, account_asset_len, out_error);
}

bool openpit_engine_configure_order_size_limit(OpenPitEngine * engine, OpenPitStringView name, const OpenPitPretradePoliciesOrderSizeBrokerBarrier * broker, bool has_broker, const OpenPitPretradePoliciesOrderSizeAssetBarrier * asset, size_t asset_len, bool has_asset, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier * account_asset, size_t account_asset_len, bool has_account_asset, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_order_size_limit(engine, name, broker, has_broker, asset, asset_len, has_asset, account_asset, account_asset_len, has_account_asset, out_error);
}

bool openpit_engine_builder_add_builtin_order_validation_policy(OpenPitEngineBuilder * builder, uint16_t policy_group_id, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_order_validation_policy(builder, policy_group_id, out_error);
}

bool openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(OpenPitEngineBuilder * builder, uint16_t policy_group_id, const OpenPitPretradePoliciesPnlBoundsBarrier * broker, size_t broker_len, const OpenPitPretradePoliciesPnlBoundsAccountBarrier * account, size_t account_len, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(builder, policy_group_id, broker, broker_len, account, account_len, out_error);
}

bool openpit_engine_configure_pnl_bounds_killswitch(OpenPitEngine * engine, OpenPitStringView name, const OpenPitPretradePoliciesPnlBoundsBarrier * broker, size_t broker_len, bool has_broker, const OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate * account, size_t account_len, bool has_account, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_pnl_bounds_killswitch(engine, name, broker, broker_len, has_broker, account, account_len, has_account, out_error);
}

bool openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(OpenPitEngine * engine, OpenPitStringView name, OpenPitParamAccountId account_id, OpenPitStringView settlement_asset, OpenPitParamPnl pnl, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(engine, name, account_id, settlement_asset, pnl, out_error);
}

bool openpit_engine_builder_add_builtin_rate_limit_policy(OpenPitEngineBuilder * builder, uint16_t policy_group_id, const OpenPitPretradePoliciesRateLimitBrokerBarrier * broker, const OpenPitPretradePoliciesRateLimitAssetBarrier * asset, size_t asset_len, const OpenPitPretradePoliciesRateLimitAccountBarrier * account, size_t account_len, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier * account_asset, size_t account_asset_len, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_rate_limit_policy(builder, policy_group_id, broker, asset, asset_len, account, account_len, account_asset, account_asset_len, out_error);
}

bool openpit_engine_configure_rate_limit(OpenPitEngine * engine, OpenPitStringView name, const OpenPitPretradePoliciesRateLimitBrokerBarrier * broker, bool has_broker, const OpenPitPretradePoliciesRateLimitAssetBarrier * asset, size_t asset_len, bool has_asset, const OpenPitPretradePoliciesRateLimitAccountBarrier * account, size_t account_len, bool has_account, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier * account_asset, size_t account_asset_len, bool has_account_asset, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_rate_limit(engine, name, broker, has_broker, asset, asset_len, has_asset, account, account_len, has_account, account_asset, account_asset_len, has_account_asset, out_error);
}

bool openpit_engine_builder_add_builtin_spot_funds_policy(OpenPitEngineBuilder * builder, const OpenPitMarketDataService * market_data, const uint16_t * market_slippage_bps, uint8_t pricing_source, const OpenPitPretradePoliciesSpotFundsOverride * instrument_overrides, size_t overrides_len, uint16_t policy_group_id, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_spot_funds_policy(builder, market_data, market_slippage_bps, pricing_source, instrument_overrides, overrides_len, policy_group_id, out_error);
}

bool openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(OpenPitEngineBuilder * builder, const OpenPitMarketDataService * market_data, uint16_t policy_group_id, const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier * global, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier * account_group, size_t account_group_len, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier * account, size_t account_len, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(builder, market_data, policy_group_id, global, account_group, account_group_len, account, account_len, out_error);
}

bool openpit_engine_configure_spot_funds(OpenPitEngine * engine, OpenPitStringView name, uint16_t global_slippage_bps, bool has_global_slippage_bps, uint8_t pricing_source, bool has_pricing_source, const OpenPitPretradePoliciesSpotFundsOverride * instrument_overrides, size_t overrides_len, bool has_overrides, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_spot_funds(engine, name, global_slippage_bps, has_global_slippage_bps, pricing_source, has_pricing_source, instrument_overrides, overrides_len, has_overrides, out_error);
}

bool openpit_engine_configure_spot_funds_pnl_bounds_killswitch(OpenPitEngine * engine, OpenPitStringView name, const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier * global, bool has_global, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier * account_group, size_t account_group_len, bool has_account_group, const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier * account, size_t account_len, bool has_account, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_spot_funds_pnl_bounds_killswitch(engine, name, global, has_global, account_group, account_group_len, has_account_group, account, account_len, has_account, out_error);
}

OpenPitPretradeAccountBlockList * openpit_engine_configure_spot_funds_set_account_pnl(OpenPitEngine * engine, OpenPitStringView name, OpenPitParamAccountId account_id, OpenPitPnlState state, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_spot_funds_set_account_pnl(engine, name, account_id, state, out_error);
}

bool openpit_engine_configure_spot_funds_global_limit_mode(OpenPitEngine * engine, OpenPitStringView name, OpenPitPretradePoliciesSpotFundsLimitMode mode, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_spot_funds_global_limit_mode(engine, name, mode, out_error);
}

bool openpit_engine_configure_spot_funds_account_limit_mode(OpenPitEngine * engine, OpenPitStringView name, OpenPitParamAccountId account_id, OpenPitPretradePoliciesSpotFundsLimitMode mode, bool has_mode, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_spot_funds_account_limit_mode(engine, name, account_id, mode, has_mode, out_error);
}

bool openpit_engine_configure_spot_funds_account_group_limit_mode(OpenPitEngine * engine, OpenPitStringView name, OpenPitParamAccountGroupId account_group_id, OpenPitPretradePoliciesSpotFundsLimitMode mode, bool has_mode, OpenPitConfigureError ** out_error) {
    return _fn_openpit_engine_configure_spot_funds_account_group_limit_mode(engine, name, account_group_id, mode, has_mode, out_error);
}

OpenPitStringView openpit_get_runtime_version(void) {
    return _fn_openpit_get_runtime_version();
}

OpenPitStringView openpit_get_runtime_build_profile(void) {
    return _fn_openpit_get_runtime_build_profile();
}

void openpit_account_control_block(const OpenPitAccountControl * control, OpenPitPretradeAccountBlock block) {
    _fn_openpit_account_control_block(control, block);
}

OpenPitAccountControl * openpit_account_control_clone(const OpenPitAccountControl * control) {
    return _fn_openpit_account_control_clone(control);
}

void openpit_destroy_account_control(OpenPitAccountControl * control) {
    _fn_openpit_destroy_account_control(control);
}

OpenPitAccountControl * openpit_pretrade_context_get_account_control(const OpenPitPretradeContext * ctx) {
    return _fn_openpit_pretrade_context_get_account_control(ctx);
}

OpenPitAccountControl * openpit_account_adjustment_context_get_account_control(const OpenPitAccountAdjustmentContext * ctx) {
    return _fn_openpit_account_adjustment_context_get_account_control(ctx);
}

bool openpit_pretrade_context_get_account_group(const OpenPitPretradeContext * ctx, OpenPitParamAccountGroupId * out_group) {
    return _fn_openpit_pretrade_context_get_account_group(ctx, out_group);
}

bool openpit_account_adjustment_context_get_account_group(const OpenPitAccountAdjustmentContext * ctx, OpenPitParamAccountGroupId * out_group) {
    return _fn_openpit_account_adjustment_context_get_account_group(ctx, out_group);
}

bool openpit_post_trade_context_get_account_group(const OpenPitPostTradeContext * ctx, OpenPitParamAccountGroupId * out_group) {
    return _fn_openpit_post_trade_context_get_account_group(ctx, out_group);
}

bool openpit_create_param_account_group_id_from_uint32(uint32_t value, OpenPitParamAccountGroupId * out, OpenPitOutError out_error) {
    return _fn_openpit_create_param_account_group_id_from_uint32(value, out, out_error);
}

bool openpit_create_param_account_group_id_from_string(OpenPitStringView value, OpenPitParamAccountGroupId * out, OpenPitOutError out_error) {
    return _fn_openpit_create_param_account_group_id_from_string(value, out, out_error);
}

void openpit_destroy_account_adjustment_outcome_list(OpenPitAccountAdjustmentOutcomeList * outcomes) {
    _fn_openpit_destroy_account_adjustment_outcome_list(outcomes);
}

size_t openpit_account_adjustment_outcome_list_len(const OpenPitAccountAdjustmentOutcomeList * list) {
    return _fn_openpit_account_adjustment_outcome_list_len(list);
}

bool openpit_account_adjustment_outcome_list_get(const OpenPitAccountAdjustmentOutcomeList * list, size_t index, OpenPitAccountAdjustmentOutcome * out_outcome) {
    return _fn_openpit_account_adjustment_outcome_list_get(list, index, out_outcome);
}

size_t openpit_account_pnl_outcome_list_len(const OpenPitAccountPnlOutcomeList * list) {
    return _fn_openpit_account_pnl_outcome_list_len(list);
}

bool openpit_account_pnl_outcome_list_get(const OpenPitAccountPnlOutcomeList * list, size_t index, OpenPitAccountPnlOutcome * out_outcome) {
    return _fn_openpit_account_pnl_outcome_list_get(list, index, out_outcome);
}

bool openpit_pretrade_pre_trade_result_push_lock_price(OpenPitPretradePreTradeResult * result, OpenPitParamPrice price, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_result_push_lock_price(result, price, out_error);
}

bool openpit_pretrade_pre_trade_result_push_account_adjustment(OpenPitPretradePreTradeResult * result, OpenPitAccountOutcomeEntry entry, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_result_push_account_adjustment(result, entry, out_error);
}

bool openpit_pretrade_post_trade_adjustment_list_push(OpenPitPostTradeAdjustmentList * list, uint16_t policy_group_id, OpenPitAccountOutcomeEntry entry, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_post_trade_adjustment_list_push(list, policy_group_id, entry, out_error);
}

bool openpit_pretrade_post_trade_account_pnl_list_push(OpenPitPostTradeAccountPnlList * list, OpenPitAccountPnlOutcome outcome, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_post_trade_account_pnl_list_push(list, outcome, out_error);
}

bool openpit_pretrade_account_adjustment_result_push_account_outcome(OpenPitPretradeAccountAdjustmentResult * result, OpenPitAccountOutcomeEntry entry, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_account_adjustment_result_push_account_outcome(result, entry, out_error);
}

void openpit_pretrade_account_adjustment_result_push_account_block(OpenPitPretradeAccountAdjustmentResult * result, OpenPitPretradeAccountBlock block) {
    _fn_openpit_pretrade_account_adjustment_result_push_account_block(result, block);
}

void openpit_destroy_shared_bytes(OpenPitSharedBytes * handle) {
    _fn_openpit_destroy_shared_bytes(handle);
}

OpenPitBytesView openpit_shared_bytes_view(const OpenPitSharedBytes * handle) {
    return _fn_openpit_shared_bytes_view(handle);
}

OpenPitMarketDataQuote openpit_create_marketdata_quote(void) {
    return _fn_openpit_create_marketdata_quote();
}

OpenPitMarketDataQuoteTtl openpit_create_marketdata_quote_ttl_infinite(void) {
    return _fn_openpit_create_marketdata_quote_ttl_infinite();
}

OpenPitMarketDataQuoteTtl openpit_create_marketdata_quote_ttl_within(uint64_t secs, uint32_t nanos) {
    return _fn_openpit_create_marketdata_quote_ttl_within(secs, nanos);
}

OpenPitMarketDataService * openpit_create_marketdata_service(uint8_t mode, OpenPitMarketDataQuoteTtl default_ttl, OpenPitOutError out_error) {
    return _fn_openpit_create_marketdata_service(mode, default_ttl, out_error);
}

void openpit_destroy_marketdata_service(OpenPitMarketDataService * service) {
    _fn_openpit_destroy_marketdata_service(service);
}

OpenPitMarketDataService * openpit_marketdata_service_clone(const OpenPitMarketDataService * service) {
    return _fn_openpit_marketdata_service_clone(service);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_register(const OpenPitMarketDataService * service, const OpenPitInstrument * instrument, OpenPitMarketDataInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_register(service, instrument, out_id, out_error);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_register_with_ttl(const OpenPitMarketDataService * service, const OpenPitInstrument * instrument, OpenPitMarketDataQuoteTtl ttl, OpenPitMarketDataInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_register_with_ttl(service, instrument, ttl, out_id, out_error);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_register_with_id(const OpenPitMarketDataService * service, const OpenPitInstrument * instrument, OpenPitMarketDataInstrumentId instrument_id, OpenPitMarketDataInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_register_with_id(service, instrument, instrument_id, out_id, out_error);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_register_with_id_and_ttl(const OpenPitMarketDataService * service, const OpenPitInstrument * instrument, OpenPitMarketDataInstrumentId instrument_id, OpenPitMarketDataQuoteTtl ttl, OpenPitMarketDataInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_register_with_id_and_ttl(service, instrument, instrument_id, ttl, out_id, out_error);
}

void openpit_marketdata_service_set_account_ttl(const OpenPitMarketDataService * service, OpenPitParamAccountId account_id, OpenPitMarketDataQuoteTtl ttl) {
    _fn_openpit_marketdata_service_set_account_ttl(service, account_id, ttl);
}

void openpit_marketdata_service_clear_account_ttl(const OpenPitMarketDataService * service, OpenPitParamAccountId account_id) {
    _fn_openpit_marketdata_service_clear_account_ttl(service, account_id);
}

void openpit_marketdata_service_set_account_group_ttl(const OpenPitMarketDataService * service, OpenPitParamAccountGroupId account_group_id, OpenPitMarketDataQuoteTtl ttl) {
    _fn_openpit_marketdata_service_set_account_group_ttl(service, account_group_id, ttl);
}

void openpit_marketdata_service_clear_account_group_ttl(const OpenPitMarketDataService * service, OpenPitParamAccountGroupId account_group_id) {
    _fn_openpit_marketdata_service_clear_account_group_ttl(service, account_group_id);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_set_instrument_ttl(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitMarketDataQuoteTtl ttl) {
    return _fn_openpit_marketdata_service_set_instrument_ttl(service, instrument_id, ttl);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_clear_instrument_ttl(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id) {
    return _fn_openpit_marketdata_service_clear_instrument_ttl(service, instrument_id);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_set_instrument_account_ttl(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitParamAccountId account_id, OpenPitMarketDataQuoteTtl ttl) {
    return _fn_openpit_marketdata_service_set_instrument_account_ttl(service, instrument_id, account_id, ttl);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_clear_instrument_account_ttl(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitParamAccountId account_id) {
    return _fn_openpit_marketdata_service_clear_instrument_account_ttl(service, instrument_id, account_id);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_set_instrument_account_group_ttl(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitParamAccountGroupId account_group_id, OpenPitMarketDataQuoteTtl ttl) {
    return _fn_openpit_marketdata_service_set_instrument_account_group_ttl(service, instrument_id, account_group_id, ttl);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_clear_instrument_account_group_ttl(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitParamAccountGroupId account_group_id) {
    return _fn_openpit_marketdata_service_clear_instrument_account_group_ttl(service, instrument_id, account_group_id);
}

void openpit_marketdata_service_clear(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id) {
    _fn_openpit_marketdata_service_clear(service, instrument_id);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_push(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitMarketDataQuote quote, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_push(service, instrument_id, quote, out_error);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_patch(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitMarketDataQuote quote, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_push_patch(service, instrument_id, quote, out_error);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_for(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitMarketDataQuote quote, const OpenPitParamAccountId * account_ids, size_t account_ids_len, const OpenPitParamAccountGroupId * account_group_ids, size_t account_group_ids_len, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_push_for(service, instrument_id, quote, account_ids, account_ids_len, account_group_ids, account_group_ids_len, out_error);
}

OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_for_patch(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitMarketDataQuote quote, const OpenPitParamAccountId * account_ids, size_t account_ids_len, const OpenPitParamAccountGroupId * account_group_ids, size_t account_group_ids_len, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_push_for_patch(service, instrument_id, quote, account_ids, account_ids_len, account_group_ids, account_group_ids_len, out_error);
}

bool openpit_marketdata_service_push_by_instrument(const OpenPitMarketDataService * service, const OpenPitInstrument * instrument, OpenPitMarketDataQuote quote, OpenPitMarketDataInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_push_by_instrument(service, instrument, quote, out_id, out_error);
}

bool openpit_marketdata_service_push_by_instrument_patch(const OpenPitMarketDataService * service, const OpenPitInstrument * instrument, OpenPitMarketDataQuote quote, OpenPitMarketDataInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_marketdata_service_push_by_instrument_patch(service, instrument, quote, out_id, out_error);
}

OpenPitMarketDataGetStatus openpit_marketdata_service_get(const OpenPitMarketDataService * service, OpenPitMarketDataInstrumentId instrument_id, OpenPitParamAccountId account_id, OpenPitMarketDataAccountGroupResolver resolve_account_group, void * user_data, OpenPitMarketDataQuoteResolution resolution, OpenPitMarketDataQuote * out_quote) {
    return _fn_openpit_marketdata_service_get(service, instrument_id, account_id, resolve_account_group, user_data, resolution, out_quote);
}

bool openpit_marketdata_service_resolve(const OpenPitMarketDataService * service, const OpenPitInstrument * instrument, OpenPitMarketDataInstrumentId * out_id) {
    return _fn_openpit_marketdata_service_resolve(service, instrument, out_id);
}

OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock(void) {
    return _fn_openpit_create_pretrade_pre_trade_lock();
}

void openpit_destroy_pretrade_pre_trade_lock(OpenPitPretradePreTradeLock * handle) {
    _fn_openpit_destroy_pretrade_pre_trade_lock(handle);
}

OpenPitPretradePreTradeLock * openpit_pretrade_pre_trade_lock_clone(const OpenPitPretradePreTradeLock * lock) {
    return _fn_openpit_pretrade_pre_trade_lock_clone(lock);
}

size_t openpit_pretrade_pre_trade_lock_len(const OpenPitPretradePreTradeLock * lock) {
    return _fn_openpit_pretrade_pre_trade_lock_len(lock);
}

bool openpit_pretrade_pre_trade_lock_is_empty(const OpenPitPretradePreTradeLock * lock) {
    return _fn_openpit_pretrade_pre_trade_lock_is_empty(lock);
}

bool openpit_pretrade_pre_trade_lock_push(OpenPitPretradePreTradeLock * lock, uint16_t policy_group_id, OpenPitParamPrice price, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_lock_push(lock, policy_group_id, price, out_error);
}

bool openpit_pretrade_pre_trade_lock_push_many(OpenPitPretradePreTradeLock * lock, const OpenPitPretradePreTradeLockEntry * entries_ptr, size_t entries_len, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_lock_push_many(lock, entries_ptr, entries_len, out_error);
}

OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_entries(const OpenPitPretradePreTradeLockEntry * entries_ptr, size_t entries_len, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_pre_trade_lock_from_entries(entries_ptr, entries_len, out_error);
}

bool openpit_pretrade_pre_trade_lock_merge(OpenPitPretradePreTradeLock * dst, const OpenPitPretradePreTradeLock * src, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_lock_merge(dst, src, out_error);
}

void openpit_destroy_pretrade_pre_trade_lock_prices(OpenPitPretradePreTradeLockPrices * handle) {
    _fn_openpit_destroy_pretrade_pre_trade_lock_prices(handle);
}

OpenPitPretradePreTradeLockPricesView openpit_pretrade_pre_trade_lock_prices_view(const OpenPitPretradePreTradeLockPrices * handle) {
    return _fn_openpit_pretrade_pre_trade_lock_prices_view(handle);
}

OpenPitPretradePreTradeLockPricesStatus openpit_pretrade_pre_trade_lock_prices_of(const OpenPitPretradePreTradeLock * lock, uint16_t policy_group_id, OpenPitParamPrice * out_price, OpenPitPretradePreTradeLockPrices ** out_prices, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_lock_prices_of(lock, policy_group_id, out_price, out_prices, out_error);
}

OpenPitPretradePreTradeLockEntries * openpit_pretrade_pre_trade_lock_entries(const OpenPitPretradePreTradeLock * lock) {
    return _fn_openpit_pretrade_pre_trade_lock_entries(lock);
}

void openpit_destroy_pretrade_pre_trade_lock_entries(OpenPitPretradePreTradeLockEntries * handle) {
    _fn_openpit_destroy_pretrade_pre_trade_lock_entries(handle);
}

OpenPitPretradePreTradeLockEntriesView openpit_pretrade_pre_trade_lock_entries_view(const OpenPitPretradePreTradeLockEntries * handle) {
    return _fn_openpit_pretrade_pre_trade_lock_entries_view(handle);
}

OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_msgpack(const OpenPitPretradePreTradeLock * lock, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_lock_to_msgpack(lock, out_error);
}

OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_msgpack(const uint8_t * data_ptr, size_t data_len, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_pre_trade_lock_from_msgpack(data_ptr, data_len, out_error);
}

OpenPitSharedString * openpit_pretrade_pre_trade_lock_to_json(const OpenPitPretradePreTradeLock * lock, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_lock_to_json(lock, out_error);
}

OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_json(const uint8_t * text_ptr, size_t text_len, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_pre_trade_lock_from_json(text_ptr, text_len, out_error);
}

OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_cbor(const OpenPitPretradePreTradeLock * lock, OpenPitOutError out_error) {
    return _fn_openpit_pretrade_pre_trade_lock_to_cbor(lock, out_error);
}

OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_cbor(const uint8_t * data_ptr, size_t data_len, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_pre_trade_lock_from_cbor(data_ptr, data_len, out_error);
}

OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_raw(const OpenPitPretradePreTradeLock * lock) {
    return _fn_openpit_pretrade_pre_trade_lock_to_raw(lock);
}

OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_raw(const uint8_t * data_ptr, size_t data_len, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_pre_trade_lock_from_raw(data_ptr, data_len, out_error);
}

OpenPitReferenceBook * openpit_create_reference_book(void) {
    return _fn_openpit_create_reference_book();
}

void openpit_destroy_reference_book(OpenPitReferenceBook * book) {
    _fn_openpit_destroy_reference_book(book);
}

OpenPitReferenceBookRegisterStatus openpit_reference_book_register(OpenPitReferenceBook * book, const OpenPitInstrument * instrument, OpenPitInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_reference_book_register(book, instrument, out_id, out_error);
}

OpenPitReferenceBookRegisterStatus openpit_reference_book_register_with_id(OpenPitReferenceBook * book, const OpenPitInstrument * instrument, OpenPitInstrumentId instrument_id, OpenPitInstrumentId * out_id, OpenPitOutError out_error) {
    return _fn_openpit_reference_book_register_with_id(book, instrument, instrument_id, out_id, out_error);
}

bool openpit_reference_book_resolve(const OpenPitReferenceBook * book, const OpenPitInstrument * instrument, OpenPitInstrumentId * out_id) {
    return _fn_openpit_reference_book_resolve(book, instrument, out_id);
}

OpenPitReferenceBookStatus openpit_reference_book_set_settlement_scheme(OpenPitReferenceBook * book, OpenPitInstrumentId instrument_id, OpenPitSettlementScheme settlement_scheme, OpenPitOutError out_error) {
    return _fn_openpit_reference_book_set_settlement_scheme(book, instrument_id, settlement_scheme, out_error);
}

OpenPitReferenceBookStatus openpit_reference_book_clear_settlement_scheme(OpenPitReferenceBook * book, OpenPitInstrumentId instrument_id, OpenPitOutError out_error) {
    return _fn_openpit_reference_book_clear_settlement_scheme(book, instrument_id, out_error);
}

OpenPitReferenceBookStatus openpit_reference_book_get_settlement_scheme(const OpenPitReferenceBook * book, OpenPitInstrumentId instrument_id, OpenPitSettlementScheme * out_scheme, bool * out_is_set, OpenPitOutError out_error) {
    return _fn_openpit_reference_book_get_settlement_scheme(book, instrument_id, out_scheme, out_is_set, out_error);
}

void openpit_destroy_shared_string(OpenPitSharedString * handle) {
    _fn_openpit_destroy_shared_string(handle);
}

OpenPitStringView openpit_shared_string_view(const OpenPitSharedString * handle) {
    return _fn_openpit_shared_string_view(handle);
}
