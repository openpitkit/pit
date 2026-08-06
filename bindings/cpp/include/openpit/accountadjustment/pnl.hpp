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

#include "openpit/error.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/param/param.hpp"

#include <openpit.h>

#include <cstdint>
#include <optional>
#include <string>
#include <variant>

namespace openpit::accountadjustment {

/// Reason why a realized-PnL amount could not be calculated.
///
/// When failures coincide, SpotFunds uses this priority from highest to lowest:
/// `ArithmeticOverflow`, `MissingAccountCurrency`, `MissingFx`,
/// `MissingCostBasis`, then `MissingInitialPnl`.
enum class PnlHaltReason : std::uint8_t {
  /// No quote has been published for a required FX conversion.
  ///
  /// The last available quote remains valid even when it is stale.
  MissingFx = 1,
  /// The account currency required for the ledger was unavailable.
  MissingAccountCurrency = 2,
  /// The initial realized PnL needed to continue the ledger was unavailable.
  MissingInitialPnl = 3,
  /// The position cost basis needed to calculate PnL was unavailable.
  MissingCostBasis = 4,
  /// Exact realized-PnL arithmetic overflowed.
  ArithmeticOverflow = 5,
};

namespace detail {
class PnlStateAccess;
}  // namespace detail

/// Current state of a realized-PnL accumulator.
class PnlState {
 public:
  explicit PnlState(param::Pnl value) : m_value(value) {}
  explicit PnlState(PnlHaltReason reason) : m_value(reason) {}

  [[nodiscard]] const param::Pnl* Value() const noexcept {
    return std::get_if<param::Pnl>(&m_value);
  }

  [[nodiscard]] std::optional<PnlHaltReason> HaltReason() const noexcept {
    if (const auto* reason = std::get_if<PnlHaltReason>(&m_value)) {
      return *reason;
    }
    return std::nullopt;
  }

 private:
  friend class detail::PnlStateAccess;

  std::variant<param::Pnl, PnlHaltReason> m_value;
};

namespace detail {

class PnlStateAccess final {
 private:
  [[nodiscard]] static PnlState FromNative(const OpenPitPnlState& raw) {
    switch (raw.kind) {
      case OPENPIT_PNL_STATE_HALTED:
        return PnlState(static_cast<PnlHaltReason>(raw.halt_reason));
      case OPENPIT_PNL_STATE_VALUE:
      default:
        return PnlState(::openpit::detail::FromNative<param::Pnl>(raw.value));
    }
  }

  [[nodiscard]] static OpenPitPnlState Native(param::Pnl pnl) noexcept {
    OpenPitPnlState raw{};
    raw.kind = OPENPIT_PNL_STATE_VALUE;
    raw.value = ::openpit::detail::Native(pnl);
    raw.halt_reason = OPENPIT_PNL_HALT_REASON_NONE;
    return raw;
  }

  [[nodiscard]] static OpenPitPnlState Native(PnlHaltReason reason) noexcept {
    OpenPitPnlState raw{};
    raw.kind = OPENPIT_PNL_STATE_HALTED;
    raw.halt_reason = static_cast<OpenPitPnlHaltReason>(reason);
    return raw;
  }

  [[nodiscard]] static OpenPitPnlState Native(const PnlState& state) noexcept {
    if (const auto* value = state.Value()) {
      return Native(*value);
    }
    return Native(*state.HaltReason());
  }

  friend class ::openpit::accountadjustment::AccountPnlOperation;
  friend class ::openpit::Configurator;
  friend struct ::openpit::accountadjustment::BalanceOperation;
};

}  // namespace detail

/// Replaces the account-wide realized-PnL accumulator.
class AccountPnlOperation {
 public:
  explicit AccountPnlOperation(PnlState state) : m_state(state) {}
  explicit AccountPnlOperation(param::Pnl pnl) : m_state(pnl) {}
  explicit AccountPnlOperation(PnlHaltReason reason) : m_state(reason) {}

  [[nodiscard]] const param::Pnl* Value() const noexcept {
    return m_state.Value();
  }

  [[nodiscard]] std::optional<PnlHaltReason> HaltReason() const noexcept {
    return m_state.HaltReason();
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static AccountPnlOperation FromRaw(
      const OpenPitAccountAdjustmentAccountPnlOperation& raw) {
    return AccountPnlOperation(detail::PnlStateAccess::FromNative(raw.state));
  }

  [[nodiscard]] OpenPitAccountAdjustmentAccountPnlOperation Native()
      const noexcept {
    OpenPitAccountAdjustmentAccountPnlOperation raw{};
    raw.state = detail::PnlStateAccess::Native(m_state);
    return raw;
  }

  PnlState m_state;
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static PnlOutcomeAmount FromRaw(
      const OpenPitPnlOutcomeAmount& raw) {
    return PnlOutcomeAmount(
        ::openpit::detail::FromNative<param::Pnl>(raw.delta),
        ::openpit::detail::FromNative<param::Pnl>(raw.absolute));
  }

  [[nodiscard]] OpenPitPnlOutcomeAmount Native() const noexcept {
    OpenPitPnlOutcomeAmount raw{};
    raw.delta = ::openpit::detail::Native(delta);
    raw.absolute = ::openpit::detail::Native(absolute);
    return raw;
  }
};

namespace detail {
using RawPnlOutcomeOptional = ::OpenPitPnlOutcomeOptional;
}  // namespace detail

class PnlOutcome {
 public:
  explicit PnlOutcome(PnlOutcomeAmount outcomeAmount)
      : m_result(outcomeAmount) {}
  explicit PnlOutcome(PnlHaltReason reason) : m_result(reason) {}

