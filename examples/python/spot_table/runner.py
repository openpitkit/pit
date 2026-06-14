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

"""Sequential table runner for the spot_table example.

Executes a parsed scenario against a single ``no_sync`` engine, operation by
operation in row order. TICK rows are replayed live at their row position; the
run stops at the first verdict mismatch and returns a partial report.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field

import builder
import openpit
from market_feed import MarketFeed
from table import Frontmatter, Row

# Mode names the execution strategy of a runner. This example only ships the
# single-threaded sequential strategy.
MODE_SYNC = "sync"


# ---------------------------------------------------------------------------
# Reports and statistics
# ---------------------------------------------------------------------------


@dataclass
class LatencyStats:
    """Latency samples for one operation kind."""

    count: int = 0
    total_ns: int = 0
    min_ns: int = 0
    max_ns: int = 0

    def observe(self, ns: int) -> None:
        """Fold one latency sample into the running statistics."""
        self.count += 1
        self.total_ns += ns
        if self.count == 1 or ns < self.min_ns:
            self.min_ns = ns
        if ns > self.max_ns:
            self.max_ns = ns

    def avg_ns(self) -> int:
        """Return the mean latency in nanoseconds."""
        if self.count == 0:
            return 0
        return self.total_ns // self.count

    def merge(self, other: LatencyStats) -> None:
        """Fold another sample set into self, for aggregating repeat iterations."""
        if other.count == 0:
            return
        if self.count == 0 or other.min_ns < self.min_ns:
            self.min_ns = other.min_ns
        if other.max_ns > self.max_ns:
            self.max_ns = other.max_ns
        self.count += other.count
        self.total_ns += other.total_ns


@dataclass
class Failure:
    """The first mismatch or runtime error seen during a run."""

    row: Row
    message: str


@dataclass
class Report:
    """The per-run outcome of executing a table."""

    mode: str
    accounts: dict[str, int] = field(default_factory=dict)
    total: int = 0  # executable rows (SEED/GROUP/ORDER/FILL; excludes TICK)
    wall_clock_ns: int = 0
    order: LatencyStats = field(default_factory=LatencyStats)
    fill: LatencyStats = field(default_factory=LatencyStats)
    first_fail: Failure | None = None

    def accounts_count(self) -> int:
        """Return the number of distinct accounts touched."""
        return len(self.accounts)


@dataclass
class EngineAggregate:
    """Accumulates one engine's statistics across repeat iterations."""

    mode: str = MODE_SYNC
    accounts: int = 0
    ops: int = 0
    order: LatencyStats = field(default_factory=LatencyStats)
    fill: LatencyStats = field(default_factory=LatencyStats)

    def add(self, report: Report | None) -> None:
        """Fold one iteration's report into the aggregate."""
        if report is None:
            return
        self.mode = report.mode
        self.accounts = report.accounts_count()
        self.ops += report.total
        self.order.merge(report.order)
        self.fill.merge(report.fill)


# ---------------------------------------------------------------------------
# Reject-code resolution
# ---------------------------------------------------------------------------

# Case-insensitive map from the table's `reject` column to the reject codes the
# runner recognizes.
_RECOGNIZED = {
    c.value.lower(): c
    for c in (
        openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD,
        openpit.pretrade.RejectCode.INVALID_FIELD_FORMAT,
        openpit.pretrade.RejectCode.INVALID_FIELD_VALUE,
        openpit.pretrade.RejectCode.UNSUPPORTED_ORDER_TYPE,
        openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS,
        openpit.pretrade.RejectCode.INSUFFICIENT_MARGIN,
        openpit.pretrade.RejectCode.INSUFFICIENT_POSITION,
        openpit.pretrade.RejectCode.MARK_PRICE_UNAVAILABLE,
        openpit.pretrade.RejectCode.ORDER_VALUE_CALCULATION_FAILED,
        openpit.pretrade.RejectCode.ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED,
    )
}


def resolve_code(name: str) -> openpit.pretrade.RejectCode | None:
    """Resolve a table reject label to a recognized code, case-insensitively."""
    return _RECOGNIZED.get(name.strip().lower())


def contains_code(
    rejects: list[openpit.pretrade.Reject], want: openpit.pretrade.RejectCode
) -> bool:
    """Report whether any reject carries the wanted code."""
    # Reject.code is the CamelCase string; RejectCode is a StrEnum that compares
    # equal to its own value, so the string-vs-enum check is exact.
    return any(r.code == want for r in rejects)


def describe_rejects(rejects: list[openpit.pretrade.Reject]) -> str:
    """Render the rejects as a sorted, comma-separated list of code strings."""
    return ",".join(sorted(r.code for r in rejects))


# ---------------------------------------------------------------------------
# Engine build
# ---------------------------------------------------------------------------


