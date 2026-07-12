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
#include "openpit/error.hpp"
#include "openpit/instrument_id.hpp"
#include "openpit/model.hpp"

#include <openpit.h>

#include <cstdint>
#include <optional>

namespace openpit {

// Unit used to express a settlement delay.
enum class SettlementUnit : std::uint8_t {
  // Business days in the caller-provided settlement calendar. This is the
  // zero/default value.
  BusinessDays = OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS,
  // Consecutive calendar days.
  CalendarDays = OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS,
};

// Settlement delay for one delivery or payment leg.
struct SettlementLag {
  std::uint64_t n = 0;
  SettlementUnit unit = SettlementUnit::BusinessDays;

  [[nodiscard]] constexpr bool operator==(
      const SettlementLag& other) const noexcept {
    return n == other.n && unit == other.unit;
  }
};

// Independent settlement delays for delivery and payment legs.
struct SettlementScheme {
  SettlementLag delivery;
  SettlementLag payment;

  [[nodiscard]] static constexpr SettlementScheme Uniform(
      std::uint64_t n) noexcept {
    const SettlementLag lag{n, SettlementUnit::BusinessDays};
    return SettlementScheme{lag, lag};
  }

  [[nodiscard]] constexpr bool operator==(
      const SettlementScheme& other) const noexcept {
    return delivery == other.delivery && payment == other.payment;
  }
};

// Outcome of a reference-book registration. Boundary failures throw Error.
enum class ReferenceBookRegisterStatus : std::uint8_t {
  Ok = OpenPitReferenceBookRegisterStatus_Ok,
  DuplicateId = OpenPitReferenceBookRegisterStatus_DuplicateId,
  DuplicateInstrument = OpenPitReferenceBookRegisterStatus_DuplicateInstrument,
};

// Value result from a reference-book registration.
struct ReferenceBookRegisterResult {
  ReferenceBookRegisterStatus status = ReferenceBookRegisterStatus::Ok;
  std::optional<InstrumentId> instrumentId;
  std::optional<InstrumentId> conflictingInstrumentId;
  std::optional<model::Instrument> conflictingInstrument;

  [[nodiscard]] bool Ok() const noexcept {
    return status == ReferenceBookRegisterStatus::Ok;
  }
};

// Outcome of a reference-book settlement update. Boundary failures throw Error.
enum class ReferenceBookStatus : std::uint8_t {
  Ok = OpenPitReferenceBookStatus_Ok,
  UnknownInstrument = OpenPitReferenceBookStatus_UnknownInstrument,
};

// Value result from a reference-book settlement lookup. An Ok result without a
// scheme means that the instrument is registered but has no configuration.
struct ReferenceBookSettlementSchemeResult {
  ReferenceBookStatus status = ReferenceBookStatus::Ok;
  std::optional<SettlementScheme> settlementScheme;

  [[nodiscard]] bool Ok() const noexcept {
    return status == ReferenceBookStatus::Ok;
  }
};

namespace detail {

struct ReferenceBookDeleter {
  void operator()(OpenPitReferenceBook* handle) const noexcept {
    openpit_destroy_reference_book(handle);
  }
};

[[nodiscard]] inline OpenPitSettlementLag ToRaw(SettlementLag value) noexcept {
  return OpenPitSettlementLag{value.n,
                              static_cast<OpenPitSettlementUnit>(value.unit)};
}

[[nodiscard]] inline SettlementLag FromRaw(
    OpenPitSettlementLag value) noexcept {
  return SettlementLag{value.n, static_cast<SettlementUnit>(value.unit)};
}

[[nodiscard]] inline OpenPitSettlementScheme ToRaw(
    SettlementScheme value) noexcept {
  return OpenPitSettlementScheme{ToRaw(value.delivery), ToRaw(value.payment)};
}

[[nodiscard]] inline SettlementScheme FromRaw(
    OpenPitSettlementScheme value) noexcept {
  return SettlementScheme{FromRaw(value.delivery), FromRaw(value.payment)};
}

}  // namespace detail

// Move-only RAII owner for typed per-instrument reference data.
class ReferenceBook {
 public:
  ReferenceBook() : m_handle(openpit_create_reference_book()) {
    if (!m_handle) {
      throw Error("openpit_create_reference_book failed");
    }
  }

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  [[nodiscard]] ReferenceBookRegisterResult Register(
      const model::Instrument& instrument) {
    const OpenPitInstrument raw = instrument.Raw();
    OpenPitInstrumentId id = 0;
    OpenPitSharedString* error = nullptr;
    const OpenPitReferenceBookRegisterStatus status =
        openpit_reference_book_register(m_handle.Get(), &raw, &id, &error);
    return MapRegister(status, error, "openpit_reference_book_register", id,
                       instrument, std::nullopt);
  }

