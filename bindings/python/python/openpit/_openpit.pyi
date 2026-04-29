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

import decimal
import typing

from . import param
from .account_adjustment import AccountAdjustmentPolicy
from .pretrade import CheckPreTradeStartPolicy, PreTradePolicy

_ROUNDING_STRATEGY_DEFAULT: str
_ROUNDING_STRATEGY_BANKER: str
_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT: str
_ROUNDING_STRATEGY_CONSERVATIVE_LOSS: str
_LEVERAGE_SCALE: int
_LEVERAGE_MIN: int
_LEVERAGE_MAX: int
_LEVERAGE_STEP: float

def _validate_asset(value: str) -> None: ...

class RejectError(Exception):
    """Python exception type exposed by the native module."""

class ParamError(ValueError):
    """Numeric parameter validation and arithmetic error."""

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

    @property
    def user_data(self) -> int:
        """Opaque caller-defined integer token. ``0`` means "not set". The SDK never
        inspects, dereferences, or frees it."""

class Leverage:
    """Per-order leverage multiplier."""

    SCALE: typing.ClassVar[int]
    MIN: typing.ClassVar[int]
    MAX: typing.ClassVar[int]
    STEP: typing.ClassVar[float]

    def __init__(self, value: int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``int`` or ``from_int``."""

    @staticmethod
    def from_int(value: int) -> Leverage: ...
    @staticmethod
    def from_float(value: float) -> Leverage:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``from_int`` with an integer value."""

    @property
    def value(self) -> float: ...
    def calculate_margin_required(
        self,
        notional: Notional | decimal.Decimal | str | int | float,
    ) -> Notional: ...

class AccountId:
    """Type-safe account identifier."""

    @staticmethod
    def from_u64(value: int) -> AccountId: ...
    @staticmethod
    def from_str(value: str) -> AccountId: ...
    @property
    def value(self) -> int: ...

class Quantity:
    """Instrument quantity value type."""

    ZERO: typing.ClassVar[Quantity]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Quantity: ...
    @staticmethod
    def from_str(value: str) -> Quantity: ...
    @staticmethod
    def from_int(value: int) -> Quantity: ...
    @staticmethod
    def from_u64(value: int) -> Quantity: ...
    @staticmethod
    def from_float(value: float) -> Quantity:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> Quantity: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Quantity:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Quantity: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def calculate_volume(self, price: Price) -> Volume: ...
    def __add__(self, other: Quantity) -> Quantity: ...
    def __sub__(self, other: Quantity) -> Quantity: ...
    def __mul__(self, other: int | float) -> Quantity: ...
    def __rmul__(self, other: int | float) -> Quantity: ...
    def __truediv__(self, other: int | float) -> Quantity: ...
    def __mod__(self, other: int | float) -> Quantity: ...

class Price:
    """Instrument price value type."""

    ZERO: typing.ClassVar[Price]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Price: ...
    @staticmethod
    def from_str(value: str) -> Price: ...
    @staticmethod
    def from_int(value: int) -> Price: ...
    @staticmethod
    def from_u64(value: int) -> Price: ...
    @staticmethod
    def from_float(value: float) -> Price:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> Price: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Price:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Price: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def calculate_volume(self, quantity: Quantity) -> Volume: ...
    def __add__(self, other: Price) -> Price: ...
    def __sub__(self, other: Price) -> Price: ...
    def __neg__(self) -> Price: ...
    def __mul__(self, other: int | float) -> Price: ...
    def __rmul__(self, other: int | float) -> Price: ...
    def __truediv__(self, other: int | float) -> Price: ...
    def __mod__(self, other: int | float) -> Price: ...

class Pnl:
    """Profit and loss value type."""

    ZERO: typing.ClassVar[Pnl]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Pnl: ...
    @staticmethod
    def from_str(value: str) -> Pnl: ...
    @staticmethod
    def from_int(value: int) -> Pnl: ...
    @staticmethod
    def from_u64(value: int) -> Pnl: ...
    @staticmethod
    def from_float(value: float) -> Pnl:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> Pnl: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Pnl:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Pnl: ...
    @staticmethod
    def from_fee(fee: Fee) -> Pnl: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_cash_flow(self) -> CashFlow: ...
    def to_position_size(self) -> PositionSize: ...
    def __add__(self, other: Pnl) -> Pnl: ...
    def __sub__(self, other: Pnl) -> Pnl: ...
    def __neg__(self) -> Pnl: ...
    def __mul__(self, other: int | float) -> Pnl: ...
    def __rmul__(self, other: int | float) -> Pnl: ...
    def __truediv__(self, other: int | float) -> Pnl: ...
    def __mod__(self, other: int | float) -> Pnl: ...

class Fee:
    """Fee value type."""

    ZERO: typing.ClassVar[Fee]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Fee: ...
    @staticmethod
    def from_str(value: str) -> Fee: ...
    @staticmethod
    def from_int(value: int) -> Fee: ...
    @staticmethod
    def from_u64(value: int) -> Fee: ...
    @staticmethod
    def from_float(value: float) -> Fee:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> Fee: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Fee:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Fee: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_pnl(self) -> Pnl: ...
    def to_position_size(self) -> PositionSize: ...
    def to_cash_flow(self) -> CashFlow: ...
    def __add__(self, other: Fee) -> Fee: ...
    def __sub__(self, other: Fee) -> Fee: ...
    def __neg__(self) -> Fee: ...
    def __mul__(self, other: int | float) -> Fee: ...
    def __rmul__(self, other: int | float) -> Fee: ...
    def __truediv__(self, other: int | float) -> Fee: ...
    def __mod__(self, other: int | float) -> Fee: ...

class Volume:
    """Notional volume value type."""

    ZERO: typing.ClassVar[Volume]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Volume: ...
    @staticmethod
    def from_str(value: str) -> Volume: ...
    @staticmethod
    def from_int(value: int) -> Volume: ...
    @staticmethod
    def from_u64(value: int) -> Volume: ...
    @staticmethod
    def from_float(value: float) -> Volume:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> Volume: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Volume:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Volume: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_cash_flow_inflow(self) -> CashFlow: ...
    def to_cash_flow_outflow(self) -> CashFlow: ...
    def calculate_quantity(self, price: Price) -> Quantity: ...
    def to_notional(self) -> Notional: ...
    def __add__(self, other: Volume) -> Volume: ...
    def __sub__(self, other: Volume) -> Volume: ...
    def __mul__(self, other: int | float) -> Volume: ...
    def __rmul__(self, other: int | float) -> Volume: ...
    def __truediv__(self, other: int | float) -> Volume: ...
    def __mod__(self, other: int | float) -> Volume: ...

class Notional:
    """Monetary position exposure value type.

    Represents the absolute monetary value of a position in the settlement
    currency: ``|price| × quantity``. Always non-negative. Used for margin
    and risk calculation.
    """

    ZERO: typing.ClassVar[Notional]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Notional: ...
    @staticmethod
    def from_str(value: str) -> Notional: ...
    @staticmethod
    def from_int(value: int) -> Notional: ...
    @staticmethod
    def from_u64(value: int) -> Notional: ...
    @staticmethod
    def from_float(value: float) -> Notional:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> Notional: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Notional:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Notional: ...
    @staticmethod
    def from_volume(volume: Volume) -> Notional: ...
    @staticmethod
    def from_price_quantity(price: Price, quantity: Quantity) -> Notional: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_volume(self) -> Volume: ...
    def calculate_margin_required(
        self,
        leverage: Leverage | int | float,
    ) -> Notional: ...
    def __add__(self, other: Notional) -> Notional: ...
    def __sub__(self, other: Notional) -> Notional: ...
    def __mul__(self, other: int | float) -> Notional: ...
    def __rmul__(self, other: int | float) -> Notional: ...
    def __truediv__(self, other: int | float) -> Notional: ...
    def __mod__(self, other: int | float) -> Notional: ...

class CashFlow:
    """Cash-flow contribution value type."""

    ZERO: typing.ClassVar[CashFlow]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> CashFlow: ...
    @staticmethod
    def from_str(value: str) -> CashFlow: ...
    @staticmethod
    def from_int(value: int) -> CashFlow: ...
    @staticmethod
    def from_u64(value: int) -> CashFlow: ...
    @staticmethod
    def from_float(value: float) -> CashFlow:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> CashFlow: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> CashFlow:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> CashFlow: ...
    @staticmethod
    def from_pnl(pnl: Pnl) -> CashFlow: ...
    @staticmethod
    def from_fee(fee: Fee) -> CashFlow: ...
    @staticmethod
    def from_volume_inflow(volume: Volume) -> CashFlow: ...
    @staticmethod
    def from_volume_outflow(volume: Volume) -> CashFlow: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def __add__(self, other: CashFlow) -> CashFlow: ...
    def __sub__(self, other: CashFlow) -> CashFlow: ...
    def __neg__(self) -> CashFlow: ...
    def __mul__(self, other: int | float) -> CashFlow: ...
    def __rmul__(self, other: int | float) -> CashFlow: ...
    def __truediv__(self, other: int | float) -> CashFlow: ...
    def __mod__(self, other: int | float) -> CashFlow: ...

class PositionSize:
    """Signed position-size value type."""

    ZERO: typing.ClassVar[PositionSize]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> PositionSize: ...
    @staticmethod
    def from_str(value: str) -> PositionSize: ...
    @staticmethod
    def from_int(value: int) -> PositionSize: ...
    @staticmethod
    def from_u64(value: int) -> PositionSize: ...
    @staticmethod
    def from_float(value: float) -> PositionSize:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_str_rounded(value: str, scale: int, strategy: str) -> PositionSize: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> PositionSize:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> PositionSize: ...
    @staticmethod
    def from_quantity_and_side(quantity: Quantity, side: str) -> PositionSize: ...
    @staticmethod
    def from_pnl(pnl: Pnl) -> PositionSize: ...
    @staticmethod
    def from_fee(fee: Fee) -> PositionSize: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_open_quantity(self) -> tuple[Quantity, str]: ...
    def to_close_quantity(self) -> tuple[Quantity, str | None]: ...
    def checked_add_quantity(self, qty: Quantity, side: str) -> PositionSize: ...
    def __add__(self, other: PositionSize) -> PositionSize: ...
    def __sub__(self, other: PositionSize) -> PositionSize: ...
    def __neg__(self) -> PositionSize: ...
    def __mul__(self, other: int | float) -> PositionSize: ...
    def __rmul__(self, other: int | float) -> PositionSize: ...
    def __truediv__(self, other: int | float) -> PositionSize: ...
    def __mod__(self, other: int | float) -> PositionSize: ...

class Instrument:
    """Trading instrument with underlying and settlement assets."""

    def __init__(
        self,
        underlying_asset: param.Asset,
        settlement_asset: param.Asset,
    ) -> None:
        """Create an instrument."""
        _ = (underlying_asset, settlement_asset)

    @property
    def underlying_asset(self) -> param.Asset:
        """Underlying asset symbol string."""

    @property
    def settlement_asset(self) -> param.Asset:
        """Settlement asset symbol string."""

class OrderOperation:
    """Main order parameters group."""

    def __init__(
        self,
        *,
        underlying_asset: param.Asset | None = None,
        settlement_asset: param.Asset | None = None,
        account_id: param.AccountId | None = None,
        side: param.Side | None = None,
        trade_amount: TradeAmount | None = None,
        price: param.Price | None = None,
    ) -> None:
        """Create an order operation group."""
        _ = (underlying_asset, settlement_asset, account_id, side, trade_amount, price)

    @property
    def underlying_asset(self) -> param.Asset | None:
        """Underlying asset symbol string."""

    @underlying_asset.setter
    def underlying_asset(self, value: param.Asset | None) -> None: ...
    @property
    def settlement_asset(self) -> param.Asset | None:
        """Settlement asset symbol string."""

    @settlement_asset.setter
    def settlement_asset(self, value: param.Asset | None) -> None: ...
    @property
    def account_id(self) -> param.AccountId | None:
        """Account identifier."""

    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def side(self) -> param.Side | None:
        """Order side."""

    @side.setter
    def side(self, value: param.Side | None) -> None: ...
    @property
    def trade_amount(self) -> TradeAmount | None: ...
    @trade_amount.setter
    def trade_amount(self, value: TradeAmount | None) -> None: ...
    @property
    def price(self) -> param.Price | None:
        """Order price."""

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
    def position_side(self) -> param.PositionSide | None:
        """Position side."""

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
        leverage: param.Leverage | int | float | None = None,
        collateral_asset: param.Asset | None = None,
        auto_borrow: bool = False,
    ) -> None:
        """Create an order margin group."""
        _ = (leverage, collateral_asset, auto_borrow)

    @property
    def leverage(self) -> param.Leverage | None:
        """Optional leverage override."""

    @leverage.setter
    def leverage(self, value: param.Leverage | int | float | None) -> None: ...
    @property
    def collateral_asset(self) -> param.Asset | None:
        """Collateral asset string."""

    @collateral_asset.setter
    def collateral_asset(self, value: param.Asset | None) -> None: ...
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
        underlying_asset: param.Asset | None = None,
        settlement_asset: param.Asset | None = None,
        account_id: param.AccountId | None = None,
        side: param.Side | None = None,
    ) -> None:
        """Create an execution report operation group."""
        _ = (underlying_asset, settlement_asset, account_id, side)

    @property
    def underlying_asset(self) -> param.Asset | None:
        """Underlying asset symbol string."""

    @underlying_asset.setter
    def underlying_asset(self, value: param.Asset | None) -> None: ...
    @property
    def settlement_asset(self) -> param.Asset | None:
        """Settlement asset symbol string."""

    @settlement_asset.setter
    def settlement_asset(self, value: param.Asset | None) -> None: ...
    @property
    def account_id(self) -> param.AccountId | None:
        """Account identifier."""

    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def side(self) -> param.Side | None:
        """Trade side."""

    @side.setter
    def side(self, value: param.Side | None) -> None: ...

