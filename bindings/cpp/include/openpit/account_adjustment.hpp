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
#include "openpit/detail/handle.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/reject.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <variant>
#include <vector>

// Account-adjustment value types: the request payload a caller hands to
// `Engine::ApplyAccountAdjustment` (added by the engine slice) and the outcomes
// / batch error it produces.
//
// Each request payload mirrors the native runtime POD view
// (`OpenPitAccountAdjustment`,
// ...). Optional groups and fields read as an empty `std::optional` when
// absent, matching the native runtime `is_set` / `*_NotSet` / `Absent`
// convention and the native SDK; the binding adds NO validation of its own —
// every request rule is enforced by the core. `FromRaw` copies a borrowed C
// view into owned values; `Raw()` rebuilds a C view whose string fields borrow
// this object's storage and is therefore valid only while the object is alive
// and unchanged. Financial fields are carried by the `openpit::param` value
// types, never by `double`; negative amounts are permitted here (only a
// pre-trade reserve forbids them) and any overflow surfaces as a
// reject/account-block outcome from the core, not as a C++ failure.
//
// `OutcomeList` and `BatchError` are owning RAII handles over the
// caller-owned C results of an apply call.

namespace openpit::accountadjustment {

using OutcomeAmountOptional = OpenPitOutcomeAmountOptional;
using PnlOutcomeAmountOptional = OpenPitPnlOutcomeAmountOptional;

//------------------------------------------------------------------------------
// BalanceOperation

/// Balance-operation payload of an account adjustment.
///
/// `realizedPnl` force-sets the absolute account-currency realized PnL. It is
/// not a delta and does not perform FX conversion. When absent, the current
/// value is left unchanged and no realized-PnL outcome is emitted. A force-set
/// outcome reports `delta = new - prior` and `absolute = new`.
struct BalanceOperation {
  std::optional<std::string> asset;
  std::optional<param::Price> averageEntryPrice;
  /// Optional absolute realized-PnL force-set.
  std::optional<param::Pnl> realizedPnl;

  BalanceOperation() = default;

  [[nodiscard]] static BalanceOperation FromRaw(
      const OpenPitAccountAdjustmentBalanceOperation& raw) {
    BalanceOperation out;
    const ::openpit::StringView asset(raw.asset);
    if (!asset.Empty()) {
      out.asset = asset.ToString();
    }
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice =
          param::Price::FromRaw(raw.average_entry_price.value);
    }
    if (raw.realized_pnl.is_set) {
      out.realizedPnl = param::Pnl::FromRaw(raw.realized_pnl.value);
    }
    return out;
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentBalanceOperation Raw() const noexcept {
    OpenPitAccountAdjustmentBalanceOperation raw{};
    if (asset) {
      raw.asset = ::openpit::MakeStringView(*asset);
    }
    if (averageEntryPrice) {
      raw.average_entry_price.value = averageEntryPrice->Raw();
      raw.average_entry_price.is_set = true;
    }
    if (realizedPnl) {
      raw.realized_pnl.value = realizedPnl->Raw();
      raw.realized_pnl.is_set = true;
    }
    return raw;
  }
};

//------------------------------------------------------------------------------
// PositionOperation

// Position-operation payload of an adjustment: the position's instrument,
// collateral asset, and optional average entry price, leverage, and mode. Each
// field is absent (empty optional) when its C view / sentinel is unset.
struct PositionOperation {
  std::optional<model::Instrument> instrument;
  std::optional<std::string> collateralAsset;
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
      out.collateralAsset = collateral.ToString();
    }
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice =
          param::Price::FromRaw(raw.average_entry_price.value);
    }
    out.leverage = param::Leverage::FromRawOption(raw.leverage);
    out.mode = model::detail::FromRawEnum<model::PositionMode>(
        raw.mode, OpenPitParamPositionMode_NotSet);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentPositionOperation Raw() const noexcept {
    OpenPitAccountAdjustmentPositionOperation raw{};
    if (instrument) {
      raw.instrument = instrument->Raw();
    }
    if (collateralAsset) {
      raw.collateral_asset = ::openpit::MakeStringView(*collateralAsset);
    }
    if (averageEntryPrice) {
      raw.average_entry_price.value = averageEntryPrice->Raw();
      raw.average_entry_price.is_set = true;
    }
    raw.leverage = param::Leverage::RawOption(leverage);
    raw.mode = model::detail::ToRawEnum(mode, OpenPitParamPositionMode_NotSet);
    return raw;
  }
};

