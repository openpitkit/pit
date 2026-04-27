# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://github.com/openpitkit and the OWNERS file for details.

from __future__ import annotations

import enum

from .. import _enum


@enum.unique
class RejectScope(_enum.StrEnum):
    """Scope of a business reject returned by a policy."""

    ORDER = "order"
    ACCOUNT = "account"

    @classmethod
    # @typing.override
    def _missing_(cls, value: object) -> RejectScope | None:
        raise ValueError("scope must be either 'order' or 'account'")


@enum.unique
class RejectCode(_enum.StrEnum):
    """Stable machine-readable reject codes used across built-in and custom policies."""

    MISSING_REQUIRED_FIELD = "MissingRequiredField"
    INVALID_FIELD_FORMAT = "InvalidFieldFormat"
    INVALID_FIELD_VALUE = "InvalidFieldValue"
    UNSUPPORTED_ORDER_TYPE = "UnsupportedOrderType"
    UNSUPPORTED_TIME_IN_FORCE = "UnsupportedTimeInForce"
    UNSUPPORTED_ORDER_ATTRIBUTE = "UnsupportedOrderAttribute"
    DUPLICATE_CLIENT_ORDER_ID = "DuplicateClientOrderId"
    TOO_LATE_TO_ENTER = "TooLateToEnter"
    EXCHANGE_CLOSED = "ExchangeClosed"
    UNKNOWN_INSTRUMENT = "UnknownInstrument"
    UNKNOWN_ACCOUNT = "UnknownAccount"
    UNKNOWN_VENUE = "UnknownVenue"
    UNKNOWN_CLEARING_ACCOUNT = "UnknownClearingAccount"
    UNKNOWN_COLLATERAL_ASSET = "UnknownCollateralAsset"
    INSUFFICIENT_FUNDS = "InsufficientFunds"
    INSUFFICIENT_MARGIN = "InsufficientMargin"
    INSUFFICIENT_POSITION = "InsufficientPosition"
    CREDIT_LIMIT_EXCEEDED = "CreditLimitExceeded"
    RISK_LIMIT_EXCEEDED = "RiskLimitExceeded"
    ORDER_EXCEEDS_LIMIT = "OrderExceedsLimit"
    ORDER_QTY_EXCEEDS_LIMIT = "OrderQtyExceedsLimit"
    ORDER_NOTIONAL_EXCEEDS_LIMIT = "OrderNotionalExceedsLimit"
    POSITION_LIMIT_EXCEEDED = "PositionLimitExceeded"
    CONCENTRATION_LIMIT_EXCEEDED = "ConcentrationLimitExceeded"
    LEVERAGE_LIMIT_EXCEEDED = "LeverageLimitExceeded"
    RATE_LIMIT_EXCEEDED = "RateLimitExceeded"
    PNL_KILL_SWITCH_TRIGGERED = "PnlKillSwitchTriggered"
    ACCOUNT_BLOCKED = "AccountBlocked"
    ACCOUNT_NOT_AUTHORIZED = "AccountNotAuthorized"
    COMPLIANCE_RESTRICTION = "ComplianceRestriction"
    INSTRUMENT_RESTRICTED = "InstrumentRestricted"
    JURISDICTION_RESTRICTION = "JurisdictionRestriction"
    WASH_TRADE_PREVENTION = "WashTradePrevention"
    SELF_MATCH_PREVENTION = "SelfMatchPrevention"
    SHORT_SALE_RESTRICTION = "ShortSaleRestriction"
    RISK_CONFIGURATION_MISSING = "RiskConfigurationMissing"
    REFERENCE_DATA_UNAVAILABLE = "ReferenceDataUnavailable"
    ORDER_VALUE_CALCULATION_FAILED = "OrderValueCalculationFailed"
    SYSTEM_UNAVAILABLE = "SystemUnavailable"
    CUSTOM = "Custom"
    OTHER = "Other"


RejectScope.ORDER.__doc__ = "Reject that applies only to the current order."
RejectScope.ACCOUNT.__doc__ = "Reject that applies at account scope."