class FinancialImpact:
    """Realized P&L and fee group."""

    def __init__(
        self,
        *,
        pnl: param.Pnl,
        fee: param.Fee,
    ) -> None:
        """Create a financial impact group."""
        _ = (pnl, fee)

    @property
    def pnl(self) -> param.Pnl:
        """Realized PnL value."""

    @pnl.setter
    def pnl(self, value: param.Pnl) -> None: ...
    @property
    def fee(self) -> param.Fee:
        """Fee value."""

    @fee.setter
    def fee(self, value: param.Fee) -> None: ...

class ExecutionReportFillDetails:
    """Fill execution details group."""

    def __init__(
        self,
        *,
        last_trade: Trade | None = None,
        leaves_quantity: param.Quantity | None = None,
        lock: PreTradeLock,
        is_final: bool | None = None,
    ) -> None:
        """Create a fill details group."""
        _ = (last_trade, leaves_quantity, lock, is_final)

    @property
    def last_trade(self) -> Trade | None:
        """Last executed trade."""

    @last_trade.setter
    def last_trade(self, value: Trade | None) -> None: ...
    @property
    def leaves_quantity(self) -> param.Quantity | None:
        """Remaining order quantity."""

    @leaves_quantity.setter
    def leaves_quantity(self, value: param.Quantity | None) -> None: ...
    @property
    def lock(self) -> PreTradeLock:
        """Order lock payload."""

    @lock.setter
    def lock(self, value: PreTradeLock) -> None: ...
    @property
    def is_final(self) -> bool | None:
        """Whether this report closes the order's report stream.

        The order is filled, cancelled, or rejected.
        """

    @is_final.setter
    def is_final(self, value: bool | None) -> None: ...

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
    def position_effect(self) -> param.PositionEffect | None:
        """Position effect."""

    @position_effect.setter
    def position_effect(self, value: param.PositionEffect | None) -> None: ...
    @property
    def position_side(self) -> param.PositionSide | None:
        """Position side."""

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

