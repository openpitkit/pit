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


class Amount(_AccountAdjustmentAmount):
    """Grouped balance/held/incoming adjustment payload."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> Amount:
        _ = args, kwargs
        return _AccountAdjustmentAmount.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        balance: AdjustmentAmount | None = None,
        held: AdjustmentAmount | None = None,
        incoming: AdjustmentAmount | None = None,
    ) -> None:
        _AccountAdjustmentAmount.balance.__set__(self, balance)
        _AccountAdjustmentAmount.held.__set__(self, held)
        _AccountAdjustmentAmount.incoming.__set__(self, incoming)

    # @typing.override
    @property
    def balance(self) -> AdjustmentAmount | None:
        """Actual resulting balance/position value."""
        value = _AccountAdjustmentAmount.balance.__get__(self, type(self))
        if value is None:
            return None
        if value.is_delta:
            return AdjustmentAmount.delta(value.as_delta)
        return AdjustmentAmount.absolute(value.as_absolute)

    # @typing.override
    @property
    def held(self) -> AdjustmentAmount | None:
        """Amount earmarked for outgoing settlement.

        Unavailable for immediate use.
        """
        value = _AccountAdjustmentAmount.held.__get__(self, type(self))
        if value is None:
            return None
        if value.is_delta:
            return AdjustmentAmount.delta(value.as_delta)
        return AdjustmentAmount.absolute(value.as_absolute)

    # @typing.override
    @property
    def incoming(self) -> AdjustmentAmount | None:
        """Amount in-flight for incoming acquisition and not yet finalized."""
        value = _AccountAdjustmentAmount.incoming.__get__(self, type(self))
        if value is None:
            return None
        if value.is_delta:
            return AdjustmentAmount.delta(value.as_delta)
        return AdjustmentAmount.absolute(value.as_absolute)

    def __repr__(self) -> str:
        return _AccountAdjustmentAmount.__repr__(self)


class BalanceOperation(_AccountAdjustmentBalanceOperation):
    """Direct physical balance adjustment."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> BalanceOperation:
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


class PositionOperation(_AccountAdjustmentPositionOperation):
    """Direct derivatives-like position adjustment."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> PositionOperation:
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


class Bounds(_AccountAdjustmentBounds):
    """Optional post-adjustment inclusive limits."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> Bounds:
        _ = args, kwargs
        return _AccountAdjustmentBounds.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        balance_upper: PositionSize | None = None,
        balance_lower: PositionSize | None = None,
        held_upper: PositionSize | None = None,
        held_lower: PositionSize | None = None,
        incoming_upper: PositionSize | None = None,
        incoming_lower: PositionSize | None = None,
    ) -> None:
        _AccountAdjustmentBounds.balance_upper.__set__(self, balance_upper)
        _AccountAdjustmentBounds.balance_lower.__set__(self, balance_lower)
        _AccountAdjustmentBounds.held_upper.__set__(self, held_upper)
        _AccountAdjustmentBounds.held_lower.__set__(self, held_lower)
        _AccountAdjustmentBounds.incoming_upper.__set__(self, incoming_upper)
        _AccountAdjustmentBounds.incoming_lower.__set__(self, incoming_lower)

    @property
    def balance_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for balance."""
        return _AccountAdjustmentBounds.balance_upper.__get__(self, type(self))

    @property
    def balance_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for balance."""
        return _AccountAdjustmentBounds.balance_lower.__get__(self, type(self))

    @property
    def held_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for held."""
        return _AccountAdjustmentBounds.held_upper.__get__(self, type(self))

    @property
    def held_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for held."""
        return _AccountAdjustmentBounds.held_lower.__get__(self, type(self))

    @property
    def incoming_upper(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for incoming."""
        return _AccountAdjustmentBounds.incoming_upper.__get__(self, type(self))

    @property
    def incoming_lower(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for incoming."""
        return _AccountAdjustmentBounds.incoming_lower.__get__(self, type(self))

    def __repr__(self) -> str:
        return _AccountAdjustmentBounds.__repr__(self)


class Adjustment(_AccountAdjustment):
    """Extensible non-trading account-adjustment model.

    Snapshot semantics:
    On ``Engine.apply_account_adjustment(...)``, each adjustment item is
    snapshotted at submission time for policy evaluation. Mutations of
    adjustment objects after submission do not affect the in-flight batch.
    """

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> Adjustment:
        _ = args, kwargs
        return _AccountAdjustment.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        operation: BalanceOperation | PositionOperation | None = None,
        amount: Amount | None = None,
        bounds: Bounds | None = None,
    ) -> None:
        # Structural aggregate check is intentionally kept in Python because
        # operation is a Python-only union wrapper at the public boundary.
        if operation is not None and not isinstance(
            operation,
            (BalanceOperation, PositionOperation),
        ):
            raise TypeError(
                "operation must be "
                "openpit.account_adjustment.BalanceOperation or "
                "openpit.account_adjustment.PositionOperation"
            )
        # Structural checks for aggregate groups stay at Python boundary to keep
        # explicit API-contract errors for wrong wrapper types.
        _require_instance(amount, Amount, name="amount")
        _require_instance(bounds, Bounds, name="bounds")
        _AccountAdjustment.operation.__set__(self, operation)
        _AccountAdjustment.amount.__set__(self, amount)
        _AccountAdjustment.bounds.__set__(self, bounds)
        self.__dict__["_py_operation"] = operation
        self.__dict__["_py_amount"] = amount
        self.__dict__["_py_bounds"] = bounds

    @property
    def operation(
        self,
    ) -> BalanceOperation | PositionOperation | None:
        """Adjustment operation details group."""
        return self.__dict__.get("_py_operation")

    @property
    def amount(self) -> Amount | None:
        """Adjustment amount deltas group."""
        return self.__dict__.get("_py_amount")

    @property
    def bounds(self) -> Bounds | None:
        """Optional post-adjustment bounds group."""
        return self.__dict__.get("_py_bounds")

    def __repr__(self) -> str:
        return _AccountAdjustment.__repr__(self)


__all__ = [
    "Adjustment",
    "Amount",
    "BalanceOperation",
    "Bounds",
    "PositionOperation",
]