//------------------------------------------------------------------------------
// Operation

// Discriminated operation of an adjustment: at most one of a `BalanceOperation`
// or a `PositionOperation`. Because the native runtime carries a single
// discriminant, supplying both at once is not representable; an absent
// operation is modeled as an empty `std::optional<Operation>` on the owning
// `AccountAdjustment`, not as a value here. Mirrors the native runtime
// `OpenPitAccountAdjustmentOperation` `Balance` / `Position` kinds (the
// `Absent` kind maps to the empty optional).
class Operation {
 public:
  [[nodiscard]] static Operation OfBalance(BalanceOperation balance) {
    return Operation(std::move(balance));
  }

  [[nodiscard]] static Operation OfPosition(PositionOperation position) {
    return Operation(std::move(position));
  }

  // nullopt when the discriminant is `Absent`.
  [[nodiscard]] static std::optional<Operation> FromRaw(
      const OpenPitAccountAdjustmentOperation& raw) {
    switch (raw.kind) {
      case OpenPitAccountAdjustmentOperationKind_Balance:
        return Operation(BalanceOperation::FromRaw(raw.balance));
      case OpenPitAccountAdjustmentOperationKind_Position:
        return Operation(PositionOperation::FromRaw(raw.position));
      default:
        return std::nullopt;
    }
  }

  // Borrows the contained operation's string storage; valid only while this
  // object stays alive. The payload not selected by the kind is left zeroed.
  [[nodiscard]] OpenPitAccountAdjustmentOperation Raw() const noexcept {
    OpenPitAccountAdjustmentOperation raw{};
    if (const auto* balance = std::get_if<BalanceOperation>(&m_value)) {
      raw.kind = OpenPitAccountAdjustmentOperationKind_Balance;
      raw.balance = balance->Raw();
    } else {
      const auto& position = std::get<PositionOperation>(m_value);
      raw.kind = OpenPitAccountAdjustmentOperationKind_Position;
      raw.position = position.Raw();
    }
    return raw;
  }

  [[nodiscard]] bool IsBalance() const noexcept {
    return std::holds_alternative<BalanceOperation>(m_value);
  }

  [[nodiscard]] bool IsPosition() const noexcept {
    return std::holds_alternative<PositionOperation>(m_value);
  }

  // The balance payload; present only when `IsBalance()`.
  [[nodiscard]] const BalanceOperation* AsBalance() const noexcept {
    return std::get_if<BalanceOperation>(&m_value);
  }

  // The position payload; present only when `IsPosition()`.
  [[nodiscard]] const PositionOperation* AsPosition() const noexcept {
    return std::get_if<PositionOperation>(&m_value);
  }

 private:
  explicit Operation(BalanceOperation balance) : m_value(std::move(balance)) {}
  explicit Operation(PositionOperation position)
      : m_value(std::move(position)) {}

  std::variant<BalanceOperation, PositionOperation> m_value;
};

//------------------------------------------------------------------------------
// Amount

// Optional amount-change group of an adjustment: a signed delta/absolute change
// to the `balance`, `held`, and `incoming` components. Each component is absent
// (empty optional) when its C kind is `NotSet`; the whole group is absent on
// the owning `AccountAdjustment` when its C `is_set` flag is false. Each
// present value is a `param::AdjustmentAmount`, which may be negative.
struct Amount {
  std::optional<param::AdjustmentAmount> balance;
  std::optional<param::AdjustmentAmount> held;
  std::optional<param::AdjustmentAmount> incoming;

  Amount() = default;

  [[nodiscard]] static Amount FromRaw(
      const OpenPitAccountAdjustmentAmount& raw) {
    Amount out;
    out.balance = param::AdjustmentAmount::FromRaw(raw.balance);
    out.held = param::AdjustmentAmount::FromRaw(raw.held);
    out.incoming = param::AdjustmentAmount::FromRaw(raw.incoming);
    return out;
  }

  [[nodiscard]] OpenPitAccountAdjustmentAmount Raw() const noexcept {
    OpenPitAccountAdjustmentAmount raw{};
    if (balance) {
      raw.balance = balance->Raw();
    }
    if (held) {
      raw.held = held->Raw();
    }
    if (incoming) {
      raw.incoming = incoming->Raw();
    }
    return raw;
  }
};

//------------------------------------------------------------------------------
// Bounds