class TradeAmount:
    """Quantity- or volume-based trade amount."""

    def __init__(self, other: TradeAmount) -> None:
        """Copy / subclass constructor."""
        _ = other

    @staticmethod
    def quantity(value: param.Quantity | str | int | float) -> TradeAmount:
        """Create a quantity-based trade amount."""
        _ = value

    @staticmethod
    def volume(value: param.Volume | str | int | float) -> TradeAmount:
        """Create a volume-based trade amount."""
        _ = value

    @property
    def is_quantity(self) -> bool:
        """True when the amount is expressed as quantity (instrument units)."""

    @property
    def is_volume(self) -> bool:
        """True when the amount is expressed as volume (notional units)."""

    @property
    def as_quantity(self) -> param.Quantity | None:
        """Inner quantity, or None when the amount is volume-based."""

    @property
    def as_volume(self) -> param.Volume | None:
        """Inner volume, or None when the amount is quantity-based."""

class AdjustmentAmount:
    """Delta-or-absolute adjustment payload."""

    def __init__(self, other: AdjustmentAmount) -> None:
        """Copy / subclass constructor."""
        _ = other

    @staticmethod
    def delta(value: param.PositionSize) -> AdjustmentAmount:
        _ = value

    @staticmethod
    def absolute(value: param.PositionSize) -> AdjustmentAmount:
        """Create an absolute-type adjustment."""
        _ = value

    @property
    def is_delta(self) -> bool:
        """True when the adjustment is a signed delta."""

    @property
    def is_absolute(self) -> bool:
        """True when the adjustment sets an absolute value."""

    @property
    def as_delta(self) -> param.PositionSize | None:
        """Inner position size when delta, otherwise None."""

    @property
    def as_absolute(self) -> param.PositionSize | None:
        """Inner position size when absolute, otherwise None."""

