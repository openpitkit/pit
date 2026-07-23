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

#pragma once

#include <openpit.h>

#include <string_view>

namespace openpit::param::detail {

using RawAccountGroupId = ::OpenPitParamAccountGroupId;
using RawAccountId = ::OpenPitParamAccountId;
using RawAdjustmentAmount = ::OpenPitParamAdjustmentAmount;
using RawAsset = ::OpenPitStringView;
using RawCashFlow = ::OpenPitParamCashFlow;
using RawFee = ::OpenPitParamFee;
using RawLeverage = ::OpenPitParamLeverage;
using RawMonetaryAmount = ::OpenPitParamMonetaryAmount;
using RawNotional = ::OpenPitParamNotional;
using RawPnl = ::OpenPitParamPnl;
using RawPositionSize = ::OpenPitParamPositionSize;
using RawPrice = ::OpenPitParamPrice;
using RawQuantity = ::OpenPitParamQuantity;
using RawVolume = ::OpenPitParamVolume;
using RawSharedString = ::OpenPitSharedString;

using RawCashFlowOptional = ::OpenPitParamCashFlowOptional;
using RawFeeOptional = ::OpenPitParamFeeOptional;
using RawMonetaryAmountOptional = ::OpenPitParamMonetaryAmountOptional;
using RawNotionalOptional = ::OpenPitParamNotionalOptional;
using RawPnlOptional = ::OpenPitParamPnlOptional;
using RawPositionSizeOptional = ::OpenPitParamPositionSizeOptional;
using RawPriceOptional = ::OpenPitParamPriceOptional;
using RawQuantityOptional = ::OpenPitParamQuantityOptional;
using RawVolumeOptional = ::OpenPitParamVolumeOptional;

template <typename NativeType, auto CreateFromDecimal, auto CreateFromString,
          auto CreateFromSigned, auto CreateFromUnsigned, auto CreateFromFloat,
          auto CreateStringRounded, auto CreateFloatRounded,
          auto CreateDecimalRounded, auto ToFloat, auto IsZero, auto Compare,
          auto Add, auto Subtract, auto ToString, auto GetDecimal>
struct ValueTraits {
  using Native = NativeType;