  [[nodiscard]] ReferenceBookRegisterResult Register(
      const model::Instrument& instrument, InstrumentId id) {
    const OpenPitInstrument raw = instrument.Raw();
    OpenPitInstrumentId resolved = 0;
    OpenPitSharedString* error = nullptr;
    const OpenPitReferenceBookRegisterStatus status =
        openpit_reference_book_register_with_id(m_handle.Get(), &raw, id.Raw(),
                                                &resolved, &error);
    return MapRegister(status, error, "openpit_reference_book_register_with_id",
                       resolved, instrument, id);
  }

  [[nodiscard]] std::optional<InstrumentId> Resolve(
      const model::Instrument& instrument) const {
    const OpenPitInstrument raw = instrument.Raw();
    OpenPitInstrumentId id = 0;
    if (!openpit_reference_book_resolve(m_handle.Get(), &raw, &id)) {
      return std::nullopt;
    }
    return InstrumentId(id);
  }

  [[nodiscard]] ReferenceBookStatus SetSettlementScheme(
      InstrumentId id, SettlementScheme scheme) {
    OpenPitSharedString* error = nullptr;
    const OpenPitReferenceBookStatus status =
        openpit_reference_book_set_settlement_scheme(
            m_handle.Get(), id.Raw(), detail::ToRaw(scheme), &error);
    return MapStatus(status, error,
                     "openpit_reference_book_set_settlement_scheme");
  }

  [[nodiscard]] ReferenceBookStatus ClearSettlementScheme(InstrumentId id) {
    OpenPitSharedString* error = nullptr;
    const OpenPitReferenceBookStatus status =
        openpit_reference_book_clear_settlement_scheme(m_handle.Get(), id.Raw(),
                                                       &error);
    return MapStatus(status, error,
                     "openpit_reference_book_clear_settlement_scheme");
  }

  [[nodiscard]] ReferenceBookSettlementSchemeResult SettlementSchemeFor(
      InstrumentId id) const {
    OpenPitSettlementScheme raw{};
    bool isSet = false;
    OpenPitSharedString* error = nullptr;
    const OpenPitReferenceBookStatus status =
        openpit_reference_book_get_settlement_scheme(m_handle.Get(), id.Raw(),
                                                     &raw, &isSet, &error);
    return MapSettlementScheme(status, isSet, raw, error,
                               "openpit_reference_book_get_settlement_scheme");
  }

 private:
  [[nodiscard]] static ReferenceBookRegisterResult MapRegister(
      OpenPitReferenceBookRegisterStatus status, OpenPitSharedString* error,
      const char* fallback, OpenPitInstrumentId id,
      const model::Instrument& instrument,
      std::optional<InstrumentId> requestedId) {
    if (status == OpenPitReferenceBookRegisterStatus_Error) {
      detail::ThrowFromSharedString(error, fallback);
    }
    ReferenceBookRegisterResult result;
    result.status = static_cast<ReferenceBookRegisterStatus>(status);
    if (result.Ok()) {
      result.instrumentId = InstrumentId(id);
    } else if (status == OpenPitReferenceBookRegisterStatus_DuplicateId) {
      result.conflictingInstrumentId = requestedId;
    } else if (status ==
               OpenPitReferenceBookRegisterStatus_DuplicateInstrument) {
      result.conflictingInstrument = instrument;
    }
    return result;
  }

  [[nodiscard]] static ReferenceBookStatus MapStatus(
      OpenPitReferenceBookStatus status, OpenPitSharedString* error,
      const char* fallback) {
    if (status == OpenPitReferenceBookStatus_Error) {
      detail::ThrowFromSharedString(error, fallback);
    }
    return static_cast<ReferenceBookStatus>(status);
  }

  [[nodiscard]] static ReferenceBookSettlementSchemeResult MapSettlementScheme(
      OpenPitReferenceBookStatus status, bool isSet,
      OpenPitSettlementScheme scheme, OpenPitSharedString* error,
      const char* fallback) {
    if (status == OpenPitReferenceBookStatus_Error) {
      detail::ThrowFromSharedString(error, fallback);
    }
    ReferenceBookSettlementSchemeResult result;
    result.status = static_cast<ReferenceBookStatus>(status);
    if (result.Ok() && isSet) {
      result.settlementScheme = detail::FromRaw(scheme);
    }
    return result;
  }

  detail::Handle<OpenPitReferenceBook, detail::ReferenceBookDeleter> m_handle;
};

}  // namespace openpit
