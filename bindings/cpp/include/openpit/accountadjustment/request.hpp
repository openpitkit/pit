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

//------------------------------------------------------------------------------
// AccountAdjustment

// Full adjustment request payload mirroring the native runtime
// `OpenPitAccountAdjustment`. The `operation`, `amount`, and `bounds` groups
// are each optional; `userData` is an opaque caller token the SDK never
// inspects (zero means unset). The account this applies to is not part of the
// payload: it is passed separately to `Engine::ApplyAccountAdjustment`.
struct AccountAdjustment {
  std::optional<Operation> operation;
  std::optional<Amount> amount;
  std::optional<Bounds> bounds;
  std::uintptr_t userData = 0;

  AccountAdjustment() = default;

  [[nodiscard]] static AccountAdjustment FromRaw(
      const OpenPitAccountAdjustment& raw) {
    AccountAdjustment out;
    out.operation = Operation::FromRaw(raw.operation);
    if (raw.amount.is_set) {
      out.amount = Amount::FromRaw(raw.amount.value);
    }
    if (raw.bounds.is_set) {
      out.bounds = Bounds::FromRaw(raw.bounds.value);
    }
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  [[nodiscard]] OpenPitAccountAdjustment Raw() const noexcept {
    OpenPitAccountAdjustment raw{};
    if (operation) {
      raw.operation = operation->Raw();
    }
    if (amount) {
      raw.amount.value = amount->Raw();
      raw.amount.is_set = true;
    }
    if (bounds) {
      raw.bounds.value = bounds->Raw();
      raw.bounds.is_set = true;
    }
    raw.user_data = reinterpret_cast<void*>(userData);
    return raw;
  }
};

}  // namespace openpit::accountadjustment