class Trade:
    """Trade payload with price and quantity."""

    def __init__(self, *, price: param.Price, quantity: param.Quantity) -> None:
        _ = (price, quantity)

    @property
    def price(self) -> param.Price:
        """Trade price."""

    @property
    def quantity(self) -> param.Quantity:
        """Trade quantity."""

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
        asset: param.Asset | None = None,
        average_entry_price: param.Price | None = None,
    ) -> None:
        _ = (asset, average_entry_price)

    @property
    def asset(self) -> param.Asset | None: ...
    @asset.setter
    def asset(self, value: param.Asset | None) -> None: ...
    @property
    def average_entry_price(self) -> param.Price | None: ...
    @average_entry_price.setter
    def average_entry_price(self, value: param.Price | None) -> None: ...

class AccountAdjustmentPositionOperation:
    """Derivatives-like position account-adjustment operation group."""

    def __init__(
        self,
        *,
        underlying_asset: param.Asset | None = None,
        settlement_asset: param.Asset | None = None,
        collateral_asset: param.Asset | None = None,
        average_entry_price: param.Price | None = None,
        mode: param.PositionMode | None = None,
        leverage: param.Leverage | int | float | None = None,
    ) -> None:
        _ = (
            underlying_asset,
            settlement_asset,
            collateral_asset,
            average_entry_price,
            mode,
            leverage,
        )

    @property
    def underlying_asset(self) -> param.Asset | None: ...
    @underlying_asset.setter
    def underlying_asset(self, value: param.Asset | None) -> None: ...
    @property
    def settlement_asset(self) -> param.Asset | None: ...
    @settlement_asset.setter
    def settlement_asset(self, value: param.Asset | None) -> None: ...
    @property
    def collateral_asset(self) -> param.Asset | None: ...
    @collateral_asset.setter
    def collateral_asset(self, value: param.Asset | None) -> None: ...
    @property
    def average_entry_price(self) -> param.Price | None: ...
    @average_entry_price.setter
    def average_entry_price(self, value: param.Price | None) -> None: ...
    @property
    def mode(self) -> param.PositionMode | None: ...
    @mode.setter
    def mode(self, value: param.PositionMode | None) -> None: ...
    @property
    def leverage(self) -> param.Leverage | None: ...
    @leverage.setter
    def leverage(self, value: param.Leverage | int | float | None) -> None: ...

