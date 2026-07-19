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

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// Reject value types.
//
// A `Reject` is an expected business outcome of a pre-trade check, not an
// error, so it is a value type and never thrown. `RejectScope` / `RejectCode`
// map 1:1 from the native runtime enums.
//
// The canonical definitions live in `openpit::pretrade`, matching the public
// policy API. They are also re-exported under `openpit::reject` as a short
// namespace; both spellings name the same types.

namespace openpit::pretrade {

// Broad area to which a reject applies. Mirrors `OpenPitPretradeRejectScope`;
// zero is not a valid scope.
//
enum class RejectScope : std::uint8_t {
  Order = OPENPIT_PRETRADE_REJECT_SCOPE_ORDER,
  Account = OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT,
};

// Stable machine-readable reject classification. Mirrors
// `OpenPitPretradeRejectCode`. Unknown incoming codes map to `Other`.
//
enum class RejectCode : std::uint16_t {
  MissingRequiredField = OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD,
  InvalidFieldFormat = OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT,
  InvalidFieldValue = OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE,
  UnsupportedOrderType = OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE,
  UnsupportedTimeInForce =
      OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE,
  UnsupportedOrderAttribute =
      OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE,
  DuplicateClientOrderId =
      OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID,
  TooLateToEnter = OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER,
  ExchangeClosed = OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED,
  UnknownInstrument = OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT,
  UnknownAccount = OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT,
  UnknownVenue = OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE,
  UnknownClearingAccount =
      OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT,
  UnknownCollateralAsset =
      OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET,
  InsufficientFunds = OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS,
  InsufficientMargin = OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN,
  InsufficientPosition = OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION,
  CreditLimitExceeded = OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED,
  RiskLimitExceeded = OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED,
  OrderExceedsLimit = OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT,
  OrderQtyExceedsLimit = OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT,
  OrderNotionalExceedsLimit =
      OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT,
  PositionLimitExceeded = OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED,
  ConcentrationLimitExceeded =
      OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED,
  LeverageLimitExceeded = OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED,
  RateLimitExceeded = OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED,
  PnlKillSwitchTriggered =
      OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED,
  AccountBlocked = OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED,
  AccountNotAuthorized = OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED,
  ComplianceRestriction = OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION,
  InstrumentRestricted = OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED,
  JurisdictionRestriction =
      OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION,
  WashTradePrevention = OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION,
  SelfMatchPrevention = OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION,
  ShortSaleRestriction = OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION,
  RiskConfigurationMissing =
      OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING,
  ReferenceDataUnavailable =
      OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE,
  OrderValueCalculationFailed =
      OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED,
  SystemUnavailable = OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE,
  MarkPriceUnavailable = OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE,
  AccountAdjustmentBoundsExceeded =
      OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED,
  ArithmeticOverflow = OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW,
  Custom = OPENPIT_PRETRADE_REJECT_CODE_CUSTOM,
  Other = OPENPIT_PRETRADE_REJECT_CODE_OTHER,
};

// A single pre-trade rejection record.
//
// Field order follows the native runtime `OpenPitPretradeReject` (largest
// first) so a view conversion stays mechanical. `userData` is an opaque
// caller-defined token the SDK never inspects; zero means unset.
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

  // Copies the borrowed string views out of a C reject record.
  [[nodiscard]] static Reject FromRaw(const OpenPitPretradeReject& raw) {
    Reject out;
    out.policy = ::openpit::StringView(raw.policy).ToString();
    out.reason = ::openpit::StringView(raw.reason).ToString();
    out.details = ::openpit::StringView(raw.details).ToString();
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    out.code = static_cast<RejectCode>(raw.code);
    out.scope = static_cast<RejectScope>(raw.scope);
    return out;
  }

  // Builds a C reject record whose string views borrow this object's strings;
  // valid only while this `Reject` is alive and unchanged.
  [[nodiscard]] OpenPitPretradeReject Raw() const noexcept {
    OpenPitPretradeReject raw{};
    raw.policy = ::openpit::MakeStringView(policy);
    raw.reason = ::openpit::MakeStringView(reason);
    raw.details = ::openpit::MakeStringView(details);
    raw.user_data = reinterpret_cast<void*>(userData);
    raw.code = static_cast<OpenPitPretradeRejectCode>(
        static_cast<std::uint16_t>(code));
    raw.scope = static_cast<OpenPitPretradeRejectScope>(
        static_cast<std::uint8_t>(scope));
    return raw;
  }
};

// Accumulator handed to a policy: a policy reports zero or more rejects into it
// during a pre-trade check. A non-empty decision means the order is rejected.
struct PolicyDecision {
  std::vector<Reject> rejects;

  [[nodiscard]] bool IsRejected() const noexcept { return !rejects.empty(); }

  void Push(Reject reject) { rejects.push_back(std::move(reject)); }
};

}  // namespace openpit::pretrade

namespace openpit::reject {

// Convenience re-exports of the canonical reject types from
// `openpit::pretrade`.
using RejectScope = ::openpit::pretrade::RejectScope;
using RejectCode = ::openpit::pretrade::RejectCode;
using Reject = ::openpit::pretrade::Reject;
using PolicyDecision = ::openpit::pretrade::PolicyDecision;

}  // namespace openpit::reject
