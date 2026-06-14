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
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_int(99224416),
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

- `full_sync()` - concurrent invocation on the same handle is safe;
  sequential cross-thread invocation is also safe.
- `no_sync()` - the handle stays on the OS thread that created the
  engine.
- `account_sync()` - concurrent invocation on the same handle is safe when
  the caller pins each account to a single chain (one queue or one worker at a
  time), so calls for the same account are never concurrent.
"""

from contextlib import suppress

from . import core as core
from . import marketdata as marketdata
from . import param, pretrade
from ._openpit import (
    AccountBlockError,
    AccountGroupRegistrationError,
    Accounts,
    ConfigureErrorKind,
    Engine,
    EngineBuilder,
    PolicyConfigureError,
    ReadyEngineBuilder,
    RejectError,
    SyncedEngineBuilder,
)
from .core import (
    AccountAdjustment,
    AccountAdjustmentAmount,
    AccountAdjustmentBalanceOperation,
    AccountAdjustmentBounds,
    AccountAdjustmentContext,
    AccountAdjustmentPositionOperation,
    AccountControl,
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
`full_sync()`, `no_sync()`, or `account_sync()`.

Prefer `no_sync()` when you do not explicitly work with multiple threads
yourself: it has zero synchronization overhead and is the right default for
embeddings that drive the engine from a single thread (synchronous code or one
asyncio loop). Use `full_sync()` when sharing the engine across threads
concurrently. Use `account_sync()` for sharded sequential workloads where
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

Policy names must be unique within one engine configuration.
"""

RejectError.__doc__ = """
Exception raised for Python binding misuse or callback-level failures.

Normal policy rejects are returned through result objects instead of raising
this exception.
"""


def _set_doc(obj, doc: str) -> None:
    with suppress(AttributeError, TypeError):
        obj.__doc__ = doc


_PRE_TRADE_DOC = """Register a pre-trade policy.

Registered policies may implement start checks, main pre-trade checks,
post-trade feedback, and account-adjustment validation through
``openpit.pretrade.Policy``.
    """
_set_doc(SyncedEngineBuilder.pre_trade, _PRE_TRADE_DOC)
_set_doc(ReadyEngineBuilder.pre_trade, _PRE_TRADE_DOC)

_MARKET_DATA_DOC = """Create a market-data service builder.

The returned builder produces a shared ``openpit.marketdata.MarketDataService``
handle that can be passed to built-in policies and updated by quote producers.
    """
_set_doc(SyncedEngineBuilder.market_data, _MARKET_DATA_DOC)
_set_doc(ReadyEngineBuilder.market_data, _MARKET_DATA_DOC)

