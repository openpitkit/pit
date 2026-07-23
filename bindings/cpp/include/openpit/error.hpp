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
// conditions, and SDK boundary failures such as construction failure or invalid
// lifecycle use. Expected business outcomes (pre-trade rejects and similar)
// are return values, never exceptions, and never appear on hot paths. `Error`
// carries the message and, when available, the parameter error code.

namespace openpit {

// Machine-readable category of an exact-value boundary failure.
enum class ParamErrorCode : std::uint32_t {
  Unspecified = 0,
  Negative = 1,
  DivisionByZero = 2,
  Overflow = 3,
  Underflow = 4,
  InvalidFloat = 5,
  InvalidFormat = 6,
  InvalidPrice = 7,
  InvalidLeverage = 8,
  AssetEmpty = 9,
  AccountIdEmpty = 10,
  Other = 0xffffffffU,
};

// Machine-readable category of a runtime policy reconfiguration failure.
enum class ConfigureErrorKind : std::uint32_t {
  Unknown = 0,
  TypeMismatch = 1,
  Validation = 2,
  NestedConfiguration = 3,
};

class Error : public std::exception {
 public:
  explicit Error(std::string message)
      : m_message(std::move(message)), m_code(std::nullopt) {}

  Error(std::string message, ParamErrorCode code)
      : m_message(std::move(message)), m_code(code) {}

  [[nodiscard]] const char* what() const noexcept override {
    return m_message.c_str();
  }

  [[nodiscard]] const std::string& Message() const noexcept {
    return m_message;
  }

  // The exact-value error code, when this error originated from a typed value
  // failure; absent for generic boundary failures.
  [[nodiscard]] std::optional<ParamErrorCode> Code() const noexcept {
    return m_code;
  }

 private:
  std::string m_message;
  std::optional<ParamErrorCode> m_code;
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

class ErrorAccess final {
 public:
  [[nodiscard]] static std::string TakeString(OpenPitSharedString* handle) {
    return FromNative<SharedString>(handle).ToString();
  }

  [[nodiscard]] static std::string CopyString(OpenPitStringView view) {
    return FromNative<StringView>(view).ToString();
  }
};

struct ParamErrorDeleter {
  void operator()(OpenPitParamError* error) const noexcept {
    openpit_destroy_param_error(error);
  }
};

struct ConfigureErrorDeleter {
  void operator()(OpenPitConfigureError* error) const noexcept {
    openpit_destroy_configure_error(error);
  }
};

// Throws an `Error` built from a caller-owned `OpenPitSharedString` produced by
// an `OpenPitOutError`, releasing the handle. `fallback` is used when no
// message handle was written. This function does not return.
[[noreturn]] inline void ThrowFromSharedString(OpenPitSharedString* error,
                                               const char* fallback) {
  if (error != nullptr) {
    throw Error(ErrorAccess::TakeString(error));
  }
  throw Error(std::string(fallback));
}

// Throws an `Error` built from a caller-owned `OpenPitParamError`, releasing
// the handle. `fallback` is used when no error handle was written. This
// function does not return.
[[noreturn]] inline void ThrowFromParamError(OpenPitParamError* error,
                                             const char* fallback) {
  if (error != nullptr) {
    detail::Handle<OpenPitParamError, ParamErrorDeleter> owned(error);
    const ParamErrorCode code = static_cast<ParamErrorCode>(owned.Get()->code);
    std::string message = ErrorAccess::CopyString(
        openpit_shared_string_view(owned.Get()->message));
    throw Error(std::move(message), code);
  }
  throw Error(std::string(fallback));
}

template <typename Native, typename Function>
[[nodiscard]] inline std::string StringifyNative(Native value,
                                                 Function function,
                                                 const char* fallback) {
  OpenPitParamError* error = nullptr;
  OpenPitSharedString* handle = function(value, &error);
  if (handle == nullptr) {
    ThrowFromParamError(error, fallback);
  }
  return ErrorAccess::TakeString(handle);
}

// Throws a `ConfigureError` built from a caller-owned
// `OpenPitConfigureError`, releasing the handle. `fallback` is used when no
// error handle was written. This function does not return.
[[noreturn]] inline void ThrowFromConfigureError(OpenPitConfigureError* error,
                                                 const char* fallback) {
  if (error != nullptr) {
    detail::Handle<OpenPitConfigureError, ConfigureErrorDeleter> owned(error);
    ConfigureErrorKind kind = static_cast<ConfigureErrorKind>(
        openpit_configure_error_get_kind(owned.Get()));
    std::string message = ErrorAccess::CopyString(
        openpit_configure_error_get_message(owned.Get()));
    throw ConfigureError(std::move(message), kind);
  }
  throw ConfigureError(std::string(fallback), ConfigureErrorKind::Validation);
}

}  // namespace detail
}  // namespace openpit
