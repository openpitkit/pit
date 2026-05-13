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

"""Built-in pre-trade policy builders for the Python binding."""

from __future__ import annotations

import dataclasses
import datetime
import typing

from .._openpit import OrderSizeLimit

if typing.TYPE_CHECKING:
    from .. import param

OrderSizeLimit.__doc__ = """
Order-size limits (quantity and notional cap).

Args:
    max_quantity: Maximum allowed order quantity.
    max_notional: Maximum allowed notional volume.

Use as ``limit`` inside :class:`OrderSizeBrokerBarrier`,
:class:`OrderSizeAssetBarrier`, or :class:`OrderSizeAccountAssetBarrier`.
"""

# ---------------------------------------------------------------------------
# Rate limit
# ---------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class RateLimit:
    """Maximum orders within a sliding time window.

    Args:
        max_orders: Maximum number of orders accepted within *window*.
        window: Length of the sliding time window.
    """

    max_orders: int
    window: datetime.timedelta


@dataclasses.dataclass(frozen=True)
class RateLimitBrokerBarrier:
    """Rate limit applied across the entire broker.

    Args:
        limit: The rate limit definition.
    """

    limit: RateLimit


@dataclasses.dataclass(frozen=True)
class RateLimitAssetBarrier:
    """Rate limit applied per settlement asset.

    Args:
        limit: The rate limit definition.
        settlement_asset: Settlement asset symbol this barrier tracks.
    """

    limit: RateLimit
    settlement_asset: str


@dataclasses.dataclass(frozen=True)
class RateLimitAccountBarrier:
    """Rate limit applied per account.

    Args:
        limit: The rate limit definition.
        account_id: Account this barrier applies to.
    """

    limit: RateLimit
    account_id: param.AccountId


@dataclasses.dataclass(frozen=True)
class RateLimitAccountAssetBarrier:
    """Rate limit applied per (account, settlement asset) pair.

    Args:
        limit: The rate limit definition.
        account_id: Account this barrier applies to.
        settlement_asset: Settlement asset symbol this barrier tracks.
    """

    limit: RateLimit
    account_id: param.AccountId
    settlement_asset: str


class RateLimitReadyBuilder:
    """Fully-configured rate-limit policy builder.

    Obtain an instance via :func:`build_rate_limit` and one of its axis
    methods.  All axis methods return ``self`` so they can be chained.
    Pass to ``SyncedEngineBuilder.builtin()`` or
    ``ReadyEngineBuilder.builtin()`` to register on an engine.
    """

    def __init__(self) -> None:
        self._broker: RateLimitBrokerBarrier | None = None
        self._asset: list[RateLimitAssetBarrier] = []
        self._account: list[RateLimitAccountBarrier] = []
        self._account_asset: list[RateLimitAccountAssetBarrier] = []

    def broker_barrier(self, barrier: RateLimitBrokerBarrier) -> RateLimitReadyBuilder:
        """Set or replace the broker-wide rate limit."""
        self._broker = barrier
        return self

    def asset_barriers(self, *barriers: RateLimitAssetBarrier) -> RateLimitReadyBuilder:
        """Append per-settlement-asset rate-limit barriers."""
        self._asset.extend(barriers)
        return self

    def account_barriers(
        self, *barriers: RateLimitAccountBarrier
    ) -> RateLimitReadyBuilder:
        """Append per-account rate-limit barriers."""
        self._account.extend(barriers)
        return self

    def account_asset_barriers(
        self, *barriers: RateLimitAccountAssetBarrier
    ) -> RateLimitReadyBuilder:
        """Append per-(account, settlement-asset) rate-limit barriers."""
        self._account_asset.extend(barriers)
        return self

    def _build(self, builder: typing.Any) -> None:
        """Contract hook invoked by ``builtin()`` to register this policy."""

        def _nanos(limit: RateLimit) -> int:
            micros = limit.window // datetime.timedelta(microseconds=1)
            return micros * 1_000

        broker = None
        if self._broker is not None:
            b = self._broker
            broker = (
                b.limit.max_orders,
                _nanos(b.limit),
            )

        builder._add_builtin_rate_limit(
            broker=broker,
            asset_barriers=[
                (
                    b.settlement_asset,
                    b.limit.max_orders,
                    _nanos(b.limit),
                )
                for b in self._asset
            ],
            account_barriers=[
                (
                    b.account_id.value,
                    b.limit.max_orders,
                    _nanos(b.limit),
                )
                for b in self._account
            ],
            account_asset_barriers=[
                (
                    b.account_id.value,
                    b.settlement_asset,
                    b.limit.max_orders,
                    _nanos(b.limit),
                )
                for b in self._account_asset
            ],
        )


