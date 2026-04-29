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

from .._openpit import (
    AccountAdjustmentBatchResult,
    ExecuteResult,
    PostTradeResult,
    PreTradeRequest,
    PreTradeReservation,
    Reject,
    StartPreTradeResult,
)
from .._openpit import (
    PreTradeLock as _PreTradeLock,
)
from ..param import Price
from . import policies
from ._enum import RejectCode, RejectScope
from .policy import (
    CheckPreTradeStartPolicy,
    PolicyDecision,
    PolicyReject,
    PreTradeContext,
    PreTradePolicy,
)

RejectCode.__module__ = __name__
RejectScope.__module__ = __name__


PreTradeContext.__doc__ = """
Opaque context object passed to Python policy callbacks.

The object identifies the current engine evaluation context. Treat it as
read-only. Future releases may expose additional query methods on this object;
policies should not create instances directly.
"""

ExecuteResult.__doc__ = """
Result of ``PreTradeRequest.execute`` or ``Engine.execute_pre_trade``.

Attributes:
    ok: ``True`` when all evaluated stages accepted the order.
    reservation: ``PreTradeReservation`` on success, otherwise ``None``.
    rejects: Ordered list of business rejects when ``ok`` is ``False``.

Successful results must be finalized by committing or rolling back the
reservation. Failed results never contain a reservation.
"""

PostTradeResult.__doc__ = """
Result of ``openpit.Engine.apply_execution_report``.

Attributes:
    kill_switch_triggered: ``True`` when at least one policy reports that new
        requests for the account should be blocked after processing the report.
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

PreTradeRequest.__doc__ = """
Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

The handle is single-use. Calling ``execute`` more than once raises
``RuntimeError``. Keep the handle only while the caller is ready to enter the
main stage; it represents a snapshot of the submitted order.
"""

PreTradeReservation.__doc__ = """
Single-use reservation handle returned by successful main-stage execution.

Call ``commit`` only after the order has been accepted by the downstream venue
or transport. Call ``rollback`` when submission fails or the caller decides not
to send the order. Calling either finalizer more than once raises
``RuntimeError``.
"""

StartPreTradeResult.__doc__ = """
Result of ``Engine.start_pre_trade``.

Attributes:
    ok: ``True`` when start-stage policies accepted the order.
    request: Deferred ``PreTradeRequest`` on success, otherwise ``None``.
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


class PreTradeLock(_PreTradeLock):
    """Pre-trade price lock payload."""

    # @typing.override
    def __new__(cls, *args: object, **kwargs: object) -> "PreTradeLock":
        return _PreTradeLock.__new__(cls, *args, **kwargs)

    # @typing.override
    def __init__(self, price: Price | None = None) -> None:
        if price is not None and not isinstance(price, Price):
            raise TypeError(f"price must be {Price.__module__}.{Price.__name__}")

    # @typing.override
    @property
    def price(self) -> Price | None:
        return _PreTradeLock.price.__get__(self, type(self))

    # @typing.override
    def __repr__(self) -> str:
        return f"PreTradeLock(price={self.price!r})"


__all__ = [
    "AccountAdjustmentBatchResult",
    "CheckPreTradeStartPolicy",
    "ExecuteResult",
    "PreTradePolicy",
    "PreTradeContext",
    "PolicyDecision",
    "PolicyReject",
    "PostTradeResult",
    "PreTradeLock",
    "Reject",
    "Rejects",
    "RejectCode",
    "RejectScope",
    "PreTradeRequest",
    "PreTradeReservation",
    "StartPreTradeResult",
    "policies",
]
