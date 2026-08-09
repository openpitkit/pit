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

// Applied-but-not-finalized drop-copy state. Move-only RAII: resolve it exactly
// once with `Commit()` or `Rollback()`; destruction rolls back any
// still-pending mutations. Both resolutions are idempotent at the pointer
// level.
//
// Account-control operations and rate-limit attempts are applied before
// `ApplyDropCopy` returns and stay outside this finalization boundary.
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
class DropCopyOperation {
 public:
  DropCopyOperation() = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Finalizes the operation, applying the prepared state permanently. A
  // throwing mutation commit callback rethrows here and arms the engine kill
  // switch; see the class documentation.
  void Commit() {
    detail::RawDropCopyOperation* handle = RequireHandle();
    ::openpit::detail::CallbackExceptionScope callbackExceptions;
    openpit_pretrade_drop_copy_operation_commit(handle);
    callbackExceptions.ThrowIfPending();
  }

  // Cancels the operation, compensating the prepared state. A throwing mutation
  // rollback callback rethrows here and arms the engine kill switch; see the
  // class documentation.
  void Rollback() {
    detail::RawDropCopyOperation* handle = RequireHandle();
    ::openpit::detail::CallbackExceptionScope callbackExceptions;
    openpit_pretrade_drop_copy_operation_rollback(handle);
    callbackExceptions.ThrowIfPending();
  }

  // Returns an owned lock snapshot detached from the operation state.
  [[nodiscard]] PreTradeLock Lock() const {
    return ::openpit::detail::FromNative<PreTradeLock>(
        openpit_pretrade_drop_copy_operation_get_lock(RequireHandle()));
  }

  [[nodiscard]] std::vector<::openpit::accountadjustment::Outcome>
  AccountAdjustments() const {
    const auto outcomes = ::openpit::detail::FromNative<
        ::openpit::accountadjustment::OutcomeList>(
        openpit_pretrade_drop_copy_operation_get_account_adjustments(
            RequireHandle()));
    return outcomes.ToVector();
  }

  // Returns the request's first account block. This is request-local history;
  // use IsAccountBlocked() for the apply-time registry snapshot. Throws `Error`
  // if the C ABI does not provide the promised detached list.
  [[nodiscard]] std::optional<::openpit::accounts::AccountBlock> AccountBlock()
      const {
    OpenPitPretradeAccountBlockList* blocks =
        openpit_pretrade_drop_copy_operation_get_account_block(RequireHandle());
    ::openpit::detail::Handle<OpenPitPretradeAccountBlockList,
                              detail::AccountBlockListDeleter>
        owner(blocks);
    if (blocks == nullptr) {
      throw ::openpit::Error(
          "openpit_pretrade_drop_copy_operation_get_account_block returned "
          "null");
    }
    if (openpit_pretrade_account_block_list_len(owner.Get()) == 0) {
      return std::nullopt;
    }
    OpenPitPretradeAccountBlock raw{};
    if (!openpit_pretrade_account_block_list_get(owner.Get(), 0, &raw)) {
      throw ::openpit::Error("openpit_pretrade_account_block_list_get failed");
    }
    return ::openpit::detail::FromNative<::openpit::accounts::AccountBlock>(
        raw);
  }

  // Returns the blocked-state snapshot captured before ApplyDropCopy returned.
  // The snapshot does not track later registry changes.
  [[nodiscard]] bool IsAccountBlocked() const {
    return openpit_pretrade_drop_copy_operation_is_account_blocked(
        RequireHandle());
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit DropCopyOperation(detail::RawDropCopyOperation* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawDropCopyOperation* Native() const noexcept {
    return m_handle.Get();
  }

  [[nodiscard]] detail::RawDropCopyOperation* RequireHandle() const {
    if (!m_handle) {
      throw ::openpit::Error("drop-copy operation is empty");
    }
    return m_handle.Get();
  }

  ::openpit::detail::Handle<detail::RawDropCopyOperation,
                            detail::DropCopyOperationDeleter>
      m_handle;
};

// Outcome of `ApplyDropCopy`: exactly one channel is populated. An `operation`
// means the order was applied and the prepared state awaits resolution; a
// non-empty `rejects` means a fatal evaluation failure aborted the operation.
// Runtime failures throw instead of producing this value.
struct DropCopyResult {
  std::optional<DropCopyOperation> operation;
  std::vector<::openpit::pretrade::Reject> rejects;

  // Whether the order was applied and an operation is available.
  [[nodiscard]] bool Passed() const noexcept { return operation.has_value(); }
};

}  // namespace openpit::pretrade
