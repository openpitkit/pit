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

#include "openpit/string.hpp"

#include <openpit.h>

#include <exception>
#include <optional>
#include <string>
#include <utility>

// Error model.
//
// `openpit::Error` is thrown only for programmer mistakes, exceptional
// conditions, and runtime boundary failures (construction failure, invalid use,
// a C call writing its `out_error`). Expected business outcomes (pre-trade
// rejects and similar) are return values, never exceptions, and never appear on
// hot paths.
//
// The native runtime reports boundary failures two ways:
//   - a generic `OpenPitSharedString*` written through an `OpenPitOutError`
//     out-pointer (no machine code);
//   - a typed `OpenPitParamError*` carrying an `OpenPitParamErrorCode` plus a
//     message, used by the fallible param constructors/arithmetic.
// `Error` carries the message and, when available, the param error code.

namespace openpit {

// Machine-readable category of a runtime policy reconfiguration failure.
// Mirrors `OpenPitConfigureErrorKind`.
enum class ConfigureErrorKind : std::uint32_t {
  Unknown = OpenPitConfigureErrorKind_Unknown,
  TypeMismatch = OpenPitConfigureErrorKind_TypeMismatch,
  Validation = OpenPitConfigureErrorKind_Validation,
  NestedConfiguration = OpenPitConfigureErrorKind_NestedConfiguration,
};

class Error : public std::exception {
 public:
  explicit Error(std::string message)
      : m_message(std::move(message)), m_code(std::nullopt) {}

  Error(std::string message, OpenPitParamErrorCode code)
      : m_message(std::move(message)), m_code(code) {}

  [[nodiscard]] const char* what() const noexcept override {
    return m_message.c_str();
  }

  [[nodiscard]] const std::string& Message() const noexcept {
    return m_message;
  }

  // The native runtime param error code, when this error originated from a
  // typed param failure; absent for generic boundary failures.
  [[nodiscard]] std::optional<OpenPitParamErrorCode> Code() const noexcept {
    return m_code;
  }

 private:
  std::string m_message;
  std::optional<OpenPitParamErrorCode> m_code;
};

// Structured error thrown by runtime `Configure*` calls.
class ConfigureError : public Error {
 public:
  ConfigureError(std::string message, ConfigureErrorKind kind)
      : Error(std::move(message)), m_kind(kind) {}

  [[nodiscard]] ConfigureErrorKind Kind() const noexcept { return m_kind; }

 private:
  ConfigureErrorKind m_kind;
};

namespace detail {

// Throws an `Error` built from a caller-owned `OpenPitSharedString` produced by
// an `OpenPitOutError`, releasing the handle. `fallback` is used when no
// message handle was written. This function does not return.
[[noreturn]] inline void ThrowFromSharedString(OpenPitSharedString* error,
                                               const char* fallback) {
  if (error != nullptr) {
    std::string message = SharedStringView(error).ToString();
    openpit_destroy_shared_string(error);
    throw Error(std::move(message));
  }
  throw Error(std::string(fallback));
}

// Throws an `Error` built from a caller-owned `OpenPitParamError`, releasing
// the handle. `fallback` is used when no error handle was written. This
// function does not return.
[[noreturn]] inline void ThrowFromParamError(OpenPitParamError* error,
                                             const char* fallback) {
  if (error != nullptr) {
    OpenPitParamErrorCode code = error->code;
    std::string message =
        StringView(openpit_shared_string_view(error->message)).ToString();
    openpit_destroy_param_error(error);
    throw Error(std::move(message), code);
  }
  throw Error(std::string(fallback));
}

// Throws a `ConfigureError` built from a caller-owned
// `OpenPitConfigureError`, releasing the handle. `fallback` is used when no
// error handle was written. This function does not return.
[[noreturn]] inline void ThrowFromConfigureError(OpenPitConfigureError* error,
                                                 const char* fallback) {
  if (error != nullptr) {
    ConfigureErrorKind kind = static_cast<ConfigureErrorKind>(
        openpit_configure_error_get_kind(error));
    std::string message =
        StringView(openpit_configure_error_get_message(error)).ToString();
    openpit_destroy_configure_error(error);
    throw ConfigureError(std::move(message), kind);
  }
  throw ConfigureError(std::string(fallback), ConfigureErrorKind::Validation);
}

}  // namespace detail
}  // namespace openpit
