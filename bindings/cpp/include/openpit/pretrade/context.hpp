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
#include "openpit/accounts.hpp"
#include "openpit/model.hpp"
#include "openpit/reject.hpp"

#include <openpit.h>

#include <optional>
#include <string>
#include <string_view>
#include <utility>

// Pre-trade context wrapper.
//
// `Context` is the main-stage pre-trade context handed to a policy callback. It
// bundles two things a policy needs:
//   - the order under check, as the polymorphic `openpit::Order` base, so the
//     adapter templates in `openpit/adapters.hpp` can recover a client order
//     type via `dynamic_cast` (see `ContextOrder`);
//   - the borrowed callback-scoped native context pointer
//     (`OpenPitPretradeContext*`), exposing the account-group / account-control
//     queries the native runtime provides on it.
//
// A `Context` is non-owning and valid only for the duration of the callback
// that produced it: the referenced order and the native context pointer both
// outlive it only within that callback. It is neither copyable nor movable, to
// discourage retaining it past the callback.
//
// This header also defines the three free functions the existing adapter header
// (`openpit/adapters.hpp`) forward-declares and calls:
// `MakeTypeMismatchReject`, `PushReject`, and `ContextOrder`.

namespace openpit::pretrade {

/// \brief Main-stage pre-trade context passed to a custom policy check.
//
// Main-stage pre-trade context.
//
// Construct one around the order being checked plus the borrowed native context
// pointer. The native pointer may be null in unit tests that exercise a policy
// without a live engine; the account queries then report "absent".
class Context {
 public:
  // Wraps an order plus the borrowed callback-scoped native context pointer.
  // Neither is copied; both must outlive this `Context`.
  Context(const ::openpit::Order& order,
          const OpenPitPretradeContext* native) noexcept
      : m_order(&order), m_native(native) {}

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

  // The borrowed native context pointer; null when none was supplied.
  [[nodiscard]] const OpenPitPretradeContext* Native() const noexcept {
    return m_native;
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
    return ::openpit::accounts::AccountControl(control);
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
      return ::openpit::param::AccountGroupId::FromRaw(group);
    }
    return std::nullopt;
  }

 private:
  const ::openpit::Order* m_order;
  const OpenPitPretradeContext* m_native;
};

// Builds an order-or-account scoped reject carrying an "expected type" detail.
//
// Defined here to satisfy the forward declaration in `openpit/adapters.hpp`;
// the adapter templates call it to report a payload type mismatch
// deterministically. `expected_type_name` is stored in the reject `details`.
[[nodiscard]] inline Reject MakeTypeMismatchReject(
    std::string_view policy_name, RejectScope scope, RejectCode code,
    std::string_view reason, std::string_view expected_type_name) {
  return Reject(std::string(policy_name), scope, code, std::string(reason),
                std::string(expected_type_name));
}

// Appends a reject to a policy decision. Satisfies the forward declaration in
// `openpit/adapters.hpp`.
inline void PushReject(PolicyDecision& decision, Reject reject) {
  decision.Push(std::move(reject));
}

// Recovers the polymorphic order base from a `Context`. Satisfies the forward
// declaration in `openpit/adapters.hpp`; the adapter templates `dynamic_cast`
// the result to a client order type.
[[nodiscard]] inline const ::openpit::Order& ContextOrder(
    const Context& context) {
  return context.Order();
}

}  // namespace openpit::pretrade
