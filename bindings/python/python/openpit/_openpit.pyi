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

"""Typing stubs for the native ``openpit._openpit`` extension module."""

from __future__ import annotations

from typing import ClassVar, Literal

from .pretrade import CheckPreTradeStartPolicy, Policy

NumericValue = str | int | float

class RejectError(Exception):
    """Python exception type exposed by the native module."""

class RejectCode:
    """Stable reject code constants."""

    MISSING_REQUIRED_FIELD: ClassVar[str]
    INVALID_FIELD_FORMAT: ClassVar[str]
    INVALID_FIELD_VALUE: ClassVar[str]
    UNSUPPORTED_ORDER_TYPE: ClassVar[str]
    UNSUPPORTED_TIME_IN_FORCE: ClassVar[str]
    UNSUPPORTED_ORDER_ATTRIBUTE: ClassVar[str]
    DUPLICATE_CLIENT_ORDER_ID: ClassVar[str]
    TOO_LATE_TO_ENTER: ClassVar[str]
    EXCHANGE_CLOSED: ClassVar[str]
    UNKNOWN_INSTRUMENT: ClassVar[str]
    UNKNOWN_ACCOUNT: ClassVar[str]
    UNKNOWN_VENUE: ClassVar[str]
    UNKNOWN_CLEARING_ACCOUNT: ClassVar[str]
    UNKNOWN_COLLATERAL_ASSET: ClassVar[str]
    INSUFFICIENT_FUNDS: ClassVar[str]
    INSUFFICIENT_MARGIN: ClassVar[str]
    INSUFFICIENT_POSITION: ClassVar[str]
    CREDIT_LIMIT_EXCEEDED: ClassVar[str]
    RISK_LIMIT_EXCEEDED: ClassVar[str]
    ORDER_EXCEEDS_LIMIT: ClassVar[str]
    ORDER_QTY_EXCEEDS_LIMIT: ClassVar[str]
    ORDER_NOTIONAL_EXCEEDS_LIMIT: ClassVar[str]
    POSITION_LIMIT_EXCEEDED: ClassVar[str]
    CONCENTRATION_LIMIT_EXCEEDED: ClassVar[str]
    LEVERAGE_LIMIT_EXCEEDED: ClassVar[str]
    RATE_LIMIT_EXCEEDED: ClassVar[str]
    PNL_KILL_SWITCH_TRIGGERED: ClassVar[str]
    ACCOUNT_BLOCKED: ClassVar[str]
    ACCOUNT_NOT_AUTHORIZED: ClassVar[str]
    COMPLIANCE_RESTRICTION: ClassVar[str]
    INSTRUMENT_RESTRICTED: ClassVar[str]
    JURISDICTION_RESTRICTION: ClassVar[str]
    WASH_TRADE_PREVENTION: ClassVar[str]
    SELF_MATCH_PREVENTION: ClassVar[str]
    SHORT_SALE_RESTRICTION: ClassVar[str]
    RISK_CONFIGURATION_MISSING: ClassVar[str]
    REFERENCE_DATA_UNAVAILABLE: ClassVar[str]
    ORDER_VALUE_CALCULATION_FAILED: ClassVar[str]
    SYSTEM_UNAVAILABLE: ClassVar[str]
    OTHER: ClassVar[str]

class Reject:
    """Business reject returned by pre-trade checks."""

    @property
    def code(self) -> str:
        """Reject code string."""

    @property
    def reason(self) -> str:
        """Human-readable reason."""

    @property
    def details(self) -> str:
        """Additional reject details."""

    @property
    def policy(self) -> str:
        """Policy name that produced the reject."""

    @property
    def scope(self) -> str:
        """Reject scope (``order`` or ``account``)."""

class Instrument:
    """Trading instrument with underlying and settlement assets."""

    def __init__(self, *, underlying_asset: str, settlement_asset: str) -> None:
        """Create an instrument."""
        _ = (underlying_asset, settlement_asset)

    @property
    def underlying_asset(self) -> str:
        """Underlying asset symbol."""

    @property
    def settlement_asset(self) -> str:
        """Settlement asset symbol."""

class Order:
    """Order input for pre-trade checks."""

    def __init__(
        self,
        *,
        underlying_asset: str,
        settlement_asset: str,
        side: Literal["buy", "sell"],
        quantity: NumericValue,
        price: NumericValue,
    ) -> None:
        """Create an order."""
        _ = (underlying_asset, settlement_asset, side, quantity, price)

    @property
    def underlying_asset(self) -> str:
        """Underlying asset symbol."""

    @property
    def settlement_asset(self) -> str:
        """Settlement asset symbol."""

    @property
    def side(self) -> Literal["buy", "sell"]:
        """Order side."""

    @property
    def quantity(self) -> str:
        """Order quantity."""

    @property
    def price(self) -> str:
        """Order price."""

