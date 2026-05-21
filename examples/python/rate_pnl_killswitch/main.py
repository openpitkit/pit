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

"""Example rate_pnl_killswitch.

Demonstrates how an algorithmic trading desk can wrap OpenPit's RateLimit and
PnlBoundsKillSwitch policies around a Python strategy so that a runaway
strategy is halted before it floods the venue with orders or burns through the
loss budget.

What is illustrated:

- building an engine with two killswitch policies side-by-side
- feeding the engine via a single Event stream (orders + fills)
- separating venue/strategy side-effects behind a Reactor protocol
- aggregating accepted/rejected counts, pre-trade latency, and cumulative
  P&L over the run

Audience: an algo trader who wants an independent supervisor that prevents the
strategy from "going crazy".

What you typically change to adapt this example to your own application:

1. Engine policies and limits - see ``build_engine`` below.
2. The order/report stream - the ``scenario_stream`` generator in ``main()``
   is a one-shot replay; real systems plug in a coroutine driven by venue
   and strategy events.
3. The reactor implementation - replace ``LoggingReactor`` with code that
   actually submits orders to the venue, updates your strategy book, and
   halts the strategy when ``account_blocks`` is non-empty.
"""

from __future__ import annotations

import datetime
import sys
import time
from collections.abc import Iterable, Iterator
from dataclasses import dataclass, field
from decimal import Decimal
from typing import Protocol

import openpit
from openpit import pretrade

# =============================================================================
# Section 1 - public extension points.
# The two event types, the Reactor protocol, and the Stats dataclass are the
# only things application code interacts with. ``run`` below is
# policy-agnostic; it only knows these types.
# =============================================================================


@dataclass(frozen=True)
class OrderEvent:
    """A strategy-emitted order intent waiting for pre-trade evaluation."""

    order: openpit.Order


@dataclass(frozen=True)
class ReportEvent:
    """A venue-emitted execution report. ``realized_pnl`` mirrors the value
    stored in ``report.financial_impact.pnl`` so the example can track the
    running balance outside the engine - production code would read it from
    its strategy book instead."""

    report: openpit.ExecutionReport
    realized_pnl: Decimal


Event = OrderEvent | ReportEvent


class Reactor(Protocol):
    """Engine-verdict consumer. Plug your venue client and strategy book here."""

    def on_accepted(self, order: openpit.Order) -> None:
        """Pre-trade has reserved and committed the order. Send it to the venue."""

    def on_rejected(self, order: openpit.Order, rejects: list[pretrade.Reject]) -> None:
        """A policy refused the order. Inspect ``rejects[i].code`` to choose
        between retry / throttle / escalate."""

    def on_report(
        self,
        report: openpit.ExecutionReport,
        result: openpit.PostTradeResult,
    ) -> None:
        """The engine consumed a venue execution report. When
        ``result.account_blocks`` is non-empty when the strategy must stop
        sending orders for this account until operators clear the state."""


@dataclass
class Stats:
    """Timing and trading outcomes over a run."""

    accepted: int = 0
    rejected: int = 0
    pre_trade_calls: int = 0
    reports: int = 0
    kill_switch: bool = False
    kill_switch_on_trade: int = 0  # 1-based index of the tripping report
    pnl: Decimal = field(default_factory=lambda: Decimal("0"))
    total_pre_trade_ns: int = 0
    min_pre_trade_ns: int = 0
    max_pre_trade_ns: int = 0

    @property
    def avg_pre_trade_ns(self) -> int:
        if self.pre_trade_calls == 0:
            return 0
        return self.total_pre_trade_ns // self.pre_trade_calls


# =============================================================================
# Section 2 - engine wiring.
# The two killswitch policies and the engine builder. Tune the limits to your
# risk tolerance.
# =============================================================================


@dataclass(frozen=True)
class Limits:
    """Killswitch parameters; the call site reads like a risk-policy declaration."""

    settlement_asset: str
    pnl_lower_bound: str
    pnl_upper_bound: str
    max_orders_burst: int
    rate_window: datetime.timedelta


def build_engine(limits: Limits) -> openpit.Engine:
    """Wire the engine with the two killswitch policies plus order validation.

    The combination answers a single question: "is my strategy trading too
    fast or losing too much?".
    """
    policies = pretrade.policies
    return (
        openpit.Engine.builder()
        .full_sync()
        # OrderValidation must be present so the engine refuses malformed
        # orders before the killswitch policies see them.
        .builtin(policies.build_order_validation())
        # PnL bounds halt the account permanently when realized P&L crosses
        # either edge of the corridor. Both bounds are optional - this
        # example configures both for completeness.
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=limits.settlement_asset,
                    lower_bound=openpit.param.Pnl(limits.pnl_lower_bound),
                    upper_bound=openpit.param.Pnl(limits.pnl_upper_bound),
                )
            )
        )
        # Rate limit catches a strategy stuck in a tight loop. The example
        # uses the broker (global) axis; see the Policies wiki page for
        # per-asset and per-account axes.
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=limits.max_orders_burst,
                        window=limits.rate_window,
                    )
                )
            )
        )
        .build()
    )


