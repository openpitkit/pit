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

#include "openpit/account_id.hpp"
#include "openpit/error.hpp"
#include "openpit/param.hpp"

#include <openpit.h>

#include <cassert>
#include <cstdint>
#include <exception>
#include <string>
#include <variant>

namespace openpit::accountadjustment {

/// Reason why a realized-PnL amount could not be calculated.
enum class PnlHaltReason : std::uint8_t {
  /// No quote has been published for a required FX conversion.
  ///
  /// The last available quote remains valid even when it is stale.
  MissingFx = OPENPIT_PNL_HALT_REASON_MISSING_FX,
  /// The account currency required for the ledger was unavailable.
  MissingAccountCurrency = OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY,
  /// The initial realized PnL needed to continue the ledger was unavailable.
  MissingInitialPnl = OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL,
  /// The position cost basis needed to calculate PnL was unavailable.
  MissingCostBasis = OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS,
  /// Exact realized-PnL arithmetic overflowed.
  ArithmeticOverflow = OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW,
};

namespace detail {

[[nodiscard]] inline std::variant<param::Pnl, PnlHaltReason> PnlValueFromRaw(
    const OpenPitPnlState& raw) {
  switch (raw.kind) {
    case OPENPIT_PNL_STATE_VALUE:
      if (raw.halt_reason != OPENPIT_PNL_HALT_REASON_NONE) {
        throw ::openpit::Error(
            "value PnL state must not contain a halt reason");
      }
      return param::Pnl::FromRaw(raw.value);
    case OPENPIT_PNL_STATE_HALTED:
      switch (raw.halt_reason) {
        case OPENPIT_PNL_HALT_REASON_MISSING_FX:
          return PnlHaltReason::MissingFx;
        case OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY:
          return PnlHaltReason::MissingAccountCurrency;
        case OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL:
          return PnlHaltReason::MissingInitialPnl;
        case OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS:
          return PnlHaltReason::MissingCostBasis;
        case OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW:
          return PnlHaltReason::ArithmeticOverflow;
        case OPENPIT_PNL_HALT_REASON_NONE:
          throw ::openpit::Error("halted PnL state requires a halt reason");
        default:
          throw ::openpit::Error(
              "invalid PnL halt reason code " +
              std::to_string(static_cast<unsigned int>(raw.halt_reason)));
      }
    default:
      throw ::openpit::Error(
          "invalid PnL state kind " +
          std::to_string(static_cast<unsigned int>(raw.kind)));
  }
}

[[nodiscard]] inline OpenPitPnlState PnlValueRaw(param::Pnl pnl) noexcept {
  OpenPitPnlState raw{};
  raw.kind = OPENPIT_PNL_STATE_VALUE;
  raw.value = pnl.Raw();
  raw.halt_reason = OPENPIT_PNL_HALT_REASON_NONE;
  return raw;
}

[[nodiscard]] inline OpenPitPnlState PnlValueRaw(
    PnlHaltReason reason) noexcept {
  OpenPitPnlState raw{};
  raw.kind = OPENPIT_PNL_STATE_HALTED;
  switch (reason) {
    case PnlHaltReason::MissingFx:
    case PnlHaltReason::MissingAccountCurrency:
    case PnlHaltReason::MissingInitialPnl:
    case PnlHaltReason::MissingCostBasis:
    case PnlHaltReason::ArithmeticOverflow:
      raw.halt_reason = static_cast<OpenPitPnlHaltReason>(reason);
      return raw;
    default:
      // Invalid enum values violate the debug-only caller contract. Keep this
      // conversion noexcept and terminating instead of adding runtime error
      // handling to every valid conversion.
      assert(false && "invalid PnL halt reason value");
      std::terminate();
  }
}

[[nodiscard]] inline OpenPitPnlState PnlValueRaw(
    const std::variant<param::Pnl, PnlHaltReason>& value) noexcept {
  if (const auto* pnl = std::get_if<param::Pnl>(&value)) {
    return PnlValueRaw(*pnl);
  }
  return PnlValueRaw(std::get<PnlHaltReason>(value));
}

}  // namespace detail

/// Replaces the account-wide realized-PnL accumulator.
class AccountPnlOperation {
 public:
  explicit AccountPnlOperation(param::Pnl pnl) : m_value(pnl) {}
  explicit AccountPnlOperation(PnlHaltReason reason) : m_value(reason) {}

  /// Returns the replacement PnL or its explicit halt reason.
  [[nodiscard]] const std::variant<param::Pnl, PnlHaltReason>& Get()
      const noexcept {
    return m_value;
  }

  [[nodiscard]] static AccountPnlOperation FromRaw(
      const OpenPitAccountAdjustmentAccountPnlOperation& raw) {
    const auto value = detail::PnlValueFromRaw(raw.state);
    if (const auto* pnl = std::get_if<param::Pnl>(&value)) {
      return AccountPnlOperation(*pnl);
    }
    return AccountPnlOperation(std::get<PnlHaltReason>(value));
  }

  [[nodiscard]] OpenPitAccountAdjustmentAccountPnlOperation Raw()
      const noexcept {
    OpenPitAccountAdjustmentAccountPnlOperation raw{};
    raw.state = detail::PnlValueRaw(m_value);
    return raw;
  }

 private:
  std::variant<param::Pnl, PnlHaltReason> m_value;
};

/// Realized-PnL change and resulting absolute value.
///
/// Both values are denominated in the account currency.
struct PnlOutcomeAmount {
  /// Signed realized-PnL change applied by the operation.
  param::Pnl delta;
  /// Cumulative realized PnL after the operation.
  param::Pnl absolute;