class RateLimitBuilder:
    """Entry point for the rate-limit policy builder.

    Call :func:`build_rate_limit` to obtain an instance, then call one of
    the axis methods to obtain a :class:`RateLimitReadyBuilder`.
    """

    def __init__(self) -> None:
        self._ready = RateLimitReadyBuilder()

    def broker_barrier(self, barrier: RateLimitBrokerBarrier) -> RateLimitReadyBuilder:
        """Set the broker-wide rate limit and return a ready builder."""
        return self._ready.broker_barrier(barrier)

    def asset_barriers(self, *barriers: RateLimitAssetBarrier) -> RateLimitReadyBuilder:
        """Add per-settlement-asset barriers and return a ready builder."""
        return self._ready.asset_barriers(*barriers)

    def account_barriers(
        self, *barriers: RateLimitAccountBarrier
    ) -> RateLimitReadyBuilder:
        """Add per-account barriers and return a ready builder."""
        return self._ready.account_barriers(*barriers)

    def account_asset_barriers(
        self, *barriers: RateLimitAccountAssetBarrier
    ) -> RateLimitReadyBuilder:
        """Add per-(account, settlement-asset) barriers and return a ready builder."""
        return self._ready.account_asset_barriers(*barriers)


def build_rate_limit() -> RateLimitBuilder:
    """Return a new rate-limit policy builder."""
    return RateLimitBuilder()


# ---------------------------------------------------------------------------
# Order size limit
# ---------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class OrderSizeBrokerBarrier:
    """Order size limit applied across the entire broker.

    Args:
        limit: Quantity and notional caps.
    """

    limit: OrderSizeLimit


@dataclasses.dataclass(frozen=True)
class OrderSizeAssetBarrier:
    """Order size limit applied per settlement asset.

    Args:
        limit: Quantity and notional caps.
        settlement_asset: Settlement asset symbol this barrier tracks.
    """

    limit: OrderSizeLimit
    settlement_asset: str


@dataclasses.dataclass(frozen=True)
class OrderSizeAccountAssetBarrier:
    """Order size limit applied per (account, settlement asset) pair.

    Args:
        limit: Quantity and notional caps.
        account_id: Account this barrier applies to.
        settlement_asset: Settlement asset symbol this barrier tracks.
    """

    limit: OrderSizeLimit
    account_id: param.AccountId
    settlement_asset: str


