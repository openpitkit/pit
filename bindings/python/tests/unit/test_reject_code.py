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
# Please see https://openpit.dev and the OWNERS file for details.

import openpit


def test_reject_code_evaluation_failure_classification_is_exhaustive() -> None:
    expected = {
        openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD: True,
        openpit.pretrade.RejectCode.INVALID_FIELD_FORMAT: False,
        openpit.pretrade.RejectCode.INVALID_FIELD_VALUE: False,
        openpit.pretrade.RejectCode.UNSUPPORTED_ORDER_TYPE: False,
        openpit.pretrade.RejectCode.UNSUPPORTED_TIME_IN_FORCE: False,
        openpit.pretrade.RejectCode.UNSUPPORTED_ORDER_ATTRIBUTE: False,
        openpit.pretrade.RejectCode.DUPLICATE_CLIENT_ORDER_ID: False,
        openpit.pretrade.RejectCode.TOO_LATE_TO_ENTER: False,
        openpit.pretrade.RejectCode.EXCHANGE_CLOSED: False,
        openpit.pretrade.RejectCode.UNKNOWN_INSTRUMENT: True,
        openpit.pretrade.RejectCode.UNKNOWN_ACCOUNT: True,
        openpit.pretrade.RejectCode.UNKNOWN_VENUE: True,
        openpit.pretrade.RejectCode.UNKNOWN_CLEARING_ACCOUNT: True,
        openpit.pretrade.RejectCode.UNKNOWN_COLLATERAL_ASSET: True,
        openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS: False,
        openpit.pretrade.RejectCode.INSUFFICIENT_MARGIN: False,
        openpit.pretrade.RejectCode.INSUFFICIENT_POSITION: False,
        openpit.pretrade.RejectCode.CREDIT_LIMIT_EXCEEDED: False,
        openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED: False,
        openpit.pretrade.RejectCode.ORDER_EXCEEDS_LIMIT: False,
        openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT: False,
        openpit.pretrade.RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT: False,
        openpit.pretrade.RejectCode.POSITION_LIMIT_EXCEEDED: False,
        openpit.pretrade.RejectCode.CONCENTRATION_LIMIT_EXCEEDED: False,
        openpit.pretrade.RejectCode.LEVERAGE_LIMIT_EXCEEDED: False,
        openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED: False,
        openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED: False,
        openpit.pretrade.RejectCode.ACCOUNT_BLOCKED: False,
        openpit.pretrade.RejectCode.ACCOUNT_NOT_AUTHORIZED: False,
        openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION: False,
        openpit.pretrade.RejectCode.INSTRUMENT_RESTRICTED: False,
        openpit.pretrade.RejectCode.JURISDICTION_RESTRICTION: False,
        openpit.pretrade.RejectCode.WASH_TRADE_PREVENTION: False,
        openpit.pretrade.RejectCode.SELF_MATCH_PREVENTION: False,
        openpit.pretrade.RejectCode.SHORT_SALE_RESTRICTION: False,
        openpit.pretrade.RejectCode.RISK_CONFIGURATION_MISSING: True,
        openpit.pretrade.RejectCode.REFERENCE_DATA_UNAVAILABLE: True,
        openpit.pretrade.RejectCode.ORDER_VALUE_CALCULATION_FAILED: True,
        openpit.pretrade.RejectCode.SYSTEM_UNAVAILABLE: True,
        openpit.pretrade.RejectCode.MARK_PRICE_UNAVAILABLE: True,
        openpit.pretrade.RejectCode.ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED: False,
        openpit.pretrade.RejectCode.ARITHMETIC_OVERFLOW: True,
        openpit.pretrade.RejectCode.CUSTOM: False,
        openpit.pretrade.RejectCode.OTHER: False,
    }

    assert set(openpit.pretrade.RejectCode) == set(expected)
    for code, is_evaluation_failure in expected.items():
        assert code.is_evaluation_failure() == is_evaluation_failure
