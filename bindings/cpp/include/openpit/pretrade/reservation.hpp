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
#include "openpit/pretrade/detail/lists.hpp"
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
//
// Inspect `Reviews()` before sending. Keep a reviewed order unsent and call
// `Commit()` immediately. The reservation retains holds only when every mark
// is `ReviewWithHold`.
// A policy instance registered in a review mode that marked the order in its
// main-stage check retains none of its own. Save the first run's `Lock()`.
// `alreadyHeld` asserts Spot Funds' own hold. Spot Funds kept it only when
// every mark is with-hold and no Spot Funds instance registered in a review
// mode marked the order. On approval, rerun with `confirmed`, and also set
// `alreadyHeld` only if Spot Funds kept its hold. Otherwise the rerun carries
// `confirmed` only.
// With `alreadyHeld`, Spot Funds runs every check against the current balance
// with this order's own hold given back, writes nothing, and adds no prices
// to the rerun's lock. For a market order, this run prices the hold it gives
// back, so a price move since the first run shifts that amount. Keep a clean
// rerun's reservation until sending, and call `Commit()` once the venue accepts
// the order. If it is not sent or the venue does not accept it, call
// `Rollback()` and, if Spot Funds kept its hold in the review, release that
// hold as described below.
//
// Once an already-held rerun is committed, for each report of the order use the
// rerun's lock supplemented with the first run's Spot Funds prices. Read each
// Spot Funds group's prices with `PreTradeLock::PricesOf()` and append them
// with `PreTradeLock::PushMany()`. Merging the whole first lock would duplicate
// prices for groups recorded in both runs. If Spot Funds is the only policy
// that locks prices, use the first lock directly. Passing only the rerun's lock
// blocks the account with `RejectCode::MissingRequiredField`.
//
// Once a review in which Spot Funds kept its hold is committed, only a final
// cancel execution report carrying the remaining reserved quantity releases
// that hold. Send it whenever the order ends without a committed rerun: the
// reviewer rejects it, the rerun rejects it, or the rerun is rolled back
// because the order is not sent or the venue does not accept it.
// That report carries the first run's lock.
// Rolling back or rejecting the rerun does not touch the first run's hold.
// A without-hold review holds nothing and needs no cancel report.
// When a Spot Funds instance registered in a review mode marked the order, that
// instance holds nothing, and a final cancel report that carries the first
// run's lock and a remaining reserved quantity blocks the account with
// `RejectCode::MissingRequiredField`, because that lock has no price for that
// instance.
// The engine trusts `alreadyHeld` without checking whether a matching hold
// exists. Setting it when Spot Funds kept no hold leaves the order without a
// Spot Funds hold.
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

  /// Returns the review marks from both stages. A non-empty list requires a
  /// human decision before sending the order.
  [[nodiscard]] std::vector<::openpit::pretrade::Reject> Reviews() const {
    return detail::ListAccess::DrainRejects(
        openpit_pretrade_pre_trade_reservation_get_reviews(RequireHandle()));
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