class AccountAdjustmentBounds:
    """Optional post-adjustment bounds group."""

    def __init__(
        self,
        *,
        total_upper: param.PositionSize | None = None,
        total_lower: param.PositionSize | None = None,
        reserved_upper: param.PositionSize | None = None,
        reserved_lower: param.PositionSize | None = None,
        pending_upper: param.PositionSize | None = None,
        pending_lower: param.PositionSize | None = None,
    ) -> None:
        _ = (
            total_upper,
            total_lower,
            reserved_upper,
            reserved_lower,
            pending_upper,
            pending_lower,
        )

    @property
    def total_upper(self) -> param.PositionSize | None: ...
    @total_upper.setter
    def total_upper(self, value: param.PositionSize | None) -> None: ...
    @property
    def total_lower(self) -> param.PositionSize | None: ...
    @total_lower.setter
    def total_lower(self, value: param.PositionSize | None) -> None: ...
    @property
    def reserved_upper(self) -> param.PositionSize | None: ...
    @reserved_upper.setter
    def reserved_upper(self, value: param.PositionSize | None) -> None: ...
    @property
    def reserved_lower(self) -> param.PositionSize | None: ...
    @reserved_lower.setter
    def reserved_lower(self, value: param.PositionSize | None) -> None: ...
    @property
    def pending_upper(self) -> param.PositionSize | None: ...
    @pending_upper.setter
    def pending_upper(self, value: param.PositionSize | None) -> None: ...
    @property
    def pending_lower(self) -> param.PositionSize | None: ...
    @pending_lower.setter
    def pending_lower(self, value: param.PositionSize | None) -> None: ...