  static constexpr auto FromDecimal = CreateFromDecimal;
  static constexpr auto FromString = CreateFromString;
  static constexpr auto FromInt64 = CreateFromSigned;
  static constexpr auto FromUint64 = CreateFromUnsigned;
  static constexpr auto FromDouble = CreateFromFloat;
  static constexpr auto FromStringRounded = CreateStringRounded;
  static constexpr auto FromDoubleRounded = CreateFloatRounded;
  static constexpr auto FromDecimalRounded = CreateDecimalRounded;
  static constexpr auto ToDouble = ToFloat;
  static constexpr auto Zero = IsZero;
  static constexpr auto CompareValues = Compare;
  static constexpr auto CheckedAdd = Add;
  static constexpr auto CheckedSubtract = Subtract;
  static constexpr auto String = ToString;
  static constexpr auto Decimal = GetDecimal;
};

template <auto MultiplyInt64, auto MultiplyUint64, auto MultiplyFloat,
          auto DivideInt64, auto DivideUint64, auto DivideFloat,
          auto RemainderInt64, auto RemainderUint64, auto RemainderFloat>
struct ArithmeticTraits {
  static constexpr auto CheckedMulInt = MultiplyInt64;
  static constexpr auto CheckedMulUint = MultiplyUint64;
  static constexpr auto CheckedMulFloat = MultiplyFloat;
  static constexpr auto CheckedDivInt = DivideInt64;
  static constexpr auto CheckedDivUint = DivideUint64;
  static constexpr auto CheckedDivFloat = DivideFloat;
  static constexpr auto CheckedRemInt = RemainderInt64;
  static constexpr auto CheckedRemUint = RemainderUint64;
  static constexpr auto CheckedRemFloat = RemainderFloat;
};

struct PriceTraits
    : ValueTraits<
          RawPrice, ::openpit_create_param_price,
          ::openpit_create_param_price_from_string,
          ::openpit_create_param_price_from_int64,
          ::openpit_create_param_price_from_uint64,
          ::openpit_create_param_price_from_f64,
          ::openpit_create_param_price_from_string_rounded,
          ::openpit_create_param_price_from_f64_rounded,
          ::openpit_create_param_price_from_decimal_rounded,
          ::openpit_param_price_to_f64, ::openpit_param_price_is_zero,
          ::openpit_param_price_compare, ::openpit_param_price_checked_add,
          ::openpit_param_price_checked_sub, ::openpit_param_price_to_string,
          ::openpit_param_price_get_decimal>,
      ArithmeticTraits<::openpit_param_price_checked_mul_i64,
                       ::openpit_param_price_checked_mul_u64,
                       ::openpit_param_price_checked_mul_f64,
                       ::openpit_param_price_checked_div_i64,
                       ::openpit_param_price_checked_div_u64,
                       ::openpit_param_price_checked_div_f64,
                       ::openpit_param_price_checked_rem_i64,
                       ::openpit_param_price_checked_rem_u64,
                       ::openpit_param_price_checked_rem_f64> {
  static constexpr std::string_view Name = "price";
  static constexpr auto CheckedNeg = ::openpit_param_price_checked_neg;
};

struct QuantityTraits
    : ValueTraits<RawQuantity, ::openpit_create_param_quantity,
                  ::openpit_create_param_quantity_from_string,
                  ::openpit_create_param_quantity_from_int64,
                  ::openpit_create_param_quantity_from_uint64,
                  ::openpit_create_param_quantity_from_f64,
                  ::openpit_create_param_quantity_from_string_rounded,
                  ::openpit_create_param_quantity_from_f64_rounded,
                  ::openpit_create_param_quantity_from_decimal_rounded,
                  ::openpit_param_quantity_to_f64,
                  ::openpit_param_quantity_is_zero,
                  ::openpit_param_quantity_compare,
                  ::openpit_param_quantity_checked_add,
                  ::openpit_param_quantity_checked_sub,
                  ::openpit_param_quantity_to_string,
                  ::openpit_param_quantity_get_decimal>,
      ArithmeticTraits<::openpit_param_quantity_checked_mul_i64,
                       ::openpit_param_quantity_checked_mul_u64,
                       ::openpit_param_quantity_checked_mul_f64,
                       ::openpit_param_quantity_checked_div_i64,
                       ::openpit_param_quantity_checked_div_u64,
                       ::openpit_param_quantity_checked_div_f64,
                       ::openpit_param_quantity_checked_rem_i64,
                       ::openpit_param_quantity_checked_rem_u64,
                       ::openpit_param_quantity_checked_rem_f64> {
  static constexpr std::string_view Name = "quantity";
};

struct VolumeTraits
    : ValueTraits<
          RawVolume, ::openpit_create_param_volume,
          ::openpit_create_param_volume_from_string,
          ::openpit_create_param_volume_from_int64,
          ::openpit_create_param_volume_from_uint64,
          ::openpit_create_param_volume_from_f64,
          ::openpit_create_param_volume_from_string_rounded,
          ::openpit_create_param_volume_from_f64_rounded,
          ::openpit_create_param_volume_from_decimal_rounded,
          ::openpit_param_volume_to_f64, ::openpit_param_volume_is_zero,
          ::openpit_param_volume_compare, ::openpit_param_volume_checked_add,
          ::openpit_param_volume_checked_sub, ::openpit_param_volume_to_string,
          ::openpit_param_volume_get_decimal>,
      ArithmeticTraits<::openpit_param_volume_checked_mul_i64,
                       ::openpit_param_volume_checked_mul_u64,
                       ::openpit_param_volume_checked_mul_f64,
                       ::openpit_param_volume_checked_div_i64,
                       ::openpit_param_volume_checked_div_u64,
                       ::openpit_param_volume_checked_div_f64,
                       ::openpit_param_volume_checked_rem_i64,
                       ::openpit_param_volume_checked_rem_u64,
                       ::openpit_param_volume_checked_rem_f64> {
  static constexpr std::string_view Name = "volume";
};

struct PnlTraits
    : ValueTraits<RawPnl, ::openpit_create_param_pnl,
                  ::openpit_create_param_pnl_from_string,
                  ::openpit_create_param_pnl_from_int64,
                  ::openpit_create_param_pnl_from_uint64,
                  ::openpit_create_param_pnl_from_f64,
                  ::openpit_create_param_pnl_from_string_rounded,
                  ::openpit_create_param_pnl_from_f64_rounded,
                  ::openpit_create_param_pnl_from_decimal_rounded,
                  ::openpit_param_pnl_to_f64, ::openpit_param_pnl_is_zero,
                  ::openpit_param_pnl_compare, ::openpit_param_pnl_checked_add,
                  ::openpit_param_pnl_checked_sub,
                  ::openpit_param_pnl_to_string,
                  ::openpit_param_pnl_get_decimal>,
      ArithmeticTraits<::openpit_param_pnl_checked_mul_i64,
                       ::openpit_param_pnl_checked_mul_u64,
                       ::openpit_param_pnl_checked_mul_f64,
                       ::openpit_param_pnl_checked_div_i64,
                       ::openpit_param_pnl_checked_div_u64,
                       ::openpit_param_pnl_checked_div_f64,
                       ::openpit_param_pnl_checked_rem_i64,
                       ::openpit_param_pnl_checked_rem_u64,
                       ::openpit_param_pnl_checked_rem_f64> {
  static constexpr std::string_view Name = "pnl";
  static constexpr auto CheckedNeg = ::openpit_param_pnl_checked_neg;
};

struct FeeTraits
    : ValueTraits<RawFee, ::openpit_create_param_fee,
                  ::openpit_create_param_fee_from_string,
                  ::openpit_create_param_fee_from_int64,
                  ::openpit_create_param_fee_from_uint64,
                  ::openpit_create_param_fee_from_f64,
                  ::openpit_create_param_fee_from_string_rounded,
                  ::openpit_create_param_fee_from_f64_rounded,
                  ::openpit_create_param_fee_from_decimal_rounded,
                  ::openpit_param_fee_to_f64, ::openpit_param_fee_is_zero,
                  ::openpit_param_fee_compare, ::openpit_param_fee_checked_add,
                  ::openpit_param_fee_checked_sub,
                  ::openpit_param_fee_to_string,
                  ::openpit_param_fee_get_decimal>,
      ArithmeticTraits<::openpit_param_fee_checked_mul_i64,
                       ::openpit_param_fee_checked_mul_u64,
                       ::openpit_param_fee_checked_mul_f64,
                       ::openpit_param_fee_checked_div_i64,
                       ::openpit_param_fee_checked_div_u64,
                       ::openpit_param_fee_checked_div_f64,
                       ::openpit_param_fee_checked_rem_i64,
                       ::openpit_param_fee_checked_rem_u64,
                       ::openpit_param_fee_checked_rem_f64> {
  static constexpr std::string_view Name = "fee";
  static constexpr auto CheckedNeg = ::openpit_param_fee_checked_neg;
};

struct PositionSizeTraits
    : ValueTraits<RawPositionSize, ::openpit_create_param_position_size,
                  ::openpit_create_param_position_size_from_string,
                  ::openpit_create_param_position_size_from_int64,
                  ::openpit_create_param_position_size_from_uint64,
                  ::openpit_create_param_position_size_from_f64,
                  ::openpit_create_param_position_size_from_string_rounded,
                  ::openpit_create_param_position_size_from_f64_rounded,
                  ::openpit_create_param_position_size_from_decimal_rounded,
                  ::openpit_param_position_size_to_f64,
                  ::openpit_param_position_size_is_zero,
                  ::openpit_param_position_size_compare,
                  ::openpit_param_position_size_checked_add,
                  ::openpit_param_position_size_checked_sub,
                  ::openpit_param_position_size_to_string,
                  ::openpit_param_position_size_get_decimal>,
      ArithmeticTraits<::openpit_param_position_size_checked_mul_i64,
                       ::openpit_param_position_size_checked_mul_u64,
                       ::openpit_param_position_size_checked_mul_f64,
                       ::openpit_param_position_size_checked_div_i64,
                       ::openpit_param_position_size_checked_div_u64,
                       ::openpit_param_position_size_checked_div_f64,
                       ::openpit_param_position_size_checked_rem_i64,
                       ::openpit_param_position_size_checked_rem_u64,
                       ::openpit_param_position_size_checked_rem_f64> {
  static constexpr std::string_view Name = "position size";
  static constexpr auto CheckedNeg = ::openpit_param_position_size_checked_neg;
};

struct CashFlowTraits
    : ValueTraits<RawCashFlow, ::openpit_create_param_cash_flow,
                  ::openpit_create_param_cash_flow_from_string,
                  ::openpit_create_param_cash_flow_from_int64,
                  ::openpit_create_param_cash_flow_from_uint64,
                  ::openpit_create_param_cash_flow_from_f64,
                  ::openpit_create_param_cash_flow_from_string_rounded,
                  ::openpit_create_param_cash_flow_from_f64_rounded,
                  ::openpit_create_param_cash_flow_from_decimal_rounded,
                  ::openpit_param_cash_flow_to_f64,
                  ::openpit_param_cash_flow_is_zero,
                  ::openpit_param_cash_flow_compare,
                  ::openpit_param_cash_flow_checked_add,
                  ::openpit_param_cash_flow_checked_sub,
                  ::openpit_param_cash_flow_to_string,
                  ::openpit_param_cash_flow_get_decimal>,
      ArithmeticTraits<::openpit_param_cash_flow_checked_mul_i64,
                       ::openpit_param_cash_flow_checked_mul_u64,
                       ::openpit_param_cash_flow_checked_mul_f64,
                       ::openpit_param_cash_flow_checked_div_i64,
                       ::openpit_param_cash_flow_checked_div_u64,
                       ::openpit_param_cash_flow_checked_div_f64,
                       ::openpit_param_cash_flow_checked_rem_i64,
                       ::openpit_param_cash_flow_checked_rem_u64,
                       ::openpit_param_cash_flow_checked_rem_f64> {
  static constexpr std::string_view Name = "cash flow";
  static constexpr auto CheckedNeg = ::openpit_param_cash_flow_checked_neg;
};

struct NotionalTraits
    : ValueTraits<RawNotional, ::openpit_create_param_notional,
                  ::openpit_create_param_notional_from_string,
                  ::openpit_create_param_notional_from_int64,
                  ::openpit_create_param_notional_from_uint64,
                  ::openpit_create_param_notional_from_f64,
                  ::openpit_create_param_notional_from_string_rounded,
                  ::openpit_create_param_notional_from_f64_rounded,
                  ::openpit_create_param_notional_from_decimal_rounded,
                  ::openpit_param_notional_to_f64,
                  ::openpit_param_notional_is_zero,
                  ::openpit_param_notional_compare,
                  ::openpit_param_notional_checked_add,
                  ::openpit_param_notional_checked_sub,
                  ::openpit_param_notional_to_string,
                  ::openpit_param_notional_get_decimal>,
      ArithmeticTraits<::openpit_param_notional_checked_mul_i64,
                       ::openpit_param_notional_checked_mul_u64,
                       ::openpit_param_notional_checked_mul_f64,
                       ::openpit_param_notional_checked_div_i64,
                       ::openpit_param_notional_checked_div_u64,
                       ::openpit_param_notional_checked_div_f64,
                       ::openpit_param_notional_checked_rem_i64,
                       ::openpit_param_notional_checked_rem_u64,
                       ::openpit_param_notional_checked_rem_f64> {
  static constexpr std::string_view Name = "notional";
};

}  // namespace openpit::param::detail
