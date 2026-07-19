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
# Please see https://openpit.dev and the OWNERS file for details.

"""Pre-trade pipeline components: policies, rejects, requests, and reservations."""

from __future__ import annotations

import typing
from collections.abc import Iterable  # noqa: F401  (used in string annotations)

from .._openpit import (
    _DEFAULT_POLICY_GROUP_ID,
    AccountAdjustmentBatchResult,
    AccountAdjustmentOutcome,
    AccountBlock,
    AccountControl,
    AccountOutcomeEntry,
    AccountPnlOutcome,
    DryRunReport,
    ExecuteResult,
    OutcomeAmount,
    PnlHaltReason,
    PnlOutcome,
    PnlOutcomeAmount,
    PolicyConfigurationResult,
    PostTradeContext,
    PostTradeResult,
    Reject,
    Request,
    Reservation,
    StartResult,
)
from .._openpit import (
    Lock as _Lock,
)
from ..param import Price
from . import policies
from ._enum import RejectCode, RejectScope
from .policy import (
    Context,
    Policy,
    PolicyAccountAdjustmentResult,
    PolicyDecision,
    PolicyPreTradeResult,
    PolicyReject,
)

PolicyGroupId: typing.TypeAlias = int
"""Group identifier used to tag policies and their outcomes."""

DEFAULT_POLICY_GROUP_ID: PolicyGroupId = _DEFAULT_POLICY_GROUP_ID

RejectCode.__module__ = __name__
RejectScope.__module__ = __name__


Context.__doc__ = """
Opaque context object passed to Python policy callbacks.

The object identifies the current engine evaluation context. Treat it as
read-only. Future releases may expose additional query methods on this object;
policies should not create instances directly.

``account_control`` is an :class:`AccountControl` when the engine exposes the
account-block facility for the evaluated account, otherwise ``None``. A policy
may capture it into a :class:`openpit.Mutation` rollback/commit closure to block
the account on a deferred failure. The handle is valid only within this
request's pre-trade processing (through its reservation's commit or rollback);
using it afterwards is unspecified.
"""

ExecuteResult.__doc__ = """
Result of ``Request.execute`` or ``Engine.execute_pre_trade``.

Attributes:
    ok: ``True`` when all evaluated stages accepted the order.
    reservation: ``Reservation`` on success, otherwise ``None``.
    rejects: Ordered list of business rejects when ``ok`` is ``False``.

Successful results must be finalized by committing or rolling back the
reservation. Failed results never contain a reservation.
"""

PostTradeResult.__doc__ = """
Result of ``openpit.Engine.apply_execution_report``.

Post-trade processing is not atomic: ``account_pnls`` and
``account_adjustments`` reflect storage mutations that have already been
applied, so callers must propagate both downstream even when
``account_blocks`` is non-empty.

Attributes:
    account_blocks: List of account blocks reported by policies. Non-empty
        when at least one policy entered a blocked state after the report.
    account_pnls: List of policy-tagged account-level PnL outcomes. Each
        outcome has either ``pnl`` or ``halt_reason``.
    account_adjustments: List of per-asset position outcomes reported by
        policies, in policy registration order.
"""

PnlHaltReason.__doc__ = """
Reason why a realized-PnL value could not be calculated.

The reason identifies the missing input or arithmetic failure that halted the
specific position or account accumulator. A halt remains sticky until that
exact accumulator is explicitly force-set.
"""

AccountPnlOutcome.__doc__ = """
Policy-tagged account-level realized-PnL outcome.

Exactly one of ``pnl`` and ``halt_reason`` is present. SpotFunds emits a halted
outcome only for the report that transitions the account accumulator to halted;
later reports omit the unchanged halt. A position PnL force-set does not re-arm
this account accumulator.
"""

PnlOutcome.__doc__ = """
Realized-PnL result for one position.

Exactly one of ``pnl`` and ``halt_reason`` is present. The halt reason is
reported only by the operation that first fails; later operations omit the
position PnL until an adjustment force-sets that position's PnL. Re-arming it
does not re-arm account PnL or another position.
"""

