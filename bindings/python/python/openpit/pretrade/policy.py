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
        instrument: openpit.Instrument,
        side: openpit.param.Side,
        trade_amount: openpit.param.TradeAmount,
        account_id: openpit.param.AccountId,
        price: openpit.param.Price | None = None,
    ) -> None:
        super().__init__(
            operation=openpit.OrderOperation(
                instrument=instrument,
                side=side,
                trade_amount=trade_amount,
                account_id=account_id,
                price=price,
            )
        )
        self.strategy = "broker-default"


class BrokerReport(openpit.ExecutionReport):
    @typing.override
    def __init__(
        self,
        *,
        instrument: openpit.Instrument,
        side: openpit.param.Side,
        account_id: openpit.param.AccountId,
        pnl: openpit.param.Pnl,
        fee: openpit.param.Fee,
    ) -> None:
        super().__init__(
            operation=openpit.ExecutionReportOperation(
                instrument=instrument,
                side=side,
                account_id=account_id,
            ),
            financial_impact=openpit.FinancialImpact(
                pnl=pnl,
                fee=fee,
            ),
        )
        self.source = "broker-fill"
```

This keeps custom metadata on the same object that reaches policy callbacks and
preserves one explicit engine-facing contract.
"""

from __future__ import annotations

import abc
import collections.abc
import dataclasses
import typing

if typing.TYPE_CHECKING:
    from .. import ExecutionReport, Order
from .._openpit import PreTradeContext
from ..core import Mutation
from ._enum import RejectScope


@dataclasses.dataclass(frozen=True)
class PolicyReject:
    """
    Business reject produced by a custom policy.

    Canonical reject model for policy interfaces.
    Field semantics match the engine reject payload:
    ``code``, ``reason``, ``details``, and ``scope``.

    This type models a normal reject path. Do not raise exceptions for normal
    risk decisions. Return this object instead.

    Attributes:
        code: Stable machine-readable reject code string from
            :class:`openpit.pretrade.RejectCode`.
        reason: Short human-readable reason.
        details: Detailed context for logs/diagnostics.
        scope: Reject scope, either ``"order"`` or ``"account"``.
        user_data: Opaque caller-defined integer token copied through reject
            flows. ``0`` means "not set". The SDK never inspects it; lifetime
            and thread-safety are caller-managed (see
            ``pit.wiki/Threading-Contract.md``).
    """

    code: str
    reason: str
    details: str
    scope: RejectScope = RejectScope.ORDER
    user_data: int = 0

    def __post_init__(self) -> None:
        if not isinstance(self.scope, RejectScope):
            raise TypeError("scope must be openpit.pretrade.RejectScope")


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
    - all configured start policies are evaluated; reject lists are merged
    - no rollback support for this stage

    Implementation rule:
    - return a tuple/list of :class:`PolicyReject` for normal risk rejects
    - return an empty tuple/list for success
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
    def check_pre_trade_start(
        self,
        ctx: PreTradeContext,
        order: Order,
    ) -> collections.abc.Iterable[PolicyReject]:
        """
        Evaluate an order in start stage.

        Args:
            ctx: Context of the current pre-trade operation.
            order: Incoming order candidate. This must be
                :class:`openpit.Order` or one of its subclasses.

        Returns:
            Iterable[PolicyReject]:
                - empty iterable if the order passes this policy
                - one or more :class:`PolicyReject` if this policy rejects
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


class PreTradePolicy(abc.ABC):
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
    def perform_pre_trade_check(
        self,
        ctx: PreTradeContext,
        order: Order,
    ) -> PolicyDecision:
        """
        Evaluate order context in main stage.

        Args:
            ctx: Context of the current pre-trade operation.
            order: Incoming order candidate.

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
