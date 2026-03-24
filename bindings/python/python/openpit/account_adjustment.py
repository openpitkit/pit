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
        _require_instance(total, AdjustmentAmount, name="total")
        _require_instance(reserved, AdjustmentAmount, name="reserved")
        _require_instance(pending, AdjustmentAmount, name="pending")
        _AccountAdjustmentAmount.total.__set__(self, total)
        _AccountAdjustmentAmount.reserved.__set__(self, reserved)
        _AccountAdjustmentAmount.pending.__set__(self, pending)

    # @typing.override
    @property
    def total(self) -> AdjustmentAmount | None:
        """Actual resulting balance/position value."""
        return _AccountAdjustmentAmount.total.__get__(self, type(self))

    # @typing.override
    @property
    def reserved(self) -> AdjustmentAmount | None:
        """Amount earmarked for outgoing settlement.

        Unavailable for immediate use.
        """
        return _AccountAdjustmentAmount.reserved.__get__(self, type(self))

    # @typing.override
    @property
    def pending(self) -> AdjustmentAmount | None:
        """Amount in-flight for incoming acquisition and not yet finalized."""
        return _AccountAdjustmentAmount.pending.__get__(self, type(self))

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
        _require_instance(asset, Asset, name="asset")
        _require_instance(average_entry_price, Price, name="average_entry_price")
        _AccountAdjustmentBalanceOperation.asset.__set__(self, asset.value)
        _AccountAdjustmentBalanceOperation.average_entry_price.__set__(
            self, average_entry_price
        )

    # @typing.override
    @property
    def asset(self) -> Asset:
        return Asset(_AccountAdjustmentBalanceOperation.asset.__get__(self, type(self)))

    # @typing.override
    @property
    def average_entry_price(self) -> Price | None:
        """Optional cost basis for the adjusted physical balance."""
        value = _AccountAdjustmentBalanceOperation.average_entry_price.__get__(
            self, type(self)
        )
        return None if value is None else Price(value)

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
        leverage: Leverage | None = None,
    ) -> None:
        _require_instance(instrument, Instrument, name="instrument")
        _require_instance(collateral_asset, Asset, name="collateral_asset")
        _require_instance(average_entry_price, Price, name="average_entry_price")
        _require_instance(mode, PositionMode, name="mode")
        _require_instance(leverage, Leverage, name="leverage")
        _AccountAdjustmentPositionOperation.underlying_asset.__set__(
            self, instrument.underlying_asset.value
        )
        _AccountAdjustmentPositionOperation.settlement_asset.__set__(
            self, instrument.settlement_asset.value
        )
        _AccountAdjustmentPositionOperation.collateral_asset.__set__(
            self, collateral_asset.value
        )
        _AccountAdjustmentPositionOperation.average_entry_price.__set__(
            self, average_entry_price
        )
        _AccountAdjustmentPositionOperation.mode.__set__(self, mode)
        _AccountAdjustmentPositionOperation.leverage.__set__(self, leverage)
        self.__dict__["_py_instrument"] = instrument

    @property
    def instrument(self) -> Instrument:
        return self.__dict__["_py_instrument"]

    # @typing.override
    @property
    def collateral_asset(self) -> Asset:
        value = _AccountAdjustmentPositionOperation.collateral_asset.__get__(
            self, type(self)
        )
        return Asset(value)

    # @typing.override
    @property
    def average_entry_price(self) -> Price:
        """Average entry price for the adjusted position state."""
        return Price(
            _AccountAdjustmentPositionOperation.average_entry_price.__get__(
                self, type(self)
            )
        )

    # @typing.override
    @property
    def mode(self) -> PositionMode:
        """Netting vs hedged position representation."""
        return PositionMode(
            _AccountAdjustmentPositionOperation.mode.__get__(self, type(self))
        )

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
        total_upper_bound: PositionSize | None = None,
        total_lower_bound: PositionSize | None = None,
        reserved_upper_bound: PositionSize | None = None,
        reserved_lower_bound: PositionSize | None = None,
        pending_upper_bound: PositionSize | None = None,
        pending_lower_bound: PositionSize | None = None,
    ) -> None:
        _require_instance(total_upper_bound, PositionSize, name="total_upper_bound")
        _require_instance(total_lower_bound, PositionSize, name="total_lower_bound")
        _require_instance(
            reserved_upper_bound, PositionSize, name="reserved_upper_bound"
        )
        _require_instance(
            reserved_lower_bound, PositionSize, name="reserved_lower_bound"
        )
        _require_instance(pending_upper_bound, PositionSize, name="pending_upper_bound")
        _require_instance(pending_lower_bound, PositionSize, name="pending_lower_bound")
        _AccountAdjustmentBounds.total_upper_bound.__set__(self, total_upper_bound)
        _AccountAdjustmentBounds.total_lower_bound.__set__(self, total_lower_bound)
        _AccountAdjustmentBounds.reserved_upper_bound.__set__(
            self, reserved_upper_bound
        )
        _AccountAdjustmentBounds.reserved_lower_bound.__set__(
            self, reserved_lower_bound
        )
        _AccountAdjustmentBounds.pending_upper_bound.__set__(self, pending_upper_bound)
        _AccountAdjustmentBounds.pending_lower_bound.__set__(self, pending_lower_bound)

    @property
    def total_upper_bound(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for total."""
        return _AccountAdjustmentBounds.total_upper_bound.__get__(self, type(self))

    @property
    def total_lower_bound(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for total."""
        return _AccountAdjustmentBounds.total_lower_bound.__get__(self, type(self))

    @property
    def reserved_upper_bound(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for reserved."""
        return _AccountAdjustmentBounds.reserved_upper_bound.__get__(self, type(self))

    @property
    def reserved_lower_bound(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for reserved."""
        return _AccountAdjustmentBounds.reserved_lower_bound.__get__(self, type(self))

    @property
    def pending_upper_bound(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive upper bound for pending."""
        return _AccountAdjustmentBounds.pending_upper_bound.__get__(self, type(self))

    @property
    def pending_lower_bound(self) -> PositionSize | None:
        """Allowed post-adjustment inclusive lower bound for pending."""
        return _AccountAdjustmentBounds.pending_lower_bound.__get__(self, type(self))

    def __repr__(self) -> str:
        return _AccountAdjustmentBounds.__repr__(self)


class AccountAdjustment(_AccountAdjustment):
    """Extensible non-trading account-adjustment model."""

    # @typing.override
    def __new__(cls, *args: typing.Any, **kwargs: typing.Any) -> AccountAdjustment:
        _ = args, kwargs
        return _AccountAdjustment.__new__(cls)

    # @typing.override
    def __init__(
        self,
        *,
        operation: AccountAdjustmentBalanceOperation
        | AccountAdjustmentPositionOperation
        | None = None,
        amount: AccountAdjustmentAmount | None = None,
        bounds: AccountAdjustmentBounds | None = None,
    ) -> None:
        if operation is not None and not isinstance(
            operation,
            (AccountAdjustmentBalanceOperation, AccountAdjustmentPositionOperation),
        ):
            raise TypeError(
                "operation must be "
                "openpit.account_adjustment.AccountAdjustmentBalanceOperation or "
                "openpit.account_adjustment.AccountAdjustmentPositionOperation"
            )
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
        return self.__dict__.get("_py_operation")

    @property
    def amount(self) -> AccountAdjustmentAmount | None:
        return self.__dict__.get("_py_amount")

    @property
    def bounds(self) -> AccountAdjustmentBounds | None:
        return self.__dict__.get("_py_bounds")

    def __repr__(self) -> str:
        return _AccountAdjustment.__repr__(self)


__all__ = [
    "AccountAdjustment",
    "AccountAdjustmentAmount",
    "AccountAdjustmentBalanceOperation",
    "AccountAdjustmentBounds",
    "AccountAdjustmentPositionOperation",
]
