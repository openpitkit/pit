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

import collections.abc

from .account_adjustment import (
    AccountAdjustment,
    AccountAdjustmentAmount,
    AccountAdjustmentBalanceOperation,
    AccountAdjustmentBounds,
    AccountAdjustmentPositionOperation,
)
from .param import (
    AccountId,
    Asset,
    Fee,
    Leverage,
    Pnl,
    PositionEffect,
    PositionSide,
    Price,
    Quantity,
    Side,
    Trade,
    TradeAmount,
)
from .pretrade import PreTradeLock

class AccountAdjustmentContext:
    """Context of the current account-adjustment operation."""

class Mutation:
    """Commit/rollback action pair registered by a policy."""

    def __init__(
        self,
        commit: collections.abc.Callable[[], None],
        rollback: collections.abc.Callable[[], None],
    ) -> None: ...

class Instrument:
    """Trading instrument definition."""

    def __init__(
        self,
        underlying_asset: Asset,
        settlement_asset: Asset,
    ) -> None: ...
    @property
    def underlying_asset(self) -> Asset:
        """Returns the asset that is bought or sold."""

    @property
    def settlement_asset(self) -> Asset:
        """Returns the asset used for monetary settlement."""

class OrderOperation:
    """Main operation parameters that describe side, instrument, price, and amount."""

    def __init__(
        self,
        *,
        instrument: Instrument,
        side: Side,
        trade_amount: TradeAmount,
        account_id: AccountId,
        price: Price | None = None,
    ) -> None: ...
    @property
    def instrument(self) -> Instrument:
        """Traded instrument."""

    @property
    def account_id(self) -> AccountId:
        """Account identifier associated with the order."""

    @property
    def side(self) -> Side:
        """Order side."""

    @property
    def trade_amount(self) -> TradeAmount:
        """Requested trade amount; context is determined by value type."""

    @property
    def price(self) -> Price | None:
        """Requested worst execution price. ``None`` means market price."""

class OrderPosition:
    """Position management parameters."""

    def __init__(
        self,
        *,
        position_side: PositionSide | None = None,
        reduce_only: bool = False,
        close_position: bool = False,
    ) -> None: ...
    @property
    def position_side(self) -> PositionSide | None:
        """Hedge-mode leg targeted by the order."""

    @property
    def reduce_only(self) -> bool:
        """Restricts the order to exposure-reducing execution only."""

    @property
    def close_position(self) -> bool:
        """Marks intent to close the entire open position."""

class OrderMargin:
    """Margin configuration parameters."""

    def __init__(
        self,
        *,
        leverage: Leverage | int | float | None = None,
        collateral_asset: Asset | None = None,
        auto_borrow: bool = False,
    ) -> None: ...
    @property
    def leverage(self) -> Leverage | None:
        """Per-order leverage target."""

    @property
    def collateral_asset(self) -> Asset | None:
        """Collateral currency for this order."""

    @property
    def auto_borrow(self) -> bool:
        """Whether auto-borrow may cover a temporary collateral shortage."""

class Order:
    """Extensible Python order model accepted by ``openpit.Engine.start_pre_trade``."""

    def __init__(
        self,
        *,
        operation: OrderOperation | None = None,
        position: OrderPosition | None = None,
        margin: OrderMargin | None = None,
    ) -> None: ...
    @property
    def operation(self) -> OrderOperation | None:
        """Main operation parameters group."""

    @property
    def position(self) -> OrderPosition | None:
        """Position management parameters group."""

    @property
    def margin(self) -> OrderMargin | None:
        """Margin configuration parameters group."""

class ExecutionReportOperation:
    """Main operation parameters reported by the execution."""

    def __init__(
        self,
        *,
        instrument: Instrument,
        side: Side,
        account_id: AccountId,
    ) -> None: ...
    @property
    def instrument(self) -> Instrument:
        """Traded instrument."""

    @property
    def account_id(self) -> AccountId:
        """Account identifier associated with the execution report."""

    @property
    def side(self) -> Side:
        """Economic direction of the reported execution event."""

class FinancialImpact:
    """Financial impact parameters reported by the execution."""

    def __init__(self, *, pnl: Pnl, fee: Fee) -> None: ...
    @property
    def pnl(self) -> Pnl:
        """Realized trading result contributed by this report."""

    @property
    def fee(self) -> Fee:
        """Fee or rebate associated with this report event."""

class ExecutionReportFillDetails:
    """Trade reported by the execution."""

    def __init__(
        self,
        *,
        last_trade: Trade | None = None,
        leaves_quantity: Quantity | None = None,
        lock: PreTradeLock,
        is_final: bool | None = None,
    ) -> None: ...
    @property
    def last_trade(self) -> Trade | None:
        """Actual execution trade."""

    @property
    def leaves_quantity(self) -> Quantity | None:
        """Remaining order quantity after this fill."""

    @property
    def lock(self) -> PreTradeLock:
        """Order lock payload."""

    @property
    def is_final(self) -> bool | None:
        """Whether this report closes the order's report stream.

        The order is filled, cancelled, or rejected.
        """

class ExecutionReportPositionImpact:
    """Position impact parameters reported by the execution."""

    def __init__(
        self,
        *,
        position_effect: PositionEffect | None = None,
        position_side: PositionSide | None = None,
    ) -> None: ...
    @property
    def position_effect(self) -> PositionEffect | None:
        """Whether this execution opened or closed exposure."""

    @property
    def position_side(self) -> PositionSide | None:
        """Hedge-mode leg affected by this execution."""

class ExecutionReport:
    """Extensible Python execution-report model consumed by post-trade callbacks."""

    def __init__(
        self,
        *,
        operation: ExecutionReportOperation | None = None,
        financial_impact: FinancialImpact | None = None,
        fill: ExecutionReportFillDetails | None = None,
        position_impact: ExecutionReportPositionImpact | None = None,
    ) -> None: ...
    @property
    def operation(self) -> ExecutionReportOperation | None:
        """Main operation parameters group."""

    @property
    def financial_impact(self) -> FinancialImpact | None:
        """Financial impact parameters group."""

    @property
    def fill(self) -> ExecutionReportFillDetails | None:
        """Fill details group."""

    @property
    def position_impact(self) -> ExecutionReportPositionImpact | None:
        """Position impact data group."""

__all__ = [
    "AccountAdjustment",
    "AccountAdjustmentAmount",
    "AccountAdjustmentBalanceOperation",
    "AccountAdjustmentBounds",
    "AccountAdjustmentPositionOperation",
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
