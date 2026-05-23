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
 * Please see https://github.com/openpitkit and the OWNERS file for details.
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
static bool (*_fn_openpit_create_param_pnl_from_str)(OpenPitStringView, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_f64)(double, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_i64)(int64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_u64)(uint64_t, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_pnl_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_create_param_price_from_str)(OpenPitStringView, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_f64)(double, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_i64)(int64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_u64)(uint64_t, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_price_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_create_param_quantity_from_str)(OpenPitStringView, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_f64)(double, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_i64)(int64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_u64)(uint64_t, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_quantity_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_create_param_volume_from_str)(OpenPitStringView, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_f64)(double, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_i64)(int64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_u64)(uint64_t, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_volume_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_create_param_cash_flow_from_str)(OpenPitStringView, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_f64)(double, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_i64)(int64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_u64)(uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_cash_flow_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_create_param_position_size_from_str)(OpenPitStringView, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_f64)(double, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_i64)(int64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_u64)(uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_position_size_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_create_param_fee_from_str)(OpenPitStringView, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_f64)(double, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_i64)(int64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_u64)(uint64_t, OpenPitParamFee *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_fee_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_create_param_notional_from_str)(OpenPitStringView, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_f64)(double, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_i64)(int64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_u64)(uint64_t, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_create_param_notional_from_str_rounded)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError) = NULL;
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
static bool (*_fn_openpit_param_quantity_calculate_volume)(OpenPitParamQuantity, OpenPitParamPrice, OpenPitParamVolume *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_volume_calculate_quantity)(OpenPitParamVolume, OpenPitParamPrice, OpenPitParamQuantity *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_to_cash_flow)(OpenPitParamPnl, OpenPitParamCashFlow *, OpenPitOutParamError) = NULL;
static bool (*_fn_openpit_param_pnl_to_position_size)(OpenPitParamPnl, OpenPitParamPositionSize *, OpenPitOutParamError) = NULL;
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
static OpenPitParamAccountId (*_fn_openpit_create_param_account_id_from_u64)(uint64_t) = NULL;
static bool (*_fn_openpit_create_param_account_id_from_str)(OpenPitStringView, OpenPitParamAccountId *, OpenPitOutParamError) = NULL;
static OpenPitSharedString * (*_fn_openpit_create_param_asset_from_str)(OpenPitStringView, OpenPitOutParamError) = NULL;
static void (*_fn_openpit_destroy_param_asset)(OpenPitSharedString *) = NULL;
static OpenPitPretradeRejectList * (*_fn_openpit_pretrade_create_reject_list)(size_t) = NULL;
static void (*_fn_openpit_pretrade_destroy_reject_list)(OpenPitPretradeRejectList *) = NULL;
static void (*_fn_openpit_pretrade_reject_list_push)(OpenPitPretradeRejectList *, OpenPitPretradeReject) = NULL;
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
static OpenPitEngine * (*_fn_openpit_engine_builder_build)(OpenPitEngineBuilder *, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_engine)(OpenPitEngine *) = NULL;
static OpenPitPretradeStatus (*_fn_openpit_engine_start_pre_trade)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeRequest **, OpenPitPretradeRejectList **, OpenPitOutError) = NULL;
static OpenPitPretradeStatus (*_fn_openpit_engine_execute_pre_trade)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError) = NULL;
static OpenPitPretradeStatus (*_fn_openpit_pretrade_pre_trade_request_execute)(OpenPitPretradePreTradeRequest *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_request)(OpenPitPretradePreTradeRequest *) = NULL;
static void (*_fn_openpit_pretrade_pre_trade_reservation_commit)(OpenPitPretradePreTradeReservation *) = NULL;
static void (*_fn_openpit_pretrade_pre_trade_reservation_rollback)(OpenPitPretradePreTradeReservation *) = NULL;
static OpenPitPretradePreTradeLock (*_fn_openpit_pretrade_pre_trade_reservation_get_lock)(const OpenPitPretradePreTradeReservation *) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_reservation)(OpenPitPretradePreTradeReservation *) = NULL;
static bool (*_fn_openpit_engine_apply_execution_report)(OpenPitEngine *, const OpenPitExecutionReport *, OpenPitPretradeAccountBlockList **, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_account_adjustment_batch_error)(OpenPitAccountAdjustmentBatchError *) = NULL;
static size_t (*_fn_openpit_account_adjustment_batch_error_get_failed_adjustment_index)(const OpenPitAccountAdjustmentBatchError *) = NULL;
static const OpenPitPretradeRejectList * (*_fn_openpit_account_adjustment_batch_error_get_rejects)(const OpenPitAccountAdjustmentBatchError *) = NULL;
static OpenPitAccountAdjustmentApplyStatus (*_fn_openpit_engine_apply_account_adjustment)(OpenPitEngine *, OpenPitParamAccountId, const OpenPitAccountAdjustment *, size_t, OpenPitAccountAdjustmentBatchError **, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_order_validation_policy)(OpenPitEngineBuilder *, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_rate_limit_policy)(OpenPitEngineBuilder *, const OpenPitPretradePoliciesRateLimitBrokerBarrier *, const OpenPitPretradePoliciesRateLimitAssetBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_order_size_limit_policy)(OpenPitEngineBuilder *, const OpenPitPretradePoliciesOrderSizeBrokerBarrier *, const OpenPitPretradePoliciesOrderSizeAssetBarrier *, size_t, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier *, size_t, OpenPitOutError) = NULL;
static bool (*_fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy)(OpenPitEngineBuilder *, const OpenPitPretradePoliciesPnlBoundsBarrier *, size_t, const OpenPitPretradePoliciesPnlBoundsAccountBarrier *, size_t, OpenPitOutError) = NULL;
static void (*_fn_openpit_destroy_pretrade_pre_trade_policy)(OpenPitPretradePreTradePolicy *) = NULL;
static OpenPitStringView (*_fn_openpit_pretrade_pre_trade_policy_get_name)(const OpenPitPretradePreTradePolicy *) = NULL;
static bool (*_fn_openpit_engine_builder_add_pre_trade_policy)(OpenPitEngineBuilder *, OpenPitPretradePreTradePolicy *, OpenPitOutError) = NULL;
static bool (*_fn_openpit_mutations_push)(OpenPitMutations *, OpenPitMutationFn, OpenPitMutationFn, void *, OpenPitMutationFreeFn, OpenPitOutError) = NULL;
static OpenPitPretradePreTradePolicy * (*_fn_openpit_create_pretrade_custom_pre_trade_policy)(OpenPitStringView, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyApplyExecutionReportFn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn, OpenPitPretradePreTradePolicyFreeUserDataFn, void *, OpenPitOutError) = NULL;
static OpenPitStringView (*_fn_openpit_get_runtime_version)(void) = NULL;
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
    _fn_openpit_create_param_pnl_from_str = (bool (*)(OpenPitStringView, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_str");
    if (_fn_openpit_create_param_pnl_from_str == NULL) return "openpit_create_param_pnl_from_str";
    _fn_openpit_create_param_pnl_from_f64 = (bool (*)(double, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_f64");
    if (_fn_openpit_create_param_pnl_from_f64 == NULL) return "openpit_create_param_pnl_from_f64";
    _fn_openpit_create_param_pnl_from_i64 = (bool (*)(int64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_i64");
    if (_fn_openpit_create_param_pnl_from_i64 == NULL) return "openpit_create_param_pnl_from_i64";
    _fn_openpit_create_param_pnl_from_u64 = (bool (*)(uint64_t, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_u64");
    if (_fn_openpit_create_param_pnl_from_u64 == NULL) return "openpit_create_param_pnl_from_u64";
    _fn_openpit_create_param_pnl_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPnl *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_pnl_from_str_rounded");
    if (_fn_openpit_create_param_pnl_from_str_rounded == NULL) return "openpit_create_param_pnl_from_str_rounded";
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
    _fn_openpit_create_param_price_from_str = (bool (*)(OpenPitStringView, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_str");
    if (_fn_openpit_create_param_price_from_str == NULL) return "openpit_create_param_price_from_str";
    _fn_openpit_create_param_price_from_f64 = (bool (*)(double, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_f64");
    if (_fn_openpit_create_param_price_from_f64 == NULL) return "openpit_create_param_price_from_f64";
    _fn_openpit_create_param_price_from_i64 = (bool (*)(int64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_i64");
    if (_fn_openpit_create_param_price_from_i64 == NULL) return "openpit_create_param_price_from_i64";
    _fn_openpit_create_param_price_from_u64 = (bool (*)(uint64_t, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_u64");
    if (_fn_openpit_create_param_price_from_u64 == NULL) return "openpit_create_param_price_from_u64";
    _fn_openpit_create_param_price_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPrice *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_price_from_str_rounded");
    if (_fn_openpit_create_param_price_from_str_rounded == NULL) return "openpit_create_param_price_from_str_rounded";
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
    _fn_openpit_create_param_quantity_from_str = (bool (*)(OpenPitStringView, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_str");
    if (_fn_openpit_create_param_quantity_from_str == NULL) return "openpit_create_param_quantity_from_str";
    _fn_openpit_create_param_quantity_from_f64 = (bool (*)(double, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_f64");
    if (_fn_openpit_create_param_quantity_from_f64 == NULL) return "openpit_create_param_quantity_from_f64";
    _fn_openpit_create_param_quantity_from_i64 = (bool (*)(int64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_i64");
    if (_fn_openpit_create_param_quantity_from_i64 == NULL) return "openpit_create_param_quantity_from_i64";
    _fn_openpit_create_param_quantity_from_u64 = (bool (*)(uint64_t, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_u64");
    if (_fn_openpit_create_param_quantity_from_u64 == NULL) return "openpit_create_param_quantity_from_u64";
    _fn_openpit_create_param_quantity_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_quantity_from_str_rounded");
    if (_fn_openpit_create_param_quantity_from_str_rounded == NULL) return "openpit_create_param_quantity_from_str_rounded";
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
    _fn_openpit_create_param_volume_from_str = (bool (*)(OpenPitStringView, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_str");
    if (_fn_openpit_create_param_volume_from_str == NULL) return "openpit_create_param_volume_from_str";
    _fn_openpit_create_param_volume_from_f64 = (bool (*)(double, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_f64");
    if (_fn_openpit_create_param_volume_from_f64 == NULL) return "openpit_create_param_volume_from_f64";
    _fn_openpit_create_param_volume_from_i64 = (bool (*)(int64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_i64");
    if (_fn_openpit_create_param_volume_from_i64 == NULL) return "openpit_create_param_volume_from_i64";
    _fn_openpit_create_param_volume_from_u64 = (bool (*)(uint64_t, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_u64");
    if (_fn_openpit_create_param_volume_from_u64 == NULL) return "openpit_create_param_volume_from_u64";
    _fn_openpit_create_param_volume_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_volume_from_str_rounded");
    if (_fn_openpit_create_param_volume_from_str_rounded == NULL) return "openpit_create_param_volume_from_str_rounded";
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
    _fn_openpit_create_param_cash_flow_from_str = (bool (*)(OpenPitStringView, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_str");
    if (_fn_openpit_create_param_cash_flow_from_str == NULL) return "openpit_create_param_cash_flow_from_str";
    _fn_openpit_create_param_cash_flow_from_f64 = (bool (*)(double, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_f64");
    if (_fn_openpit_create_param_cash_flow_from_f64 == NULL) return "openpit_create_param_cash_flow_from_f64";
    _fn_openpit_create_param_cash_flow_from_i64 = (bool (*)(int64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_i64");
    if (_fn_openpit_create_param_cash_flow_from_i64 == NULL) return "openpit_create_param_cash_flow_from_i64";
    _fn_openpit_create_param_cash_flow_from_u64 = (bool (*)(uint64_t, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_u64");
    if (_fn_openpit_create_param_cash_flow_from_u64 == NULL) return "openpit_create_param_cash_flow_from_u64";
    _fn_openpit_create_param_cash_flow_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_cash_flow_from_str_rounded");
    if (_fn_openpit_create_param_cash_flow_from_str_rounded == NULL) return "openpit_create_param_cash_flow_from_str_rounded";
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
    _fn_openpit_create_param_position_size_from_str = (bool (*)(OpenPitStringView, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_str");
    if (_fn_openpit_create_param_position_size_from_str == NULL) return "openpit_create_param_position_size_from_str";
    _fn_openpit_create_param_position_size_from_f64 = (bool (*)(double, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_f64");
    if (_fn_openpit_create_param_position_size_from_f64 == NULL) return "openpit_create_param_position_size_from_f64";
    _fn_openpit_create_param_position_size_from_i64 = (bool (*)(int64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_i64");
    if (_fn_openpit_create_param_position_size_from_i64 == NULL) return "openpit_create_param_position_size_from_i64";
    _fn_openpit_create_param_position_size_from_u64 = (bool (*)(uint64_t, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_u64");
    if (_fn_openpit_create_param_position_size_from_u64 == NULL) return "openpit_create_param_position_size_from_u64";
    _fn_openpit_create_param_position_size_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_position_size_from_str_rounded");
    if (_fn_openpit_create_param_position_size_from_str_rounded == NULL) return "openpit_create_param_position_size_from_str_rounded";
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
    _fn_openpit_create_param_fee_from_str = (bool (*)(OpenPitStringView, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_str");
    if (_fn_openpit_create_param_fee_from_str == NULL) return "openpit_create_param_fee_from_str";
    _fn_openpit_create_param_fee_from_f64 = (bool (*)(double, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_f64");
    if (_fn_openpit_create_param_fee_from_f64 == NULL) return "openpit_create_param_fee_from_f64";
    _fn_openpit_create_param_fee_from_i64 = (bool (*)(int64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_i64");
    if (_fn_openpit_create_param_fee_from_i64 == NULL) return "openpit_create_param_fee_from_i64";
    _fn_openpit_create_param_fee_from_u64 = (bool (*)(uint64_t, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_u64");
    if (_fn_openpit_create_param_fee_from_u64 == NULL) return "openpit_create_param_fee_from_u64";
    _fn_openpit_create_param_fee_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamFee *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_fee_from_str_rounded");
    if (_fn_openpit_create_param_fee_from_str_rounded == NULL) return "openpit_create_param_fee_from_str_rounded";
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
    _fn_openpit_create_param_notional_from_str = (bool (*)(OpenPitStringView, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_str");
    if (_fn_openpit_create_param_notional_from_str == NULL) return "openpit_create_param_notional_from_str";
    _fn_openpit_create_param_notional_from_f64 = (bool (*)(double, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_f64");
    if (_fn_openpit_create_param_notional_from_f64 == NULL) return "openpit_create_param_notional_from_f64";
    _fn_openpit_create_param_notional_from_i64 = (bool (*)(int64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_i64");
    if (_fn_openpit_create_param_notional_from_i64 == NULL) return "openpit_create_param_notional_from_i64";
    _fn_openpit_create_param_notional_from_u64 = (bool (*)(uint64_t, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_u64");
    if (_fn_openpit_create_param_notional_from_u64 == NULL) return "openpit_create_param_notional_from_u64";
    _fn_openpit_create_param_notional_from_str_rounded = (bool (*)(OpenPitStringView, uint32_t, OpenPitParamRoundingStrategy, OpenPitParamNotional *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_notional_from_str_rounded");
    if (_fn_openpit_create_param_notional_from_str_rounded == NULL) return "openpit_create_param_notional_from_str_rounded";
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
    _fn_openpit_param_quantity_calculate_volume = (bool (*)(OpenPitParamQuantity, OpenPitParamPrice, OpenPitParamVolume *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_quantity_calculate_volume");
    if (_fn_openpit_param_quantity_calculate_volume == NULL) return "openpit_param_quantity_calculate_volume";
    _fn_openpit_param_volume_calculate_quantity = (bool (*)(OpenPitParamVolume, OpenPitParamPrice, OpenPitParamQuantity *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_volume_calculate_quantity");
    if (_fn_openpit_param_volume_calculate_quantity == NULL) return "openpit_param_volume_calculate_quantity";
    _fn_openpit_param_pnl_to_cash_flow = (bool (*)(OpenPitParamPnl, OpenPitParamCashFlow *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_to_cash_flow");
    if (_fn_openpit_param_pnl_to_cash_flow == NULL) return "openpit_param_pnl_to_cash_flow";
    _fn_openpit_param_pnl_to_position_size = (bool (*)(OpenPitParamPnl, OpenPitParamPositionSize *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_param_pnl_to_position_size");
    if (_fn_openpit_param_pnl_to_position_size == NULL) return "openpit_param_pnl_to_position_size";
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
    _fn_openpit_create_param_account_id_from_u64 = (OpenPitParamAccountId (*)(uint64_t))openpit_dlsym(handle, "openpit_create_param_account_id_from_u64");
    if (_fn_openpit_create_param_account_id_from_u64 == NULL) return "openpit_create_param_account_id_from_u64";
    _fn_openpit_create_param_account_id_from_str = (bool (*)(OpenPitStringView, OpenPitParamAccountId *, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_account_id_from_str");
    if (_fn_openpit_create_param_account_id_from_str == NULL) return "openpit_create_param_account_id_from_str";
    _fn_openpit_create_param_asset_from_str = (OpenPitSharedString * (*)(OpenPitStringView, OpenPitOutParamError))openpit_dlsym(handle, "openpit_create_param_asset_from_str");
    if (_fn_openpit_create_param_asset_from_str == NULL) return "openpit_create_param_asset_from_str";
    _fn_openpit_destroy_param_asset = (void (*)(OpenPitSharedString *))openpit_dlsym(handle, "openpit_destroy_param_asset");
    if (_fn_openpit_destroy_param_asset == NULL) return "openpit_destroy_param_asset";
    _fn_openpit_pretrade_create_reject_list = (OpenPitPretradeRejectList * (*)(size_t))openpit_dlsym(handle, "openpit_pretrade_create_reject_list");
    if (_fn_openpit_pretrade_create_reject_list == NULL) return "openpit_pretrade_create_reject_list";
    _fn_openpit_pretrade_destroy_reject_list = (void (*)(OpenPitPretradeRejectList *))openpit_dlsym(handle, "openpit_pretrade_destroy_reject_list");
    if (_fn_openpit_pretrade_destroy_reject_list == NULL) return "openpit_pretrade_destroy_reject_list";
    _fn_openpit_pretrade_reject_list_push = (void (*)(OpenPitPretradeRejectList *, OpenPitPretradeReject))openpit_dlsym(handle, "openpit_pretrade_reject_list_push");
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
    _fn_openpit_engine_builder_build = (OpenPitEngine * (*)(OpenPitEngineBuilder *, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_build");
    if (_fn_openpit_engine_builder_build == NULL) return "openpit_engine_builder_build";
    _fn_openpit_destroy_engine = (void (*)(OpenPitEngine *))openpit_dlsym(handle, "openpit_destroy_engine");
    if (_fn_openpit_destroy_engine == NULL) return "openpit_destroy_engine";
    _fn_openpit_engine_start_pre_trade = (OpenPitPretradeStatus (*)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeRequest **, OpenPitPretradeRejectList **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_start_pre_trade");
    if (_fn_openpit_engine_start_pre_trade == NULL) return "openpit_engine_start_pre_trade";
    _fn_openpit_engine_execute_pre_trade = (OpenPitPretradeStatus (*)(OpenPitEngine *, const OpenPitOrder *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_execute_pre_trade");
    if (_fn_openpit_engine_execute_pre_trade == NULL) return "openpit_engine_execute_pre_trade";
    _fn_openpit_pretrade_pre_trade_request_execute = (OpenPitPretradeStatus (*)(OpenPitPretradePreTradeRequest *, OpenPitPretradePreTradeReservation **, OpenPitPretradeRejectList **, OpenPitOutError))openpit_dlsym(handle, "openpit_pretrade_pre_trade_request_execute");
    if (_fn_openpit_pretrade_pre_trade_request_execute == NULL) return "openpit_pretrade_pre_trade_request_execute";
    _fn_openpit_destroy_pretrade_pre_trade_request = (void (*)(OpenPitPretradePreTradeRequest *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_request");
    if (_fn_openpit_destroy_pretrade_pre_trade_request == NULL) return "openpit_destroy_pretrade_pre_trade_request";
    _fn_openpit_pretrade_pre_trade_reservation_commit = (void (*)(OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_commit");
    if (_fn_openpit_pretrade_pre_trade_reservation_commit == NULL) return "openpit_pretrade_pre_trade_reservation_commit";
    _fn_openpit_pretrade_pre_trade_reservation_rollback = (void (*)(OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_rollback");
    if (_fn_openpit_pretrade_pre_trade_reservation_rollback == NULL) return "openpit_pretrade_pre_trade_reservation_rollback";
    _fn_openpit_pretrade_pre_trade_reservation_get_lock = (OpenPitPretradePreTradeLock (*)(const OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_reservation_get_lock");
    if (_fn_openpit_pretrade_pre_trade_reservation_get_lock == NULL) return "openpit_pretrade_pre_trade_reservation_get_lock";
    _fn_openpit_destroy_pretrade_pre_trade_reservation = (void (*)(OpenPitPretradePreTradeReservation *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_reservation");
    if (_fn_openpit_destroy_pretrade_pre_trade_reservation == NULL) return "openpit_destroy_pretrade_pre_trade_reservation";
    _fn_openpit_engine_apply_execution_report = (bool (*)(OpenPitEngine *, const OpenPitExecutionReport *, OpenPitPretradeAccountBlockList **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_apply_execution_report");
    if (_fn_openpit_engine_apply_execution_report == NULL) return "openpit_engine_apply_execution_report";
    _fn_openpit_destroy_account_adjustment_batch_error = (void (*)(OpenPitAccountAdjustmentBatchError *))openpit_dlsym(handle, "openpit_destroy_account_adjustment_batch_error");
    if (_fn_openpit_destroy_account_adjustment_batch_error == NULL) return "openpit_destroy_account_adjustment_batch_error";
    _fn_openpit_account_adjustment_batch_error_get_failed_adjustment_index = (size_t (*)(const OpenPitAccountAdjustmentBatchError *))openpit_dlsym(handle, "openpit_account_adjustment_batch_error_get_failed_adjustment_index");
    if (_fn_openpit_account_adjustment_batch_error_get_failed_adjustment_index == NULL) return "openpit_account_adjustment_batch_error_get_failed_adjustment_index";
    _fn_openpit_account_adjustment_batch_error_get_rejects = (const OpenPitPretradeRejectList * (*)(const OpenPitAccountAdjustmentBatchError *))openpit_dlsym(handle, "openpit_account_adjustment_batch_error_get_rejects");
    if (_fn_openpit_account_adjustment_batch_error_get_rejects == NULL) return "openpit_account_adjustment_batch_error_get_rejects";
    _fn_openpit_engine_apply_account_adjustment = (OpenPitAccountAdjustmentApplyStatus (*)(OpenPitEngine *, OpenPitParamAccountId, const OpenPitAccountAdjustment *, size_t, OpenPitAccountAdjustmentBatchError **, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_apply_account_adjustment");
    if (_fn_openpit_engine_apply_account_adjustment == NULL) return "openpit_engine_apply_account_adjustment";
    _fn_openpit_engine_builder_add_builtin_order_validation_policy = (bool (*)(OpenPitEngineBuilder *, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_order_validation_policy");
    if (_fn_openpit_engine_builder_add_builtin_order_validation_policy == NULL) return "openpit_engine_builder_add_builtin_order_validation_policy";
    _fn_openpit_engine_builder_add_builtin_rate_limit_policy = (bool (*)(OpenPitEngineBuilder *, const OpenPitPretradePoliciesRateLimitBrokerBarrier *, const OpenPitPretradePoliciesRateLimitAssetBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountBarrier *, size_t, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_rate_limit_policy");
    if (_fn_openpit_engine_builder_add_builtin_rate_limit_policy == NULL) return "openpit_engine_builder_add_builtin_rate_limit_policy";
    _fn_openpit_engine_builder_add_builtin_order_size_limit_policy = (bool (*)(OpenPitEngineBuilder *, const OpenPitPretradePoliciesOrderSizeBrokerBarrier *, const OpenPitPretradePoliciesOrderSizeAssetBarrier *, size_t, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_order_size_limit_policy");
    if (_fn_openpit_engine_builder_add_builtin_order_size_limit_policy == NULL) return "openpit_engine_builder_add_builtin_order_size_limit_policy";
    _fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy = (bool (*)(OpenPitEngineBuilder *, const OpenPitPretradePoliciesPnlBoundsBarrier *, size_t, const OpenPitPretradePoliciesPnlBoundsAccountBarrier *, size_t, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy");
    if (_fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy == NULL) return "openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy";
    _fn_openpit_destroy_pretrade_pre_trade_policy = (void (*)(OpenPitPretradePreTradePolicy *))openpit_dlsym(handle, "openpit_destroy_pretrade_pre_trade_policy");
    if (_fn_openpit_destroy_pretrade_pre_trade_policy == NULL) return "openpit_destroy_pretrade_pre_trade_policy";
    _fn_openpit_pretrade_pre_trade_policy_get_name = (OpenPitStringView (*)(const OpenPitPretradePreTradePolicy *))openpit_dlsym(handle, "openpit_pretrade_pre_trade_policy_get_name");
    if (_fn_openpit_pretrade_pre_trade_policy_get_name == NULL) return "openpit_pretrade_pre_trade_policy_get_name";
    _fn_openpit_engine_builder_add_pre_trade_policy = (bool (*)(OpenPitEngineBuilder *, OpenPitPretradePreTradePolicy *, OpenPitOutError))openpit_dlsym(handle, "openpit_engine_builder_add_pre_trade_policy");
    if (_fn_openpit_engine_builder_add_pre_trade_policy == NULL) return "openpit_engine_builder_add_pre_trade_policy";
    _fn_openpit_mutations_push = (bool (*)(OpenPitMutations *, OpenPitMutationFn, OpenPitMutationFn, void *, OpenPitMutationFreeFn, OpenPitOutError))openpit_dlsym(handle, "openpit_mutations_push");
    if (_fn_openpit_mutations_push == NULL) return "openpit_mutations_push";
    _fn_openpit_create_pretrade_custom_pre_trade_policy = (OpenPitPretradePreTradePolicy * (*)(OpenPitStringView, OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn, OpenPitPretradePreTradePolicyApplyExecutionReportFn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn, OpenPitPretradePreTradePolicyFreeUserDataFn, void *, OpenPitOutError))openpit_dlsym(handle, "openpit_create_pretrade_custom_pre_trade_policy");
    if (_fn_openpit_create_pretrade_custom_pre_trade_policy == NULL) return "openpit_create_pretrade_custom_pre_trade_policy";
    _fn_openpit_get_runtime_version = (OpenPitStringView (*)(void))openpit_dlsym(handle, "openpit_get_runtime_version");
    if (_fn_openpit_get_runtime_version == NULL) return "openpit_get_runtime_version";
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

bool openpit_create_param_pnl_from_str(OpenPitStringView value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_str(value, out, out_error);
}

bool openpit_create_param_pnl_from_f64(double value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_f64(value, out, out_error);
}

bool openpit_create_param_pnl_from_i64(int64_t value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_i64(value, out, out_error);
}

bool openpit_create_param_pnl_from_u64(uint64_t value, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_u64(value, out, out_error);
}

bool openpit_create_param_pnl_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPnl * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_pnl_from_str_rounded(value, scale, rounding, out, out_error);
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

bool openpit_create_param_price_from_str(OpenPitStringView value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_str(value, out, out_error);
}

bool openpit_create_param_price_from_f64(double value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_f64(value, out, out_error);
}

bool openpit_create_param_price_from_i64(int64_t value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_i64(value, out, out_error);
}

bool openpit_create_param_price_from_u64(uint64_t value, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_u64(value, out, out_error);
}

bool openpit_create_param_price_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPrice * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_price_from_str_rounded(value, scale, rounding, out, out_error);
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

bool openpit_create_param_quantity_from_str(OpenPitStringView value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_str(value, out, out_error);
}

bool openpit_create_param_quantity_from_f64(double value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_f64(value, out, out_error);
}

bool openpit_create_param_quantity_from_i64(int64_t value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_i64(value, out, out_error);
}

bool openpit_create_param_quantity_from_u64(uint64_t value, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_u64(value, out, out_error);
}

bool openpit_create_param_quantity_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamQuantity * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_quantity_from_str_rounded(value, scale, rounding, out, out_error);
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

bool openpit_create_param_volume_from_str(OpenPitStringView value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_str(value, out, out_error);
}

bool openpit_create_param_volume_from_f64(double value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_f64(value, out, out_error);
}

bool openpit_create_param_volume_from_i64(int64_t value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_i64(value, out, out_error);
}

bool openpit_create_param_volume_from_u64(uint64_t value, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_u64(value, out, out_error);
}

bool openpit_create_param_volume_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamVolume * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_volume_from_str_rounded(value, scale, rounding, out, out_error);
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

bool openpit_create_param_cash_flow_from_str(OpenPitStringView value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_str(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_f64(double value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_f64(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_i64(int64_t value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_i64(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_u64(uint64_t value, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_u64(value, out, out_error);
}

bool openpit_create_param_cash_flow_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamCashFlow * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_cash_flow_from_str_rounded(value, scale, rounding, out, out_error);
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

bool openpit_create_param_position_size_from_str(OpenPitStringView value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_str(value, out, out_error);
}

bool openpit_create_param_position_size_from_f64(double value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_f64(value, out, out_error);
}

bool openpit_create_param_position_size_from_i64(int64_t value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_i64(value, out, out_error);
}

bool openpit_create_param_position_size_from_u64(uint64_t value, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_u64(value, out, out_error);
}

bool openpit_create_param_position_size_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamPositionSize * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_position_size_from_str_rounded(value, scale, rounding, out, out_error);
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

bool openpit_create_param_fee_from_str(OpenPitStringView value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_str(value, out, out_error);
}

bool openpit_create_param_fee_from_f64(double value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_f64(value, out, out_error);
}

bool openpit_create_param_fee_from_i64(int64_t value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_i64(value, out, out_error);
}

bool openpit_create_param_fee_from_u64(uint64_t value, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_u64(value, out, out_error);
}

bool openpit_create_param_fee_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamFee * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_fee_from_str_rounded(value, scale, rounding, out, out_error);
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

bool openpit_create_param_notional_from_str(OpenPitStringView value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_str(value, out, out_error);
}

bool openpit_create_param_notional_from_f64(double value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_f64(value, out, out_error);
}

bool openpit_create_param_notional_from_i64(int64_t value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_i64(value, out, out_error);
}

bool openpit_create_param_notional_from_u64(uint64_t value, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_u64(value, out, out_error);
}

bool openpit_create_param_notional_from_str_rounded(OpenPitStringView value, uint32_t scale, OpenPitParamRoundingStrategy rounding, OpenPitParamNotional * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_notional_from_str_rounded(value, scale, rounding, out, out_error);
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

OpenPitParamAccountId openpit_create_param_account_id_from_u64(uint64_t value) {
    return _fn_openpit_create_param_account_id_from_u64(value);
}

bool openpit_create_param_account_id_from_str(OpenPitStringView value, OpenPitParamAccountId * out, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_account_id_from_str(value, out, out_error);
}

OpenPitSharedString * openpit_create_param_asset_from_str(OpenPitStringView value, OpenPitOutParamError out_error) {
    return _fn_openpit_create_param_asset_from_str(value, out_error);
}

void openpit_destroy_param_asset(OpenPitSharedString * handle) {
    _fn_openpit_destroy_param_asset(handle);
}

OpenPitPretradeRejectList * openpit_pretrade_create_reject_list(size_t reserve) {
    return _fn_openpit_pretrade_create_reject_list(reserve);
}

void openpit_pretrade_destroy_reject_list(OpenPitPretradeRejectList * rejects) {
    _fn_openpit_pretrade_destroy_reject_list(rejects);
}

void openpit_pretrade_reject_list_push(OpenPitPretradeRejectList * list, OpenPitPretradeReject reject) {
    _fn_openpit_pretrade_reject_list_push(list, reject);
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

OpenPitEngine * openpit_engine_builder_build(OpenPitEngineBuilder * builder, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_build(builder, out_error);
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

OpenPitPretradePreTradeLock openpit_pretrade_pre_trade_reservation_get_lock(const OpenPitPretradePreTradeReservation * reservation) {
    return _fn_openpit_pretrade_pre_trade_reservation_get_lock(reservation);
}

void openpit_destroy_pretrade_pre_trade_reservation(OpenPitPretradePreTradeReservation * reservation) {
    _fn_openpit_destroy_pretrade_pre_trade_reservation(reservation);
}

bool openpit_engine_apply_execution_report(OpenPitEngine * engine, const OpenPitExecutionReport * report, OpenPitPretradeAccountBlockList ** out_blocks, OpenPitOutError out_error) {
    return _fn_openpit_engine_apply_execution_report(engine, report, out_blocks, out_error);
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

OpenPitAccountAdjustmentApplyStatus openpit_engine_apply_account_adjustment(OpenPitEngine * engine, OpenPitParamAccountId account_id, const OpenPitAccountAdjustment * adjustments, size_t adjustments_len, OpenPitAccountAdjustmentBatchError ** out_reject, OpenPitOutError out_error) {
    return _fn_openpit_engine_apply_account_adjustment(engine, account_id, adjustments, adjustments_len, out_reject, out_error);
}

bool openpit_engine_builder_add_builtin_order_validation_policy(OpenPitEngineBuilder * builder, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_order_validation_policy(builder, out_error);
}

bool openpit_engine_builder_add_builtin_rate_limit_policy(OpenPitEngineBuilder * builder, const OpenPitPretradePoliciesRateLimitBrokerBarrier * broker, const OpenPitPretradePoliciesRateLimitAssetBarrier * asset, size_t asset_len, const OpenPitPretradePoliciesRateLimitAccountBarrier * account, size_t account_len, const OpenPitPretradePoliciesRateLimitAccountAssetBarrier * account_asset, size_t account_asset_len, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_rate_limit_policy(builder, broker, asset, asset_len, account, account_len, account_asset, account_asset_len, out_error);
}

bool openpit_engine_builder_add_builtin_order_size_limit_policy(OpenPitEngineBuilder * builder, const OpenPitPretradePoliciesOrderSizeBrokerBarrier * broker, const OpenPitPretradePoliciesOrderSizeAssetBarrier * asset, size_t asset_len, const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier * account_asset, size_t account_asset_len, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_order_size_limit_policy(builder, broker, asset, asset_len, account_asset, account_asset_len, out_error);
}

bool openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(OpenPitEngineBuilder * builder, const OpenPitPretradePoliciesPnlBoundsBarrier * broker, size_t broker_len, const OpenPitPretradePoliciesPnlBoundsAccountBarrier * account, size_t account_len, OpenPitOutError out_error) {
    return _fn_openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(builder, broker, broker_len, account, account_len, out_error);
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

OpenPitPretradePreTradePolicy * openpit_create_pretrade_custom_pre_trade_policy(OpenPitStringView name, OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_fn, OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_fn, OpenPitPretradePreTradePolicyApplyExecutionReportFn apply_execution_report_fn, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn apply_account_adjustment_fn, OpenPitPretradePreTradePolicyFreeUserDataFn free_user_data_fn, void * user_data, OpenPitOutError out_error) {
    return _fn_openpit_create_pretrade_custom_pre_trade_policy(name, check_pre_trade_start_fn, perform_pre_trade_check_fn, apply_execution_report_fn, apply_account_adjustment_fn, free_user_data_fn, user_data, out_error);
}

OpenPitStringView openpit_get_runtime_version(void) {
    return _fn_openpit_get_runtime_version();
}

void openpit_destroy_shared_string(OpenPitSharedString * handle) {
    _fn_openpit_destroy_shared_string(handle);
}

OpenPitStringView openpit_shared_string_view(const OpenPitSharedString * handle) {
    return _fn_openpit_shared_string_view(handle);
}
