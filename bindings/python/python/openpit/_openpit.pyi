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

import typing

from . import param
from .pretrade import CheckPreTradeStartPolicy, Policy

class RejectError(Exception):
    """Python exception type exposed by the native module."""

class RejectCode:
    """Stable reject code constants."""

    MISSING_REQUIRED_FIELD: typing.ClassVar[str]
    INVALID_FIELD_FORMAT: typing.ClassVar[str]
    INVALID_FIELD_VALUE: typing.ClassVar[str]
    UNSUPPORTED_ORDER_TYPE: typing.ClassVar[str]
    UNSUPPORTED_TIME_IN_FORCE: typing.ClassVar[str]
    UNSUPPORTED_ORDER_ATTRIBUTE: typing.ClassVar[str]
    DUPLICATE_CLIENT_ORDER_ID: typing.ClassVar[str]
    TOO_LATE_TO_ENTER: typing.ClassVar[str]
    EXCHANGE_CLOSED: typing.ClassVar[str]
    UNKNOWN_INSTRUMENT: typing.ClassVar[str]
    UNKNOWN_ACCOUNT: typing.ClassVar[str]
    UNKNOWN_VENUE: typing.ClassVar[str]
    UNKNOWN_CLEARING_ACCOUNT: typing.ClassVar[str]
    UNKNOWN_COLLATERAL_ASSET: typing.ClassVar[str]
    INSUFFICIENT_FUNDS: typing.ClassVar[str]
    INSUFFICIENT_MARGIN: typing.ClassVar[str]
    INSUFFICIENT_POSITION: typing.ClassVar[str]
    CREDIT_LIMIT_EXCEEDED: typing.ClassVar[str]
    RISK_LIMIT_EXCEEDED: typing.ClassVar[str]
    ORDER_EXCEEDS_LIMIT: typing.ClassVar[str]
    ORDER_QTY_EXCEEDS_LIMIT: typing.ClassVar[str]
    ORDER_NOTIONAL_EXCEEDS_LIMIT: typing.ClassVar[str]
    POSITION_LIMIT_EXCEEDED: typing.ClassVar[str]
    CONCENTRATION_LIMIT_EXCEEDED: typing.ClassVar[str]
    LEVERAGE_LIMIT_EXCEEDED: typing.ClassVar[str]
    RATE_LIMIT_EXCEEDED: typing.ClassVar[str]
    PNL_KILL_SWITCH_TRIGGERED: typing.ClassVar[str]
    ACCOUNT_BLOCKED: typing.ClassVar[str]
    ACCOUNT_NOT_AUTHORIZED: typing.ClassVar[str]
    COMPLIANCE_RESTRICTION: typing.ClassVar[str]
    INSTRUMENT_RESTRICTED: typing.ClassVar[str]
    JURISDICTION_RESTRICTION: typing.ClassVar[str]
    WASH_TRADE_PREVENTION: typing.ClassVar[str]
    SELF_MATCH_PREVENTION: typing.ClassVar[str]
    SHORT_SALE_RESTRICTION: typing.ClassVar[str]
    RISK_CONFIGURATION_MISSING: typing.ClassVar[str]
    REFERENCE_DATA_UNAVAILABLE: typing.ClassVar[str]
    ORDER_VALUE_CALCULATION_FAILED: typing.ClassVar[str]
    SYSTEM_UNAVAILABLE: typing.ClassVar[str]
    OTHER: typing.ClassVar[str]

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

    def __init__(
        self,
        underlying_asset: str,
        settlement_asset: str,
    ) -> None:
        """Create an instrument."""
        _ = (underlying_asset, settlement_asset)

    @property
    def underlying_asset(self) -> str:
        """Underlying asset symbol string."""

    @property
    def settlement_asset(self) -> str:
        """Settlement asset symbol string."""