# =============================================================================
# Section 3 - the engine loop.
# ``run`` consumes the event stream, calls the engine, and notifies the
# reactor. This function is policy-agnostic - reuse it as-is in your code.
# =============================================================================


def run(engine: openpit.Engine, stream: Iterable[Event], reactor: Reactor) -> Stats:
    """Drive the engine until ``stream`` is exhausted and return aggregate stats.

    The engine is owned by the caller. Exceptions raised here come from
    infrastructure failures, not business rejects (those go to
    ``reactor.on_rejected``).
    """
    stats = Stats()
    perf_counter_ns = time.perf_counter_ns  # bind once to shave a name lookup
    for event in stream:
        if isinstance(event, OrderEvent):
            _run_pre_trade(engine, event.order, stats, reactor, perf_counter_ns)
        else:
            _run_report(engine, event, stats, reactor)
    return stats


def _run_pre_trade(
    engine: openpit.Engine,
    order: openpit.Order,
    stats: Stats,
    reactor: Reactor,
    perf_counter_ns,
) -> None:
    start = perf_counter_ns()
    result = engine.execute_pre_trade(order=order)
    elapsed = perf_counter_ns() - start

    stats.pre_trade_calls += 1
    stats.total_pre_trade_ns += elapsed
    if stats.pre_trade_calls == 1 or elapsed < stats.min_pre_trade_ns:
        stats.min_pre_trade_ns = elapsed
    if elapsed > stats.max_pre_trade_ns:
        stats.max_pre_trade_ns = elapsed

    if not result:
        stats.rejected += 1
        reactor.on_rejected(order, list(result.rejects))
        return

    # On accept, persist the reservation. ``commit`` finalizes the reserved
    # state; call ``rollback`` instead to release it if you decide not to
    # submit the order to the venue.
    result.reservation.commit()
    stats.accepted += 1
    reactor.on_accepted(order)


def _run_report(
    engine: openpit.Engine,
    event: ReportEvent,
    stats: Stats,
    reactor: Reactor,
) -> None:
    result = engine.apply_execution_report(report=event.report)
    stats.reports += 1
    stats.pnl += event.realized_pnl
    if result.account_blocks and not stats.kill_switch:
        stats.kill_switch = True
        stats.kill_switch_on_trade = stats.reports
    reactor.on_report(event.report, result)


# =============================================================================
# Section 4 - the scenario.
# A scripted feed that exercises the kill-switch policies. In your
# own application this is the place you delete entirely - your real strategy
# produces events.
# =============================================================================


# The burst overshoots the rate-limit ceiling by a few orders so the policy
# rejects the tail of the burst. The accepted orders then produce a stream of
# small-loss reports, and the final report contributes a large loss that
# pushes cumulative P&L past the lower bound and trips the kill switch on
# the last trade.
SCENARIO_ATTEMPTS = 105
SCENARIO_MAX_ORDERS_BURST = 100
SCENARIO_ACCEPTED_REPORTS = 100
SCENARIO_ACCOUNT = 99_224_416

# 99 * (-0.5) + (-460) = -509.5 < -500 - the kill switch fires on the final
# report; every earlier report keeps cumulative P&L well inside the corridor
# (-49.5 at worst).
SCENARIO_REPORT_PNL = Decimal("-0.5")
SCENARIO_FINAL_REPORT_PNL = Decimal("-460")
SCENARIO_LOWER_BOUND = "-500"
SCENARIO_UPPER_BOUND = "500"
SCENARIO_RATE_WINDOW = datetime.timedelta(seconds=10)
SCENARIO_ORDER_PRICE = "185"
SCENARIO_ORDER_QTY = "100"
SCENARIO_ASSET_TRADED = "AAPL"
SCENARIO_ASSET_SETTLE = "USD"


def build_order() -> openpit.Order:
    """Build a buy-AAPL order intent. A real strategy assembles this from a
    signal and current market data."""
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(SCENARIO_ASSET_TRADED, SCENARIO_ASSET_SETTLE),
            account_id=openpit.param.AccountId.from_u64(SCENARIO_ACCOUNT),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(SCENARIO_ORDER_QTY),
            price=openpit.param.Price(SCENARIO_ORDER_PRICE),
        ),
    )


def build_report(pnl: Decimal) -> openpit.ExecutionReport:
    """Build a combined-mode execution report. "Combined" means the fee is
    embedded in pnl, so the fee field is set to zero; see the Policies wiki
    page for the alternative "separate" convention."""
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(SCENARIO_ASSET_TRADED, SCENARIO_ASSET_SETTLE),
            account_id=openpit.param.AccountId.from_u64(SCENARIO_ACCOUNT),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl(str(pnl)),
            fee=openpit.param.Fee("0"),
        ),
    )


