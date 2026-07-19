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
#include "openpit/model.hpp"
#include "openpit/param.hpp"
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
  std::optional<std::variant<param::Pnl, PnlHaltReason>> realizedPnl;
  std::optional<param::Price> averageEntryPrice;

  BalanceOperation() = default;

  [[nodiscard]] static BalanceOperation FromRaw(
      const OpenPitAccountAdjustmentBalanceOperation& raw) {
    BalanceOperation out;
    const ::openpit::StringView asset(raw.asset);
    if (!asset.Empty()) {
      out.asset = param::Asset::FromRaw(raw.asset);
    }
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice =
          param::Price::FromRaw(raw.average_entry_price.value);
    }
    if (raw.realized_pnl.is_set) {
      out.realizedPnl = detail::PnlValueFromRaw(raw.realized_pnl.value);
    }
    return out;
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentBalanceOperation Raw() const noexcept {
    OpenPitAccountAdjustmentBalanceOperation raw{};
    if (asset) {
      raw.asset = asset->Raw();
    }
    if (averageEntryPrice) {
      raw.average_entry_price.value = averageEntryPrice->Raw();
      raw.average_entry_price.is_set = true;
    }
    if (realizedPnl) {
      raw.realized_pnl.value = detail::PnlValueRaw(*realizedPnl);
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
// (empty optional) when its C view / sentinel is unset.
struct PositionOperation {
  std::optional<model::Instrument> instrument;
  std::optional<param::Asset> collateralAsset;
  std::optional<param::Price> averageEntryPrice;
  std::optional<param::Leverage> leverage;
  std::optional<model::PositionMode> mode;

  PositionOperation() = default;

  [[nodiscard]] static PositionOperation FromRaw(
      const OpenPitAccountAdjustmentPositionOperation& raw) {
    PositionOperation out;
    out.instrument = model::Instrument::FromRaw(raw.instrument);
    const ::openpit::StringView collateral(raw.collateral_asset);
    if (!collateral.Empty()) {
      out.collateralAsset = param::Asset::FromRaw(raw.collateral_asset);
    }
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice =
          param::Price::FromRaw(raw.average_entry_price.value);
    }
    out.leverage = param::Leverage::FromRawOption(raw.leverage);
    out.mode = model::detail::FromRawEnum<model::PositionMode>(
        raw.mode, OPENPIT_PARAM_POSITION_MODE_NOT_SET);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentPositionOperation Raw() const noexcept {
    OpenPitAccountAdjustmentPositionOperation raw{};
    if (instrument) {
      raw.instrument = instrument->Raw();
    }
    if (collateralAsset) {
      raw.collateral_asset = collateralAsset->Raw();
    }
    if (averageEntryPrice) {
      raw.average_entry_price.value = averageEntryPrice->Raw();
      raw.average_entry_price.is_set = true;
    }
    raw.leverage = param::Leverage::RawOption(leverage);
    raw.mode =
        model::detail::ToRawEnum(mode, OPENPIT_PARAM_POSITION_MODE_NOT_SET);
    return raw;
  }
};

//------------------------------------------------------------------------------
// Operation

// Discriminated operation of an adjustment. Because the native runtime carries
// a single discriminant, supplying multiple operations at once is not
// representable; an absent operation is modeled as an empty
// `std::optional<Operation>` on the owning `AccountAdjustment`.
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

  // nullopt when the discriminant is `Absent`.
  [[nodiscard]] static std::optional<Operation> FromRaw(
      const OpenPitAccountAdjustmentOperation& raw) {
    switch (raw.kind) {
      case OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_BALANCE:
        return Operation(BalanceOperation::FromRaw(raw.balance));
      case OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_POSITION:
        return Operation(PositionOperation::FromRaw(raw.position));
      case OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ACCOUNT_PNL:
        return Operation(AccountPnlOperation::FromRaw(raw.account_pnl));
      default:
        return std::nullopt;
    }
  }

  // Borrows the contained operation's string storage; valid only while this
  // object stays alive. The payload not selected by the kind is left zeroed.
  [[nodiscard]] OpenPitAccountAdjustmentOperation Raw() const noexcept {
    OpenPitAccountAdjustmentOperation raw{};
    if (const auto* balance = std::get_if<BalanceOperation>(&m_value)) {
      raw.kind = OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_BALANCE;
      raw.balance = balance->Raw();
    } else if (const auto* position =
                   std::get_if<PositionOperation>(&m_value)) {
      raw.kind = OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_POSITION;
      raw.position = position->Raw();
    } else {
      const auto& accountPnl = std::get<AccountPnlOperation>(m_value);
      raw.kind = OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ACCOUNT_PNL;
      raw.account_pnl = accountPnl.Raw();
    }
    return raw;
  }

  [[nodiscard]] bool IsBalance() const noexcept {
    return std::holds_alternative<BalanceOperation>(m_value);
  }

  [[nodiscard]] bool IsPosition() const noexcept {
    return std::holds_alternative<PositionOperation>(m_value);
  }

  [[nodiscard]] bool IsAccountPnl() const noexcept {
    return std::holds_alternative<AccountPnlOperation>(m_value);
  }

  // The balance payload; present only when `IsBalance()`.
  [[nodiscard]] const BalanceOperation* AsBalance() const noexcept {
    return std::get_if<BalanceOperation>(&m_value);
  }

  // The position payload; present only when `IsPosition()`.
  [[nodiscard]] const PositionOperation* AsPosition() const noexcept {
    return std::get_if<PositionOperation>(&m_value);
  }

  // The account-PnL payload; present only when `IsAccountPnl()`.
  [[nodiscard]] const AccountPnlOperation* AsAccountPnl() const noexcept {
    return std::get_if<AccountPnlOperation>(&m_value);
  }

 private:
  explicit Operation(BalanceOperation balance) : m_value(std::move(balance)) {}
  explicit Operation(PositionOperation position)
      : m_value(std::move(position)) {}
  explicit Operation(AccountPnlOperation pnl) : m_value(pnl) {}

  std::variant<BalanceOperation, PositionOperation, AccountPnlOperation>
      m_value;
};

}  // namespace openpit::accountadjustment
