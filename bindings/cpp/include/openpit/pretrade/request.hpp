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

#include "openpit/detail/callback_error.hpp"
#include "openpit/detail/native_access.hpp"
#include "openpit/error.hpp"
#include "openpit/model/model.hpp"
#include "openpit/pretrade/detail/lists.hpp"
#include "openpit/pretrade/detail/request.hpp"
#include "openpit/pretrade/reservation.hpp"

#include <openpit.h>

#include <cstddef>
#include <memory>
#include <optional>
#include <string_view>
#include <vector>

namespace openpit::pretrade {

/// Outcome of running the full pre-trade pipeline (`ExecutePreTrade` or
/// `Request::Execute`): exactly one channel is populated. A `reservation` means
/// the order passed and reserved state awaits resolution; a non-empty `rejects`
/// means it was rejected. The engine's reserved review-rollback report is a
/// rejected result; callback exceptions otherwise propagate as documented by
/// `ExecutePreTrade` and `Request::Execute`. Runtime boundary failures throw
/// `openpit::Error` instead of producing this value.
struct ExecuteResult;

// Deferred pre-trade request returned by `StartPreTrade`. Move-only RAII:
// `Execute()` runs the remaining stages once; destruction abandons an
// unexecuted request without creating a reservation. The request owns the
// submitted order and preserves its concrete type for the main stage.
class Request {
 public:
  Request() = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  /// Runs the main-stage pre-trade checks.
  ///
  /// If discarding a review instance's tentative state fails, returns the
  /// rejected result with policy "Engine", scope `RejectScope::Order`, code
  /// `RejectCode::SystemUnavailable`, and reason "review rollback failed",
  /// instead of throwing the rollback callback's exception. The engine kill
  /// switch is armed; later policies still ran. A policy callback exception in
  /// the same call takes precedence and propagates. Any other mutation rollback
  /// failure, including ordinary reject compensation, propagates with its
  /// original exception type, unless a review rollback in the same call also
  /// failed: then the full reject list is returned and nothing is thrown.
  ///
  /// This identity is the engine's reserved report. A custom policy emitting
  /// it is treated as the engine, so a mutation rollback exception parked in
  /// the same call is not thrown.
  [[nodiscard]] ExecuteResult Execute();

  /// Returns the start-stage review marks. A non-empty list requires a human
  /// decision before sending the order. Throws `Error` for an empty or
  /// executed request. After `Execute()`, read marks from the reservation.
  [[nodiscard]] std::vector<::openpit::pretrade::Reject> Reviews() const {
    OpenPitSharedString* error = nullptr;
    OpenPitPretradeRejectList* reviews =
        openpit_pretrade_pre_trade_request_get_reviews(RequireHandle(), &error);
    if (reviews == nullptr) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_pre_trade_request_get_reviews failed");
    }
    return detail::ListAccess::DrainRejects(reviews);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit Request(detail::RequestInit init) noexcept
      : m_order(std::move(init.order)), m_handle(init.handle) {}

  [[nodiscard]] detail::RawRequest* Native() const noexcept {
    return m_handle.Get();
  }

  [[nodiscard]] detail::RawRequest* RequireHandle() const {
    if (!m_handle) {
      throw ::openpit::Error("pre-trade request is empty");
    }
    return m_handle.Get();
  }

  [[nodiscard]] const ::openpit::Order& RequireOrder() const {
    if (!m_order) {
      throw ::openpit::Error("pre-trade request has no order");
    }
    return *m_order;
  }

  std::unique_ptr<const ::openpit::Order> m_order;
  ::openpit::detail::Handle<detail::RawRequest, detail::PreTradeRequestDeleter>
      m_handle;
};

struct ExecuteResult {
  std::optional<Reservation> reservation;
  std::vector<::openpit::pretrade::Reject> rejects;

  /// Returns whether a reservation is available. If its `Reviews()` is
  /// non-empty, a human decision is required before sending the order.
  [[nodiscard]] bool Passed() const noexcept { return reservation.has_value(); }
};

namespace detail {

// Drops a parked mutation-callback exception when a rejected execute carries
// the engine's reserved review-rollback report. Reads the raw list without
// decoding it, so a path that still rethrows cannot lose the exception.
inline void DiscardReviewRollbackException(
    OpenPitPretradeStatus status, const OpenPitPretradeRejectList* rejects,
    ::openpit::detail::CallbackExceptionScope& callbackExceptions) noexcept {
  if (status != OpenPitPretradeStatus_Rejected ||
      !callbackExceptions.HasPendingMutation() || rejects == nullptr) {
    return;
  }
  const auto equals = [](OpenPitStringView view,
                         std::string_view expected) noexcept {
    return view.ptr != nullptr &&
           std::string_view(reinterpret_cast<const char*>(view.ptr),
                            view.len) == expected;
  };
  const std::size_t count = openpit_pretrade_reject_list_len(rejects);
  for (std::size_t index = 0; index < count; ++index) {
    OpenPitPretradeReject raw{};
    // The engine's reserved review-rollback report must stay equal to
    // new_review_rollback_reject in the core.
    if (openpit_pretrade_reject_list_get(rejects, index, &raw) &&
        equals(raw.policy, "Engine") &&
        raw.scope ==
            static_cast<OpenPitPretradeRejectScope>(RejectScope::Order) &&
        raw.code == static_cast<OpenPitPretradeRejectCode>(
                        RejectCode::SystemUnavailable) &&
        equals(raw.reason, "review rollback failed")) {
      callbackExceptions.DiscardPending();
      return;
    }
  }
}

}  // namespace detail

[[nodiscard]] inline ExecuteResult Request::Execute() {
  OpenPitPretradePreTradeReservation* reservation = nullptr;
  OpenPitPretradeRejectList* rejects = nullptr;
  OpenPitSharedString* error = nullptr;
  detail::RawRequest* const request = RequireHandle();
  const ::openpit::detail::CurrentOrderGuard orderGuard(RequireOrder());
  ::openpit::detail::CallbackExceptionScope callbackExceptions;
  const OpenPitPretradeStatus status =
      openpit_pretrade_pre_trade_request_execute(request, &reservation,
                                                 &rejects, &error);
  detail::DiscardReviewRollbackException(status, rejects, callbackExceptions);
  if (callbackExceptions.HasPending()) {
    openpit_destroy_pretrade_pre_trade_reservation(reservation);
    openpit_destroy_pretrade_reject_list(rejects);
    openpit_destroy_shared_string(error);
  }
  callbackExceptions.ThrowIfPending();
  if (status == OpenPitPretradeStatus_Error) {
    ::openpit::detail::ThrowFromSharedString(
        error, "openpit_pretrade_pre_trade_request_execute failed");
  }
  ExecuteResult out;
  if (status == OpenPitPretradeStatus_Rejected) {
    if (rejects == nullptr) {
      throw ::openpit::Error(
          "openpit_pretrade_pre_trade_request_execute returned Rejected "
          "without rejects");
    }
    out.rejects = detail::ListAccess::DrainRejects(rejects);
    return out;
  }
  if (status != OpenPitPretradeStatus_Passed) {
    throw ::openpit::Error(
        "openpit_pretrade_pre_trade_request_execute returned an invalid "
        "status");
  }
  if (reservation == nullptr) {
    throw ::openpit::Error(
        "openpit_pretrade_pre_trade_request_execute returned Passed "
        "without a reservation");
  }
  out.reservation = ::openpit::detail::FromNative<Reservation>(reservation);
  return out;
}

}  // namespace openpit::pretrade