class OrderOperation:
    """Main order parameters group."""

    def __init__(
        self,
        *,
        underlying_asset: str | None = None,
        settlement_asset: str | None = None,
        account_id: param.AccountId | None = None,
        side: param.Side | None = None,
        trade_amount: param.Quantity | param.Volume | None = None,
        price: param.Price | None = None,
    ) -> None:
        """Create an order operation group."""
        _ = (underlying_asset, settlement_asset, account_id, side, trade_amount, price)

    @property
    def underlying_asset(self) -> str | None:
        """Underlying asset symbol string."""

    @underlying_asset.setter
    def underlying_asset(self, value: str | None) -> None: ...
    @property
    def settlement_asset(self) -> str | None:
        """Settlement asset symbol string."""

    @settlement_asset.setter
    def settlement_asset(self, value: str | None) -> None: ...
    @property
    def account_id(self) -> param.AccountId | None:
        """Account identifier."""

    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def side(self) -> str | None:
        """Order side string."""

    @side.setter
    def side(self, value: param.Side | None) -> None: ...
    @property
    def trade_amount(self) -> param.Quantity | param.Volume | None:
        """Trade amount as Quantity or Volume."""

    @trade_amount.setter
    def trade_amount(self, value: param.Quantity | param.Volume | None) -> None: ...
    @property
    def price(self) -> str | None:
        """Price string."""

    @price.setter
    def price(self, value: param.Price | None) -> None: ...

class OrderPosition:
    """Position-management parameters group."""

    def __init__(
        self,
        *,
        position_side: param.PositionSide | None = None,
        reduce_only: bool = False,
        close_position: bool = False,
    ) -> None:
        """Create an order position group."""
        _ = (position_side, reduce_only, close_position)

    @property
    def position_side(self) -> str | None:
        """Position side string."""

    @position_side.setter
    def position_side(self, value: param.PositionSide | None) -> None: ...
    @property
    def reduce_only(self) -> bool:
        """Reduce-only flag."""

    @reduce_only.setter
    def reduce_only(self, value: bool) -> None: ...
    @property
    def close_position(self) -> bool:
        """Close-position flag."""

    @close_position.setter
    def close_position(self, value: bool) -> None: ...

class OrderMargin:
    """Margin-trading parameters group."""

    def __init__(
        self,
        *,
        leverage: param.Leverage | None = None,
        collateral_asset: str | None = None,
        auto_borrow: bool = False,
    ) -> None:
        """Create an order margin group."""
        _ = (leverage, collateral_asset, auto_borrow)

    @property
    def leverage(self) -> param.Leverage | None:
        """Optional leverage override."""

    @leverage.setter
    def leverage(self, value: param.Leverage | None) -> None: ...
    @property
    def collateral_asset(self) -> str | None:
        """Collateral asset string."""

    @collateral_asset.setter
    def collateral_asset(self, value: str | None) -> None: ...
    @property
    def auto_borrow(self) -> bool:
        """Auto-borrow flag."""

    @auto_borrow.setter
    def auto_borrow(self, value: bool) -> None: ...

class Order:
    """Extensible order model accepted by ``Engine.start_pre_trade``."""

    def __init__(
        self,
        *,
        operation: OrderOperation | None = None,
        position: OrderPosition | None = None,
        margin: OrderMargin | None = None,
    ) -> None:
        """Create an order with optional groups."""
        _ = (operation, position, margin)

    @property
    def operation(self) -> OrderOperation | None:
        """Main order parameters group."""

    @operation.setter
    def operation(self, value: OrderOperation | None) -> None: ...
    @property
    def position(self) -> OrderPosition | None:
        """Position-management parameters group."""

    @position.setter
    def position(self, value: OrderPosition | None) -> None: ...
    @property
    def margin(self) -> OrderMargin | None:
        """Margin-trading parameters group."""

    @margin.setter
    def margin(self, value: OrderMargin | None) -> None: ...

class ExecutionReportOperation:
    """Execution-report instrument and side group."""

    def __init__(
        self,
        *,
        underlying_asset: str | None = None,
        settlement_asset: str | None = None,
        account_id: param.AccountId | None = None,
        side: param.Side | None = None,
    ) -> None:
        """Create an execution report operation group."""
        _ = (underlying_asset, settlement_asset, account_id, side)

    @property
    def underlying_asset(self) -> str | None:
        """Underlying asset symbol string."""

    @underlying_asset.setter
    def underlying_asset(self, value: str | None) -> None: ...
    @property
    def settlement_asset(self) -> str | None:
        """Settlement asset symbol string."""

    @settlement_asset.setter
    def settlement_asset(self, value: str | None) -> None: ...
    @property
    def account_id(self) -> param.AccountId | None:
        """Account identifier."""

    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def side(self) -> str | None:
        """Trade side string."""

    @side.setter
    def side(self, value: param.Side | None) -> None: ...

class FinancialImpact:
    """Realized P&L and fee group."""

    def __init__(
        self,
        *,
        pnl: param.Pnl | None = None,
        fee: param.Fee | None = None,
    ) -> None:
        """Create a financial impact group."""
        _ = (pnl, fee)

    @property
    def pnl(self) -> str | None:
        """Realized PnL value string."""

    @pnl.setter
    def pnl(self, value: param.Pnl | None) -> None: ...
    @property
    def fee(self) -> str | None:
        """Fee value string."""

    @fee.setter
    def fee(self, value: param.Fee | None) -> None: ...