// Optional bounds group of an adjustment: per-component upper/lower clamps for
// `balance`, `held`, and `incoming`. Each bound is absent (empty optional) when
// its C `is_set` flag is false; the whole group is absent on the owning
// `AccountAdjustment` when its C `is_set` flag is false.
struct Bounds {
  std::optional<param::PositionSize> balanceUpper;
  std::optional<param::PositionSize> balanceLower;
  std::optional<param::PositionSize> heldUpper;
  std::optional<param::PositionSize> heldLower;
  std::optional<param::PositionSize> incomingUpper;
  std::optional<param::PositionSize> incomingLower;

  Bounds() = default;

  [[nodiscard]] static Bounds FromRaw(
      const OpenPitAccountAdjustmentBounds& raw) {
    Bounds out;
    out.balanceUpper = ReadBound(raw.balance_upper);
    out.balanceLower = ReadBound(raw.balance_lower);
    out.heldUpper = ReadBound(raw.held_upper);
    out.heldLower = ReadBound(raw.held_lower);
    out.incomingUpper = ReadBound(raw.incoming_upper);
    out.incomingLower = ReadBound(raw.incoming_lower);
    return out;
  }

  [[nodiscard]] OpenPitAccountAdjustmentBounds Raw() const noexcept {
    OpenPitAccountAdjustmentBounds raw{};
    WriteBound(raw.balance_upper, balanceUpper);
    WriteBound(raw.balance_lower, balanceLower);
    WriteBound(raw.held_upper, heldUpper);
    WriteBound(raw.held_lower, heldLower);
    WriteBound(raw.incoming_upper, incomingUpper);
    WriteBound(raw.incoming_lower, incomingLower);
    return raw;
  }

 private:
  [[nodiscard]] static std::optional<param::PositionSize> ReadBound(
      const param::PositionSizeOptional& field) {
    if (!field.is_set) {
      return std::nullopt;
    }
    return param::PositionSize::FromRaw(field.value);
  }

  static void WriteBound(
      param::PositionSizeOptional& field,
      const std::optional<param::PositionSize>& value) noexcept {
    if (value) {
      field.value = value->Raw();
      field.is_set = true;
    }
  }
};

//------------------------------------------------------------------------------
// AccountAdjustment

// Full adjustment request payload mirroring the native runtime
// `OpenPitAccountAdjustment`. The `operation`, `amount`, and `bounds` groups
// are each optional; `userData` is an opaque caller token the SDK never
// inspects (zero means unset). The account this applies to is not part of the
// payload: it is passed separately to `Engine::ApplyAccountAdjustment`.
struct AccountAdjustment {
  std::optional<Operation> operation;
  std::optional<Amount> amount;
  std::optional<Bounds> bounds;
  std::uintptr_t userData = 0;

  AccountAdjustment() = default;

  [[nodiscard]] static AccountAdjustment FromRaw(
      const OpenPitAccountAdjustment& raw) {
    AccountAdjustment out;
    out.operation = Operation::FromRaw(raw.operation);
    if (raw.amount.is_set) {
      out.amount = Amount::FromRaw(raw.amount.value);
    }
    if (raw.bounds.is_set) {
      out.bounds = Bounds::FromRaw(raw.bounds.value);
    }
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustment Raw() const noexcept {
    OpenPitAccountAdjustment raw{};
    if (operation) {
      raw.operation = operation->Raw();
    }
    if (amount) {
      raw.amount.value = amount->Raw();
      raw.amount.is_set = true;
    }
    if (bounds) {
      raw.bounds.value = bounds->Raw();
      raw.bounds.is_set = true;
    }
    raw.user_data = reinterpret_cast<void*>(userData);
    return raw;
  }
};

//------------------------------------------------------------------------------
// OutcomeAmount

// A delta/absolute pair an adjustment outcome reports for one component.
// `delta` is the signed change relative to the component value at operation
// start and is authoritative; `absolute` is a convenience snapshot taken when
// the policy returned. Both are always present.
struct OutcomeAmount {
  param::PositionSize delta;
  param::PositionSize absolute;

  OutcomeAmount(param::PositionSize outcomeDelta,
                param::PositionSize outcomeAbsolute)
      : delta(outcomeDelta), absolute(outcomeAbsolute) {}

  [[nodiscard]] static OutcomeAmount FromRaw(const OpenPitOutcomeAmount& raw) {
    return OutcomeAmount(param::PositionSize::FromRaw(raw.delta),
                         param::PositionSize::FromRaw(raw.absolute));
  }