class OrderSizeLimitReadyBuilder:
    """Fully-configured order-size-limit policy builder.

    Obtain via :func:`build_order_size_limit` followed by
    :meth:`~OrderSizeLimitBuilder.broker_barrier`.  All axis methods
    return ``self`` for chaining.  Pass to
    ``SyncedEngineBuilder.builtin()`` or ``ReadyEngineBuilder.builtin()``.
    """

    def __init__(self) -> None:
        self._broker: OrderSizeBrokerBarrier | None = None
        self._asset: list[OrderSizeAssetBarrier] = []
        self._account_asset: list[OrderSizeAccountAssetBarrier] = []

    def broker_barrier(
        self, barrier: OrderSizeBrokerBarrier
    ) -> OrderSizeLimitReadyBuilder:
        """Set or replace the broker-wide order-size limit."""
        self._broker = barrier
        return self

    def asset_barriers(
        self, *barriers: OrderSizeAssetBarrier
    ) -> OrderSizeLimitReadyBuilder:
        """Append per-settlement-asset order-size barriers."""
        self._asset.extend(barriers)
        return self

    def account_asset_barriers(
        self, *barriers: OrderSizeAccountAssetBarrier
    ) -> OrderSizeLimitReadyBuilder:
        """Append per-(account, settlement-asset) order-size barriers."""
        self._account_asset.extend(barriers)
        return self

    def _build(self, builder: typing.Any) -> None:
        """Contract hook invoked by ``builtin()`` to register this policy."""
        builder._add_builtin_order_size_limit(
            broker=self._broker.limit if self._broker is not None else None,
            asset_barriers=[(b.limit, b.settlement_asset) for b in self._asset],
            account_asset_barriers=[
                (b.limit, b.account_id.value, b.settlement_asset)
                for b in self._account_asset
            ],
        )


class OrderSizeLimitBuilder:
    """Entry point for the order-size-limit policy builder.

    Call :func:`build_order_size_limit` to obtain an instance.  Call
    :meth:`broker_barrier` to obtain an :class:`OrderSizeLimitReadyBuilder`
    that can be passed to ``builtin()``.  Additional axes
    (:meth:`asset_barriers`, :meth:`account_asset_barriers`) stage barriers
    before the broker barrier is set.
    """

    def __init__(self) -> None:
        self._ready = OrderSizeLimitReadyBuilder()

    def broker_barrier(
        self, barrier: OrderSizeBrokerBarrier
    ) -> OrderSizeLimitReadyBuilder:
        """Set the broker-wide order-size limit and return a ready builder."""
        return self._ready.broker_barrier(barrier)

    def asset_barriers(
        self, *barriers: OrderSizeAssetBarrier
    ) -> OrderSizeLimitReadyBuilder:
        """Add per-settlement-asset barriers and return a ready builder."""
        return self._ready.asset_barriers(*barriers)

    def account_asset_barriers(
        self, *barriers: OrderSizeAccountAssetBarrier
    ) -> OrderSizeLimitReadyBuilder:
        """Add per-(account, settlement-asset) barriers and return a ready builder."""
        return self._ready.account_asset_barriers(*barriers)


def build_order_size_limit() -> OrderSizeLimitBuilder:
    """Return a new order-size-limit policy builder."""
    return OrderSizeLimitBuilder()


# ---------------------------------------------------------------------------
# P&L bounds kill-switch
# ---------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class PnlBoundsBrokerBarrier:
    """Broker-level P&L bounds barrier.

    Args:
        settlement_asset: Settlement asset tracked by this barrier.
        lower_bound: Optional lower P&L bound (typically a negative loss limit).
        upper_bound: Optional upper P&L bound (typically a positive profit limit).

    At least one of *lower_bound* or *upper_bound* must be provided.
    """

    settlement_asset: str
    lower_bound: param.Pnl | None = None
    upper_bound: param.Pnl | None = None


@dataclasses.dataclass(frozen=True)
class PnlBoundsAccountAssetBarrier:
    """Per-account P&L bounds barrier.

    Args:
        account_id: Account this barrier applies to.
        settlement_asset: Settlement asset tracked by this barrier.
        initial_pnl: Initial P&L offset loaded at construction.
        lower_bound: Optional lower P&L bound.
        upper_bound: Optional upper P&L bound.

    At least one of *lower_bound* or *upper_bound* must be provided.
    """

    account_id: param.AccountId
    settlement_asset: str
    initial_pnl: param.Pnl
    lower_bound: param.Pnl | None = None
    upper_bound: param.Pnl | None = None


