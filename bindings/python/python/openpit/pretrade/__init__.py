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

"""Pre-trade policies, rejects, requests, and reservations."""

from __future__ import annotations

import typing
from collections.abc import Iterable  # noqa: F401  (used in string annotations)

from .._openpit import (
    _DEFAULT_POLICY_GROUP_ID,
    AccountAdjustmentBatchResult,
    AccountAdjustmentOutcome,
    AccountBlock,
    AccountBlockOutcome,
    AccountBlockOutcomes,
    AccountControl,
    AccountOutcomeEntry,
    AccountPnlOutcome,
    DropCopyOperation,
    DropCopyResult,
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
from ._enum import RejectCode, RejectScope, RuleMode
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
RuleMode.__module__ = __name__


Context.__doc__ = """
Opaque context object passed to Python policy callbacks.

The object identifies the current engine evaluation context. Treat it as
read-only and do not create instances directly. ``is_drop_copy`` reports
whether ordinary rejects are non-enforcing for the current operation.

``record_drop_copy_start_mutation(mutation)`` registers bookkeeping applied
tentatively during the start hook with the current drop-copy operation. Commit
finalizes that state when the caller commits the operation. Rollback must undo
it on an evaluation or callback failure and when the caller rolls the operation
back, including when its commit callback was not reached.

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
    ok: ``True`` when the evaluated stages produced no effective rejects.
        Same as ``bool(result)``. Check ``reviews`` before submission.
    reservation: ``Reservation`` on success, otherwise ``None``.
    rejects: Ordered list of business rejects when ``ok`` is ``False``.
    reviews: Ordered review marks. A non-empty list requires a human decision
        before submission. See ``Reservation`` for the review lifecycle.

If there are review marks, commit the reservation immediately and keep the
order unsent. The reservation keeps holds and policy results only if every
review mark is with-hold.
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

When failures coincide, SpotFunds uses this priority from highest to lowest:
``ARITHMETIC_OVERFLOW``, ``MISSING_ACCOUNT_CURRENCY``, ``MISSING_FX``,
``MISSING_COST_BASIS``, then ``MISSING_INITIAL_PNL``.

``MISSING_INITIAL_PNL`` is the lowest-priority defensive catch-all when no
more specific missing input applies.
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
Reject or review mark returned by start-stage or main-stage checks.

Rejects are normal policy outcomes, not exceptional failures.

Attributes:
    code: Stable machine-readable reject code.
    reason: Short human-readable reason.
    details: Diagnostic text intended for logs and operators.
    policy: Name of the policy that produced the reject.
    scope: ``RejectScope.ORDER`` or ``RejectScope.ACCOUNT``.
    mode: ``RuleMode`` for the pre-trade start and main stages. Both stages
        report evaluation failures as ``RuleMode.REJECT``. Any
        account-adjustment entry rejects the batch and is reported as
        ``RuleMode.REJECT``. Drop copy applies bookkeeping in every mode,
        aborts on evaluation failures, and omits review marks and their
        account blocks.
    user_data: Opaque caller-defined integer token copied from custom rejects.
"""

Request.__doc__ = """
Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

The handle is single-use. Calling ``execute`` more than once raises
``RuntimeError``. Keep the handle only while the caller is ready to enter the
main stage; it represents a snapshot of the submitted order.

``reviews`` contains start-stage review marks and raises ``RuntimeError``
after execution. Execute the request and inspect the result's ``reviews``
before acting on the order.
"""

Reservation.__doc__ = """
Single-use reservation handle returned by successful main-stage execution.

When ``reviews`` is empty, call ``commit`` after the downstream venue or
transport accepts the order. Call ``rollback`` if submission fails or the
order will not be sent. Calling either finalizer more than once raises
``RuntimeError``. Read the accessors before finalizing the reservation.

If ``reviews`` is non-empty, leave the order unsent and call ``commit``
immediately. The reservation retains holds only when every mark is
``RuleMode.REVIEW_WITH_HOLD``.
A policy instance registered in a review mode that marked the order in its
main-stage check retains none of its own. Save the first run's ``lock`` before
committing.
``already_held`` asserts Spot Funds' own hold. Spot Funds kept it only when
every mark of that first run is with-hold and no Spot Funds instance registered
in a review mode marked the order. On approval, rerun with ``confirmed=True``,
and also set ``already_held=True`` only if Spot Funds kept its hold. Otherwise
the rerun carries ``confirmed=True`` only.
With ``already_held=True``, Spot Funds runs every check against the current
balance with this order's own hold given back, writes nothing, and adds no
prices to the rerun's lock. For a market order, this run prices the hold it
gives back, so a price move since the first run shifts that amount. Keep a clean
rerun's reservation until sending, and commit once the venue accepts the order.
If it is not sent or the venue does not accept it, call ``rollback`` and, if
Spot Funds kept its hold in the review, release that hold as described below.

Once an already-held rerun is committed, for each report of the order use the
rerun's lock supplemented with the first run's Spot Funds prices. Read each Spot
Funds group's prices with ``Lock.prices_of`` and append them with
``Lock.push_many``. Merging the whole first lock would duplicate prices for
groups recorded in both runs. If Spot Funds is the only policy that locks
prices, use the first lock directly. Passing only the rerun's lock blocks the
account with ``RejectCode.MISSING_REQUIRED_FIELD``.

Once a review in which Spot Funds kept its hold is committed, only a final
cancel execution report carrying the remaining reserved quantity releases that
hold. Send it whenever the order ends without a committed rerun: the reviewer
rejects it, the rerun rejects it, or the rerun is rolled back because the order
is not sent or the venue does not accept it.
That report carries the first run's lock.
Rolling back or rejecting the rerun does not touch the first run's hold.
A without-hold review of a first run holds nothing and needs no cancel report.
When a Spot Funds instance registered in a review mode marked the order in a
first run, that instance holds nothing, and a final cancel report that carries
the first run's lock and a remaining reserved quantity blocks the account with
``RejectCode.MISSING_REQUIRED_FIELD``, because that lock has no price for that
instance.
A review of an already-held rerun changes no Spot Funds hold: whatever its
marks, the hold the first review kept stays. On approval, rerun with
``confirmed=True`` and ``already_held=True`` again; if the order ends without a
committed approval rerun, release that hold with a final cancel report carrying
the remaining reserved quantity and the first run's Spot Funds lock prices.
The engine trusts ``already_held`` without checking whether a matching hold
exists. Setting it when Spot Funds kept no hold leaves the order without a Spot
Funds hold.

Attributes:
    lock: Current reservation ``Lock`` payload.
    account_adjustments: List of ``AccountAdjustmentOutcome`` objects captured
        by the reservation.
    reviews: Marks from the evaluated stages requiring a human decision.
"""

DropCopyOperation.__doc__ = """
Single-use handle for the bookkeeping applied by ``Engine.apply_drop_copy``.

Call ``commit`` once the historical order is stored, or ``rollback`` when
storing it failed and the applied bookkeeping must be reverted. Calling either
finalizer more than once raises ``RuntimeError``, and garbage collection of an
unfinalized operation rolls it back. Account-control operations published by
the drop-copy pipeline and consumed rate-limit attempts stay outside that
boundary. Every accessor below raises ``RuntimeError`` after the operation is
finalized, so read them before committing or rolling back.

Attributes:
    lock: ``Lock`` context produced by the applied request.
    account_adjustments: List of ``AccountAdjustmentOutcome`` objects applied
        by the request.
    account_block: First ``AccountBlock`` requested by this operation, or
        ``None``. This is request-local history; ``is_account_blocked``
        reports the apply-time registry snapshot captured before
        ``apply_drop_copy`` returned.
    is_account_blocked: Apply-time blocked-state snapshot for the order
        account. It does not track later registry changes.
"""

DropCopyResult.__doc__ = """
Result of ``Engine.apply_drop_copy``.

Attributes:
    ok: ``True`` when the historical order was applied.
    operation: ``DropCopyOperation`` on success, otherwise ``None``.
    rejects: Fatal evaluation rejects when ``ok`` is ``False``.

Successful results must be finalized by committing or rolling back the
operation. Failed results never contain an operation.
"""

StartResult.__doc__ = """
Result of ``Engine.start_pre_trade``.

Attributes:
    ok: ``True`` when the start stage produced no effective rejects. Same as
        ``bool(result)``. Check ``reviews`` before submission.
    request: Deferred ``Request`` on success, otherwise ``None``.
    rejects: Merged start-stage reject list when ``ok`` is ``False``.
    reviews: Start-stage review marks. A non-empty list requires a human
        decision before submission. Execute the request to get the final
        verdict and inspect the executed reservation's ``reviews``.
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
    account_blocks: Policy-reported blocks for the account supplied to the
        operation. A SpotFunds account-PnL force-set exposes its breach or halt
        block even when the account already has a block. The engine processes
        each block request before returning and preserves the existing first
        cause.
"""

DryRunReport.__doc__ = """
Inert verdict of a non-mutating pre-trade dry-run.

Describes what ``Engine.execute_pre_trade`` or ``Engine.start_pre_trade``
would have produced for the same order and engine state, without moving any
engine state: no rate-limit budget is spent, no reservation or hold is
applied, and no account is blocked.

Attributes:
    is_pass: ``True`` when the evaluated stages would produce no effective
        rejects. Same as ``bool(report)``. Check ``reviews`` before submission.
    rejects: List of ``Reject`` objects when ``is_pass`` is ``False``,
        otherwise ``None``.
    reviews: Review marks that a live run would return. A non-empty list
        requires a human decision before submission. In a live run, follow
        the lifecycle described by ``Reservation``.
    lock: ``Lock`` the main stage would have produced. Empty when the start
        stage would have rejected or no policy locks a price.
    account_adjustments: List of ``AccountAdjustmentOutcome`` objects the
        main stage would have produced. Empty when the start stage would have
        rejected or no policy reports an adjustment.
    account_block: ``AccountBlock`` that a real call would have recorded in
        the engine's blocked-accounts registry, or ``None`` when the order
        would have passed or produced a non-account reject.
"""

Rejects = list[Reject]


class Lock(_Lock):
    """Pre-trade price lock grouping `(policy_group_id, price)` pairs."""

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
    "AccountBlockOutcome",
    "AccountBlockOutcomes",
    "AccountControl",
    "AccountOutcomeEntry",
    "AccountPnlOutcome",
    "DEFAULT_POLICY_GROUP_ID",
    "DropCopyOperation",
    "DropCopyResult",
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
    "RuleMode",
    "Request",
    "Reservation",
    "StartResult",
    "policies",
]
