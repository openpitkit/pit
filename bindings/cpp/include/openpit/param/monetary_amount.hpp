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

#include "openpit/detail/native_access.hpp"
#include "openpit/param/asset.hpp"
#include "openpit/param/detail/native.hpp"
#include "openpit/param/value_types.hpp"

#include <optional>
#include <utility>

namespace openpit::param {

/// Exact signed fee-style amount paired with its currency.
class MonetaryAmount final {
 public:
  MonetaryAmount(Fee amount, Asset currency)
      : m_amount(amount), m_currency(std::move(currency)) {}

  [[nodiscard]] Fee Amount() const noexcept { return m_amount; }

  [[nodiscard]] const Asset& Currency() const noexcept { return m_currency; }

  [[nodiscard]] bool operator==(const MonetaryAmount& other) const {
    return m_amount == other.m_amount && m_currency == other.m_currency;
  }

  [[nodiscard]] bool operator!=(const MonetaryAmount& other) const {
    return !(*this == other);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit MonetaryAmount(const detail::RawMonetaryAmount& native)
      : MonetaryAmount(::openpit::detail::FromNative<Fee>(native.amount),
                       ::openpit::detail::FromNative<Asset>(native.currency)) {}

  [[nodiscard]] detail::RawMonetaryAmount Native() const noexcept {
    detail::RawMonetaryAmount native{};
    native.amount = ::openpit::detail::Native(m_amount);
    native.currency = ::openpit::detail::Native(m_currency);
    return native;
  }

  Fee m_amount;
  Asset m_currency;
};

namespace detail {

class MonetaryAmountAccess final {
 private:
  [[nodiscard]] static std::optional<MonetaryAmount> FromNative(
      const RawMonetaryAmountOptional& native) {
    if (!native.is_set) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<MonetaryAmount>(native.value);
  }

  [[nodiscard]] static RawMonetaryAmountOptional Native(
      const std::optional<MonetaryAmount>& value) noexcept {
    RawMonetaryAmountOptional native{};
    if (value) {
      native.value = ::openpit::detail::Native(*value);
      native.is_set = true;
    }
    return native;
  }

  friend struct ::openpit::model::Fill;
};

}  // namespace detail
}  // namespace openpit::param
