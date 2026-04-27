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

import abc
import typing

from ._openpit import AccountAdjustment as _AccountAdjustment
from ._openpit import AccountAdjustmentAmount as _AccountAdjustmentAmount
from ._openpit import (
    AccountAdjustmentBalanceOperation as _AccountAdjustmentBalanceOperation,
)
from ._openpit import AccountAdjustmentBounds as _AccountAdjustmentBounds
from ._openpit import (
    AccountAdjustmentPositionOperation as _AccountAdjustmentPositionOperation,
)
from .core import Instrument
from .param import (
    AdjustmentAmount,
    Asset,
    Leverage,
    PositionMode,
    PositionSize,
    Price,
)

if typing.TYPE_CHECKING:
    from . import AccountAdjustmentContext
    from .core import Mutation
    from .param import AccountId
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
        PolicyDecision | typing.Iterable[PolicyReject] | tuple[Mutation, ...] | None
    ): ...


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


class AccountAdjustmentAmount(_AccountAdjustmentAmount):
    """Grouped total/reserved/pending adjustment payload."""

    # @typing.override
    def __new__(
        cls, *args: typing.Any, **kwargs: typing.Any
    ) -> AccountAdjustmentAmount:
        _ = args, kwargs
        return _AccountAdjustmentAmount.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        total: AdjustmentAmount | None = None,
        reserved: AdjustmentAmount | None = None,
        pending: AdjustmentAmount | None = None,
    ) -> None:
        _AccountAdjustmentAmount.total.__set__(self, total)
        _AccountAdjustmentAmount.reserved.__set__(self, reserved)
        _AccountAdjustmentAmount.pending.__set__(self, pending)

    # @typing.override
    @property
    def total(self) -> AdjustmentAmount | None:
        """Actual resulting balance/position value."""
        value = _AccountAdjustmentAmount.total.__get__(self, type(self))
        if value is None:
            return None
        if value.is_delta:
            return AdjustmentAmount.delta(value.as_delta)
        return AdjustmentAmount.absolute(value.as_absolute)

    # @typing.override
    @property
    def reserved(self) -> AdjustmentAmount | None:
        """Amount earmarked for outgoing settlement.

        Unavailable for immediate use.
        """
        value = _AccountAdjustmentAmount.reserved.__get__(self, type(self))
        if value is None:
            return None
        if value.is_delta:
            return AdjustmentAmount.delta(value.as_delta)
        return AdjustmentAmount.absolute(value.as_absolute)

    # @typing.override
    @property
    def pending(self) -> AdjustmentAmount | None:
        """Amount in-flight for incoming acquisition and not yet finalized."""
        value = _AccountAdjustmentAmount.pending.__get__(self, type(self))
        if value is None:
            return None
        if value.is_delta:
            return AdjustmentAmount.delta(value.as_delta)
        return AdjustmentAmount.absolute(value.as_absolute)

    def __repr__(self) -> str:
        return _AccountAdjustmentAmount.__repr__(self)


class AccountAdjustmentBalanceOperation(_AccountAdjustmentBalanceOperation):
    """Direct physical balance adjustment."""

    # @typing.override
    def __new__(
        cls, *args: typing.Any, **kwargs: typing.Any
    ) -> AccountAdjustmentBalanceOperation:
        _ = args, kwargs
        return _AccountAdjustmentBalanceOperation.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        asset: Asset,
        average_entry_price: Price | None = None,
    ) -> None:
        _AccountAdjustmentBalanceOperation.asset.__set__(self, asset)
        _AccountAdjustmentBalanceOperation.average_entry_price.__set__(
            self, average_entry_price
        )

    # @typing.override
    @property
    def asset(self) -> Asset:
        """Adjusted balance asset."""
        return _AccountAdjustmentBalanceOperation.asset.__get__(self, type(self))

    # @typing.override
    @property
    def average_entry_price(self) -> Price | None:
        """Optional cost basis for the adjusted physical balance."""
        value = _AccountAdjustmentBalanceOperation.average_entry_price.__get__(
            self, type(self)
        )
        return value

    def __repr__(self) -> str:
        return _AccountAdjustmentBalanceOperation.__repr__(self)


class AccountAdjustmentPositionOperation(_AccountAdjustmentPositionOperation):
    """Direct derivatives-like position adjustment."""

    # @typing.override
    def __new__(
        cls, *args: typing.Any, **kwargs: typing.Any
    ) -> AccountAdjustmentPositionOperation:
        _ = args, kwargs
        return _AccountAdjustmentPositionOperation.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        instrument: Instrument,
        collateral_asset: Asset,
        average_entry_price: Price,
        mode: PositionMode,
        leverage: Leverage | int | float | None = None,
    ) -> None:
        # Structural aggregate check is intentionally kept in Python so
        # aggregate misuse fails with a clear contract error instead of
        # indirect attribute/field errors during aggregation.
        _require_instance(instrument, Instrument, name="instrument")
        _AccountAdjustmentPositionOperation.underlying_asset.__set__(
            self, instrument.underlying_asset
        )
        _AccountAdjustmentPositionOperation.settlement_asset.__set__(
            self, instrument.settlement_asset
        )
        _AccountAdjustmentPositionOperation.collateral_asset.__set__(
            self, collateral_asset
        )
        _AccountAdjustmentPositionOperation.average_entry_price.__set__(
            self, average_entry_price
        )
        _AccountAdjustmentPositionOperation.mode.__set__(self, mode)
        _AccountAdjustmentPositionOperation.leverage.__set__(self, leverage)
        self.__dict__["_py_instrument"] = instrument

    @property
    def instrument(self) -> Instrument:
        """Adjusted position instrument."""
        return self.__dict__["_py_instrument"]

    # @typing.override
    @property
    def collateral_asset(self) -> Asset:
        """Collateral asset used by the adjusted position."""
        return _AccountAdjustmentPositionOperation.collateral_asset.__get__(
            self, type(self)
        )

    # @typing.override
    @property
    def average_entry_price(self) -> Price:
        """Average entry price for the adjusted position state."""
        return _AccountAdjustmentPositionOperation.average_entry_price.__get__(
            self, type(self)
        )

    # @typing.override
    @property
    def mode(self) -> PositionMode:
        """Netting vs hedged position representation."""
        return _AccountAdjustmentPositionOperation.mode.__get__(self, type(self))

    # @typing.override
    @property
    def leverage(self) -> Leverage | None:
        """Optional leverage snapshot/setting carried with the position adjustment."""
        return _AccountAdjustmentPositionOperation.leverage.__get__(self, type(self))

    def __repr__(self) -> str:
        return _AccountAdjustmentPositionOperation.__repr__(self)


