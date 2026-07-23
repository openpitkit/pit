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

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts/accounts.hpp"
#include "openpit/detail/callback_error.hpp"
#include "openpit/detail/native_access.hpp"
#include "openpit/error.hpp"
#include "openpit/pretrade/detail/lists.hpp"
#include "openpit/pretrade/detail/request.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"

#include <openpit.h>

#include <optional>
#include <vector>

namespace openpit::pretrade {

// Reserved-but-not-finalized pre-trade state. Move-only RAII: resolve it
// exactly once with `Commit()` or `Rollback()`; destruction rolls back any
// still-pending mutations. Both resolutions are idempotent at the pointer
// level.
class Reservation {
 public:
  Reservation() = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Finalizes the reservation, applying the reserved state permanently.
  void Commit() {
    detail::RawReservation* handle = RequireHandle();
    ::openpit::detail::ClearPendingCallbackException();
    openpit_pretrade_pre_trade_reservation_commit(handle);
    ::openpit::detail::ThrowIfPendingCallbackException();
  }

  // Cancels the reservation, releasing the reserved state.
  void Rollback() {
    detail::RawReservation* handle = RequireHandle();
    ::openpit::detail::ClearPendingCallbackException();
    openpit_pretrade_pre_trade_reservation_rollback(handle);
    ::openpit::detail::ThrowIfPendingCallbackException();
  }

  // Returns an owned lock snapshot detached from the reservation state.
  [[nodiscard]] PreTradeLock Lock() const {
    return ::openpit::detail::FromNative<PreTradeLock>(
        openpit_pretrade_pre_trade_reservation_get_lock(RequireHandle()));
  }

  [[nodiscard]] std::vector<::openpit::accountadjustment::Outcome>
  AccountAdjustments() const {
    const auto outcomes = ::openpit::detail::FromNative<
        ::openpit::accountadjustment::OutcomeList>(
        openpit_pretrade_pre_trade_reservation_get_account_adjustments(
            RequireHandle()));
    return outcomes.ToVector();
  }

  /// Returns the winning account block produced by this reservation's
  /// pre-trade pipeline.
  [[nodiscard]] std::optional<::openpit::accounts::AccountBlock> AccountBlock()
      const {
    OpenPitPretradeAccountBlockList* blocks =
        openpit_pretrade_pre_trade_reservation_get_account_block(
            RequireHandle());
    ::openpit::detail::Handle<OpenPitPretradeAccountBlockList,
                              detail::AccountBlockListDeleter>
        owner(blocks);
    if (blocks == nullptr) {
      return std::nullopt;
    }
    OpenPitPretradeAccountBlock raw{};
    std::optional<::openpit::accounts::AccountBlock> out;
    if (openpit_pretrade_account_block_list_get(owner.Get(), 0, &raw)) {
      out =
          ::openpit::detail::FromNative<::openpit::accounts::AccountBlock>(raw);
    }
    return out;
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit Reservation(detail::RawReservation* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawReservation* Native() const noexcept {
    return m_handle.Get();
  }

  [[nodiscard]] detail::RawReservation* RequireHandle() const {
    if (!m_handle) {
      throw ::openpit::Error("pre-trade reservation is empty");
    }
    return m_handle.Get();
  }

  ::openpit::detail::Handle<detail::RawReservation,
                            detail::PreTradeReservationDeleter>
      m_handle;
};

}  // namespace openpit::pretrade
