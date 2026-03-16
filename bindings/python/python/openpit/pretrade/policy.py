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

"""
Python policy interfaces and decision types exposed by openpit.

The recommended integration style is to derive boundary models directly from
the engine contracts and add project-specific fields in subclasses:

```python
import typing

import openpit

if not hasattr(typing, "override"):
    def _override(method):
        return method

    typing.override = _override  # type: ignore[attr-defined]


class BrokerOrder(openpit.Order):
    @typing.override
    def __init__(
        self,
        *,
        underlying_asset: openpit.param.Asset,
        settlement_asset: openpit.param.Asset,
        side: openpit.param.Side,
        trade_amount: openpit.param.Quantity | openpit.param.Volume,
        price: openpit.param.Price,
    ) -> None:
        super().__init__(
            underlying_asset=underlying_asset,
            settlement_asset=settlement_asset,
            side=side,
            trade_amount=trade_amount,
            price=price,
        )
        self.strategy = "broker-default"


class BrokerReport(openpit.ExecutionReport):
    @typing.override
    def __init__(
        self,
        *,
        underlying_asset: openpit.param.Asset,
        settlement_asset: openpit.param.Asset,
        pnl: openpit.param.Pnl,
        fee: openpit.param.Fee,
    ) -> None:
        super().__init__(
            underlying_asset=underlying_asset,
            settlement_asset=settlement_asset,
            pnl=pnl,
            fee=fee,
        )
        self.source = "broker-fill"
```

This keeps custom metadata on the same object that reaches policy callbacks and
preserves one explicit engine-facing contract.
"""

from __future__ import annotations

import abc
import dataclasses
import typing

if typing.TYPE_CHECKING:
    from .. import ExecutionReport, Order
from ..param import Asset, Volume
from ._enum import MutationKind, RejectScope


@dataclasses.dataclass(frozen=True)
class PolicyContext:
    """
    Immutable context passed into :meth:`Policy.perform_pre_trade_check`.

    Attributes:
        order: The original order object under evaluation. This is typed as
            :class:`openpit.Order` and should be an instance of
            :class:`openpit.Order` or one of its subclasses.
    """

    order: Order


@dataclasses.dataclass(frozen=True)
class PolicyReject:
    """
    Business reject produced by a custom policy.

    This type models a normal reject path. Do not raise exceptions for normal
    risk decisions. Return this object instead.

    Attributes:
        code: Stable machine-readable reject code string from
            :class:`openpit.RejectCode`.
        reason: Short human-readable reason.
        details: Detailed context for logs/diagnostics.
        scope: Reject scope, either ``"order"`` or ``"account"``.
    """

    code: str
    reason: str
    details: str
    scope: RejectScope = RejectScope.ORDER

    def __post_init__(self) -> None:
        if not isinstance(self.scope, RejectScope):
            raise TypeError("scope must be openpit.pretrade.RejectScope")


@dataclasses.dataclass(frozen=True)
class RiskMutation:
    """
    Closed-set mutation descriptor consumed by the Rust engine.

    Use class constructors :meth:`reserve_notional` and :meth:`kill_switch`.
    Avoid creating instances manually unless you need full control.
    """

    kind: MutationKind
    settlement_asset: Asset | None = None
    amount: Volume | None = None
    kill_switch_id: str | None = None
    enabled: bool | None = None

    def __post_init__(self) -> None:
        if self.kind == MutationKind.RESERVE_NOTIONAL:
            if not self.settlement_asset:
                raise ValueError("reserve_notional requires settlement_asset")
            if self.amount is None:
                raise ValueError("reserve_notional requires amount")
            return

        if self.kind == MutationKind.SET_KILL_SWITCH:
            if not self.kill_switch_id:
                raise ValueError("set_kill_switch requires kill_switch_id")
            if not isinstance(self.enabled, bool):
                raise ValueError("set_kill_switch requires enabled bool")
            return

        raise ValueError("unsupported mutation kind")

    @classmethod
    def reserve_notional(
        cls,
        settlement_asset: Asset,
        amount: Volume,
    ) -> RiskMutation:
        """
        Create a reserve-notional mutation.

        Args:
            settlement_asset: Settlement asset identifier.
            amount: Reserved notional amount.

        Returns:
            RiskMutation: Mutation with
                ``kind=openpit.pretrade.MutationKind.RESERVE_NOTIONAL``.
        """
        return cls(
            kind=MutationKind.RESERVE_NOTIONAL,
            settlement_asset=settlement_asset,
            amount=amount,
        )

    @classmethod
    def kill_switch(
        cls,
        kill_switch_id: str,
        enabled: bool,
    ) -> RiskMutation:
        """
        Create a kill-switch mutation.

        Args:
            kill_switch_id: Kill-switch identifier.
            enabled: Desired kill-switch state.

        Returns:
            RiskMutation: Mutation with
                ``kind=openpit.pretrade.MutationKind.SET_KILL_SWITCH``.
        """
        return cls(
            kind=MutationKind.SET_KILL_SWITCH,
            kill_switch_id=kill_switch_id,
            enabled=enabled,
        )


