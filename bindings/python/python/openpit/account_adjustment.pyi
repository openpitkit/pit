from __future__ import annotations

from .core import Instrument
from .param import (
    AdjustmentAmount,
    Asset,
    Leverage,
    Pnl,
    PositionMode,
    PositionSize,
    Price,
)

class Amount:
    """Grouped balance/held/incoming adjustment payload."""

    def __init__(
        self,
        *,
        balance: AdjustmentAmount | None = None,
        held: AdjustmentAmount | None = None,
        incoming: AdjustmentAmount | None = None,
    ) -> None: ...
    @property
    def balance(self) -> AdjustmentAmount | None:
        """Actual resulting balance/position value."""

    @property
    def held(self) -> AdjustmentAmount | None:
        """Amount earmarked for outgoing settlement.

        Unavailable for immediate use.
        """

    @property
    def incoming(self) -> AdjustmentAmount | None:
        """Amount in-flight for incoming acquisition and not yet finalized."""

class BalanceOperation:
    """Direct physical balance adjustment."""

    def __init__(
        self,
        *,
        asset: Asset,
        average_entry_price: Price | None = None,
        realized_pnl: Pnl | None = None,
    ) -> None: ...
    @property
    def asset(self) -> Asset: ...
    @property
    def average_entry_price(self) -> Price | None:
        """Optional account-currency cost basis force-set.

        The value is caller supplied; no FX is applied.
        """

    @property
    def realized_pnl(self) -> Pnl | None:
        """Optional force-set of absolute realized PnL in account currency.

        The value is caller supplied; no FX is applied.
        """

class PositionOperation:
    """Direct derivatives-like position adjustment."""

    def __init__(
        self,
        *,
        instrument: Instrument,
        collateral_asset: Asset,
        average_entry_price: Price,
        mode: PositionMode,
        leverage: Leverage | int | float | None = None,
    ) -> None: ...
    @property
    def instrument(self) -> Instrument: ...
    @property
    def collateral_asset(self) -> Asset: ...
    @property
    def average_entry_price(self) -> Price:
        """Account-currency average entry price for the adjusted position state.

        The value is caller supplied; no FX is applied.
        """

    @property
    def mode(self) -> PositionMode:
        """Netting vs hedged position representation."""

    @property
    def leverage(self) -> Leverage | None:
        """Optional leverage snapshot/setting carried with the position adjustment."""

class Bounds:
    """Optional post-adjustment inclusive limits."""

    def __init__(
        self,
        *,
        balance_upper: PositionSize | None = None,
        balance_lower: PositionSize | None = None,
        held_upper: PositionSize | None = None,
        held_lower: PositionSize | None = None,
        incoming_upper: PositionSize | None = None,
        incoming_lower: PositionSize | None = None,
    ) -> None: ...
    @property
    def balance_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for balance."""

    @property
    def balance_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for balance."""

    @property
    def held_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for held."""

    @property
    def held_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for held."""

    @property
    def incoming_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for incoming."""

    @property
    def incoming_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for incoming."""

class Adjustment:
    """Extensible non-trading account-adjustment model."""

    def __init__(
        self,
        *,
        operation: BalanceOperation | PositionOperation | None = None,
        amount: Amount | None = None,
        bounds: Bounds | None = None,
    ) -> None: ...
    @property
    def operation(
        self,
    ) -> BalanceOperation | PositionOperation | None: ...
    @property
    def amount(self) -> Amount | None: ...
    @property
    def bounds(self) -> Bounds | None: ...

__all__ = [
    "Adjustment",
    "Amount",
    "BalanceOperation",
    "Bounds",
    "PositionOperation",
]