def scenario_stream(
    order: openpit.Order,
    small_report: openpit.ExecutionReport,
    final_report: openpit.ExecutionReport,
) -> Iterator[Event]:
    """Scripted feed: three counters walked in order - order attempts, then
    small-loss reports, then one kill-switch report.

    Replace this implementation with a coroutine-driven source that selects
    over your strategy and venue feeds.
    """
    order_ev = OrderEvent(order=order)
    small_ev = ReportEvent(report=small_report, realized_pnl=SCENARIO_REPORT_PNL)
    final_ev = ReportEvent(report=final_report, realized_pnl=SCENARIO_FINAL_REPORT_PNL)

    for _ in range(SCENARIO_ATTEMPTS):
        yield order_ev
    for _ in range(SCENARIO_ACCEPTED_REPORTS - 1):
        yield small_ev
    yield final_ev


# =============================================================================
# Section 5 - the reactor.
# Plug your venue client and strategy book here.
# =============================================================================


class LoggingReactor:
    """Prints rejects and kill-switch events to stdout. Production code routes
    these to your monitoring channel and to a strategy-halt signal."""

    def __init__(self, reject_cap: int = 4) -> None:
        self._rejects_printed = 0
        self._reject_cap = reject_cap

    def on_accepted(self, order: openpit.Order) -> None:
        # In production: venue.send_order(order).
        del order

    def on_rejected(self, order: openpit.Order, rejects: list[pretrade.Reject]) -> None:
        del order
        # Cap noisy outputs in case a real run produces a long burst of
        # rate-limit rejects.
        if self._rejects_printed >= self._reject_cap:
            return
        for r in rejects:
            print(f"rejected by {r.policy} [{r.code}]: {r.reason} ({r.details})")
            self._rejects_printed += 1
            if self._rejects_printed >= self._reject_cap:
                print("... further rejects suppressed")
                return

    def on_report(
        self, report: openpit.ExecutionReport, result: openpit.PostTradeResult
    ) -> None:
        del report
        if result.account_blocks:
            print("kill switch triggered - halt new orders until cleared")


# =============================================================================
# Section 6 - main().
# The application entry point. Read top-to-bottom for the integration flow.
# =============================================================================


def main() -> int:
    # Step 1 - declare the risk limits.
    limits = Limits(
        settlement_asset=SCENARIO_ASSET_SETTLE,
        pnl_lower_bound=SCENARIO_LOWER_BOUND,
        pnl_upper_bound=SCENARIO_UPPER_BOUND,
        max_orders_burst=SCENARIO_MAX_ORDERS_BURST,
        rate_window=SCENARIO_RATE_WINDOW,
    )

    # Step 2 - build the engine. Do this once at platform start-up.
    engine = build_engine(limits)

    # Step 3 - assemble the event stream. In production this is your strategy
    # + venue listener; here it is a generator driven by the scenario
    # constants above.
    order = build_order()
    small_report = build_report(SCENARIO_REPORT_PNL)
    final_report = build_report(SCENARIO_FINAL_REPORT_PNL)
    stream = scenario_stream(order, small_report, final_report)

    # Step 4 - run the loop. Replace LoggingReactor with your venue client.
    stats = run(engine, stream, LoggingReactor(reject_cap=4))

    # Step 5 - report the outcome. In production you would push these to your
    # metrics backend.
    print()
    print("--- run summary ---")
    print(f"pnl result   : {stats.pnl} {limits.settlement_asset}")
    print(f"total trades : {stats.reports}")
    print(f"pre-trade avg: {_format_ns(stats.avg_pre_trade_ns)}")
    print(f"pre-trade min: {_format_ns(stats.min_pre_trade_ns)}")
    print(f"pre-trade max: {_format_ns(stats.max_pre_trade_ns)}")
    print(f"pre-trade tot: {_format_ns(stats.total_pre_trade_ns)}")
    print(f"accepted     : {stats.accepted}")
    print(f"rejected     : {stats.rejected}")
    if stats.kill_switch:
        print(
            f"kill switch  : TRIPPED on trade {stats.kill_switch_on_trade}"
            f" of {stats.reports}"
        )
    else:
        print("kill switch  : not triggered")
    return 0


def _format_ns(value_ns: int) -> str:
    """Render a nanosecond count as a human-readable duration string."""
    if value_ns >= 1_000_000_000:
        return f"{value_ns / 1_000_000_000:.3f}s"
    if value_ns >= 1_000_000:
        return f"{value_ns / 1_000_000:.3f}ms"
    if value_ns >= 1_000:
        return f"{value_ns / 1_000:.3f}µs"
    return f"{value_ns}ns"


if __name__ == "__main__":
    sys.exit(main())