  PnlOutcomeAmount(param::Pnl outcomeDelta, param::Pnl outcomeAbsolute)
      : delta(outcomeDelta), absolute(outcomeAbsolute) {}

  [[nodiscard]] static PnlOutcomeAmount FromRaw(
      const OpenPitPnlOutcomeAmount& raw) {
    return PnlOutcomeAmount(param::Pnl::FromRaw(raw.delta),
                            param::Pnl::FromRaw(raw.absolute));
  }

  [[nodiscard]] OpenPitPnlOutcomeAmount Raw() const noexcept {
    OpenPitPnlOutcomeAmount raw{};
    raw.delta = delta.Raw();
    raw.absolute = absolute.Raw();
    return raw;
  }
};

using PnlOutcomeOptional = OpenPitPnlOutcomeOptional;

/// Realized-PnL result: either the amount or a halt reason.
using PnlOutcomeResult = std::variant<PnlOutcomeAmount, PnlHaltReason>;

struct PnlOutcome {
  PnlOutcomeResult result;

  /// Returns the PnL amount or the reason why it is unavailable.
  [[nodiscard]] const PnlOutcomeResult& Get() const noexcept { return result; }

  [[nodiscard]] static PnlOutcome FromRaw(const OpenPitPnlOutcome& raw) {
    if (raw.halt_reason != OPENPIT_PNL_HALT_REASON_NONE && raw.amount.is_set) {
      throw ::openpit::Error("halted PnL outcome must not contain an amount");
    }
    switch (raw.halt_reason) {
      case OPENPIT_PNL_HALT_REASON_NONE:
        if (raw.amount.is_set) {
          return PnlOutcome{PnlOutcomeAmount::FromRaw(raw.amount.value)};
        }
        throw ::openpit::Error("available PnL outcome requires an amount");
      case OPENPIT_PNL_HALT_REASON_MISSING_FX:
        return PnlOutcome{PnlHaltReason::MissingFx};
      case OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY:
        return PnlOutcome{PnlHaltReason::MissingAccountCurrency};
      case OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL:
        return PnlOutcome{PnlHaltReason::MissingInitialPnl};
      case OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS:
        return PnlOutcome{PnlHaltReason::MissingCostBasis};
      case OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW:
        return PnlOutcome{PnlHaltReason::ArithmeticOverflow};
      default:
        throw ::openpit::Error(
            "invalid PnL halt reason code " +
            std::to_string(static_cast<unsigned int>(raw.halt_reason)));
    }
  }

  [[nodiscard]] OpenPitPnlOutcome Raw() const noexcept {
    OpenPitPnlOutcome raw{};
    if (const auto* amount = std::get_if<PnlOutcomeAmount>(&result)) {
      raw.halt_reason = OPENPIT_PNL_HALT_REASON_NONE;
      raw.amount.value = amount->Raw();
      raw.amount.is_set = true;
    } else {
      const PnlHaltReason reason = std::get<PnlHaltReason>(result);
      switch (reason) {
        case PnlHaltReason::MissingFx:
        case PnlHaltReason::MissingAccountCurrency:
        case PnlHaltReason::MissingInitialPnl:
        case PnlHaltReason::MissingCostBasis:
        case PnlHaltReason::ArithmeticOverflow:
          raw.halt_reason = static_cast<OpenPitPnlHaltReason>(reason);
          break;
        default:
          // Invalid enum values violate the debug-only caller contract. Keep
          // this conversion noexcept and terminating instead of adding runtime
          // error handling to every valid conversion.
          assert(false && "invalid PnL halt reason value");
          std::terminate();
      }
    }
    return raw;
  }
};

/// Account-level realized-PnL result: either the amount or a halt reason.
/// A newly halted calculation emits its reason once; later checks can reject
/// or block on the stored halt without emitting another account outcome. A
/// manager explicitly force-sets the account PnL to re-arm it. Position
/// accumulators are independent.
using AccountPnlResult = PnlOutcomeResult;

/// Account-level realized-PnL outcome.
struct AccountPnlOutcome {
  /// Account-currency PnL, or the reason why it is unavailable.
  AccountPnlResult result;
  /// Account that owns the realized-PnL ledger.
  param::AccountId accountId;
  /// Policy group of the producer that owns the ledger.
  param::GroupId policyGroupId;

  /// Returns the account PnL or the reason why it is unavailable.
  [[nodiscard]] const AccountPnlResult& Get() const noexcept { return result; }

  [[nodiscard]] static AccountPnlOutcome FromRaw(
      const OpenPitAccountPnlOutcome& raw) {
    OpenPitPnlOutcome rawResult{};
    rawResult.halt_reason = raw.halt_reason;
    rawResult.amount = raw.amount;
    AccountPnlOutcome outcome{
        PnlOutcome::FromRaw(rawResult).result,
        param::AccountId::FromRaw(raw.account_id),
        param::GroupId(raw.policy_group_id),
    };
    return outcome;
  }

  [[nodiscard]] OpenPitAccountPnlOutcome Raw() const noexcept {
    const PnlOutcome pnl{result};
    const OpenPitPnlOutcome rawResult = pnl.Raw();
    OpenPitAccountPnlOutcome raw{};
    raw.account_id = accountId.Raw();
    raw.policy_group_id = policyGroupId.Raw();
    raw.halt_reason = rawResult.halt_reason;
    raw.amount = rawResult.amount;
    return raw;
  }
};

}  // namespace openpit::accountadjustment