  [[nodiscard]] OpenPitOutcomeAmount Raw() const noexcept {
    OpenPitOutcomeAmount raw{};
    raw.delta = delta.Raw();
    raw.absolute = absolute.Raw();
    return raw;
  }
};

//------------------------------------------------------------------------------
// PnlOutcomeAmount

/// Realized-PnL change and resulting absolute value reported by an adjustment.
struct PnlOutcomeAmount {
  /// Signed realized-PnL change applied by the adjustment.
  param::Pnl delta;
  /// Cumulative realized PnL after the adjustment.
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

//------------------------------------------------------------------------------
// AccountOutcomeEntry

// Per-asset outcome an adjustment produced: the affected `asset` plus the
// settled `balance`, `held`, `incoming`, realized PnL, and average-entry-price
// amounts. Each amount is absent (empty optional) when its C `is_set` flag is
// false.
struct AccountOutcomeEntry {
  std::string asset;
  std::optional<OutcomeAmount> balance;
  std::optional<OutcomeAmount> held;
  std::optional<OutcomeAmount> incoming;
  /// Realized-PnL delta and absolute value when the adjustment force-set it.
  std::optional<PnlOutcomeAmount> realizedPnl;
  /// Average entry price after the adjustment when the policy reports it.
  std::optional<param::Price> averageEntryPrice;

  AccountOutcomeEntry() = default;

  [[nodiscard]] static AccountOutcomeEntry FromRaw(
      const OpenPitAccountOutcomeEntry& raw) {
    AccountOutcomeEntry out;
    out.asset = ::openpit::StringView(raw.asset).ToString();
    out.balance = ReadAmount(raw.balance);
    out.held = ReadAmount(raw.held);
    out.incoming = ReadAmount(raw.incoming);
    out.realizedPnl = ReadPnlAmount(raw.realized_pnl);
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice =
          param::Price::FromRaw(raw.average_entry_price.value);
    }
    return out;
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountOutcomeEntry Raw() const noexcept {
    OpenPitAccountOutcomeEntry raw{};
    raw.asset = ::openpit::MakeStringView(asset);
    WriteAmount(raw.balance, balance);
    WriteAmount(raw.held, held);
    WriteAmount(raw.incoming, incoming);
    WritePnlAmount(raw.realized_pnl, realizedPnl);
    if (averageEntryPrice) {
      raw.average_entry_price.value = averageEntryPrice->Raw();
      raw.average_entry_price.is_set = true;
    }
    return raw;
  }

 private:
  [[nodiscard]] static std::optional<OutcomeAmount> ReadAmount(
      const OutcomeAmountOptional& field) {
    if (!field.is_set) {
      return std::nullopt;
    }
    return OutcomeAmount::FromRaw(field.value);
  }

  static void WriteAmount(OutcomeAmountOptional& field,
                          const std::optional<OutcomeAmount>& value) noexcept {
    if (value) {
      field.value = value->Raw();
      field.is_set = true;
    }
  }

  [[nodiscard]] static std::optional<PnlOutcomeAmount> ReadPnlAmount(
      const PnlOutcomeAmountOptional& field) {
    if (!field.is_set) {
      return std::nullopt;
    }
    return PnlOutcomeAmount::FromRaw(field.value);
  }

  static void WritePnlAmount(
      PnlOutcomeAmountOptional& field,
      const std::optional<PnlOutcomeAmount>& value) noexcept {
    if (value) {
      field.value = value->Raw();
      field.is_set = true;
    }
  }
};

//------------------------------------------------------------------------------
// Outcome

// One account-adjustment outcome tagged with the policy group that produced it.
struct Outcome {
  param::GroupId policyGroupId;
  AccountOutcomeEntry entry;

  Outcome() = default;

  [[nodiscard]] static Outcome FromRaw(
      const OpenPitAccountAdjustmentOutcome& raw) {
    Outcome out;
    out.policyGroupId = param::GroupId(raw.policy_group_id);
    out.entry = AccountOutcomeEntry::FromRaw(raw.entry);
    return out;
  }

