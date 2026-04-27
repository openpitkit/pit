from __future__ import annotations

import abc
import collections.abc

from . import AccountAdjustmentContext
from .core import Instrument, Mutation
from .param import (
    AccountId,
    AdjustmentAmount,
    Asset,
    Leverage,
    PositionMode,
    PositionSize,
    Price,
)
from .pretrade.policy import PolicyDecision, PolicyReject

class AccountAdjustmentPolicy(abc.ABC):
    @property
    @abc.abstractmethod
    def name(self) -> str: ...
    @abc.abstractmethod
    def apply_account_adjustment(
        self,
        ctx: AccountAdjustmentContext,
        account_id: AccountId,
        adjustment: AccountAdjustment,
    ) -> (
        PolicyDecision
        | collections.abc.Iterable[PolicyReject]
        | tuple[Mutation, ...]
        | None
    ): ...

class AccountAdjustmentAmount:
    """Grouped total/reserved/pending adjustment payload."""

    def __init__(
        self,
        *,
        total: AdjustmentAmount | None = None,
        reserved: AdjustmentAmount | None = None,
        pending: AdjustmentAmount | None = None,
    ) -> None: ...
    @property
    def total(self) -> AdjustmentAmount | None:
        """Actual resulting balance/position value."""

    @property
    def reserved(self) -> AdjustmentAmount | None:
        """Amount earmarked for outgoing settlement.

        Unavailable for immediate use.
        """

    @property
    def pending(self) -> AdjustmentAmount | None:
        """Amount in-flight for incoming acquisition and not yet finalized."""

class AccountAdjustmentBalanceOperation:
    """Direct physical balance adjustment."""

    def __init__(
        self,
        *,
        asset: Asset,
        average_entry_price: Price | None = None,
    ) -> None: ...
    @property
    def asset(self) -> Asset: ...
    @property
    def average_entry_price(self) -> Price | None:
        """Optional cost basis for the adjusted physical balance."""

class AccountAdjustmentPositionOperation:
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
        """Average entry price for the adjusted position state."""

    @property
    def mode(self) -> PositionMode:
        """Netting vs hedged position representation."""

    @property
    def leverage(self) -> Leverage | None:
        """Optional leverage snapshot/setting carried with the position adjustment."""

class AccountAdjustmentBounds:
    """Optional post-adjustment inclusive limits."""

    def __init__(
        self,
        *,
        total_upper: PositionSize | None = None,
        total_lower: PositionSize | None = None,
        reserved_upper: PositionSize | None = None,
        reserved_lower: PositionSize | None = None,
        pending_upper: PositionSize | None = None,
        pending_lower: PositionSize | None = None,
    ) -> None: ...
    @property
    def total_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for total."""

    @property
    def total_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for total."""

    @property
    def reserved_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for reserved."""

    @property
    def reserved_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for reserved."""

    @property
    def pending_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for pending."""

    @property
    def pending_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for pending."""

class AccountAdjustment:
    """Extensible non-trading account-adjustment model."""

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
    ) -> None: ...
    @property
    def operation(
        self,
    ) -> (
        AccountAdjustmentBalanceOperation | AccountAdjustmentPositionOperation | None
    ): ...
    @property
    def amount(self) -> AccountAdjustmentAmount | None: ...
    @property
    def bounds(self) -> AccountAdjustmentBounds | None: ...

__all__ = [
    "AccountAdjustment",
    "AccountAdjustmentAmount",
    "AccountAdjustmentBalanceOperation",
    "AccountAdjustmentBounds",
    "AccountAdjustmentPolicy",
    "AccountAdjustmentPositionOperation",
]