class AccountAdjustment:
    """Extensible non-trading account-adjustment record."""

    def __init__(
        self,
        *,
        operation: (
            AccountAdjustmentBalanceOperation
            | AccountAdjustmentPositionOperation
            | None
        ) = None,
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
        value: (
            AccountAdjustmentBalanceOperation
            | AccountAdjustmentPositionOperation
            | None
        ),
    ) -> None: ...
    @property
    def amount(self) -> AccountAdjustmentAmount | None: ...
    @amount.setter
    def amount(self, value: AccountAdjustmentAmount | None) -> None: ...
    @property
    def bounds(self) -> AccountAdjustmentBounds | None: ...
    @bounds.setter
    def bounds(self, value: AccountAdjustmentBounds | None) -> None: ...

class PreTradeRequest:
    """
    Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

    The handle is single-use: calling ``execute`` more than once is a lifecycle
    error.
    """

    def execute(self) -> ExecuteResult:
        """Run main-stage pre-trade checks."""

class PreTradeLock:
    """Pre-trade lock payload."""

    def __init__(self, price: param.Price | None = None) -> None:
        _ = price

    @property
    def price(self) -> param.Price | None:
        """Optional locked price."""

class PreTradeReservation:
    """
    Single-use reservation handle returned by successful main-stage execution.

    Exactly one of ``commit`` or ``rollback`` must be called to finalize the
    reserved state.
    """

    def lock(self) -> PreTradeLock:
        """Current reservation lock payload."""

    def commit(self) -> None:
        """Finalize reservation as committed."""

    def rollback(self) -> None:
        """Finalize reservation as rolled back."""