  [[nodiscard]] const PnlOutcomeAmount* Amount() const noexcept {
    return std::get_if<PnlOutcomeAmount>(&m_result);
  }

  [[nodiscard]] std::optional<PnlHaltReason> HaltReason() const noexcept {
    if (const auto* reason = std::get_if<PnlHaltReason>(&m_result)) {
      return *reason;
    }
    return std::nullopt;
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static PnlOutcome FromRaw(const OpenPitPnlOutcome& raw) {
    if (raw.halt_reason == OPENPIT_PNL_HALT_REASON_NONE) {
      return PnlOutcome{
          ::openpit::detail::FromNative<PnlOutcomeAmount>(raw.amount.value)};
    }
    return PnlOutcome{static_cast<PnlHaltReason>(raw.halt_reason)};
  }

  [[nodiscard]] OpenPitPnlOutcome Native() const {
    OpenPitPnlOutcome raw{};
    if (const auto* amount = Amount()) {
      raw.halt_reason = OPENPIT_PNL_HALT_REASON_NONE;
      raw.amount.value = ::openpit::detail::Native(*amount);
      raw.amount.is_set = true;
    } else {
      const PnlHaltReason haltReason = *HaltReason();
      raw.halt_reason = static_cast<OpenPitPnlHaltReason>(haltReason);
    }
    return raw;
  }

  std::variant<PnlOutcomeAmount, PnlHaltReason> m_result;
};

/// Account-level realized-PnL result: either the amount or a halt reason.
/// SpotFunds engages this account line only for a realizing fill or a nonzero
/// fee. Opening, same-direction, and zero-quantity fills without a nonzero fee,
/// plus zero fees alone, emit no outcome and require no account currency or FX
/// for this line. A nonzero fee engages both position and account rows
/// regardless of fill quantity.
///
/// A newly halted calculation emits its reason once; later checks can reject
/// or block on the stored halt without emitting another account outcome. A
/// manager explicitly force-sets the account PnL to re-arm it. Position
/// accumulators are independent.
/// Account-level realized-PnL outcome.
class AccountPnlOutcome {
 private:
  PnlOutcome m_result;

 public:
  /// Account that owns the realized-PnL ledger.
  param::AccountId accountId;
  /// Policy group of the producer that owns the ledger.
  param::GroupId policyGroupId;

  AccountPnlOutcome(PnlOutcomeAmount outcomeAmount,
                    param::AccountId outcomeAccountId,
                    param::GroupId outcomePolicyGroupId)
      : m_result(outcomeAmount),
        accountId(outcomeAccountId),
        policyGroupId(outcomePolicyGroupId) {}

  AccountPnlOutcome(PnlHaltReason reason, param::AccountId outcomeAccountId,
                    param::GroupId outcomePolicyGroupId)
      : m_result(reason),
        accountId(outcomeAccountId),
        policyGroupId(outcomePolicyGroupId) {}

  [[nodiscard]] const PnlOutcomeAmount* Amount() const noexcept {
    return m_result.Amount();
  }

  [[nodiscard]] std::optional<PnlHaltReason> HaltReason() const noexcept {
    return m_result.HaltReason();
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static AccountPnlOutcome FromRaw(
      const OpenPitAccountPnlOutcome& raw) {
    OpenPitPnlOutcome rawResult{};
    rawResult.halt_reason = raw.halt_reason;
    rawResult.amount = raw.amount;
    const PnlOutcome pnl = ::openpit::detail::FromNative<PnlOutcome>(rawResult);
    const param::AccountId accountId =
        ::openpit::detail::FromNative<param::AccountId>(raw.account_id);
    const param::GroupId policyGroupId(raw.policy_group_id);
    if (const auto* amount = pnl.Amount()) {
      return AccountPnlOutcome(*amount, accountId, policyGroupId);
    }
    return AccountPnlOutcome(*pnl.HaltReason(), accountId, policyGroupId);
  }

  [[nodiscard]] OpenPitAccountPnlOutcome Native() const {
    const OpenPitPnlOutcome rawResult = ::openpit::detail::Native(m_result);
    OpenPitAccountPnlOutcome raw{};
    raw.account_id = ::openpit::detail::Native(accountId);
    raw.policy_group_id = ::openpit::detail::Native(policyGroupId);
    raw.halt_reason = rawResult.halt_reason;
    raw.amount = rawResult.amount;
    return raw;
  }
};

}  // namespace openpit::accountadjustment
