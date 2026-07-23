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

#include "openpit/accountadjustment/pnl.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/param.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <optional>
#include <string>
#include <utility>
#include <variant>

namespace openpit::accountadjustment {

//------------------------------------------------------------------------------
// BalanceOperation

/// Balance-operation payload of an account adjustment.
///
/// Its asset identifies the holdings slot whose balance, average entry price,
/// and realized PnL are adjusted. Average entry price and realized PnL are
/// denominated in the account currency.
struct BalanceOperation {
  std::optional<param::Asset> asset;
  std::optional<PnlState> realizedPnl;
  std::optional<param::Price> averageEntryPrice;

  BalanceOperation() = default;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static BalanceOperation FromRaw(
      const OpenPitAccountAdjustmentBalanceOperation& raw) {
    BalanceOperation out;
    const auto asset =
        ::openpit::detail::FromNative<::openpit::StringView>(raw.asset);
    if (!asset.Empty()) {
      out.asset = ::openpit::detail::FromNative<param::Asset>(raw.asset);
    }
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice = ::openpit::detail::FromNative<param::Price>(
          raw.average_entry_price.value);
    }
    if (raw.realized_pnl.is_set) {
      out.realizedPnl =
          detail::PnlStateAccess::FromNative(raw.realized_pnl.value);
    }
    return out;
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentBalanceOperation Native() const {
    OpenPitAccountAdjustmentBalanceOperation raw{};
    if (asset) {
      raw.asset = ::openpit::detail::Native(*asset);
    }
    if (averageEntryPrice) {
      raw.average_entry_price.value =
          ::openpit::detail::Native(*averageEntryPrice);
      raw.average_entry_price.is_set = true;
    }
    if (realizedPnl) {
      raw.realized_pnl.value = detail::PnlStateAccess::Native(*realizedPnl);
      raw.realized_pnl.is_set = true;
    }
    return raw;
  }
};

//------------------------------------------------------------------------------
// PositionOperation

// Position-operation payload of an adjustment: the position's instrument,
// collateral asset, and optional average entry price, leverage, and mode. The
// average entry price is denominated in account currency. Each field is absent
// when its value is not set.
struct PositionOperation {
  std::optional<model::Instrument> instrument;
  std::optional<param::Asset> collateralAsset;
  std::optional<param::Price> averageEntryPrice;
  std::optional<param::Leverage> leverage;
  std::optional<model::PositionMode> mode;

  PositionOperation() = default;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static PositionOperation FromRaw(
      const OpenPitAccountAdjustmentPositionOperation& raw) {
    PositionOperation out;
    out.instrument =
        ::openpit::detail::FromNative<model::Instrument>(raw.instrument);
    const auto collateral =
        ::openpit::detail::FromNative<::openpit::StringView>(
            raw.collateral_asset);
    if (!collateral.Empty()) {
      out.collateralAsset =
          ::openpit::detail::FromNative<param::Asset>(raw.collateral_asset);
    }
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice = ::openpit::detail::FromNative<param::Price>(
          raw.average_entry_price.value);
    }
    out.leverage = param::detail::LeverageAccess::FromNative(raw.leverage);
    out.mode = model::detail::FromRawEnum<model::PositionMode>(
        raw.mode, OPENPIT_PARAM_POSITION_MODE_NOT_SET);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentPositionOperation Native()
      const noexcept {
    OpenPitAccountAdjustmentPositionOperation raw{};
    if (instrument) {
      raw.instrument = ::openpit::detail::Native(*instrument);
    }
    if (collateralAsset) {
      raw.collateral_asset = ::openpit::detail::Native(*collateralAsset);
    }
    if (averageEntryPrice) {
      raw.average_entry_price.value =
          ::openpit::detail::Native(*averageEntryPrice);
      raw.average_entry_price.is_set = true;
    }
    raw.leverage = param::detail::LeverageAccess::Native(leverage);
    raw.mode =
        model::detail::ToRawEnum(mode, OPENPIT_PARAM_POSITION_MODE_NOT_SET);
    return raw;
  }
};

//------------------------------------------------------------------------------
// Operation

// Tagged account-adjustment operation.
class Operation {
 public:
  [[nodiscard]] static Operation OfBalance(BalanceOperation balance) {
    return Operation(std::move(balance));
  }

  [[nodiscard]] static Operation OfPosition(PositionOperation position) {
    return Operation(std::move(position));
  }

  [[nodiscard]] static Operation OfAccountPnl(AccountPnlOperation pnl) {
    return Operation(pnl);
  }

  [[nodiscard]] const BalanceOperation* AsBalance() const noexcept {
    return std::get_if<BalanceOperation>(&m_value);
  }

  [[nodiscard]] const PositionOperation* AsPosition() const noexcept {
    return std::get_if<PositionOperation>(&m_value);
  }

  [[nodiscard]] const AccountPnlOperation* AsAccountPnl() const noexcept {
    return std::get_if<AccountPnlOperation>(&m_value);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  // nullopt when the discriminant is `Absent`.
  [[nodiscard]] static std::optional<Operation> FromRaw(
      const OpenPitAccountAdjustmentOperation& raw) {
    switch (raw.kind) {
      case OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_BALANCE:
        return OfBalance(
            ::openpit::detail::FromNative<BalanceOperation>(raw.balance));
      case OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_POSITION:
        return OfPosition(
            ::openpit::detail::FromNative<PositionOperation>(raw.position));
      case OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ACCOUNT_PNL:
        return OfAccountPnl(::openpit::detail::FromNative<AccountPnlOperation>(
            raw.account_pnl));
      case OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ABSENT:
      default:
        return std::nullopt;
    }
  }

  // Borrows the contained operation's string storage; valid only while this
  // object stays alive. The payload not selected by the kind is left zeroed.
  [[nodiscard]] OpenPitAccountAdjustmentOperation Native() const {
    OpenPitAccountAdjustmentOperation raw{};
    if (const auto* balance = AsBalance()) {
      raw.kind = OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_BALANCE;
      raw.balance = ::openpit::detail::Native(*balance);
    } else if (const auto* position = AsPosition()) {
      raw.kind = OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_POSITION;
      raw.position = ::openpit::detail::Native(*position);
    } else {
      raw.kind = OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ACCOUNT_PNL;
      raw.account_pnl = ::openpit::detail::Native(*AsAccountPnl());
    }
    return raw;
  }

  explicit Operation(BalanceOperation balance) : m_value(std::move(balance)) {}
  explicit Operation(PositionOperation position)
      : m_value(std::move(position)) {}
  explicit Operation(AccountPnlOperation pnl) : m_value(pnl) {}

  std::variant<BalanceOperation, PositionOperation, AccountPnlOperation>
      m_value;
};

}  // namespace openpit::accountadjustment
