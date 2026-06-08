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

"""Assertion-driven counterpart of ``main``.

The scripted feed must trip the rate limit on the tail of the burst (a
handful of "too frequent" rejects) and then trip the kill switch on the
final execution report.
"""

from __future__ import annotations

from decimal import Decimal

import main as example
import openpit
from openpit import pretrade


class RecordingReactor:
    """Collects engine verdicts for assertion."""

    def __init__(self) -> None:
        self.accepted = 0
        self.reject_codes: list[object] = []
        self.kill_switched = False

    def on_accepted(self, order: openpit.Order) -> None:
        del order
        self.accepted += 1

    def on_rejected(self, order: openpit.Order, rejects: list[pretrade.Reject]) -> None:
        del order
        for r in rejects:
            self.reject_codes.append(r.code)

    def on_report(
        self, report: openpit.ExecutionReport, result: openpit.PostTradeResult
    ) -> None:
        del report
        if result.account_blocks:
            self.kill_switched = True


def test_scenario_trips_both_killswitches() -> None:
    engine = example.build_engine(
        example.Limits(
            settlement_asset=example.SCENARIO_ASSET_SETTLE,
            pnl_lower_bound=example.SCENARIO_LOWER_BOUND,
            pnl_upper_bound=example.SCENARIO_UPPER_BOUND,
            max_orders_burst=example.SCENARIO_MAX_ORDERS_BURST,
            rate_window=example.SCENARIO_RATE_WINDOW,
        )
    )

    reactor = RecordingReactor()
    order = example.build_order()
    small_report = example.build_report(example.SCENARIO_REPORT_PNL)
    final_report = example.build_report(example.SCENARIO_FINAL_REPORT_PNL)
    stream = example.scenario_stream(order, small_report, final_report)

    stats = example.run(engine, stream, reactor)

    want_accepted = example.SCENARIO_MAX_ORDERS_BURST
    want_rejected = example.SCENARIO_ATTEMPTS - example.SCENARIO_MAX_ORDERS_BURST
    want_reports = example.SCENARIO_ACCEPTED_REPORTS
    want_pre_trade = example.SCENARIO_ATTEMPTS

    assert stats.accepted == want_accepted, (stats.accepted, want_accepted)
    assert stats.rejected == want_rejected, (stats.rejected, want_rejected)
    assert stats.reports == want_reports, (stats.reports, want_reports)
    assert stats.pre_trade_calls == want_pre_trade

    # Kill switch must trip on the final report.
    assert stats.kill_switch
    assert reactor.kill_switched
    assert stats.kill_switch_on_trade == example.SCENARIO_ACCEPTED_REPORTS

    # 99 * (-0.5) + (-460) = -509.5, just past the -500 floor.
    expected_pnl = Decimal("-509.5")
    assert stats.pnl == expected_pnl, (stats.pnl, expected_pnl)

    # Every reject in the scenario must be a rate-limit reject: the burst
    # overshoots the ceiling within the same rate-limit window, so the tail
    # hits "too frequent".
    assert len(reactor.reject_codes) == want_rejected
    for code in reactor.reject_codes:
        assert code == pretrade.RejectCode.RATE_LIMIT_EXCEEDED, code

    assert stats.total_pre_trade_ns > 0
    assert stats.min_pre_trade_ns >= 0
    assert stats.max_pre_trade_ns >= stats.min_pre_trade_ns