class AccountAdjustmentBounds(_AccountAdjustmentBounds):
    """Optional post-adjustment inclusive limits."""

    # @typing.override
    def __new__(
        cls, *args: typing.Any, **kwargs: typing.Any
    ) -> AccountAdjustmentBounds:
        _ = args, kwargs
        return _AccountAdjustmentBounds.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        total_upper: PositionSize | None = None,
        total_lower: PositionSize | None = None,
        reserved_upper: PositionSize | None = None,
        reserved_lower: PositionSize | None = None,
        pending_upper: PositionSize | None = None,
        pending_lower: PositionSize | None = None,
    ) -> None:
        _AccountAdjustmentBounds.total_upper.__set__(self, total_upper)
        _AccountAdjustmentBounds.total_lower.__set__(self, total_lower)
        _AccountAdjustmentBounds.reserved_upper.__set__(self, reserved_upper)
        _AccountAdjustmentBounds.reserved_lower.__set__(self, reserved_lower)
        _AccountAdjustmentBounds.pending_upper.__set__(self, pending_upper)
        _AccountAdjustmentBounds.pending_lower.__set__(self, pending_lower)

    @property
    def total_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for total."""
        return _AccountAdjustmentBounds.total_upper.__get__(self, type(self))

    @property
    def total_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for total."""
        return _AccountAdjustmentBounds.total_lower.__get__(self, type(self))

    @property
    def reserved_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for reserved."""
        return _AccountAdjustmentBounds.reserved_upper.__get__(self, type(self))

    @property
    def reserved_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for reserved."""
        return _AccountAdjustmentBounds.reserved_lower.__get__(self, type(self))

    @property
    def pending_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for pending."""
        return _AccountAdjustmentBounds.pending_upper.__get__(self, type(self))

    @property
    def pending_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for pending."""
        return _AccountAdjustmentBounds.pending_lower.__get__(self, type(self))

    def __repr__(self) -> str:
        return _AccountAdjustmentBounds.__repr__(self)


class AccountAdjustment(_AccountAdjustment):
    """Extensible non-trading account-adjustment model.

    Snapshot semantics:
    On ``Engine.apply_account_adjustment(...)``, each adjustment item is
    snapshotted at submission time for policy evaluation. Mutations of
    adjustment objects after submission do not affect the in-flight batch.
    """

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> AccountAdjustment:
        _ = args, kwargs
        return _AccountAdjustment.__new__(cls)

    # @typing.override
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
        # Structural aggregate check is intentionally kept in Python because
        # operation is a Python-only union wrapper at the public boundary.
        if operation is not None and not isinstance(
            operation,
            (AccountAdjustmentBalanceOperation, AccountAdjustmentPositionOperation),
        ):
            raise TypeError(
                "operation must be "
                "openpit.account_adjustment.AccountAdjustmentBalanceOperation or "
                "openpit.account_adjustment.AccountAdjustmentPositionOperation"
            )
        # Structural checks for aggregate groups stay at Python boundary to keep
        # explicit API-contract errors for wrong wrapper types.
        _require_instance(amount, AccountAdjustmentAmount, name="amount")
        _require_instance(bounds, AccountAdjustmentBounds, name="bounds")
        _AccountAdjustment.operation.__set__(self, operation)
        _AccountAdjustment.amount.__set__(self, amount)
        _AccountAdjustment.bounds.__set__(self, bounds)
        self.__dict__["_py_operation"] = operation
        self.__dict__["_py_amount"] = amount
        self.__dict__["_py_bounds"] = bounds

    @property
    def operation(
        self,
    ) -> AccountAdjustmentBalanceOperation | AccountAdjustmentPositionOperation | None:
        """Adjustment operation details group."""
        return self.__dict__.get("_py_operation")

    @property
    def amount(self) -> AccountAdjustmentAmount | None:
        """Adjustment amount deltas group."""
        return self.__dict__.get("_py_amount")

    @property
    def bounds(self) -> AccountAdjustmentBounds | None:
        """Optional post-adjustment bounds group."""
        return self.__dict__.get("_py_bounds")

    def __repr__(self) -> str:
        return _AccountAdjustment.__repr__(self)


__all__ = [
    "AccountAdjustment",
    "AccountAdjustmentAmount",
    "AccountAdjustmentBalanceOperation",
    "AccountAdjustmentBounds",
    "AccountAdjustmentPolicy",
    "AccountAdjustmentPositionOperation",
]