_set_doc(
    ReadyEngineBuilder.build,
    """Build an engine from the registered policies.

Returns:
    Engine: Engine instance. Policy names must be unique.
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
    openpit.pretrade.StartResult: Success result with a single-use
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
    openpit.pretrade.PostTradeResult: Carries any account blocks raised by
    policies and the per-asset ``account_adjustments`` outcomes produced
    while processing the report.
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
    Engine.accounts,
    """Return a handle to the engine's account controls.

Returns:
    Accounts: Handle for registering, unregistering, and reading account-group
    membership, plus account/group pre-trade blocks. The handle shares the
    engine's single account-control state, so changes are visible to every
    other handle and to running policies; it inherits the engine's
    synchronization mode.

Every account starts in :data:`openpit.param.DEFAULT_ACCOUNT_GROUP` and joins
another group only through ``register_group``.
    """,
)
_set_doc(
    Accounts.register_group,
    """Assign accounts to an account group.

Args:
    accounts: Iterable of :class:`openpit.param.AccountId` to place in the
        group.
    group: :class:`openpit.param.AccountGroupId` the accounts join. Must not be
        :data:`openpit.param.DEFAULT_ACCOUNT_GROUP`.

Raises:
    AccountGroupRegistrationError: If any listed account already belongs to a
        different group, or if ``group`` is the reserved default group. The
        registration is atomic: on failure no account is moved.
    """,
)
_set_doc(
    Accounts.unregister_group,
    """Remove accounts from an account group.

Args:
    accounts: Iterable of :class:`openpit.param.AccountId` to remove. Every
        listed account must currently belong to ``group``.
    group: :class:`openpit.param.AccountGroupId` the accounts leave. Must not be
        :data:`openpit.param.DEFAULT_ACCOUNT_GROUP`.

Raises:
    AccountGroupRegistrationError: If any listed account is not a member of
        ``group``, or if ``group`` is the reserved default group. The removal is
        atomic: on failure no account is moved. Removed accounts fall back to
        :data:`openpit.param.DEFAULT_ACCOUNT_GROUP`.
    """,
)
_set_doc(
    Accounts.group_of,
    """Return the group an account belongs to.

Args:
    account: :class:`openpit.param.AccountId` to look up.

Returns:
    openpit.param.AccountGroupId | None: The account's group, or ``None`` when
    the account has not been assigned to any group (its membership is the
    reserved :data:`openpit.param.DEFAULT_ACCOUNT_GROUP`).
    """,
)
_set_doc(
    Accounts.block,
    """Block an account, rejecting all subsequent pre-trade requests for it.

The first block reason wins: re-blocking an already-blocked account is a no-op
and does not overwrite the stored reason. Use :meth:`replace_block_reason` to
change it.

Args:
    account: :class:`openpit.param.AccountId` to block.
    reason: Human-readable reason stored with the block. Passed back in the
        reject detail on every gated order.
    """,
)
_set_doc(
    Accounts.unblock,
    """Unblock an account, lifting the block on all subsequent pre-trade requests.

Idempotent: a no-op when the account is not blocked.

Args:
    account: :class:`openpit.param.AccountId` to unblock.
    """,
)
_set_doc(
    Accounts.replace_block_reason,
    """Replace the stored reason of an already-blocked account.

Unlike :meth:`block`, which preserves the first reason, this overwrites it
while keeping the account blocked.

Args:
    account: :class:`openpit.param.AccountId` whose block reason to update.
    reason: New human-readable reason.

Raises:
    AccountBlockError: If the account is not currently blocked.
    """,
)
_set_doc(
    Accounts.block_group,
    """Block an account group, rejecting all subsequent pre-trade requests for
its members.

Group blocking is a live predicate: an account registered into ``group``
after the block takes effect is immediately gated; an account that leaves
``group`` is no longer group-blocked unless blocked individually.

Idempotent: the first block reason wins. Use
:meth:`replace_group_block_reason` to change it.

Args:
    group: :class:`openpit.param.AccountGroupId` to block. Must not be
        :data:`openpit.param.DEFAULT_ACCOUNT_GROUP`.
    reason: Human-readable reason stored with the block.

Raises:
    AccountBlockError: If ``group`` is the reserved default group.
    """,
)
_set_doc(
    Accounts.unblock_group,
    """Unblock an account group.

Idempotent: a no-op when the group is not blocked. Accounts blocked
individually remain blocked.

Args:
    group: :class:`openpit.param.AccountGroupId` to unblock. Must not be
        :data:`openpit.param.DEFAULT_ACCOUNT_GROUP`.

Raises:
    AccountBlockError: If ``group`` is the reserved default group.
    """,
)
_set_doc(
    Accounts.replace_group_block_reason,
    """Replace the stored reason of an already-blocked account group.

Unlike :meth:`block_group`, which preserves the first reason, this overwrites
it while keeping the group blocked.

Args:
    group: :class:`openpit.param.AccountGroupId` whose block reason to update.
        Must not be :data:`openpit.param.DEFAULT_ACCOUNT_GROUP`.
    reason: New human-readable reason.

Raises:
    AccountBlockError: If ``group`` is the reserved default group, or if
        ``group`` is not currently blocked.
    """,
)

__all__ = [
    "AccountBlockError",
    "AccountGroupRegistrationError",
    "Accounts",
    "ConfigureErrorKind",
    "Engine",
    "EngineBuilder",
    "PolicyConfigureError",
    "SyncedEngineBuilder",
    "ReadyEngineBuilder",
    "AccountAdjustment",
    "AccountAdjustmentAmount",
    "AccountAdjustmentBalanceOperation",
    "AccountAdjustmentBounds",
    "AccountAdjustmentContext",
    "AccountAdjustmentPositionOperation",
    "AccountControl",
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
    "marketdata",
    "param",
    "pretrade",
]
