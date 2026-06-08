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
    ExecuteResult,
    OutcomeAmount,
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

Post-trade processing is not atomic: ``account_adjustments`` reflect storage
mutations that have already been applied, so callers must propagate them
downstream even when ``account_blocks`` is non-empty.

Attributes:
    account_blocks: List of account blocks reported by policies. Non-empty
        when at least one policy entered a blocked state after the report.
    account_adjustments: List of per-asset position outcomes reported by
        policies, in policy registration order.
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
    "DEFAULT_POLICY_GROUP_ID",
    "ExecuteResult",
    "PolicyGroupId",
    "OutcomeAmount",
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
