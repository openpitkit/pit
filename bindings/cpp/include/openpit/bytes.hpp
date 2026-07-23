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
#include <cstdint>
#include <vector>

namespace openpit::detail {

using RawBytesView = ::OpenPitBytesView;
using RawSharedBytes = ::OpenPitSharedBytes;

}  // namespace openpit::detail

namespace openpit {

// Non-owning view over an `OpenPitBytesView` (borrowed bytes).
//
// Lifetime mirrors the C contract: valid only while the producing object is
// alive. Copy out via `ToVector()` to retain.
class BytesView {
 public:
  BytesView() noexcept = default;

  [[nodiscard]] const std::uint8_t* Data() const noexcept { return m_view.ptr; }

  [[nodiscard]] std::size_t Size() const noexcept { return m_view.len; }

  [[nodiscard]] bool Empty() const noexcept {
    return m_view.ptr == nullptr || m_view.len == 0;
  }

  [[nodiscard]] std::vector<std::uint8_t> ToVector() const {
    if (m_view.ptr == nullptr) {
      return {};
    }
    return std::vector<std::uint8_t>(m_view.ptr, m_view.ptr + m_view.len);
  }

 private:
  friend class detail::NativeAccess;

  explicit BytesView(detail::RawBytesView view) noexcept : m_view(view) {}

  [[nodiscard]] detail::RawBytesView Native() const noexcept { return m_view; }

  detail::RawBytesView m_view{nullptr, 0};
};

namespace detail {

struct SharedBytesDeleter {
  void operator()(OpenPitSharedBytes* handle) const noexcept {
    openpit_destroy_shared_bytes(handle);
  }
};

}  // namespace detail

// Owning RAII wrapper over an `OpenPitSharedBytes` handle. Move-only.
class SharedBytes {
 public:
  SharedBytes() noexcept = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Borrows the handle's bytes; valid only while this object is alive.
  [[nodiscard]] BytesView View() const noexcept {
    return detail::FromNative<BytesView>(
        openpit_shared_bytes_view(m_handle.Get()));
  }

  [[nodiscard]] std::vector<std::uint8_t> ToVector() const {
    return View().ToVector();
  }

 private:
  friend class detail::NativeAccess;

  explicit SharedBytes(detail::RawSharedBytes* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawSharedBytes* Native() const noexcept {
    return m_handle.Get();
  }

  detail::Handle<detail::RawSharedBytes, detail::SharedBytesDeleter> m_handle;
};

}  // namespace openpit