class StartPreTradeResult:
    """
    Result of ``Engine.start_pre_trade``.

    On success it exposes a deferred request handle; on failure it exposes the
    merged reject list from all rejecting start-stage policies.
    """

    @property
    def ok(self) -> bool:
        """Whether start-stage checks passed."""

    @property
    def request(self) -> PreTradeRequest | None:
        """Request handle when checks pass."""

    @property
    def rejects(self) -> list[Reject]:
        """Reject list when checks fail."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``ok``."""

class ExecuteResult:
    """
    Result of ``PreTradeRequest.execute``.

    This object reports whether main-stage policies accepted the request and,
    on success, carries the single-use reservation handle that must later be
    committed or rolled back.
    """

    @property
    def ok(self) -> bool:
        """Whether main-stage checks passed."""

    @property
    def reservation(self) -> PreTradeReservation | None:
        """Reservation when checks pass."""

    @property
    def rejects(self) -> list[Reject]:
        """Reject list when checks fail."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``ok``."""

class AccountAdjustmentBatchResult:
    """Result of ``Engine.apply_account_adjustment``."""

    @property
    def ok(self) -> bool:
        """Whether the full batch passed."""

    @property
    def failed_index(self) -> int | None:
        """Zero-based index of the failing adjustment."""

    @property
    def rejects(self) -> list[Reject]:
        """Reject list when validation fails."""

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

class PreTradeContext:
    """Context of the current pre-trade operation."""

class AccountAdjustmentContext:
    """Context of the current account-adjustment operation."""

class PnlBoundsKillSwitchPolicy:
    """Built-in start-stage kill-switch policy based on PnL bounds."""

    NAME: typing.ClassVar[str]

    def __init__(
        self,
        *,
        settlement_asset: param.Asset,
        lower_bound: param.Pnl | None = None,
        upper_bound: param.Pnl | None = None,
        initial_pnl: param.Pnl,
    ) -> None:
        """Create policy with the first per-settlement bounds barrier."""
        _ = (settlement_asset, lower_bound, upper_bound, initial_pnl)

    def set_barrier(
        self,
        *,
        settlement_asset: param.Asset,
        lower_bound: param.Pnl | None = None,
        upper_bound: param.Pnl | None = None,
        initial_pnl: param.Pnl,
    ) -> None:
        """Add or update bounds for a settlement asset."""
        _ = (settlement_asset, lower_bound, upper_bound, initial_pnl)

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
            | PnlBoundsKillSwitchPolicy
            | RateLimitPolicy
            | OrderSizeLimitPolicy
        ),
    ) -> EngineBuilder:
        """Register a start-stage policy."""
        _ = policy

    def pre_trade_policy(self, policy: PreTradePolicy) -> EngineBuilder:
        """Register a main-stage policy."""
        _ = policy

    def account_adjustment_policy(
        self,
        policy: AccountAdjustmentPolicy,
    ) -> EngineBuilder:
        """Register an account-adjustment policy."""
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

    def execute_pre_trade(self, order: Order) -> ExecuteResult:
        """Run start-stage and main-stage checks in one call."""
        _ = order

    def apply_execution_report(self, report: ExecutionReport) -> PostTradeResult:
        """Apply post-trade report to policy state."""
        _ = report

    def apply_account_adjustment(
        self,
        account_id: param.AccountId,
        adjustments: typing.Iterable[AccountAdjustment],
    ) -> AccountAdjustmentBatchResult:
        """Validate a batch of account adjustments."""
        _ = account_id, adjustments