class ExecutionReport:
    """Post-trade execution feedback for policy state updates."""

    def __init__(
        self,
        *,
        underlying_asset: str,
        settlement_asset: str,
        pnl: NumericValue,
        fee: NumericValue | None = None,
    ) -> None:
        """Create an execution report."""
        _ = (underlying_asset, settlement_asset, pnl, fee)

    @property
    def underlying_asset(self) -> str:
        """Underlying asset symbol."""

    @property
    def settlement_asset(self) -> str:
        """Settlement asset symbol."""

    @property
    def pnl(self) -> str:
        """Realized PnL value."""

    @property
    def fee(self) -> str:
        """Fee value."""

class Request:
    """Pre-trade request handle."""

    def execute(self) -> ExecuteResult:
        """Run main-stage pre-trade checks."""

class Reservation:
    """Reservation handle that must be finalized once."""

    def commit(self) -> None:
        """Finalize reservation as committed."""

    def rollback(self) -> None:
        """Finalize reservation as rolled back."""

class StartPreTradeResult:
    """Result of ``Engine.start_pre_trade``."""

    @property
    def ok(self) -> bool:
        """Whether start-stage checks passed."""

    @property
    def request(self) -> Request | None:
        """Request handle when checks pass."""

    @property
    def reject(self) -> Reject | None:
        """Reject data when checks fail."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``ok``."""

class ExecuteResult:
    """Result of ``Request.execute``."""

    @property
    def ok(self) -> bool:
        """Whether main-stage checks passed."""

    @property
    def reservation(self) -> Reservation | None:
        """Reservation when checks pass."""

    @property
    def rejects(self) -> list[Reject]:
        """Reject list when checks fail."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``ok``."""

class PostTradeResult:
    """Result of ``Engine.apply_execution_report``."""

    @property
    def kill_switch_triggered(self) -> bool:
        """Whether any policy reported an active kill switch."""

class PnlKillSwitchPolicy:
    """Built-in start-stage kill-switch policy based on PnL threshold."""

    NAME: ClassVar[str]

    def __init__(self, *, settlement_asset: str, barrier: NumericValue) -> None:
        """Create policy with the first barrier."""
        _ = (settlement_asset, barrier)

    def set_barrier(self, *, settlement_asset: str, barrier: NumericValue) -> None:
        """Add or update barrier for a settlement asset."""
        _ = (settlement_asset, barrier)

    def reset_pnl(self, *, settlement_asset: str) -> None:
        """Reset accumulated PnL for a settlement asset."""
        _ = settlement_asset

class RateLimitPolicy:
    """Built-in start-stage rate limit policy."""

    NAME: ClassVar[str]

    def __init__(self, *, max_orders: int, window_seconds: int) -> None:
        """Create a rate limit policy."""
        _ = (max_orders, window_seconds)

class OrderValidationPolicy:
    """Built-in start-stage order schema/field validation policy."""

    NAME: ClassVar[str]

    def __init__(self) -> None:
        """Create the order validation policy."""

class OrderSizeLimit:
    """Order size limits for one settlement asset."""

    def __init__(
        self,
        *,
        settlement_asset: str,
        max_quantity: NumericValue,
        max_notional: NumericValue,
    ) -> None:
        """Create order size limits."""
        _ = (settlement_asset, max_quantity, max_notional)

class OrderSizeLimitPolicy:
    """Built-in start-stage order size limit policy."""

    NAME: ClassVar[str]

    def __init__(self, *, limit: OrderSizeLimit) -> None:
        """Create policy with the first limit."""
        _ = limit

    def set_limit(self, *, limit: OrderSizeLimit) -> None:
        """Add or update a limit for settlement asset."""
        _ = limit

class EngineBuilder:
    """Engine configuration builder."""

    def check_pre_trade_start_policy(
        self,
        *,
        policy: (
            CheckPreTradeStartPolicy
            | OrderValidationPolicy
            | PnlKillSwitchPolicy
            | RateLimitPolicy
            | OrderSizeLimitPolicy
        ),
    ) -> EngineBuilder:
        """Register a start-stage policy."""
        _ = policy

    def pre_trade_policy(self, *, policy: Policy) -> EngineBuilder:
        """Register a main-stage policy."""
        _ = policy

    def build(self) -> Engine:
        """Build an engine instance."""

class Engine:
    """Pre-trade risk engine."""

    @staticmethod
    def builder() -> EngineBuilder:
        """Create a new engine builder."""

    def start_pre_trade(self, *, order: Order) -> StartPreTradeResult:
        """Run start-stage pre-trade checks."""
        _ = order

    def apply_execution_report(self, *, report: ExecutionReport) -> PostTradeResult:
        """Apply post-trade report to policy state."""
        _ = report
