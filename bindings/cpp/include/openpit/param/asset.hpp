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
#include "openpit/string.hpp"

#include <memory>
#include <string>
#include <string_view>
#include <variant>

namespace openpit::param {

/// Validated, owning asset or currency identifier.
///
/// Copies share immutable string storage. Construction validates the identifier
/// and throws `openpit::Error` on invalid input.
class Asset final {
 public:
  /// Validates and owns an asset identifier.
  explicit Asset(std::string_view value) : m_storage(Validate(value)) {}

  /// Returns a borrowed view valid while this value remains alive.
  [[nodiscard]] std::string_view View() const noexcept {
    if (const auto* native =
            std::get_if<std::shared_ptr<detail::RawSharedString>>(&m_storage)) {
      return ::openpit::detail::FromNative<::openpit::StringView>(
                 ::openpit_shared_string_view(native->get()))
          .View();
    }
    return *std::get<std::shared_ptr<const std::string>>(m_storage);
  }

  [[nodiscard]] bool operator==(const Asset& other) const noexcept {
    return View() == other.View();
  }

  [[nodiscard]] bool operator!=(const Asset& other) const noexcept {
    return !(*this == other);
  }

  [[nodiscard]] bool operator<(const Asset& other) const noexcept {
    return View() < other.View();
  }

  [[nodiscard]] bool operator<=(const Asset& other) const noexcept {
    return !(other < *this);
  }

  [[nodiscard]] bool operator>(const Asset& other) const noexcept {
    return other < *this;
  }

  [[nodiscard]] bool operator>=(const Asset& other) const noexcept {
    return !(*this < other);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  struct Deleter {
    void operator()(detail::RawSharedString* handle) const noexcept {
      ::openpit_destroy_param_asset(handle);
    }
  };

  explicit Asset(detail::RawAsset native)
      : m_storage(std::make_shared<const std::string>(
            ::openpit::detail::FromNative<::openpit::StringView>(native)
                .ToString())) {}

  [[nodiscard]] detail::RawAsset Native() const noexcept {
    return ::openpit::detail::MakeStringView(View());
  }

  [[nodiscard]] static std::shared_ptr<detail::RawSharedString> Validate(
      std::string_view value) {
    ::OpenPitParamError* error = nullptr;
    detail::RawSharedString* handle = ::openpit_create_param_asset_from_string(
        ::openpit::detail::MakeStringView(value), &error);
    if (handle == nullptr) {
      ::openpit::detail::ThrowFromParamError(error, "asset validation failed");
    }
    return std::shared_ptr<detail::RawSharedString>(handle, Deleter{});
  }

  std::variant<std::shared_ptr<detail::RawSharedString>,
               std::shared_ptr<const std::string>>
      m_storage;
};

}  // namespace openpit::param