class PnlBoundsKillswitchReadyBuilder:
    """Fully-configured P&L bounds kill-switch policy builder.

    Obtain via :func:`build_pnl_bounds_killswitch` and one axis method.
    All axis methods return ``self`` for chaining.  Pass to
    ``SyncedEngineBuilder.builtin()`` or ``ReadyEngineBuilder.builtin()``.
    """

    def __init__(self) -> None:
        self._broker: list[PnlBoundsBrokerBarrier] = []
        self._account: list[PnlBoundsAccountAssetBarrier] = []

    def broker_barriers(
        self, *barriers: PnlBoundsBrokerBarrier
    ) -> PnlBoundsKillswitchReadyBuilder:
        """Append broker-level P&L bounds barriers."""
        self._broker.extend(barriers)
        return self

    def account_barriers(
        self, *barriers: PnlBoundsAccountAssetBarrier
    ) -> PnlBoundsKillswitchReadyBuilder:
        """Append per-account P&L bounds barriers."""
        self._account.extend(barriers)
        return self

    def _build(self, builder: typing.Any) -> None:
        """Contract hook invoked by ``builtin()`` to register this policy."""
        builder._add_builtin_pnl_bounds_killswitch(
            broker_barriers=self._broker,
            account_barriers=self._account,
        )


class PnlBoundsKillswitchBuilder:
    """Entry point for the P&L bounds kill-switch policy builder.

    Call :func:`build_pnl_bounds_killswitch` to obtain an instance, then
    call one of the axis methods to obtain a
    :class:`PnlBoundsKillswitchReadyBuilder`.
    """

    def __init__(self) -> None:
        self._ready = PnlBoundsKillswitchReadyBuilder()

    def broker_barriers(
        self, *barriers: PnlBoundsBrokerBarrier
    ) -> PnlBoundsKillswitchReadyBuilder:
        """Add broker-level barriers and return a ready builder."""
        return self._ready.broker_barriers(*barriers)

    def account_barriers(
        self, *barriers: PnlBoundsAccountAssetBarrier
    ) -> PnlBoundsKillswitchReadyBuilder:
        """Add per-account barriers and return a ready builder."""
        return self._ready.account_barriers(*barriers)


def build_pnl_bounds_killswitch() -> PnlBoundsKillswitchBuilder:
    """Return a new P&L bounds kill-switch policy builder."""
    return PnlBoundsKillswitchBuilder()


# ---------------------------------------------------------------------------
# Order validation
# ---------------------------------------------------------------------------


class OrderValidationBuilder:
    """Order-validation policy builder.

    Pass to ``SyncedEngineBuilder.builtin()`` or
    ``ReadyEngineBuilder.builtin()`` to register on an engine.
    """

    def _build(self, builder: typing.Any) -> None:
        """Contract hook invoked by ``builtin()`` to register this policy."""
        builder._add_builtin_order_validation()


def build_order_validation() -> OrderValidationBuilder:
    """Return an order-validation policy builder."""
    return OrderValidationBuilder()


# ---------------------------------------------------------------------------

__all__ = [
    "OrderSizeLimit",
    "OrderSizeBrokerBarrier",
    "OrderSizeAssetBarrier",
    "OrderSizeAccountAssetBarrier",
    "OrderSizeLimitReadyBuilder",
    "OrderSizeLimitBuilder",
    "build_order_size_limit",
    "PnlBoundsBrokerBarrier",
    "PnlBoundsAccountAssetBarrier",
    "PnlBoundsKillswitchReadyBuilder",
    "PnlBoundsKillswitchBuilder",
    "build_pnl_bounds_killswitch",
    "RateLimit",
    "RateLimitBrokerBarrier",
    "RateLimitAssetBarrier",
    "RateLimitAccountBarrier",
    "RateLimitAccountAssetBarrier",
    "RateLimitReadyBuilder",
    "RateLimitBuilder",
    "build_rate_limit",
    "OrderValidationBuilder",
    "build_order_validation",
]
