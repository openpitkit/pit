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

#include "openpit/accounts/accounts.hpp"
#include "openpit/detail/callback_error.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/decision.hpp"

#include <openpit.h>

#include <functional>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

// Pre-trade context wrapper.
//
// `Context` is the main-stage pre-trade context handed to a policy callback. It
// bundles two things a policy needs:
//   - the order under check, as the polymorphic `openpit::Order` base, so the
//     adapter templates in `openpit/pretrade/adapters.hpp` can recover a client
//     order
//     type via `dynamic_cast` (see `ContextOrder`);
//   - callback-scoped account-group and account-control access.
//
// A `Context` is non-owning and valid only for the duration of the callback
// that produced it: the referenced order and its context both outlive it only
// within that callback. It is neither copyable nor movable, to
// discourage retaining it past the callback.
//
// This header also defines the two free functions the existing adapter header
// (`openpit/pretrade/adapters.hpp`) forward-declares and calls:
// `PushReject` and `ContextOrder`.

namespace openpit::pretrade {

namespace detail {

struct ContextInit {
  const ::openpit::Order& order;
  const OpenPitPretradeContext* native;
};

}  // namespace detail

/// \brief Main-stage pre-trade context passed to a custom policy check.
//
// Main-stage pre-trade context.
//
// Construct one around the order being checked plus the borrowed native context
// pointer. The native pointer may be null in unit tests that exercise a policy
// without a live engine; the account queries then report "absent".
class Context {
 public:
  // Convenience overload for callers without a native context (e.g. tests).
  explicit Context(const ::openpit::Order& order) noexcept
      : m_order(&order), m_native(nullptr) {}

  Context(const Context&) = delete;
  Context& operator=(const Context&) = delete;
  Context(Context&&) = delete;
  Context& operator=(Context&&) = delete;
  ~Context() = default;

  // The order under check, as the polymorphic base. Same reference
  // `ContextOrder(*this)` returns.
  [[nodiscard]] const ::openpit::Order& Order() const noexcept {
    return *m_order;
  }

  // Whether ordinary policy rejects are non-enforcing for this operation.
  [[nodiscard]] bool IsDropCopy() const noexcept {
    return m_native != nullptr &&
           openpit_pretrade_context_is_drop_copy(m_native);
  }

  // Registers a start-stage mutation owned by the current atomic drop-copy
  // operation. Apply tentative state before registration: commit finalizes it,
  // while rollback reverses it even if commit was not reached. The context
  // must come from a drop-copy callback.
  template <typename Commit, typename Rollback>
  void RecordDropCopyStartMutation(Commit&& commit, Rollback&& rollback) const {
    auto* callbacks = new StartMutationCallbacks(
        std::function<void()>(std::forward<Commit>(commit)),
        std::function<void()>(std::forward<Rollback>(rollback)));
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_context_record_drop_copy_start_mutation(
            m_native, &StartMutationCommitTrampoline,
            &StartMutationRollbackTrampoline, callbacks,
            &FreeStartMutationTrampoline, &error)) {
      delete callbacks;
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_pretrade_context_record_drop_copy_start_mutation failed");
    }
  }

  // Account-control handle for the account bound to this request, or
  // `std::nullopt` when the order carries no account id. The handle may be
  // cloned into a mutation callback, but must not outlive this pre-trade
  // transaction.
  [[nodiscard]] std::optional<::openpit::accounts::AccountControl>
  AccountControl() const {
    if (m_native == nullptr) {
      return std::nullopt;
    }
    OpenPitAccountControl* control =
        openpit_pretrade_context_get_account_control(m_native);
    if (control == nullptr) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<::openpit::accounts::AccountControl>(
        control);
  }

  // The account-group id for the order's bound account, or `std::nullopt` when
  // no account was bound or it belongs to no group. Mirrors
  // `openpit_pretrade_context_get_account_group`.
  [[nodiscard]] std::optional<::openpit::param::AccountGroupId> AccountGroup()
      const {
    if (m_native == nullptr) {
      return std::nullopt;
    }
    OpenPitParamAccountGroupId group = 0;
    if (openpit_pretrade_context_get_account_group(m_native, &group)) {
      return ::openpit::detail::FromNative<::openpit::param::AccountGroupId>(
          group);
    }
    return std::nullopt;
  }

 private:
  struct StartMutationCallbacks {
    StartMutationCallbacks(std::function<void()> onCommit,
                           std::function<void()> onRollback)
        : commit(std::move(onCommit)), rollback(std::move(onRollback)) {}

    std::function<void()> commit;
    std::function<void()> rollback;
  };

  static bool StartMutationCommitTrampoline(
      void* userData, OpenPitSharedString** outError) noexcept {
    try {
      static_cast<StartMutationCallbacks*>(userData)->commit();
      return true;
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException(outError);
      return false;
    }
  }

  static bool StartMutationRollbackTrampoline(
      void* userData, OpenPitSharedString** outError) noexcept {
    try {
      static_cast<StartMutationCallbacks*>(userData)->rollback();
      return true;
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException(outError);
      return false;
    }
  }

  static void FreeStartMutationTrampoline(void* userData) noexcept {
    delete static_cast<StartMutationCallbacks*>(userData);
  }

  friend class ::openpit::detail::NativeAccess;

  explicit Context(detail::ContextInit init) noexcept
      : m_order(&init.order), m_native(init.native) {}

  [[nodiscard]] const OpenPitPretradeContext* Native() const noexcept {
    return m_native;
  }

  const ::openpit::Order* m_order;
  const OpenPitPretradeContext* m_native;
};

// Appends a reject to a policy decision. Satisfies the forward declaration in
// `openpit/pretrade/adapters.hpp`.
inline void PushReject(PolicyDecision& decision, Reject reject) {
  decision.Push(std::move(reject));
}

// Recovers the polymorphic order base from a `Context`. Satisfies the forward
// declaration in `openpit/pretrade/adapters.hpp`; the adapter templates
// `dynamic_cast`
// the result to a client order type.
[[nodiscard]] inline const ::openpit::Order& ContextOrder(
    const Context& context) {
  return context.Order();
}

}  // namespace openpit::pretrade
