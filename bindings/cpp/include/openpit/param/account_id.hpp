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
#include "openpit/error.hpp"
#include "openpit/param/detail/native.hpp"

#include <cstdint>
#include <string>
#include <string_view>

namespace openpit::param {

/// Stable account identifier created from a numeric or textual source.
class AccountId final {
 public:
  constexpr AccountId() noexcept = default;

  [[nodiscard]] static constexpr AccountId FromUint64(
      std::uint64_t value) noexcept {
    return AccountId(value);
  }

  [[nodiscard]] static AccountId FromString(std::string_view value) {
    detail::RawAccountId native{};
    ::OpenPitParamError* error = nullptr;
    if (!::openpit_create_param_account_id_from_string(
            ::openpit::detail::MakeStringView(value), &native, &error)) {
      ::openpit::detail::ThrowFromParamError(error,
                                             "account id validation failed");
    }
    return AccountId(native);
  }

  [[nodiscard]] constexpr std::uint64_t Value() const noexcept {
    return m_value;
  }

  [[nodiscard]] std::string ToString() const { return std::to_string(m_value); }

  [[nodiscard]] constexpr bool operator==(
      const AccountId& other) const noexcept {
    return m_value == other.m_value;
  }

  [[nodiscard]] constexpr bool operator!=(
      const AccountId& other) const noexcept {
    return !(*this == other);
  }

  [[nodiscard]] constexpr bool operator<(
      const AccountId& other) const noexcept {
    return m_value < other.m_value;
  }

  [[nodiscard]] constexpr bool operator<=(
      const AccountId& other) const noexcept {
    return !(other < *this);
  }

  [[nodiscard]] constexpr bool operator>(
      const AccountId& other) const noexcept {
    return other < *this;
  }

  [[nodiscard]] constexpr bool operator>=(
      const AccountId& other) const noexcept {
    return !(*this < other);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit constexpr AccountId(detail::RawAccountId native) noexcept
      : m_value(native) {}

  [[nodiscard]] constexpr detail::RawAccountId Native() const noexcept {
    return m_value;
  }

  detail::RawAccountId m_value = 0;
};

/// Policy-group identifier local to one engine configuration.
class GroupId final {
 public:
  constexpr GroupId() noexcept = default;
  explicit constexpr GroupId(std::uint16_t value) noexcept : m_value(value) {}

  [[nodiscard]] constexpr std::uint16_t Value() const noexcept {
    return m_value;
  }

  [[nodiscard]] constexpr bool operator==(const GroupId& other) const noexcept {
    return m_value == other.m_value;
  }

  [[nodiscard]] constexpr bool operator!=(const GroupId& other) const noexcept {
    return !(*this == other);
  }

  [[nodiscard]] constexpr bool operator<(const GroupId& other) const noexcept {
    return m_value < other.m_value;
  }

  [[nodiscard]] constexpr bool operator<=(const GroupId& other) const noexcept {
    return !(other < *this);
  }

  [[nodiscard]] constexpr bool operator>(const GroupId& other) const noexcept {
    return other < *this;
  }

  [[nodiscard]] constexpr bool operator>=(const GroupId& other) const noexcept {
    return !(*this < other);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] constexpr std::uint16_t Native() const noexcept {
    return m_value;
  }

  std::uint16_t m_value = 0;
};

/// Default policy-group identifier.
inline constexpr std::uint16_t DefaultPolicyGroupId = 0;

/// Stable account-group identifier used for shared market-data resolution.
class AccountGroupId final {
 public:
  constexpr AccountGroupId() noexcept = default;

  [[nodiscard]] static AccountGroupId FromUint32(std::uint32_t value) {
    detail::RawAccountGroupId native{};
    ::OpenPitSharedString* error = nullptr;
    if (!::openpit_create_param_account_group_id_from_uint32(value, &native,
                                                             &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "account group id validation failed");
    }
    return AccountGroupId(native);
  }

  [[nodiscard]] static AccountGroupId FromString(std::string_view value) {
    detail::RawAccountGroupId native{};
    ::OpenPitSharedString* error = nullptr;
    if (!::openpit_create_param_account_group_id_from_string(
            ::openpit::detail::MakeStringView(value), &native, &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "account group id validation failed");
    }
    return AccountGroupId(native);
  }

  [[nodiscard]] constexpr bool IsDefault() const noexcept {
    return m_value == 0;
  }

  [[nodiscard]] constexpr std::uint32_t Value() const noexcept {
    return m_value;
  }

  [[nodiscard]] std::string ToString() const { return std::to_string(m_value); }

  [[nodiscard]] constexpr bool operator==(
      const AccountGroupId& other) const noexcept {
    return m_value == other.m_value;
  }

  [[nodiscard]] constexpr bool operator!=(
      const AccountGroupId& other) const noexcept {
    return !(*this == other);
  }

  [[nodiscard]] constexpr bool operator<(
      const AccountGroupId& other) const noexcept {
    return m_value < other.m_value;
  }

  [[nodiscard]] constexpr bool operator<=(
      const AccountGroupId& other) const noexcept {
    return !(other < *this);
  }

  [[nodiscard]] constexpr bool operator>(
      const AccountGroupId& other) const noexcept {
    return other < *this;
  }

  [[nodiscard]] constexpr bool operator>=(
      const AccountGroupId& other) const noexcept {
    return !(*this < other);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit constexpr AccountGroupId(detail::RawAccountGroupId native) noexcept
      : m_value(native) {}

  [[nodiscard]] constexpr detail::RawAccountGroupId Native() const noexcept {
    return m_value;
  }

  detail::RawAccountGroupId m_value = 0;
};

/// Default account group.
inline constexpr AccountGroupId DefaultAccountGroup{};

}  // namespace openpit::param