class ExecutionReportFillDetails:
    """Fill execution details group."""

    def __init__(
        self,
        *,
        fill_price: param.Price | None = None,
        fill_quantity: param.Quantity | None = None,
        is_terminal: bool = False,
    ) -> None:
        """Create a fill details group."""
        _ = (fill_price, fill_quantity, is_terminal)

    @property
    def fill_price(self) -> str | None:
        """Fill price string."""

    @fill_price.setter
    def fill_price(self, value: param.Price | None) -> None: ...
    @property
    def fill_quantity(self) -> str | None:
        """Fill quantity string."""

    @fill_quantity.setter
    def fill_quantity(self, value: param.Quantity | None) -> None: ...
    @property
    def is_terminal(self) -> bool:
        """Whether this is a terminal report."""

    @is_terminal.setter
    def is_terminal(self, value: bool) -> None: ...

class ExecutionReportPositionImpact:
    """Position-impact data group."""

    def __init__(
        self,
        *,
        position_effect: param.PositionEffect | None = None,
        position_side: param.PositionSide | None = None,
    ) -> None:
        """Create a position impact group."""
        _ = (position_effect, position_side)

    @property
    def position_effect(self) -> str | None:
        """Position effect string."""

    @position_effect.setter
    def position_effect(self, value: param.PositionEffect | None) -> None: ...
    @property
    def position_side(self) -> str | None:
        """Position side string."""

    @position_side.setter
    def position_side(self, value: param.PositionSide | None) -> None: ...

class ExecutionReport:
    """Extensible execution report model.

    Accepted by ``Engine.apply_execution_report``.
    """

    def __init__(
        self,
        *,
        operation: ExecutionReportOperation | None = None,
        financial_impact: FinancialImpact | None = None,
        fill: ExecutionReportFillDetails | None = None,
        position_impact: ExecutionReportPositionImpact | None = None,
    ) -> None:
        """Create an execution report with optional groups."""
        _ = (operation, financial_impact, fill, position_impact)

    @property
    def operation(self) -> ExecutionReportOperation | None:
        """Execution-report instrument and side group."""

    @operation.setter
    def operation(self, value: ExecutionReportOperation | None) -> None: ...
    @property
    def financial_impact(self) -> FinancialImpact | None:
        """Realized P&L and fee group."""

    @financial_impact.setter
    def financial_impact(self, value: FinancialImpact | None) -> None: ...
    @property
    def fill(self) -> ExecutionReportFillDetails | None:
        """Fill execution details group."""

    @fill.setter
    def fill(self, value: ExecutionReportFillDetails | None) -> None: ...
    @property
    def position_impact(self) -> ExecutionReportPositionImpact | None:
        """Position-impact data group."""

    @position_impact.setter
    def position_impact(self, value: ExecutionReportPositionImpact | None) -> None: ...

class AdjustmentAmount:
    """Delta-or-absolute adjustment payload wrapper."""

    def __init__(self, *, kind: str, value: param.PositionSize) -> None:
        _ = (kind, value)

    @staticmethod
    def delta(value: param.PositionSize) -> AdjustmentAmount:
        _ = value

    @staticmethod
    def absolute(value: param.PositionSize) -> AdjustmentAmount:
        _ = value

    @property
    def kind(self) -> str:
        """Variant name (`delta` or `absolute`)."""

    @property
    def value(self) -> param.PositionSize:
        """Wrapped position-size payload."""

class AccountAdjustmentAmount:
    """Grouped amount payload (`total + reserved + pending`)."""

    def __init__(
        self,
        *,
        total: AdjustmentAmount | None = None,
        reserved: AdjustmentAmount | None = None,
        pending: AdjustmentAmount | None = None,
    ) -> None:
        _ = (total, reserved, pending)

    @property
    def total(self) -> AdjustmentAmount | None: ...
    @total.setter
    def total(self, value: AdjustmentAmount | None) -> None: ...
    @property
    def reserved(self) -> AdjustmentAmount | None: ...
    @reserved.setter
    def reserved(self, value: AdjustmentAmount | None) -> None: ...
    @property
    def pending(self) -> AdjustmentAmount | None: ...
    @pending.setter
    def pending(self, value: AdjustmentAmount | None) -> None: ...

