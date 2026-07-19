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
#include "openpit/detail/handle.hpp"
#include "openpit/param.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstddef>
#include <optional>
#include <string>
#include <vector>

namespace openpit::accountadjustment {

using OutcomeAmountOptional = OpenPitOutcomeAmountOptional;

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
// AccountOutcomeEntry

// Per-asset outcome an adjustment produced: the affected `asset` plus the
// settled `balance`, `held`, `incoming`, realized PnL, and average-entry-price
// amounts. Each amount is absent (empty optional) when its C `is_set` flag is
// false.
struct AccountOutcomeEntry {
  param::Asset asset;
  std::optional<OutcomeAmount> balance;
  std::optional<OutcomeAmount> held;
  std::optional<OutcomeAmount> incoming;
  /// Realized-PnL result. The first failed calculation has a halt reason;
  /// later operations omit it until an adjustment force-sets a new PnL. A
  /// numeric result is denominated in the account currency.
  std::optional<PnlOutcome> realizedPnl;
  /// Account-currency average entry price after the adjustment.
  std::optional<param::Price> averageEntryPrice;

  explicit AccountOutcomeEntry(param::Asset entryAsset)
      : asset(std::move(entryAsset)) {}

  [[nodiscard]] static AccountOutcomeEntry FromRaw(
      const OpenPitAccountOutcomeEntry& raw) {
    AccountOutcomeEntry out(param::Asset::FromRaw(raw.asset));
    out.balance = ReadAmount(raw.balance);
    out.held = ReadAmount(raw.held);
    out.incoming = ReadAmount(raw.incoming);
    out.realizedPnl = ReadPnlOutcome(raw.realized_pnl);
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice =
          param::Price::FromRaw(raw.average_entry_price.value);
    }
    return out;
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountOutcomeEntry Raw() const noexcept {
    OpenPitAccountOutcomeEntry raw{};
    raw.asset = asset.Raw();
    WriteAmount(raw.balance, balance);
    WriteAmount(raw.held, held);
    WriteAmount(raw.incoming, incoming);
    WritePnlOutcome(raw.realized_pnl, realizedPnl);
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

  [[nodiscard]] static std::optional<PnlOutcome> ReadPnlOutcome(
      const PnlOutcomeOptional& field) {
    if (!field.is_set) {
      return std::nullopt;
    }
    return PnlOutcome::FromRaw(field.value);
  }

  static void WritePnlOutcome(PnlOutcomeOptional& field,
                              const std::optional<PnlOutcome>& value) noexcept {
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

  Outcome(param::GroupId groupId, AccountOutcomeEntry outcomeEntry)
      : policyGroupId(groupId), entry(std::move(outcomeEntry)) {}

  [[nodiscard]] static Outcome FromRaw(
      const OpenPitAccountAdjustmentOutcome& raw) {
    return Outcome(param::GroupId(raw.policy_group_id),
                   AccountOutcomeEntry::FromRaw(raw.entry));
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

}  // namespace openpit::accountadjustment