def build_spot_engine_sync(
    fm: Frontmatter, rows: list[Row]
) -> tuple[openpit.Engine, MarketFeed]:
    """Build the sequential engine: single-thread ``no_sync`` with the spot
    funds policy reading a ``no_sync`` market-data service.

    The returned feed owns the instrument registry; its instruments are
    registered up front so live TICK pushes resolve.
    """
    builder_ = openpit.Engine.builder().no_sync()
    service = (
        builder_.market_data(openpit.marketdata.QuoteTtl.infinite()).no_sync().build()
    )
    feed = MarketFeed(service)
    feed.register_instruments(rows)
    engine = builder_.builtin(
        openpit.pretrade.policies.build_spot_funds().market_data(
            service,
            global_slippage_bps=fm.slippage_bps,
            pricing_source=openpit.pretrade.policies.SpotFundsPricingSource.MARK,
        )
    ).build()
    return engine, feed


# ---------------------------------------------------------------------------
# Group membership
# ---------------------------------------------------------------------------


@dataclass
class GroupMembership:
    """Every GROUP row aggregated into the per-group account sets.

    Row order is preserved so registration is deterministic; each GROUP row is
    retained both for diagnostics and so the report's account counts are stable.
    """

    order: list[str] = field(default_factory=list)
    members: dict[str, list[openpit.param.AccountId]] = field(default_factory=dict)
    rows: list[Row] = field(default_factory=list)

    def first_row(self, label: str) -> Row:
        """Return the first GROUP row that named *label*, to anchor a failure."""
        for row in self.rows:
            if row.group == label:
                return row
        return Row()

    def count_in_report(self, report: Report) -> None:
        """Record every GROUP row toward the report's totals."""
        for row in self.rows:
            report.total += 1
            report.accounts[row.account] = report.accounts.get(row.account, 0) + 1


def collect_groups(rows: list[Row]) -> tuple[GroupMembership | None, Failure | None]:
    """Aggregate every GROUP row into a GroupMembership, preserving row order."""
    groups = GroupMembership()
    for row in rows:
        if row.action != "GROUP":
            continue
        try:
            acc = builder.account_id(row.account)
        except ValueError as exc:
            return None, Failure(row, str(exc))
        if row.group not in groups.members:
            groups.order.append(row.group)
        groups.members.setdefault(row.group, []).append(acc)
        groups.rows.append(row)
    return groups, None


def register_groups_sync(
    engine: openpit.Engine, groups: GroupMembership, report: Report
) -> Failure | None:
    """Register every aggregated GROUP membership before any dependent row runs.

    Counts each GROUP row toward the report; a registration failure is reported
    against the group's first row.
    """
    groups.count_in_report(report)
    accounts_view = engine.accounts()
    for label in groups.order:
        try:
            group_id = builder.account_group_id(label)
        except ValueError as exc:
            return Failure(groups.first_row(label), str(exc))
        try:
            accounts_view.register_group(groups.members[label], group_id)
        except Exception as exc:  # engine errors become verdict failures, not raises
            return Failure(groups.first_row(label), "register group: " + str(exc))
    return None


# ---------------------------------------------------------------------------
# Run loop
# ---------------------------------------------------------------------------


def run_sync(fm: Frontmatter, rows: list[Row], deadline_ns: int | None) -> Report:
    """Execute the table sequentially on a ``no_sync`` engine.

    TICK rows are replayed live at their row position. Stops at the first
    verdict mismatch and returns a partial report. *deadline_ns* is an optional
    ``perf_counter_ns`` budget; when reached the loop breaks early.
    """
    engine, feed = build_spot_engine_sync(fm, rows)

    report = Report(mode=MODE_SYNC)

    groups, fail = collect_groups(rows)
    if fail is not None or groups is None:
        report.first_fail = fail
        return report
    fail = register_groups_sync(engine, groups, report)
    if fail is not None:
        report.first_fail = fail
        return report

    start = time.perf_counter_ns()
    for row in rows:
        if deadline_ns is not None and time.perf_counter_ns() >= deadline_ns:
            break
        if row.action == "GROUP":
            # Registered up front in register_groups_sync.
            continue
        if row.action == "TICK":
            fail = run_sync_tick(feed, row)
            if fail is not None:
                report.first_fail = fail
                break
            continue
        try:
            acc = builder.account_id(row.account)
        except ValueError as exc:
            report.first_fail = Failure(row, str(exc))
            break
        report.total += 1
        report.accounts[row.account] = report.accounts.get(row.account, 0) + 1

        if row.action == "SEED":
            fail = run_sync_seed(engine, acc, row)
            if fail is not None:
                report.first_fail = fail
                break
        elif row.action == "ORDER":
            fail, dur = run_sync_order(engine, acc, row)
            report.order.observe(dur)
            if fail is not None:
                report.first_fail = fail
                break
        elif row.action == "FILL":
            fail, dur = run_sync_fill(engine, acc, row, feed)
            report.fill.observe(dur)
            if fail is not None:
                report.first_fail = fail
                break

    report.wall_clock_ns = time.perf_counter_ns() - start
    return report


# ---------------------------------------------------------------------------
# Per-row execution
# ---------------------------------------------------------------------------


def run_sync_tick(feed: MarketFeed, row: Row) -> Failure | None:
    """Replay one TICK row against the feed."""
    try:
        push_tick(feed, row)
    except ValueError as exc:
        return Failure(row, str(exc))
    return None


