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
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// Reject value types.
//
// A `Reject` is an expected business outcome of a pre-trade check, not an
// error, so it is a value type and never thrown.
//
// The canonical definitions live in `openpit::reject`, matching the engine and
// the other language bindings.

namespace openpit::reject {

// Broad area to which a reject applies. Zero is not a valid scope.
//
enum class RejectScope : std::uint8_t {
  Order = 1,
  Account = 2,
};

// Stable machine-readable reject classification. Unknown incoming codes map
// to `Other`.
//
enum class RejectCode : std::uint16_t {
  MissingRequiredField = 1,
  InvalidFieldFormat = 2,
  InvalidFieldValue = 3,
  UnsupportedOrderType = 4,
  UnsupportedTimeInForce = 5,
  UnsupportedOrderAttribute = 6,
  DuplicateClientOrderId = 7,
  TooLateToEnter = 8,
  ExchangeClosed = 9,
  UnknownInstrument = 10,
  UnknownAccount = 11,
  UnknownVenue = 12,
  UnknownClearingAccount = 13,
  UnknownCollateralAsset = 14,
  InsufficientFunds = 15,
  InsufficientMargin = 16,
  InsufficientPosition = 17,
  CreditLimitExceeded = 18,
  RiskLimitExceeded = 19,
  OrderExceedsLimit = 20,
  OrderQtyExceedsLimit = 21,
  OrderNotionalExceedsLimit = 22,
  PositionLimitExceeded = 23,
  ConcentrationLimitExceeded = 24,
  LeverageLimitExceeded = 25,
  RateLimitExceeded = 26,
  PnlKillSwitchTriggered = 27,
  AccountBlocked = 28,
  AccountNotAuthorized = 29,
  ComplianceRestriction = 30,
  InstrumentRestricted = 31,
  JurisdictionRestriction = 32,
  WashTradePrevention = 33,
  SelfMatchPrevention = 34,
  ShortSaleRestriction = 35,
  RiskConfigurationMissing = 36,
  ReferenceDataUnavailable = 37,
  OrderValueCalculationFailed = 38,
  SystemUnavailable = 39,
  MarkPriceUnavailable = 40,
  AccountAdjustmentBoundsExceeded = 41,
  ArithmeticOverflow = 42,
  Custom = 254,
  Other = 255,
};

// A single pre-trade rejection record.
//
// `userData` is an opaque caller-defined token the SDK never inspects; zero
// means unset.
struct Reject {
  std::string policy;
  std::string reason;
  std::string details;
  std::uintptr_t userData = 0;
  RejectCode code = RejectCode::Other;
  RejectScope scope = RejectScope::Order;

  Reject() = default;

  Reject(std::string policyName, RejectScope rejectScope, RejectCode rejectCode,
         std::string rejectReason, std::string rejectDetails)
      : policy(std::move(policyName)),
        reason(std::move(rejectReason)),
        details(std::move(rejectDetails)),
        code(rejectCode),
        scope(rejectScope) {}

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static Reject FromRaw(const OpenPitPretradeReject& raw) {
    Reject out;
    out.policy =
        ::openpit::detail::FromNative<::openpit::StringView>(raw.policy)
            .ToString();
    out.reason =
        ::openpit::detail::FromNative<::openpit::StringView>(raw.reason)
            .ToString();
    out.details =
        ::openpit::detail::FromNative<::openpit::StringView>(raw.details)
            .ToString();
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    out.code = static_cast<RejectCode>(raw.code);
    out.scope = static_cast<RejectScope>(raw.scope);
    return out;
  }

  [[nodiscard]] OpenPitPretradeReject Native() const noexcept {
    OpenPitPretradeReject raw{};
    raw.policy = ::openpit::detail::MakeStringView(policy);
    raw.reason = ::openpit::detail::MakeStringView(reason);
    raw.details = ::openpit::detail::MakeStringView(details);
    raw.user_data = reinterpret_cast<void*>(userData);
    raw.code = static_cast<OpenPitPretradeRejectCode>(
        static_cast<std::uint16_t>(code));
    raw.scope = static_cast<OpenPitPretradeRejectScope>(
        static_cast<std::uint8_t>(scope));
    return raw;
  }
};

}  // namespace openpit::reject
