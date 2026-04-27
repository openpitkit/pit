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

ExecuteResult.__doc__ = """
Result of ``PreTradeRequest.execute`` or ``Engine.execute_pre_trade``.

This object reports whether main-stage policies accepted the request and, on
success, carries the single-use reservation handle that must later be committed
or rolled back.
"""

PostTradeResult.__doc__ = """
Result of ``openpit.Engine.apply_execution_report``.

It currently reports whether any policy considers an account-level kill switch
to be active after the report has been applied.
"""

Reject.__doc__ = """
Business reject returned by start-stage or main-stage checks.

Rejects are normal policy outcomes, not exceptional failures. They carry a
stable code, human-readable reason, details, policy name, scope, and opaque
caller-defined integer ``user_data`` token (``0`` when unset). The SDK never
inspects this token; lifetime and thread-safety are caller-managed (see
``pit.wiki/Threading-Contract.md``).
"""

PreTradeRequest.__doc__ = """
Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

The handle is single-use: calling ``execute`` more than once is a lifecycle
error.
"""

PreTradeReservation.__doc__ = """
Single-use reservation handle returned by successful main-stage execution.

Exactly one of ``commit`` or ``rollback`` must be called to finalize the
reserved state.
"""

StartPreTradeResult.__doc__ = """
Result of ``Engine.start_pre_trade``.

On success it exposes a deferred request handle; on failure it exposes the
merged reject list from all rejecting start-stage policies.
"""

AccountAdjustmentBatchResult.__doc__ = """
Result of ``Engine.apply_account_adjustment``.

This object reports whether the full batch passed atomically and, on failure,
contains the failing element index and reject list.
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
