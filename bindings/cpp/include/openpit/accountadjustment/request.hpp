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

#include "openpit/accountadjustment/amount.hpp"
#include "openpit/accountadjustment/bounds.hpp"
#include "openpit/accountadjustment/operation.hpp"

#include <openpit.h>

#include <cstdint>
#include <optional>

namespace openpit::accountadjustment {

namespace detail {

using RawAccountAdjustment = ::OpenPitAccountAdjustment;

}  // namespace detail

//------------------------------------------------------------------------------
// AccountAdjustment

// Full adjustment request payload. The `operation`, `amount`, and `bounds`
// groups are each optional; `userData` is an opaque caller token the SDK never
// inspects (zero means unset). The account this applies to is not part of the
// payload: it is passed separately to `Engine::ApplyAccountAdjustment`.
struct AccountAdjustment {
  std::optional<Operation> operation;
  std::optional<Amount> amount;
  std::optional<Bounds> bounds;
  std::uintptr_t userData = 0;

  AccountAdjustment() = default;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static AccountAdjustment FromRaw(
      const detail::RawAccountAdjustment& raw) {
    AccountAdjustment out;
    out.operation = ::openpit::detail::FromNative<Operation>(raw.operation);
    if (raw.amount.is_set) {
      out.amount = ::openpit::detail::FromNative<Amount>(raw.amount.value);
    }
    if (raw.bounds.is_set) {
      out.bounds = ::openpit::detail::FromNative<Bounds>(raw.bounds.value);
    }
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  [[nodiscard]] detail::RawAccountAdjustment Native() const {
    detail::RawAccountAdjustment raw{};
    if (operation) {
      raw.operation = ::openpit::detail::Native(*operation);
    }
    if (amount) {
      raw.amount.value = ::openpit::detail::Native(*amount);
      raw.amount.is_set = true;
    }
    if (bounds) {
      raw.bounds.value = ::openpit::detail::Native(*bounds);
      raw.bounds.is_set = true;
    }
    raw.user_data = reinterpret_cast<void*>(userData);
    return raw;
  }
};

}  // namespace openpit::accountadjustment
