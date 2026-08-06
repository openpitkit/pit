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
#include "openpit/detail/native_access.hpp"
#include "openpit/param/param.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstddef>
#include <iterator>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace openpit::accountadjustment {

namespace detail {
using RawOutcomeAmountOptional = ::OpenPitOutcomeAmountOptional;
using RawOutcomeList = ::OpenPitAccountAdjustmentOutcomeList;
}  // namespace detail

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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static OutcomeAmount FromRaw(const OpenPitOutcomeAmount& raw) {
    return OutcomeAmount(
        ::openpit::detail::FromNative<param::PositionSize>(raw.delta),
        ::openpit::detail::FromNative<param::PositionSize>(raw.absolute));
  }

  [[nodiscard]] OpenPitOutcomeAmount Native() const noexcept {
    OpenPitOutcomeAmount raw{};
    raw.delta = ::openpit::detail::Native(delta);
    raw.absolute = ::openpit::detail::Native(absolute);
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
  /// Account-currency realized-PnL result. Reservations, cancels, settlement
  /// legs, opening, same-direction, and zero-quantity fills without a non-zero
  /// fee omit it, as do non-PnL adjustments. A realizing fill reports an
  /// authoritative result even when its exact contribution is zero. A
  /// non-zero fee reports the underlying asset even if the account never held
  /// it. The first failed calculation reports a halt reason; later operations
  /// omit it until an asset-scoped adjustment force-sets a new PnL.
  std::optional<PnlOutcome> realizedPnl;
  /// Account-currency average entry price after the adjustment.
  std::optional<param::Price> averageEntryPrice;

  explicit AccountOutcomeEntry(param::Asset entryAsset)
      : asset(std::move(entryAsset)) {}

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static AccountOutcomeEntry FromRaw(
      const OpenPitAccountOutcomeEntry& raw) {
    AccountOutcomeEntry out(
        ::openpit::detail::FromNative<param::Asset>(raw.asset));
    out.balance = ReadAmount(raw.balance);
    out.held = ReadAmount(raw.held);
    out.incoming = ReadAmount(raw.incoming);
    out.realizedPnl = ReadPnlOutcome(raw.realized_pnl);
    if (raw.average_entry_price.is_set) {
      out.averageEntryPrice = ::openpit::detail::FromNative<param::Price>(
          raw.average_entry_price.value);
    }
    return out;
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountOutcomeEntry Native() const {
    OpenPitAccountOutcomeEntry raw{};
    raw.asset = ::openpit::detail::Native(asset);
    WriteAmount(raw.balance, balance);
    WriteAmount(raw.held, held);
    WriteAmount(raw.incoming, incoming);
    WritePnlOutcome(raw.realized_pnl, realizedPnl);
    if (averageEntryPrice) {
      raw.average_entry_price.value =
          ::openpit::detail::Native(*averageEntryPrice);
      raw.average_entry_price.is_set = true;
    }
    return raw;
  }

  [[nodiscard]] static std::optional<OutcomeAmount> ReadAmount(
      const detail::RawOutcomeAmountOptional& field) {
    if (!field.is_set) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<OutcomeAmount>(field.value);
  }

  static void WriteAmount(detail::RawOutcomeAmountOptional& field,
                          const std::optional<OutcomeAmount>& value) noexcept {
    if (value) {
      field.value = ::openpit::detail::Native(*value);
      field.is_set = true;
    }
  }

  [[nodiscard]] static std::optional<PnlOutcome> ReadPnlOutcome(
      const detail::RawPnlOutcomeOptional& field) {
    if (!field.is_set) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<PnlOutcome>(field.value);
  }

  static void WritePnlOutcome(detail::RawPnlOutcomeOptional& field,
                              const std::optional<PnlOutcome>& value) {
    if (value) {
      field.value = ::openpit::detail::Native(*value);
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static Outcome FromRaw(
      const OpenPitAccountAdjustmentOutcome& raw) {
    return Outcome(
        param::GroupId(raw.policy_group_id),
        ::openpit::detail::FromNative<AccountOutcomeEntry>(raw.entry));
  }

  // Borrows this object's entry storage; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustmentOutcome Native() const {
    OpenPitAccountAdjustmentOutcome raw{};
    raw.policy_group_id = ::openpit::detail::Native(policyGroupId);
    raw.entry = ::openpit::detail::Native(entry);
    return raw;
  }
};

//------------------------------------------------------------------------------
// OutcomeList

namespace detail {

struct OutcomeListDeleter {
  void operator()(RawOutcomeList* handle) const noexcept {
    openpit_destroy_account_adjustment_outcome_list(handle);
  }
};

}  // namespace detail

// Owning RAII wrapper over a caller-owned `OpenPitAccountAdjustmentOutcomeList`
// returned by an apply call. Move-only. `size()`/`at()` read the borrowed C
// views; `ToVector()` copies every outcome into owned `Outcome` values.
class OutcomeList {
 public:
  class const_iterator {
   public:
    using iterator_category = std::input_iterator_tag;
    using value_type = Outcome;
    using difference_type = std::ptrdiff_t;
    using pointer = void;
    using reference = Outcome;

    const_iterator() noexcept = default;

    [[nodiscard]] Outcome operator*() const { return m_list->at(m_index); }

    const_iterator& operator++() noexcept {
      ++m_index;
      return *this;
    }

    const_iterator operator++(int) noexcept {
      const_iterator previous = *this;
      ++(*this);
      return previous;
    }

    friend bool operator==(const const_iterator& lhs,
                           const const_iterator& rhs) noexcept {
      return lhs.m_list == rhs.m_list && lhs.m_index == rhs.m_index;
    }

    friend bool operator!=(const const_iterator& lhs,
                           const const_iterator& rhs) noexcept {
      return !(lhs == rhs);
    }

   private:
    friend class OutcomeList;
    const_iterator(const OutcomeList* list, std::size_t index) noexcept
        : m_list(list), m_index(index) {}

    const OutcomeList* m_list = nullptr;
    std::size_t m_index = 0;
  };

  OutcomeList() noexcept = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  [[nodiscard]] std::size_t size() const noexcept {
    if (!m_handle) {
      return 0;
    }
    return openpit_account_adjustment_outcome_list_len(m_handle.Get());
  }

  [[nodiscard]] bool empty() const noexcept { return size() == 0; }

  [[nodiscard]] Outcome at(std::size_t index) const {
    OpenPitAccountAdjustmentOutcome raw{};
    if (m_handle.Get() == nullptr ||
        !openpit_account_adjustment_outcome_list_get(m_handle.Get(), index,
                                                     &raw)) {
      throw std::out_of_range("account adjustment outcome index out of range");
    }
    return ::openpit::detail::FromNative<Outcome>(raw);
  }

  [[nodiscard]] Outcome operator[](std::size_t index) const {
    return at(index);
  }

  [[nodiscard]] const_iterator begin() const noexcept {
    return const_iterator(this, 0);
  }

  [[nodiscard]] const_iterator end() const noexcept {
    return const_iterator(this, size());
  }

  // Copies every outcome into owned values.
  [[nodiscard]] std::vector<Outcome> ToVector() const {
    const std::size_t count = size();
    std::vector<Outcome> out;
    out.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
      OpenPitAccountAdjustmentOutcome raw{};
      if (openpit_account_adjustment_outcome_list_get(m_handle.Get(), i,
                                                      &raw)) {
        out.push_back(::openpit::detail::FromNative<Outcome>(raw));
      }
    }
    return out;
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit OutcomeList(detail::RawOutcomeList* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawOutcomeList* Native() const noexcept {
    return m_handle.Get();
  }

  ::openpit::detail::Handle<detail::RawOutcomeList, detail::OutcomeListDeleter>
      m_handle;
};

}  // namespace openpit::accountadjustment