RejectCode.MISSING_REQUIRED_FIELD.__doc__ = "A required field is absent."
RejectCode.INVALID_FIELD_FORMAT.__doc__ = "A field exists but has an invalid format."
RejectCode.INVALID_FIELD_VALUE.__doc__ = "A field exists but its value is not allowed."
RejectCode.UNSUPPORTED_ORDER_TYPE.__doc__ = (
    "The order type is not supported by the current policy set."
)
RejectCode.UNSUPPORTED_TIME_IN_FORCE.__doc__ = (
    "The time-in-force value is not supported."
)
RejectCode.UNSUPPORTED_ORDER_ATTRIBUTE.__doc__ = (
    "The request contains an unsupported order attribute."
)
RejectCode.DUPLICATE_CLIENT_ORDER_ID.__doc__ = "The client order ID is already in use."
RejectCode.TOO_LATE_TO_ENTER.__doc__ = (
    "The request arrived after the allowed entry window."
)
RejectCode.EXCHANGE_CLOSED.__doc__ = (
    "The venue or market is closed for the requested action."
)
RejectCode.UNKNOWN_INSTRUMENT.__doc__ = "The instrument is not recognized."
RejectCode.UNKNOWN_ACCOUNT.__doc__ = "The account is not recognized."
RejectCode.UNKNOWN_VENUE.__doc__ = "The venue is not recognized."
RejectCode.UNKNOWN_CLEARING_ACCOUNT.__doc__ = "The clearing account is not recognized."
RejectCode.UNKNOWN_COLLATERAL_ASSET.__doc__ = "The collateral asset is not recognized."
RejectCode.INSUFFICIENT_FUNDS.__doc__ = "Available cash is not enough."
RejectCode.INSUFFICIENT_MARGIN.__doc__ = "Available margin is not enough."
RejectCode.INSUFFICIENT_POSITION.__doc__ = "Available position is not enough."
RejectCode.CREDIT_LIMIT_EXCEEDED.__doc__ = "A credit limit would be exceeded."
RejectCode.RISK_LIMIT_EXCEEDED.__doc__ = "A general risk limit would be exceeded."
RejectCode.ORDER_EXCEEDS_LIMIT.__doc__ = (
    "More than one order-size limit would be exceeded."
)
RejectCode.ORDER_QTY_EXCEEDS_LIMIT.__doc__ = "The requested quantity exceeds its limit."
RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT.__doc__ = (
    "The requested notional exceeds its limit."
)
RejectCode.POSITION_LIMIT_EXCEEDED.__doc__ = "A position limit would be exceeded."
RejectCode.CONCENTRATION_LIMIT_EXCEEDED.__doc__ = (
    "A concentration limit would be exceeded."
)
RejectCode.LEVERAGE_LIMIT_EXCEEDED.__doc__ = "A leverage limit would be exceeded."
RejectCode.RATE_LIMIT_EXCEEDED.__doc__ = (
    "Too many requests were submitted inside the configured window."
)
RejectCode.PNL_KILL_SWITCH_TRIGGERED.__doc__ = (
    "The configured PnL kill switch is active."
)
RejectCode.ACCOUNT_BLOCKED.__doc__ = "The account is blocked from new requests."
RejectCode.ACCOUNT_NOT_AUTHORIZED.__doc__ = (
    "The account is not authorized for the requested action."
)
RejectCode.COMPLIANCE_RESTRICTION.__doc__ = "A compliance rule blocks the request."
RejectCode.INSTRUMENT_RESTRICTED.__doc__ = "The instrument is restricted."
RejectCode.JURISDICTION_RESTRICTION.__doc__ = (
    "The request is blocked by jurisdiction rules."
)
RejectCode.WASH_TRADE_PREVENTION.__doc__ = (
    "The request would violate wash-trade prevention."
)
RejectCode.SELF_MATCH_PREVENTION.__doc__ = "The request would self-match."
RejectCode.SHORT_SALE_RESTRICTION.__doc__ = (
    "The request violates a short-sale restriction."
)
RejectCode.RISK_CONFIGURATION_MISSING.__doc__ = (
    "Required risk configuration is missing."
)
RejectCode.REFERENCE_DATA_UNAVAILABLE.__doc__ = (
    "Required reference data is unavailable."
)
RejectCode.ORDER_VALUE_CALCULATION_FAILED.__doc__ = (
    "The requested order value could not be computed safely."
)
RejectCode.SYSTEM_UNAVAILABLE.__doc__ = (
    "The system cannot process the request right now."
)
RejectCode.CUSTOM.__doc__ = (
    "Custom reject code, depends on policy implementation."
    " The integer `user_data` reject field can be used for extended information."
)
RejectCode.OTHER.__doc__ = (
    "A standard code does not describe the case precisely enough."
)
