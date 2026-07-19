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
    from .. import (
        AccountAdjustment,
        AccountAdjustmentContext,
        ExecutionReport,
        Order,
    )
    from .._openpit import AccountBlock, AccountOutcomeEntry, PostTradeResult
    from ..param import AccountId, Price
from .._openpit import Context, PostTradeContext
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


@dataclasses.dataclass(frozen=True)
class PolicyPreTradeResult:
    """
    Return type of :meth:`Policy.perform_pre_trade_check`.

    Attributes:
        rejects: Rejects produced by the policy.
        mutations: Mutations registered by the policy.
        account_adjustments: Per-asset outcome entries produced by the policy.
        lock_prices: Prices to store under this policy's ``policy_group_id``.
    """

    rejects: tuple[PolicyReject, ...] = ()
    mutations: tuple[Mutation, ...] = ()
    account_adjustments: tuple[AccountOutcomeEntry, ...] = ()
    lock_prices: tuple[Price, ...] = ()

    @classmethod
    def accept(
        cls,
        mutations: typing.Iterable[Mutation] = (),
        account_adjustments: typing.Iterable[AccountOutcomeEntry] = (),
        lock_prices: typing.Iterable[Price] = (),
    ) -> PolicyPreTradeResult:
        """
        Build a successful pre-trade result.
        """
        return cls(
            rejects=(),
            mutations=tuple(mutations),
            account_adjustments=tuple(account_adjustments),
            lock_prices=tuple(lock_prices),
        )

    @classmethod
    def reject(
        cls,
        rejects: typing.Iterable[PolicyReject],
        mutations: typing.Iterable[Mutation] = (),
        account_adjustments: typing.Iterable[AccountOutcomeEntry] = (),
        lock_prices: typing.Iterable[Price] = (),
    ) -> PolicyPreTradeResult:
        """
        Build a rejecting pre-trade result.
        """
        return cls(
            rejects=tuple(rejects),
            mutations=tuple(mutations),
            account_adjustments=tuple(account_adjustments),
            lock_prices=tuple(lock_prices),
        )


@dataclasses.dataclass(frozen=True)
class PolicyAccountAdjustmentResult:
    """
    Result of :meth:`Policy.apply_account_adjustment`.

    Carries both outcomes: an empty ``rejects`` collection accepts the
    adjustment, a non-empty one business-rejects the whole batch.

    Attributes:
        rejects: Business rejects produced by the policy. Non-empty rejects
            the batch and discards the other fields.
        mutations: Mutations registered by the policy.
        account_adjustments: Per-asset outcomes produced by the policy.
        account_blocks: Account blocks recorded after the accepted batch commits.
    """

    rejects: tuple[PolicyReject, ...] = ()
    mutations: tuple[Mutation, ...] = ()
    account_adjustments: tuple[AccountOutcomeEntry, ...] = ()
    account_blocks: tuple[AccountBlock, ...] = ()


class Policy(abc.ABC):
    """
    Unified Python pre-trade policy interface.

    Stage semantics:
    - ``check_pre_trade_start`` is called during ``engine.start_pre_trade``
    - ``perform_pre_trade_check`` is called during ``request.execute()``
    - ``apply_execution_report`` applies post-trade feedback
    - ``apply_account_adjustment`` validates account-adjustment batches

    Implementation rule:
    - override the methods needed by the registration path used by the policy
    - return :class:`PolicyPreTradeResult` for main-stage outcomes
    - return :class:`openpit.pretrade.PostTradeResult` for post-trade outcomes
    - return :class:`PolicyAccountAdjustmentResult` from account adjustments;
      its ``account_adjustments`` field carries per-asset outcomes
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

    @property
    def policy_group_id(self) -> int:
        """
        Return the policy group tag.

        The engine stores pre-trade lock prices and account-adjustment outcomes
        under this group. ``0`` is the default group.
        """
        return 0

    def check_pre_trade_start(
        self,
        ctx: Context,
        order: Order,
    ) -> collections.abc.Iterable[PolicyReject]:
        """
        Evaluate an order in start stage.

        Args:
            ctx: Context of the current pre-trade operation.
                ``ctx.account_control`` is an
                :class:`openpit.pretrade.AccountControl` when the engine exposes
                the account-block facility for the order's account, otherwise
                ``None``. A policy may block the account directly or capture the
                handle into a :class:`openpit.Mutation` rollback/commit closure
                to block on a deferred failure. The handle is valid only within
                this request's pre-trade processing (through its reservation's
                commit or rollback); using it afterwards is unspecified.
            order: Incoming order candidate. This must be
                :class:`openpit.Order` or one of its subclasses.

        Returns:
            Iterable[PolicyReject]:
                - empty iterable if the order passes this policy
                - one or more :class:`PolicyReject` if this policy rejects
        """
        return ()

    def perform_pre_trade_check(
        self,
        ctx: Context,
        order: Order,
    ) -> PolicyPreTradeResult:
        """
        Evaluate order context in main stage.

        Args:
            ctx: Context of the current pre-trade operation.
                ``ctx.account_control`` is an
                :class:`openpit.pretrade.AccountControl` when the engine exposes
                the account-block facility for the order's account, otherwise
                ``None``. A policy may block the account directly or capture the
                handle into a :class:`openpit.Mutation` rollback/commit closure
                to block on a deferred failure. The handle is valid only within
                this request's pre-trade processing (through its reservation's
                commit or rollback); using it afterwards is unspecified.
            order: Incoming order candidate.

        Returns:
            PolicyPreTradeResult:
                - use ``PolicyPreTradeResult.accept(...)`` for pass path
                - use ``PolicyPreTradeResult.reject(...)`` for business rejects
        """
        return PolicyPreTradeResult.accept()

    def apply_execution_report(
        self,
        ctx: PostTradeContext,
        report: ExecutionReport,
    ) -> PostTradeResult | None:
        """
        Apply post-trade feedback to policy state.

        Args:
            ctx: Post-trade context. ``ctx.account_group`` is the
                :class:`openpit.param.AccountGroupId` of the report's account,
                or ``None`` when the account is absent or unregistered.
            report: Execution report produced after fill/close.

        Returns:
            PostTradeResult | None:
                Result with account blocks, account-level PnL outcomes, and
                account adjustments, or ``None`` if the report caused no
                visible post-trade outcome.
        """
        return None

    def apply_account_adjustment(
        self,
        ctx: AccountAdjustmentContext,
        account_id: AccountId,
        adjustment: AccountAdjustment,
    ) -> PolicyAccountAdjustmentResult:
        """
        Evaluate one account adjustment from an atomic batch.

        Args:
            ctx: Read-only engine context for the current batch operation.
            account_id: Account affected by the batch.
            adjustment: Current adjustment item.

        Returns:
            Result containing business rejects, rollback mutations, per-asset
            ``account_adjustments``, and account blocks.

            Populating ``rejects`` is the only way to business-reject an
            adjustment: a non-empty collection rejects the whole batch, so no
            item of it is applied. Leaving ``rejects`` empty accepts the
            adjustment; the default result accepts it and changes nothing.

            A non-empty ``account_blocks`` collection is recorded only after
            the complete adjustment batch commits.
        """
        return PolicyAccountAdjustmentResult()
