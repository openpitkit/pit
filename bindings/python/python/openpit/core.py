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

import typing

from ._openpit import (
    ExecutionReport as _ExecutionReport,
)
from ._openpit import (
    ExecutionReportFillDetails as _ExecutionReportFillDetails,
)
from ._openpit import (
    ExecutionReportOperation as _ExecutionReportOperation,
)
from ._openpit import (
    ExecutionReportPositionImpact as _ExecutionReportPositionImpact,
)
from ._openpit import (
    FinancialImpact as _FinancialImpact,
)
from ._openpit import (
    Instrument as _Instrument,
)
from ._openpit import (
    Order as _Order,
)
from ._openpit import (
    OrderMargin as _OrderMargin,
)
from ._openpit import (
    OrderOperation as _OrderOperation,
)
from ._openpit import (
    OrderPosition as _OrderPosition,
)
from .param import (
    Asset,
    Fee,
    Leverage,
    Pnl,
    PositionEffect,
    PositionSide,
    Price,
    Quantity,
    Side,
    Volume,
)


def _require_instance(
    value: typing.Any,
    expected_type: type[typing.Any],
    *,
    name: str,
) -> typing.Any:
    if value is None:
        return None
    if not isinstance(value, expected_type):
        raise TypeError(
            f"{name} must be {expected_type.__module__}.{expected_type.__name__}"
        )
    return value


def _convert_trade_amount(value: typing.Any) -> Quantity | Volume:
    if isinstance(value, (Quantity, Volume)):
        return value
    raise TypeError(
        "trade_amount must be openpit.param.Quantity or openpit.param.Volume"
    )


class Instrument:
    """
    Trading instrument definition.

    ``underlying_asset`` is the asset that is actually bought or sold.
    Order quantity, position size, and exposure are expressed in this asset.

    ``settlement_asset`` is the asset used for monetary settlement.
    P&L, fees, and cash flows are expressed in this asset.
    """

    def __init__(self, underlying_asset: Asset, settlement_asset: Asset) -> None:
        _require_instance(underlying_asset, Asset, name="underlying_asset")
        _require_instance(settlement_asset, Asset, name="settlement_asset")
        self._inner = _Instrument(
            underlying_asset=underlying_asset.value,
            settlement_asset=settlement_asset.value,
        )

    @property
    def underlying_asset(self) -> Asset:
        """Returns the asset that is bought or sold.

        This is the asset in which order quantity and resulting position are measured.
        """
        return Asset(self._inner.underlying_asset)

    @property
    def settlement_asset(self) -> Asset:
        """Returns the asset used for monetary settlement.

        This is the asset in which cash flow, fees, and P&L are measured.
        """
        return Asset(self._inner.settlement_asset)

    def __repr__(self) -> str:
        return repr(self._inner)