def push_tick(feed: MarketFeed, row: Row) -> None:
    """Replay one TICK row: a global push when neither account nor group is
    set, otherwise an addressed push to the named account and/or group."""
    if row.account == "" and row.group == "":
        feed.push(row.instrument, row.price)
        return
    accounts: list[openpit.param.AccountId] = []
    if row.account:
        accounts.append(builder.account_id(row.account))
    groups: list[openpit.param.AccountGroupId] = []
    if row.group:
        groups.append(builder.account_group_id(row.group))
    feed.push_for(row.instrument, row.price, accounts, groups)


def run_sync_seed(
    engine: openpit.Engine, acc: openpit.param.AccountId, row: Row
) -> Failure | None:
    """Apply one SEED row and score its verdict."""
    try:
        adj = builder.build_seed_adjustment(row)
    except ValueError as exc:
        return Failure(row, str(exc))
    try:
        result = engine.apply_account_adjustment(account_id=acc, adjustments=[adj])
    except Exception as exc:  # engine errors become verdict failures, not raises
        return Failure(row, "engine: " + str(exc))
    return check_seed_verdict(row, rejected=not result.ok)


def run_sync_order(
    engine: openpit.Engine, acc: openpit.param.AccountId, row: Row
) -> tuple[Failure | None, int]:
    """Run one ORDER row through pre-trade, finalize the reservation, and time it."""
    try:
        order = builder.build_order(row, acc)
    except ValueError as exc:
        return Failure(row, str(exc)), 0
    start = time.perf_counter_ns()
    try:
        result = engine.execute_pre_trade(order=order)
    except Exception as exc:  # engine errors become verdict failures, not raises
        return Failure(row, "engine: " + str(exc)), time.perf_counter_ns() - start
    dur = time.perf_counter_ns() - start
    fail = check_order_verdict(row, result)
    res = result.reservation
    if res is not None:
        if fail is None:
            res.commit()
        else:
            res.rollback()
    return fail, dur


def run_sync_fill(
    engine: openpit.Engine,
    acc: openpit.param.AccountId,
    row: Row,
    feed: MarketFeed,
) -> tuple[Failure | None, int]:
    """Apply one FILL row's execution report and time it."""
    try:
        report = builder.build_fill_report(row, acc, feed)
    except ValueError as exc:
        return Failure(row, str(exc)), 0
    start = time.perf_counter_ns()
    try:
        result = engine.apply_execution_report(report=report)
    except Exception as exc:  # engine errors become verdict failures, not raises
        return Failure(row, "engine: " + str(exc)), time.perf_counter_ns() - start
    dur = time.perf_counter_ns() - start
    return check_fill_verdict(row, blocked=len(result.account_blocks) > 0), dur


# ---------------------------------------------------------------------------
# Verdict checks
# ---------------------------------------------------------------------------


def check_order_verdict(row: Row, result: openpit.ExecuteResult) -> Failure | None:
    """Compare the engine's rejects against the row's expectation."""
    if row.expect == "ACCEPT":
        if not result.ok:
            return Failure(
                row,
                f"expected ACCEPT, got REJECT({describe_rejects(result.rejects)})",
            )
    elif row.expect == "REJECT":
        if result.ok:
            return Failure(row, "expected REJECT, got ACCEPT")
        if row.reject:
            want = resolve_code(row.reject)
            if want is None:
                return Failure(row, f"unknown reject code {row.reject!r} in table")
            if not contains_code(result.rejects, want):
                return Failure(
                    row,
                    f"expected REJECT({row.reject}),"
                    f" got REJECT({describe_rejects(result.rejects)})",
                )
    else:
        return Failure(row, f"ORDER row must use ACCEPT/REJECT, got {row.expect}")
    return None


def check_seed_verdict(row: Row, rejected: bool) -> Failure | None:
    """Compare a SEED outcome against the row's expected verdict."""
    if row.expect == "OK":
        if rejected:
            return Failure(row, "expected OK, SEED rejected")
    elif row.expect == "REJECT":
        if not rejected:
            return Failure(row, "expected REJECT, SEED accepted")
    else:
        return seed_fill_verdict_error(row)
    return None


def check_fill_verdict(row: Row, blocked: bool) -> Failure | None:
    """Compare a FILL outcome against the row's expected verdict."""
    if row.expect == "OK":
        if blocked:
            return Failure(row, "expected OK, got account block")
    elif row.expect == "REJECT":
        if not blocked:
            return Failure(row, "expected REJECT, FILL produced no block")
    else:
        return seed_fill_verdict_error(row)
    return None


def seed_fill_verdict_error(row: Row) -> Failure:
    """Report an expectation that SEED/FILL cannot honor (ACCEPT is ORDER-only)."""
    if row.expect == "ACCEPT":
        return Failure(
            row,
            f"{row.action} row cannot use ACCEPT (ORDER-only); use OK or REJECT",
        )
    return Failure(row, f"{row.action} row must use OK/REJECT, got {row.expect}")
