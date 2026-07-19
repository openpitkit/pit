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
// Bounds

// Optional bounds group of an adjustment: per-component upper/lower clamps for
// `balance`, `held`, and `incoming`. Each bound is absent (empty optional) when
// its C `is_set` flag is false; the whole group is absent on the owning
// `AccountAdjustment` when its C `is_set` flag is false.
struct Bounds {
  std::optional<param::PositionSize> balanceUpper;
  std::optional<param::PositionSize> balanceLower;
  std::optional<param::PositionSize> heldUpper;
  std::optional<param::PositionSize> heldLower;
  std::optional<param::PositionSize> incomingUpper;
  std::optional<param::PositionSize> incomingLower;

  Bounds() = default;

  [[nodiscard]] static Bounds FromRaw(
      const OpenPitAccountAdjustmentBounds& raw) {
    Bounds out;
    out.balanceUpper = ReadBound(raw.balance_upper);
    out.balanceLower = ReadBound(raw.balance_lower);
    out.heldUpper = ReadBound(raw.held_upper);
    out.heldLower = ReadBound(raw.held_lower);
    out.incomingUpper = ReadBound(raw.incoming_upper);
    out.incomingLower = ReadBound(raw.incoming_lower);
    return out;
  }

  [[nodiscard]] OpenPitAccountAdjustmentBounds Raw() const noexcept {
    OpenPitAccountAdjustmentBounds raw{};
    WriteBound(raw.balance_upper, balanceUpper);
    WriteBound(raw.balance_lower, balanceLower);
    WriteBound(raw.held_upper, heldUpper);
    WriteBound(raw.held_lower, heldLower);
    WriteBound(raw.incoming_upper, incomingUpper);
    WriteBound(raw.incoming_lower, incomingLower);
    return raw;
  }

 private:
  [[nodiscard]] static std::optional<param::PositionSize> ReadBound(
      const param::PositionSizeOptional& field) {
    if (!field.is_set) {
      return std::nullopt;
    }
    return param::PositionSize::FromRaw(field.value);
  }

  static void WriteBound(
      param::PositionSizeOptional& field,
      const std::optional<param::PositionSize>& value) noexcept {
    if (value) {
      field.value = value->Raw();
      field.is_set = true;
    }
  }
};

}  // namespace openpit::accountadjustment