class OrderOperation(_OrderOperation):
    """Main operation parameters that describe side, instrument, price, and amount."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> OrderOperation:
        _ = args, kwargs
        return _OrderOperation.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        instrument: Instrument,
        side: Side,
        trade_amount: Quantity | Volume,
        price: Price | None = None,
    ) -> None:
        _require_instance(instrument, Instrument, name="instrument")
        _require_instance(side, Side, name="side")
        if not isinstance(trade_amount, (Quantity, Volume)):
            raise TypeError(
                f"trade_amount must be {Quantity.__module__}.{Quantity.__name__} "
                f"or {Volume.__module__}.{Volume.__name__}"
            )
        _require_instance(price, Price, name="price")
        _OrderOperation.underlying_asset.__set__(
            self, instrument.underlying_asset.value
        )
        _OrderOperation.settlement_asset.__set__(
            self, instrument.settlement_asset.value
        )
        _OrderOperation.side.__set__(self, side)
        _OrderOperation.trade_amount.__set__(self, trade_amount)
        _OrderOperation.price.__set__(self, price)
        self.__dict__["_py_instrument"] = instrument

    @property
    def instrument(self) -> Instrument:
        """Traded instrument."""
        return self.__dict__["_py_instrument"]

    # @typing.override
    @property
    def side(self) -> Side:
        """Order side."""
        return Side(_OrderOperation.side.__get__(self, type(self)))

    # @typing.override
    @property
    def trade_amount(self) -> Quantity | Volume:
        """Requested trade amount; context is determined by value type."""
        return _convert_trade_amount(
            _OrderOperation.trade_amount.__get__(self, type(self))
        )

    # @typing.override
    @property
    def price(self) -> Price | None:
        """Requested worst execution price used for size translation and
        price-sensitive checks.

        ``None`` means the order should execute at market price.
        """
        value = _OrderOperation.price.__get__(self, type(self))
        return None if value is None else Price(value)

    def __repr__(self) -> str:
        return _OrderOperation.__repr__(self)


class OrderPosition(_OrderPosition):
    """Position management parameters."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> OrderPosition:
        _ = args, kwargs
        return _OrderPosition.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        position_side: PositionSide | None = None,
        reduce_only: bool = False,
        close_position: bool = False,
    ) -> None:
        _require_instance(position_side, PositionSide, name="position_side")
        _OrderPosition.position_side.__set__(self, position_side)
        _OrderPosition.reduce_only.__set__(self, reduce_only)
        _OrderPosition.close_position.__set__(self, close_position)

    # @typing.override
    @property
    def position_side(self) -> PositionSide | None:
        """Hedge-mode leg targeted by the order.

        ``None`` uses one-way mode semantics.
        """
        value = _OrderPosition.position_side.__get__(self, type(self))
        return None if value is None else PositionSide(value)

    # @typing.override
    @property
    def reduce_only(self) -> bool:
        """Restricts the order to exposure-reducing execution only."""
        return _OrderPosition.reduce_only.__get__(self, type(self))

    # @typing.override
    @property
    def close_position(self) -> bool:
        """Marks intent to close the entire open position for the targeted
        leg/symbol.
        """
        return _OrderPosition.close_position.__get__(self, type(self))

    def __repr__(self) -> str:
        return _OrderPosition.__repr__(self)


