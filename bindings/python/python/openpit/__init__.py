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

"""Embeddable pre-trade risk SDK for trading systems.

``openpit`` evaluates orders through a deterministic two-stage pipeline
before they leave the application. The package provides:

- :class:`Engine` — single-threaded pre-trade risk engine.
- :mod:`~openpit.param` — typed financial values (Price, Pnl, Quantity, etc.)
  with exact decimal arithmetic.
- :mod:`~openpit.pretrade` — pluggable policy interfaces, standard reject
  codes, deferred requests, and reservations.
- :mod:`~openpit.core` — order, execution-report, and account-adjustment group models.

Quickstart::

    import openpit

    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .build()
    )

    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(100.0),
            price=openpit.param.Price(185.0),
        ),
    )

    result = engine.start_pre_trade(order=order)

Threading:
The SDK never spawns OS threads: each public method runs on the OS thread that
invoked it. Concurrent invocation of public methods on the same engine handle
is undefined behavior and must be prevented by the caller. Sequential calls on
the same handle from different OS threads are supported by the SDK contract.
"""

from contextlib import suppress

from . import core as core
from . import param, pretrade
from ._openpit import Engine, EngineBuilder, RejectError
from .account_adjustment import AccountAdjustmentPolicy
from .core import (
    AccountAdjustment,
    AccountAdjustmentAmount,
    AccountAdjustmentBalanceOperation,
    AccountAdjustmentBounds,
    AccountAdjustmentContext,
    AccountAdjustmentPositionOperation,
    ExecutionReport,
    ExecutionReportFillDetails,
    ExecutionReportOperation,
    ExecutionReportPositionImpact,
    FinancialImpact,
    Instrument,
    Mutation,
    Order,
    OrderMargin,
    OrderOperation,
    OrderPosition,
)
from .param import AdjustmentAmount, Leverage, PositionMode
from .pretrade import PostTradeResult

Engine.__doc__ = """
Single-threaded pre-trade risk engine.

The engine evaluates orders through an explicit two-stage pipeline and accepts
post-trade execution reports to update cumulative policy state.

Snapshot semantics:
Inputs passed to ``start_pre_trade``, ``apply_execution_report``, and
``apply_account_adjustment`` are snapshotted at call time for evaluation.
Mutating the same objects after submission does not affect
the in-flight engine operation.
"""

EngineBuilder.__doc__ = """
Builder used to register start-stage and main-stage policies before engine creation.

Policy names must be unique across both stages in a single engine instance.
"""

RejectError.__doc__ = """
Exception raised for Python binding misuse or callback-level failures.

Normal policy rejects are returned through result objects instead of raising
this exception.
"""


def _set_doc(obj, doc: str) -> None:
    with suppress(AttributeError, TypeError):
        obj.__doc__ = doc


_set_doc(
    Engine.builder,
    """Create a new :class:`EngineBuilder`.

Returns:
    EngineBuilder: Mutable builder used to register policies before creating
    an immutable engine instance.
    """,
)
_set_doc(
    Engine.start_pre_trade,
    """Run the start stage for one order.

Args:
    order: :class:`openpit.Order` or subclass. The engine snapshots the
        current order groups before invoking policies.

Returns:
    openpit.pretrade.StartPreTradeResult: Success result with a single-use
    request handle, or failure result with one or more business rejects.

Raises:
    TypeError: If ``order`` is not an order object.
    RejectError: If a Python policy callback fails unexpectedly.
    """,
)
_set_doc(
    Engine.execute_pre_trade,
    """Run start stage and main stage as one convenience call.

Args:
    order: :class:`openpit.Order` or subclass.

Returns:
    openpit.pretrade.ExecuteResult: Success result with a reservation, or
    failure result with start-stage or main-stage rejects.

The returned reservation is still explicit: callers must call
``commit()`` after external order submission succeeds or ``rollback()``
otherwise.
    """,
)
_set_doc(
    Engine.apply_execution_report,
    """Apply post-trade feedback to all registered policies.

Args:
    report: :class:`openpit.ExecutionReport` or subclass.

Returns:
    openpit.pretrade.PostTradeResult: Reports whether any policy considers
    an account-level kill switch active after processing the report.
    """,
)
_set_doc(
    Engine.apply_account_adjustment,
    """Validate and apply a batch of non-trading account adjustments.

Args:
    account_id: :class:`openpit.param.AccountId` identifying the account.
    adjustments: Iterable of :class:`openpit.AccountAdjustment` objects.

Returns:
    openpit.pretrade.AccountAdjustmentBatchResult: Batch outcome. Failed
    results expose the first failing index and reject list.

The batch is atomic from the policy contract perspective: mutation rollbacks
are invoked when a later policy or adjustment rejects.
    """,
)
_set_doc(
    EngineBuilder.check_pre_trade_start_policy,
    """Register a start-stage policy.

Start policies run during ``Engine.start_pre_trade`` before the deferred
main-stage request exists. They return normal business rejects directly and
do not participate in main-stage rollback.
    """,
)
_set_doc(
    EngineBuilder.pre_trade_policy,
    """Register a main-stage pre-trade policy.

Main-stage policies run when ``PreTradeRequest.execute`` is called. They may
return rejects and mutations; the engine rolls mutations back when the main
stage fails.
    """,
)
_set_doc(
    EngineBuilder.account_adjustment_policy,
    """Register an account-adjustment policy.

Account-adjustment policies validate batches passed to
``Engine.apply_account_adjustment`` and may return rejects or rollback
mutations.
    """,
)
_set_doc(
    EngineBuilder.build,
    """Build an engine from the registered policies.

Returns:
    Engine: Single-threaded engine instance. Policy names must be unique
    across start-stage and main-stage pre-trade policies.
    """,
)

__all__ = [
    "Engine",
    "EngineBuilder",
    "AccountAdjustment",
    "AccountAdjustmentAmount",
    "AccountAdjustmentBalanceOperation",
    "AccountAdjustmentBounds",
    "AccountAdjustmentContext",
    "AccountAdjustmentPositionOperation",
    "AccountAdjustmentPolicy",
    "AdjustmentAmount",
    "ExecutionReport",
    "ExecutionReportFillDetails",
    "ExecutionReportOperation",
    "ExecutionReportPositionImpact",
    "FinancialImpact",
    "Instrument",
    "Leverage",
    "Mutation",
    "Order",
    "OrderMargin",
    "OrderOperation",
    "OrderPosition",
    "PostTradeResult",
    "PositionMode",
    "RejectError",
    "param",
    "pretrade",
]
