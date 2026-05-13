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

- :class:`Engine` - pre-trade risk engine.
- :mod:`~openpit.param` - typed financial values (Price, Pnl, Quantity, etc.)
  with exact decimal arithmetic.
- :mod:`~openpit.pretrade` - pluggable policy interfaces, standard reject
  codes, deferred requests, and reservations.
- :mod:`~openpit.core` - order, execution-report, and account-adjustment group models.

Quickstart::

    import openpit

    engine = (
        openpit.Engine.builder()
        .with_local_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
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
invoked it. The engine handle's threading capability depends on the sync policy
selected at builder time:

- `with_full_sync()` - concurrent invocation on the same handle is safe;
  sequential cross-thread invocation is also safe.
- `with_local_sync()` - the handle stays on the OS thread that created the
  engine.
- `with_account_sync()` - concurrent invocation on the same handle is safe when
  the caller pins each account to a single chain (one queue or one worker at a
  time), so calls for the same account are never concurrent.
"""

from contextlib import suppress

from . import core as core
from . import param, pretrade
from ._openpit import (
    Engine,
    EngineBuilder,
    ReadyEngineBuilder,
    RejectError,
    SyncedEngineBuilder,
)
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

EngineBuilder.__doc__ = """
First stage of the engine builder. Select a synchronization policy via
`with_full_sync()`, `with_local_sync()`, or `with_account_sync()`.

Prefer `with_local_sync()` when you do not explicitly work with multiple threads
yourself: it has zero synchronization overhead and is the right default for
embeddings that drive the engine from a single thread (synchronous code or one
asyncio loop). Use `with_full_sync()` when sharing the engine across threads
concurrently. Use `with_account_sync()` for sharded sequential workloads where
each account is pinned to one processing chain. Storage owned by built-in or
custom policies is always account-keyed regardless of sync mode.
"""

SyncedEngineBuilder.__doc__ = """
Second stage of the engine builder (sync policy already chosen). Add at least one
policy to obtain a :class:`ReadyEngineBuilder`.
"""

ReadyEngineBuilder.__doc__ = """
Third stage of the engine builder (at least one policy registered). Accepts additional
policies and builds the engine via ``build()``.

Policy names must be unique across start-stage and main-stage pre-trade policies.
"""

RejectError.__doc__ = """
Exception raised for Python binding misuse or callback-level failures.

Normal policy rejects are returned through result objects instead of raising
this exception.
"""


def _set_doc(obj, doc: str) -> None:
    with suppress(AttributeError, TypeError):
        obj.__doc__ = doc


_CHECK_PRE_TRADE_START_POLICY_DOC = """Register a start-stage policy.

Start policies run during ``Engine.start_pre_trade`` before the deferred
main-stage request exists. They return normal business rejects directly and
do not participate in main-stage rollback.
    """
_set_doc(
    SyncedEngineBuilder.check_pre_trade_start_policy, _CHECK_PRE_TRADE_START_POLICY_DOC
)
_set_doc(
    ReadyEngineBuilder.check_pre_trade_start_policy, _CHECK_PRE_TRADE_START_POLICY_DOC
)

_PRE_TRADE_POLICY_DOC = """Register a main-stage pre-trade policy.

Main-stage policies run when ``PreTradeRequest.execute`` is called. They may
return rejects and mutations; the engine rolls mutations back when the main
stage fails.
    """
_set_doc(SyncedEngineBuilder.pre_trade_policy, _PRE_TRADE_POLICY_DOC)
_set_doc(ReadyEngineBuilder.pre_trade_policy, _PRE_TRADE_POLICY_DOC)

_ACCOUNT_ADJUSTMENT_POLICY_DOC = """Register an account-adjustment policy.

Account-adjustment policies validate batches passed to
``Engine.apply_account_adjustment`` and may return rejects or rollback
mutations.
    """
_set_doc(SyncedEngineBuilder.account_adjustment_policy, _ACCOUNT_ADJUSTMENT_POLICY_DOC)
_set_doc(ReadyEngineBuilder.account_adjustment_policy, _ACCOUNT_ADJUSTMENT_POLICY_DOC)

_set_doc(
    ReadyEngineBuilder.build,
    """Build an engine from the registered policies.

Returns:
    Engine: Engine instance. Policy names must be unique across start-stage
    and main-stage pre-trade policies.
    """,
)
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

__all__ = [
    "Engine",
    "EngineBuilder",
    "SyncedEngineBuilder",
    "ReadyEngineBuilder",
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