class AccountAdjustmentBalanceOperation:
    """Physical-balance account-adjustment operation group."""

    def __init__(
        self,
        *,
        account_id: param.AccountId | None = None,
        asset: str | None = None,
        average_entry_price: param.Price | None = None,
    ) -> None:
        _ = (account_id, asset, average_entry_price)

    @property
    def account_id(self) -> param.AccountId | None: ...
    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def asset(self) -> str | None: ...
    @asset.setter
    def asset(self, value: str | None) -> None: ...
    @property
    def average_entry_price(self) -> str | None: ...
    @average_entry_price.setter
    def average_entry_price(self, value: param.Price | None) -> None: ...

class AccountAdjustmentPositionOperation:
    """Derivatives-like position account-adjustment operation group."""

    def __init__(
        self,
        *,
        underlying_asset: str | None = None,
        settlement_asset: str | None = None,
        account_id: param.AccountId | None = None,
        collateral_asset: str | None = None,
        average_entry_price: param.Price | None = None,
        mode: param.PositionMode | None = None,
        leverage: param.Leverage | None = None,
    ) -> None:
        _ = (
            underlying_asset,
            settlement_asset,
            account_id,
            collateral_asset,
            average_entry_price,
            mode,
            leverage,
        )

    @property
    def underlying_asset(self) -> str | None: ...
    @underlying_asset.setter
    def underlying_asset(self, value: str | None) -> None: ...
    @property
    def settlement_asset(self) -> str | None: ...
    @settlement_asset.setter
    def settlement_asset(self, value: str | None) -> None: ...
    @property
    def account_id(self) -> param.AccountId | None: ...
    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def collateral_asset(self) -> str | None: ...
    @collateral_asset.setter
    def collateral_asset(self, value: str | None) -> None: ...
    @property
    def average_entry_price(self) -> str | None: ...
    @average_entry_price.setter
    def average_entry_price(self, value: param.Price | None) -> None: ...
    @property
    def mode(self) -> str | None: ...
    @mode.setter
    def mode(self, value: param.PositionMode | None) -> None: ...
    @property
    def leverage(self) -> param.Leverage | None: ...
    @leverage.setter
    def leverage(self, value: param.Leverage | None) -> None: ...

class AccountAdjustmentBounds:
    """Optional post-adjustment bounds group."""

    def __init__(
        self,
        *,
        total_upper_bound: param.PositionSize | None = None,
        total_lower_bound: param.PositionSize | None = None,
        reserved_upper_bound: param.PositionSize | None = None,
        reserved_lower_bound: param.PositionSize | None = None,
        pending_upper_bound: param.PositionSize | None = None,
        pending_lower_bound: param.PositionSize | None = None,
    ) -> None:
        _ = (
            total_upper_bound,
            total_lower_bound,
            reserved_upper_bound,
            reserved_lower_bound,
            pending_upper_bound,
            pending_lower_bound,
        )

    @property
    def total_upper_bound(self) -> param.PositionSize | None: ...
    @total_upper_bound.setter
    def total_upper_bound(self, value: param.PositionSize | None) -> None: ...
    @property
    def total_lower_bound(self) -> param.PositionSize | None: ...
    @total_lower_bound.setter
    def total_lower_bound(self, value: param.PositionSize | None) -> None: ...
    @property
    def reserved_upper_bound(self) -> param.PositionSize | None: ...
    @reserved_upper_bound.setter
    def reserved_upper_bound(self, value: param.PositionSize | None) -> None: ...
    @property
    def reserved_lower_bound(self) -> param.PositionSize | None: ...
    @reserved_lower_bound.setter
    def reserved_lower_bound(self, value: param.PositionSize | None) -> None: ...
    @property
    def pending_upper_bound(self) -> param.PositionSize | None: ...
    @pending_upper_bound.setter
    def pending_upper_bound(self, value: param.PositionSize | None) -> None: ...
    @property
    def pending_lower_bound(self) -> param.PositionSize | None: ...
    @pending_lower_bound.setter
    def pending_lower_bound(self, value: param.PositionSize | None) -> None: ...

