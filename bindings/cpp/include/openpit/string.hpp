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

#include "openpit/detail/handle.hpp"
#include "openpit/detail/native_access.hpp"

#include <openpit.h>

#include <cstddef>
#include <string>
#include <string_view>

namespace openpit::detail {

using RawStringView = ::OpenPitStringView;
using RawSharedString = ::OpenPitSharedString;

}  // namespace openpit::detail

namespace openpit {

// Non-owning view over an `OpenPitStringView` (borrowed UTF-8 bytes).
//
// Lifetime mirrors the C contract: the view is valid only while the object that
// produced it is alive and unchanged. Copy out via `ToString()` to retain.
class StringView {
 public:
  StringView() noexcept = default;

  [[nodiscard]] const char* Data() const noexcept {
    return reinterpret_cast<const char*>(m_view.ptr);
  }

  [[nodiscard]] std::size_t Size() const noexcept { return m_view.len; }

  [[nodiscard]] bool Empty() const noexcept {
    return m_view.ptr == nullptr || m_view.len == 0;
  }

  // A borrowed `std::string_view`; empty when the source view is unset.
  [[nodiscard]] std::string_view View() const noexcept {
    if (m_view.ptr == nullptr) {
      return {};
    }
    return {Data(), m_view.len};
  }

  // Copies the bytes into an owning `std::string`.
  [[nodiscard]] std::string ToString() const { return std::string(View()); }

 private:
  friend class detail::NativeAccess;

  explicit StringView(detail::RawStringView view) noexcept : m_view(view) {}

  [[nodiscard]] detail::RawStringView Native() const noexcept { return m_view; }

  detail::RawStringView m_view{nullptr, 0};
};

namespace detail {

[[nodiscard]] inline RawStringView MakeStringView(
    std::string_view value) noexcept {
  return RawStringView{reinterpret_cast<const std::uint8_t*>(value.data()),
                       value.size()};
}

struct SharedStringDeleter {
  void operator()(OpenPitSharedString* handle) const noexcept {
    openpit_destroy_shared_string(handle);
  }
};

}  // namespace detail

// Owning RAII wrapper over an `OpenPitSharedString` handle.
//
// Reading borrows bytes from the live handle (see `View()`); use `ToString()`
// to obtain an independent copy. Move-only.
class SharedString {
 public:
  SharedString() noexcept = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Borrows the handle's bytes; valid only while this object is alive.
  [[nodiscard]] StringView View() const noexcept {
    return detail::FromNative<StringView>(
        openpit_shared_string_view(m_handle.Get()));
  }

  [[nodiscard]] std::string ToString() const { return View().ToString(); }

 private:
  friend class detail::NativeAccess;

  explicit SharedString(detail::RawSharedString* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawSharedString* Native() const noexcept {
    return m_handle.Get();
  }

  detail::Handle<detail::RawSharedString, detail::SharedStringDeleter> m_handle;
};

}  // namespace openpit
