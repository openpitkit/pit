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
    ExecuteResult,
    PostTradeResult,
    Reject,
    Request,
    Reservation,
    StartPreTradeResult,
)
from . import policies
from ._enum import MutationKind, RejectCode, RejectScope
from .policy import (
    CheckPreTradeStartPolicy,
    Mutation,
    Policy,
    PolicyContext,
    PolicyDecision,
    PolicyReject,
    RiskMutation,
)

MutationKind.__module__ = __name__
RejectCode.__module__ = __name__
RejectScope.__module__ = __name__

ExecuteResult.__doc__ = """
Result of ``Request.execute``.

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
stable code, human-readable reason, details, policy name, and scope.
"""

Request.__doc__ = """
Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

The handle is single-use: calling ``execute`` more than once is a lifecycle
error.
"""

Reservation.__doc__ = """
Single-use reservation handle returned by successful main-stage execution.

Exactly one of ``commit`` or ``rollback`` must be called to finalize the
reserved state.
"""

StartPreTradeResult.__doc__ = """
Result of ``Engine.start_pre_trade``.

On success it exposes a deferred request handle; on failure it exposes a
single business reject produced by the first rejecting start-stage policy.
"""

__all__ = [
    "CheckPreTradeStartPolicy",
    "ExecuteResult",
    "Mutation",
    "MutationKind",
    "Policy",
    "PolicyContext",
    "PolicyDecision",
    "PolicyReject",
    "PostTradeResult",
    "Reject",
    "RejectCode",
    "RejectScope",
    "Request",
    "Reservation",
    "RiskMutation",
    "StartPreTradeResult",
    "policies",
]
