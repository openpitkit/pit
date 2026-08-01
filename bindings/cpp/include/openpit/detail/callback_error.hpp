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

#include "openpit/error.hpp"

#include <cstdint>
#include <exception>
#include <string_view>
#include <utility>

namespace openpit::detail {

class CallbackExceptionScope;

inline thread_local CallbackExceptionScope* g_currentCallbackExceptionScope =
    nullptr;

// Operation-local callback exception slot. Scopes form a thread-local stack so
// a nested SDK operation cannot clear or consume its caller's exception.
class CallbackExceptionScope {
 public:
  CallbackExceptionScope() noexcept
      : m_previous(std::exchange(g_currentCallbackExceptionScope, this)) {}

  CallbackExceptionScope(const CallbackExceptionScope&) = delete;
  CallbackExceptionScope& operator=(const CallbackExceptionScope&) = delete;

  ~CallbackExceptionScope() { g_currentCallbackExceptionScope = m_previous; }

  [[nodiscard]] bool HasPending() const noexcept {
    return m_pending != nullptr;
  }

  void ThrowIfPending() {
    if (m_pending == nullptr) {
      return;
    }
    std::exception_ptr pending = std::exchange(m_pending, std::exception_ptr{});
    std::rethrow_exception(pending);
  }

 private:
  friend void CaptureCurrentCallbackException() noexcept;
  friend void CaptureCurrentCallbackException(
      OpenPitSharedString** outError) noexcept;

  CallbackExceptionScope* m_previous;
  std::exception_ptr m_pending;
};

inline void CaptureCurrentCallbackException() noexcept {
  if (g_currentCallbackExceptionScope != nullptr &&
      !g_currentCallbackExceptionScope->HasPending()) {
    g_currentCallbackExceptionScope->m_pending = std::current_exception();
  }
}

inline void CaptureCurrentCallbackException(
    OpenPitSharedString** outError) noexcept {
  const std::exception_ptr current = std::current_exception();
  if (g_currentCallbackExceptionScope != nullptr &&
      !g_currentCallbackExceptionScope->HasPending()) {
    g_currentCallbackExceptionScope->m_pending = current;
  }
  if (outError == nullptr || current == nullptr) {
    return;
  }
  std::string_view details = "C++ mutation callback threw";
  try {
    std::rethrow_exception(current);
  } catch (const std::exception& error) {
    details = error.what();
  } catch (...) {
  }
  const OpenPitStringView view{
      reinterpret_cast<const std::uint8_t*>(details.data()), details.size()};
  *outError = openpit_create_shared_string(view);
}

}  // namespace openpit::detail