class OrderMargin(_OrderMargin):
    """Margin configuration parameters."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> OrderMargin:
        _ = args, kwargs
        return _OrderMargin.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        leverage: Leverage | None = None,
        collateral_asset: Asset | None = None,
        auto_borrow: bool = False,
    ) -> None:
        _require_instance(leverage, Leverage, name="leverage")
        _require_instance(collateral_asset, Asset, name="collateral_asset")
        _OrderMargin.leverage.__set__(self, leverage)
        _OrderMargin.collateral_asset.__set__(
            self,
            None if collateral_asset is None else collateral_asset.value,
        )
        _OrderMargin.auto_borrow.__set__(self, auto_borrow)

    # @typing.override
    @property
    def leverage(self) -> Leverage | None:
        """Per-order leverage target used for margin requirement calculation.

        ``None`` means "use integration/account default leverage configuration".
        """
        return _OrderMargin.leverage.__get__(self, type(self))

    # @typing.override
    @property
    def collateral_asset(self) -> Asset | None:
        """Collateral currency intended to fund this specific order.

        ``None`` means "use default collateral asset selected by integration".
        """
        value = _OrderMargin.collateral_asset.__get__(self, type(self))
        return None if value is None else Asset(value)

    # @typing.override
    @property
    def auto_borrow(self) -> bool:
        """Whether temporary collateral shortage may be covered by auto-borrow.

        Defaults to ``False``.
        """
        return _OrderMargin.auto_borrow.__get__(self, type(self))

    def __repr__(self) -> str:
        return _OrderMargin.__repr__(self)


class Order(_Order):
    """
    Extensible order model accepted by ``openpit.Engine.start_pre_trade``.

    All groups are optional. The engine's built-in policies validate at
    runtime that the groups they need are present and produce a standard
    ``MissingRequiredField`` reject when they are not.
    """

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> Order:
        _ = args, kwargs
        return _Order.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        operation: OrderOperation | None = None,
        position: OrderPosition | None = None,
        margin: OrderMargin | None = None,
    ) -> None:
        _require_instance(operation, OrderOperation, name="operation")
        _require_instance(position, OrderPosition, name="position")
        _require_instance(margin, OrderMargin, name="margin")
        _Order.operation.__set__(self, operation)
        _Order.position.__set__(self, position)
        _Order.margin.__set__(self, margin)
        self.__dict__["_py_operation"] = operation
        self.__dict__["_py_position"] = position
        self.__dict__["_py_margin"] = margin

    # @typing.override
    @property
    def operation(self) -> OrderOperation | None:
        """Main operation parameters group."""
        return self.__dict__.get("_py_operation")

    # @typing.override
    @property
    def position(self) -> OrderPosition | None:
        """Position management parameters group."""
        return self.__dict__.get("_py_position")

    # @typing.override
    @property
    def margin(self) -> OrderMargin | None:
        """Margin configuration parameters group."""
        return self.__dict__.get("_py_margin")

    def __repr__(self) -> str:
        return _Order.__repr__(self)


class ExecutionReportOperation(_ExecutionReportOperation):
    """Main operation parameters reported by the execution."""

    # @typing.override
    def __new__(
        cls, *args: typing.Any, **kwargs: typing.Any
    ) -> ExecutionReportOperation:
        _ = args, kwargs
        return _ExecutionReportOperation.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        instrument: Instrument,
        side: Side,
    ) -> None:
        _require_instance(instrument, Instrument, name="instrument")
        _require_instance(side, Side, name="side")
        _ExecutionReportOperation.underlying_asset.__set__(
            self, instrument.underlying_asset.value
        )
        _ExecutionReportOperation.settlement_asset.__set__(
            self, instrument.settlement_asset.value
        )
        _ExecutionReportOperation.side.__set__(self, side)
        self.__dict__["_py_instrument"] = instrument

    @property
    def instrument(self) -> Instrument:
        """Traded instrument."""
        return self.__dict__["_py_instrument"]

    # @typing.override
    @property
    def side(self) -> Side:
        """Economic direction of the reported execution event."""
        return Side(_ExecutionReportOperation.side.__get__(self, type(self)))

    def __repr__(self) -> str:
        return _ExecutionReportOperation.__repr__(self)


class FinancialImpact(_FinancialImpact):
    """Financial impact parameters reported by the execution."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> FinancialImpact:
        _ = args, kwargs
        return _FinancialImpact.__new__(cls)

    # @typing.override
    def __init__(self, *, pnl: Pnl, fee: Fee) -> None:
        _require_instance(pnl, Pnl, name="pnl")
        _require_instance(fee, Fee, name="fee")
        _FinancialImpact.pnl.__set__(self, pnl)
        _FinancialImpact.fee.__set__(self, fee)

    # @typing.override
    @property
    def pnl(self) -> Pnl:
        """Realized trading result contributed by this report.

        Positive values for gains, negative values for losses.
        """
        return Pnl(_FinancialImpact.pnl.__get__(self, type(self)))

    # @typing.override
    @property
    def fee(self) -> Fee:
        """Fee or rebate associated with this report event.

        Negative values for fees, positive values for rebates.
        """
        return Fee(_FinancialImpact.fee.__get__(self, type(self)))

    def __repr__(self) -> str:
        return _FinancialImpact.__repr__(self)


class ExecutionReportFillDetails(_ExecutionReportFillDetails):
    """Trade reported by the execution."""

    # @typing.override
    def __new__(
        cls, *args: typing.Any, **kwargs: typing.Any
    ) -> ExecutionReportFillDetails:
        _ = args, kwargs
        return _ExecutionReportFillDetails.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        fill_price: Price | None = None,
        fill_quantity: Quantity | None = None,
        is_terminal: bool = False,
    ) -> None:
        _require_instance(fill_price, Price, name="fill_price")
        _require_instance(fill_quantity, Quantity, name="fill_quantity")
        _ExecutionReportFillDetails.fill_price.__set__(self, fill_price)
        _ExecutionReportFillDetails.fill_quantity.__set__(self, fill_quantity)
        _ExecutionReportFillDetails.is_terminal.__set__(self, is_terminal)

    # @typing.override
    @property
    def fill_price(self) -> Price | None:
        """Actual execution price."""
        value = _ExecutionReportFillDetails.fill_price.__get__(self, type(self))
        return None if value is None else Price(value)

    # @typing.override
    @property
    def fill_quantity(self) -> Quantity | None:
        """Executed size."""
        value = _ExecutionReportFillDetails.fill_quantity.__get__(self, type(self))
        return None if value is None else Quantity(value)

    # @typing.override
    @property
    def is_terminal(self) -> bool:
        """Whether this report closes the report stream for the order."""
        return _ExecutionReportFillDetails.is_terminal.__get__(self, type(self))

    def __repr__(self) -> str:
        return _ExecutionReportFillDetails.__repr__(self)