Reject.__doc__ = """
Business reject returned by start-stage or main-stage checks.

Rejects are normal policy outcomes, not exceptional failures.

Attributes:
    code: Stable machine-readable reject code.
    reason: Short human-readable reason.
    details: Diagnostic text intended for logs and operators.
    policy: Name of the policy that produced the reject.
    scope: ``RejectScope.ORDER`` or ``RejectScope.ACCOUNT``.
    user_data: Opaque caller-defined integer token copied from custom rejects.
"""

Request.__doc__ = """
Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

The handle is single-use. Calling ``execute`` more than once raises
``RuntimeError``. Keep the handle only while the caller is ready to enter the
main stage; it represents a snapshot of the submitted order.
"""

Reservation.__doc__ = """
Single-use reservation handle returned by successful main-stage execution.

Call ``commit`` only after the order has been accepted by the downstream venue
or transport. Call ``rollback`` when submission fails or the caller decides not
to send the order. Calling either finalizer more than once raises
``RuntimeError``.
"""

StartResult.__doc__ = """
Result of ``Engine.start_pre_trade``.

Attributes:
    ok: ``True`` when start-stage checks accepted the order.
    request: Deferred ``Request`` on success, otherwise ``None``.
    rejects: Merged start-stage reject list when ``ok`` is ``False``.
"""

AccountAdjustmentBatchResult.__doc__ = """
Result of ``Engine.apply_account_adjustment``.

Attributes:
    ok: ``True`` when the whole adjustment batch passed.
    failed_index: Zero-based index of the first rejected adjustment, or
        ``None`` on success.
    rejects: Reject list returned for the failed adjustment.
    outcomes: Per-adjustment outcomes on success; empty on rejection.
    account_blocks: Committed blocks on success; empty on rejection.
"""

PolicyConfigurationResult.__doc__ = """
Result of an accepted runtime policy configuration operation.

Attributes:
    account_blocks: Blocks recorded by the engine before the configuration
        call returns. The list is empty when the accepted change did not block
        an account.
"""

DryRunReport.__doc__ = """
Inert verdict of a non-mutating pre-trade dry-run.

Describes what ``Engine.execute_pre_trade`` or ``Engine.start_pre_trade``
would have produced for the same order and engine state, without moving any
engine state: no rate-limit budget is spent, no reservation or hold is
applied, and no account is blocked.

Attributes:
    is_pass: ``True`` when the order would have been admitted by all stages.
        Equivalent to ``bool(report)``.
    rejects: List of ``Reject`` objects when ``is_pass`` is ``False``,
        otherwise ``None``.
    account_block: ``AccountBlock`` that a real call would have recorded in
        the engine's blocked-accounts registry, or ``None`` when the order
        would have passed or produced a non-account reject.

Methods:
    lock(): ``Lock`` the main stage would have produced. Empty when the start
        stage would have rejected or no policy locks a price.
    account_adjustments(): List of ``AccountAdjustmentOutcome`` objects the
        main stage would have produced. Empty when the start stage would have
        rejected or no policy reports an adjustment.
"""

Rejects = list[Reject]


class Lock(_Lock):
    """Pre-trade price lock payload: grouped `(policy_group_id, price)` records."""

    def __new__(cls, *args: object, **kwargs: object) -> Lock:
        return _Lock.__new__(cls, *args, **kwargs)

    def __init__(
        self,
        entries: Lock | Iterable[tuple[PolicyGroupId, Price]] | None = None,
    ) -> None:
        _ = entries  # actual construction performed in _Lock.__new__

    def __repr__(self) -> str:
        return _Lock.__repr__(self)


__all__ = [
    "AccountAdjustmentBatchResult",
    "AccountAdjustmentOutcome",
    "AccountBlock",
    "AccountControl",
    "AccountOutcomeEntry",
    "AccountPnlOutcome",
    "DEFAULT_POLICY_GROUP_ID",
    "DryRunReport",
    "ExecuteResult",
    "PolicyGroupId",
    "OutcomeAmount",
    "PnlHaltReason",
    "PnlOutcome",
    "PnlOutcomeAmount",
    "PolicyConfigurationResult",
    "PolicyAccountAdjustmentResult",
    "Policy",
    "Context",
    "PolicyDecision",
    "PolicyPreTradeResult",
    "PolicyReject",
    "PostTradeContext",
    "PostTradeResult",
    "Lock",
    "Reject",
    "Rejects",
    "RejectCode",
    "RejectScope",
    "Request",
    "Reservation",
    "StartResult",
    "policies",
]