  // Borrows this object's entry storage; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentOutcome Raw() const noexcept {
    OpenPitAccountAdjustmentOutcome raw{};
    raw.policy_group_id = policyGroupId.Raw();
    raw.entry = entry.Raw();
    return raw;
  }
};

//------------------------------------------------------------------------------
// OutcomeList

namespace detail {

struct OutcomeListDeleter {
  void operator()(OpenPitAccountAdjustmentOutcomeList* handle) const noexcept {
    openpit_destroy_account_adjustment_outcome_list(handle);
  }
};

}  // namespace detail

// Owning RAII wrapper over a caller-owned `OpenPitAccountAdjustmentOutcomeList`
// returned by an apply call. Move-only. `Size()`/`Get()` read the borrowed C
// views; `ToVector()` copies every outcome into owned `Outcome` values.
class OutcomeList {
 public:
  OutcomeList() noexcept = default;

  explicit OutcomeList(OpenPitAccountAdjustmentOutcomeList* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  [[nodiscard]] OpenPitAccountAdjustmentOutcomeList* Get() const noexcept {
    return m_handle.Get();
  }

  [[nodiscard]] std::size_t Size() const noexcept {
    if (!m_handle) {
      return 0;
    }
    return openpit_account_adjustment_outcome_list_len(m_handle.Get());
  }

  [[nodiscard]] bool Empty() const noexcept { return Size() == 0; }

  // Copies the outcome at `index`, or `std::nullopt` when out of bounds.
  [[nodiscard]] std::optional<Outcome> Get(std::size_t index) const {
    OpenPitAccountAdjustmentOutcome raw{};
    if (m_handle.Get() == nullptr ||
        !openpit_account_adjustment_outcome_list_get(m_handle.Get(), index,
                                                     &raw)) {
      return std::nullopt;
    }
    return Outcome::FromRaw(raw);
  }

  // Copies every outcome into owned values.
  [[nodiscard]] std::vector<Outcome> ToVector() const {
    const std::size_t count = Size();
    std::vector<Outcome> out;
    out.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
      OpenPitAccountAdjustmentOutcome raw{};
      if (openpit_account_adjustment_outcome_list_get(m_handle.Get(), i,
                                                      &raw)) {
        out.push_back(Outcome::FromRaw(raw));
      }
    }
    return out;
  }

 private:
  ::openpit::detail::Handle<OpenPitAccountAdjustmentOutcomeList,
                            detail::OutcomeListDeleter>
      m_handle;
};

//------------------------------------------------------------------------------
// BatchError

namespace detail {

struct BatchErrorDeleter {
  void operator()(OpenPitAccountAdjustmentBatchError* handle) const noexcept {
    openpit_destroy_account_adjustment_batch_error(handle);
  }
};

}  // namespace detail

// Owning RAII wrapper over a caller-owned `OpenPitAccountAdjustmentBatchError`
// returned by an apply call when a policy rejects the batch. A rejected batch
// is an expected business outcome, so this is a value type, never thrown.
// `FailedAdjustmentIndex()` is the position of the offending adjustment in the
// applied array; `Rejects()` copies the policy rejects that caused it.
// Move-only.
class BatchError {
 public:
  BatchError() noexcept = default;

  explicit BatchError(OpenPitAccountAdjustmentBatchError* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  [[nodiscard]] OpenPitAccountAdjustmentBatchError* Get() const noexcept {
    return m_handle.Get();
  }

  // Index of the failing adjustment within the applied batch.
  [[nodiscard]] std::size_t FailedAdjustmentIndex() const noexcept {
    if (!m_handle) {
      return 0;
    }
    return openpit_account_adjustment_batch_error_get_failed_adjustment_index(
        m_handle.Get());
  }

  // Copies the policy rejects carried by this batch error. The rejects borrow
  // string memory from the batch error only during the copy; the returned
  // values own their strings.
  [[nodiscard]] std::vector<::openpit::reject::Reject> Rejects() const {
    std::vector<::openpit::reject::Reject> out;
    if (!m_handle) {
      return out;
    }
    const OpenPitPretradeRejectList* list =
        openpit_account_adjustment_batch_error_get_rejects(m_handle.Get());
    if (list == nullptr) {
      return out;
    }
    const std::size_t count = openpit_pretrade_reject_list_len(list);
    out.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
      OpenPitPretradeReject raw{};
      if (openpit_pretrade_reject_list_get(list, i, &raw)) {
        out.push_back(::openpit::reject::Reject::FromRaw(raw));
      }
    }
    return out;
  }

 private:
  ::openpit::detail::Handle<OpenPitAccountAdjustmentBatchError,
                            detail::BatchErrorDeleter>
      m_handle;
};

}  // namespace openpit::accountadjustment