class ExecutionReportPositionImpact(_ExecutionReportPositionImpact):
    """Position impact parameters reported by the execution."""

    # @typing.override
    def __new__(
        cls, *args: typing.Any, **kwargs: typing.Any
    ) -> ExecutionReportPositionImpact:
        _ = args, kwargs
        return _ExecutionReportPositionImpact.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        position_effect: PositionEffect | None = None,
        position_side: PositionSide | None = None,
    ) -> None:
        _require_instance(position_effect, PositionEffect, name="position_effect")
        _require_instance(position_side, PositionSide, name="position_side")
        _ExecutionReportPositionImpact.position_effect.__set__(self, position_effect)
        _ExecutionReportPositionImpact.position_side.__set__(self, position_side)

    # @typing.override
    @property
    def position_effect(self) -> PositionEffect | None:
        """Whether this execution opened or closed exposure."""
        value = _ExecutionReportPositionImpact.position_effect.__get__(self, type(self))
        return None if value is None else PositionEffect(value)

    # @typing.override
    @property
    def position_side(self) -> PositionSide | None:
        """Hedge-mode leg affected by this execution, when provided."""
        value = _ExecutionReportPositionImpact.position_side.__get__(self, type(self))
        return None if value is None else PositionSide(value)

    def __repr__(self) -> str:
        return _ExecutionReportPositionImpact.__repr__(self)


class ExecutionReport(_ExecutionReport):
    """
    Extensible execution-report model consumed by post-trade policies.

    All groups are optional. Policies that need specific data validate
    group presence at runtime.
    """

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> ExecutionReport:
        _ = args, kwargs
        return _ExecutionReport.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        operation: ExecutionReportOperation | None = None,
        financial_impact: FinancialImpact | None = None,
        fill: ExecutionReportFillDetails | None = None,
        position_impact: ExecutionReportPositionImpact | None = None,
    ) -> None:
        _require_instance(operation, ExecutionReportOperation, name="operation")
        _require_instance(financial_impact, FinancialImpact, name="financial_impact")
        _require_instance(fill, ExecutionReportFillDetails, name="fill")
        _require_instance(
            position_impact, ExecutionReportPositionImpact, name="position_impact"
        )
        _ExecutionReport.operation.__set__(self, operation)
        _ExecutionReport.financial_impact.__set__(self, financial_impact)
        _ExecutionReport.fill.__set__(self, fill)
        _ExecutionReport.position_impact.__set__(self, position_impact)
        self.__dict__["_py_operation"] = operation
        self.__dict__["_py_financial_impact"] = financial_impact
        self.__dict__["_py_fill"] = fill
        self.__dict__["_py_position_impact"] = position_impact

    # @typing.override
    @property
    def operation(self) -> ExecutionReportOperation | None:
        """Main operation parameters group."""
        return self.__dict__.get("_py_operation")

    # @typing.override
    @property
    def financial_impact(self) -> FinancialImpact | None:
        """Financial impact parameters group."""
        return self.__dict__.get("_py_financial_impact")

    # @typing.override
    @property
    def fill(self) -> ExecutionReportFillDetails | None:
        """Fill details group."""
        return self.__dict__.get("_py_fill")

    # @typing.override
    @property
    def position_impact(self) -> ExecutionReportPositionImpact | None:
        """Position impact data group."""
        return self.__dict__.get("_py_position_impact")

    def __repr__(self) -> str:
        return _ExecutionReport.__repr__(self)


__all__ = [
    "ExecutionReport",
    "ExecutionReportFillDetails",
    "ExecutionReportOperation",
    "ExecutionReportPositionImpact",
    "FinancialImpact",
    "Instrument",
    "Order",
    "OrderMargin",
    "OrderOperation",
    "OrderPosition",
]