class AccountAdjustment:
    """Extensible non-trading account-adjustment record."""

    def __init__(
        self,
        *,
        operation: AccountAdjustmentBalanceOperation
        | AccountAdjustmentPositionOperation
        | None = None,
        amount: AccountAdjustmentAmount | None = None,
        bounds: AccountAdjustmentBounds | None = None,
    ) -> None:
        _ = (operation, amount, bounds)

    @property
    def operation(
        self,
    ) -> (
        AccountAdjustmentBalanceOperation | AccountAdjustmentPositionOperation | None
    ): ...
    @operation.setter
    def operation(
        self,
        value: AccountAdjustmentBalanceOperation
        | AccountAdjustmentPositionOperation
        | None,
    ) -> None: ...
    @property
    def amount(self) -> AccountAdjustmentAmount | None: ...
    @amount.setter
    def amount(self, value: AccountAdjustmentAmount | None) -> None: ...
    @property
    def bounds(self) -> AccountAdjustmentBounds | None: ...
    @bounds.setter
    def bounds(self, value: AccountAdjustmentBounds | None) -> None: ...

class Request:
    """
    Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

    The handle is single-use: calling ``execute`` more than once is a lifecycle
    error.
    """

    def execute(self) -> ExecuteResult:
        """Run main-stage pre-trade checks."""

class Reservation:
    """
    Single-use reservation handle returned by successful main-stage execution.

    Exactly one of ``commit`` or ``rollback`` must be called to finalize the
    reserved state.
    """

    def commit(self) -> None:
        """Finalize reservation as committed."""

    def rollback(self) -> None:
        """Finalize reservation as rolled back."""

class StartPreTradeResult:
    """
    Result of ``Engine.start_pre_trade``.

    On success it exposes a deferred request handle; on failure it exposes a
    single business reject produced by the first rejecting start-stage policy.
    """

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
    """
    Result of ``Request.execute``.

    This object reports whether main-stage policies accepted the request and,
    on success, carries the single-use reservation handle that must later be
    committed or rolled back.
    """

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
    """
    Result of ``Engine.apply_execution_report``.

    Reports whether any policy considers an account-level kill switch to be
    active after the report has been applied.
    """

    @property
    def kill_switch_triggered(self) -> bool:
        """Whether any policy reported an active kill switch."""

class PnlKillSwitchPolicy:
    """Built-in start-stage kill-switch policy based on PnL threshold."""

    NAME: typing.ClassVar[str]

    def __init__(
        self,
        settlement_asset: param.Asset,
        barrier: param.Pnl,
    ) -> None:
        """Create policy with the first barrier."""
        _ = (settlement_asset, barrier)

    def set_barrier(
        self,
        settlement_asset: param.Asset,
        barrier: param.Pnl,
    ) -> None:
        """Add or update barrier for a settlement asset."""
        _ = (settlement_asset, barrier)

    def reset_pnl(self, settlement_asset: param.Asset) -> None:
        """Reset accumulated PnL for a settlement asset."""
        _ = settlement_asset

class RateLimitPolicy:
    """Built-in start-stage rate limit policy."""

    NAME: typing.ClassVar[str]

    def __init__(self, max_orders: int, window_seconds: int) -> None:
        """Create a rate limit policy."""
        _ = (max_orders, window_seconds)

class OrderValidationPolicy:
    """Built-in start-stage order schema/field validation policy."""

    NAME: typing.ClassVar[str]

    def __init__(self) -> None:
        """Create the order validation policy."""

class OrderSizeLimit:
    """Order size limits for one settlement asset."""

    def __init__(
        self,
        *,
        settlement_asset: param.Asset,
        max_quantity: param.Quantity,
        max_notional: param.Volume,
    ) -> None:
        """Create order size limits."""
        _ = (settlement_asset, max_quantity, max_notional)

class OrderSizeLimitPolicy:
    """Built-in start-stage order size limit policy."""

    NAME: typing.ClassVar[str]

    def __init__(self, limit: OrderSizeLimit) -> None:
        """Create policy with the first limit."""
        _ = limit

    def set_limit(self, limit: OrderSizeLimit) -> None:
        """Add or update a limit for settlement asset."""
        _ = limit

class EngineBuilder:
    """Engine configuration builder."""

    def check_pre_trade_start_policy(
        self,
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

    def pre_trade_policy(self, policy: Policy) -> EngineBuilder:
        """Register a main-stage policy."""
        _ = policy

    def build(self) -> Engine:
        """Build an engine instance."""

class Engine:
    """Pre-trade risk engine."""

    @staticmethod
    def builder() -> EngineBuilder:
        """Create a new engine builder."""

    def start_pre_trade(self, order: Order) -> StartPreTradeResult:
        """Run start-stage pre-trade checks."""
        _ = order

    def apply_execution_report(self, report: ExecutionReport) -> PostTradeResult:
        """Apply post-trade report to policy state."""
        _ = report
