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
#include "openpit/detail/callback_error.hpp"
#include "openpit/detail/native_access.hpp"
#include "openpit/error.hpp"
#include "openpit/pretrade/detail/request.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"

#include <openpit.h>

#include <vector>

namespace openpit::pretrade {

// Reserved-but-not-finalized pre-trade state. Move-only RAII: resolve it
// exactly once with `Commit()` or `Rollback()`; destruction rolls back any
// still-pending mutations. Both resolutions are idempotent at the pointer
// level.
//
// Finalization has no right to fail. A mutation callback that throws never
// fails `Commit()` or `Rollback()`: its original exception is rethrown once the
// rest of the batch has run, and independently the engine arms its kill switch.
// Every mutation a C++ policy registers is a custom policy's, so that kill
// switch blocks EVERY account; the finalizing caller is not told, it surfaces
// when the next pre-trade call is rejected with
// `RejectCode::SystemUnavailable`, and an operator clears it with
// `accounts::Accounts::UnblockAll()`. The implicit rollback on destruction arms
// the same kill switch but cannot rethrow.
class Reservation {
 public:
  Reservation() = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Finalizes the reservation, applying the reserved state permanently. A
  // throwing mutation commit callback rethrows here and arms the engine kill
  // switch; see the class documentation.
  void Commit() {
    detail::RawReservation* handle = RequireHandle();
    ::openpit::detail::CallbackExceptionScope callbackExceptions;
    openpit_pretrade_pre_trade_reservation_commit(handle);
    callbackExceptions.ThrowIfPending();
  }

  // Cancels the reservation, releasing the reserved state. A throwing mutation
  // rollback callback rethrows here and arms the engine kill switch; see the
  // class documentation.
  void Rollback() {
    detail::RawReservation* handle = RequireHandle();
    ::openpit::detail::CallbackExceptionScope callbackExceptions;
    openpit_pretrade_pre_trade_reservation_rollback(handle);
    callbackExceptions.ThrowIfPending();
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
