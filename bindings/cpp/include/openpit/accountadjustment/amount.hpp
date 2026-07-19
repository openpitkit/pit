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

#include "openpit/param.hpp"

#include <openpit.h>

#include <optional>

namespace openpit::accountadjustment {

//------------------------------------------------------------------------------
// Amount

// Optional amount-change group of an adjustment: a signed delta/absolute change
// to the `balance`, `held`, and `incoming` components. Each component is absent
// (empty optional) when its C kind is `NotSet`; the whole group is absent on
// the owning `AccountAdjustment` when its C `is_set` flag is false. Each
// present value is a `param::AdjustmentAmount`, which may be negative.
struct Amount {
  std::optional<param::AdjustmentAmount> balance;
  std::optional<param::AdjustmentAmount> held;
  std::optional<param::AdjustmentAmount> incoming;

  Amount() = default;

  [[nodiscard]] static Amount FromRaw(
      const OpenPitAccountAdjustmentAmount& raw) {
    Amount out;
    out.balance = param::AdjustmentAmount::FromRaw(raw.balance);
    out.held = param::AdjustmentAmount::FromRaw(raw.held);
    out.incoming = param::AdjustmentAmount::FromRaw(raw.incoming);
    return out;
  }

  [[nodiscard]] OpenPitAccountAdjustmentAmount Raw() const noexcept {
    OpenPitAccountAdjustmentAmount raw{};
    if (balance) {
      raw.balance = balance->Raw();
    }
    if (held) {
      raw.held = held->Raw();
    }
    if (incoming) {
      raw.incoming = incoming->Raw();
    }
    return raw;
  }
};

}  // namespace openpit::accountadjustment