@dataclasses.dataclass(frozen=True)
class Mutation:
    """
    Commit/rollback mutation pair for main-stage policies.

    The engine applies ``commit`` on successful reservation finalization and
    ``rollback`` when a reservation is rolled back or when execute stage fails.
    """

    commit: RiskMutation
    rollback: RiskMutation

    @classmethod
    def reserve_notional(
        cls,
        settlement_asset: Asset,
        commit_amount: Volume,
        rollback_amount: Volume,
    ) -> Mutation:
        """
        Build a reserve-notional commit/rollback pair.

        Args:
            settlement_asset: Settlement asset symbol.
            commit_amount: Amount to apply on commit.
            rollback_amount: Amount to apply on rollback.
        """
        return cls(
            commit=RiskMutation.reserve_notional(
                settlement_asset=settlement_asset,
                amount=commit_amount,
            ),
            rollback=RiskMutation.reserve_notional(
                settlement_asset=settlement_asset,
                amount=rollback_amount,
            ),
        )

    @classmethod
    def kill_switch(
        cls,
        kill_switch_id: str,
        commit_enabled: bool,
        rollback_enabled: bool,
    ) -> Mutation:
        """
        Build a kill-switch commit/rollback pair.

        Args:
            kill_switch_id: Kill-switch identifier.
            commit_enabled: State to apply on commit.
            rollback_enabled: State to apply on rollback.
        """
        return cls(
            commit=RiskMutation.kill_switch(
                kill_switch_id=kill_switch_id,
                enabled=commit_enabled,
            ),
            rollback=RiskMutation.kill_switch(
                kill_switch_id=kill_switch_id,
                enabled=rollback_enabled,
            ),
        )


@dataclasses.dataclass(frozen=True)
class PolicyDecision:
    """
    Return type of :meth:`Policy.perform_pre_trade_check`.

    Attributes:
        rejects: Rejects produced by the policy.
        mutations: Mutations registered by the policy.
    """

    rejects: tuple[PolicyReject, ...] = ()
    mutations: tuple[Mutation, ...] = ()

    @classmethod
    def accept(cls, mutations: typing.Iterable[Mutation] = ()) -> PolicyDecision:
        """
        Build a successful decision.

        Args:
            mutations: Optional mutations to register.

        Returns:
            PolicyDecision: Decision with empty rejects.
        """
        return cls(rejects=(), mutations=tuple(mutations))

    @classmethod
    def reject(
        cls,
        rejects: typing.Iterable[PolicyReject],
        mutations: typing.Iterable[Mutation] = (),
    ) -> PolicyDecision:
        """
        Build a rejecting decision.

        Args:
            rejects: One or more business rejects.
            mutations: Optional mutations produced before reject decision.

        Returns:
            PolicyDecision: Decision with non-empty rejects.
        """
        return cls(rejects=tuple(rejects), mutations=tuple(mutations))


class CheckPreTradeStartPolicy(abc.ABC):
    """
    Interface for start-stage Python policies.

    Stage semantics:
    - called during ``engine.start_pre_trade(order=...)``
    - evaluation stops at first reject
    - no rollback support for this stage

    Implementation rule:
    - return :class:`PolicyReject` for normal risk rejects
    - return ``None`` for success
    - raise exceptions only for programming/runtime failures
    """

    @property
    @abc.abstractmethod
    def name(self) -> str:
        """
        Return a stable, unique policy name.

        The name must be non-empty and unique within one engine config.
        """
        raise NotImplementedError("name is not implemented")

    @abc.abstractmethod
    def check_pre_trade_start(self, order: Order) -> PolicyReject | None:
        """
        Evaluate an order in start stage.

        Args:
            order: Incoming order candidate. This must be
                :class:`openpit.Order` or one of its subclasses.

        Returns:
            Optional[PolicyReject]:
                - ``None`` if the order passes this policy
                - ``PolicyReject`` if this policy rejects the order
        """
        raise NotImplementedError("check_pre_trade_start is not implemented")

    @abc.abstractmethod
    def apply_execution_report(self, report: ExecutionReport) -> bool:
        """
        Apply post-trade feedback to policy state.

        Args:
            report: Execution report produced after fill/close.

        Returns:
            bool:
                ``True`` if this policy considers kill-switch triggered after
                processing the report, otherwise ``False``.
        """
        raise NotImplementedError("apply_execution_report is not implemented")


class Policy(abc.ABC):
    """
    Interface for main-stage Python policies.

    Stage semantics:
    - called during ``request.execute()``
    - all configured policies are evaluated, even if one rejects
    - mutations are applied/rolled back by the engine according to outcome

    Implementation rule:
    - return :class:`PolicyDecision` for business outcome
    - raise exceptions only for programming/runtime failures
    """

    @property
    @abc.abstractmethod
    def name(self) -> str:
        """
        Return a stable, unique policy name.

        The name must be non-empty and unique within one engine config.
        """
        raise NotImplementedError("name is not implemented")

    @abc.abstractmethod
    def perform_pre_trade_check(self, context: PolicyContext) -> PolicyDecision:
        """
        Evaluate order context in main stage.

        Args:
            context: Immutable context with original order.

        Returns:
            PolicyDecision:
                - use ``PolicyDecision.accept(...)`` for pass path
                - use ``PolicyDecision.reject(...)`` for business rejects
        """
        raise NotImplementedError("perform_pre_trade_check is not implemented")

    @abc.abstractmethod
    def apply_execution_report(self, report: ExecutionReport) -> bool:
        """
        Apply post-trade feedback to policy state.

        Args:
            report: Execution report produced after fill/close.

        Returns:
            bool:
                ``True`` if this policy considers kill-switch triggered after
                processing the report, otherwise ``False``.
        """
        raise NotImplementedError("apply_execution_report is not implemented")
