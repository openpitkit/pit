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

import dataclasses
import typing

import openpit
import pytest


class TaggedOrder(openpit.Order):
    # @typing.override
    def __init__(self, *, order_tag: str) -> None:
        super().__init__(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    openpit.param.Asset("AAPL"),
                    openpit.param.Asset("USD"),
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.Quantity("10"),
                price=openpit.param.Price("25"),
            ),
        )
        self.order_tag = order_tag


class TaggedReport(openpit.ExecutionReport):
    # @typing.override
    def __init__(self, *, report_tag: str) -> None:
        super().__init__(
            operation=openpit.ExecutionReportOperation(
                instrument=openpit.Instrument(
                    openpit.param.Asset("AAPL"),
                    openpit.param.Asset("USD"),
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
            ),
            financial_impact=openpit.FinancialImpact(
                pnl=openpit.param.Pnl("5"),
                fee=openpit.param.Fee("1"),
            ),
        )
        self.report_tag = report_tag


@dataclasses.dataclass(frozen=True)
class InterleavingCase:
    name: str
    execute_order: tuple[int, int, int]
    finalize_actions: tuple[str, str, str]
    report_order: tuple[int, int, int]


class CaptureStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    # @typing.override
    def __init__(self, *, name: str, journal: list[str]) -> None:
        self._name = name
        self._journal = journal
        self.orders: list[openpit.Order] = []
        self.reports: list[openpit.ExecutionReport] = []

    # @typing.override
    @property
    def name(self) -> str:
        return self._name

    # @typing.override
    def check_pre_trade_start(
        self,
        *,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyReject | None:
        self.orders.append(order)
        self._journal.append(f"start:{self.name}:{order.order_tag}")
        return None

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        tagged_report = typing.cast(TaggedReport, report)
        self.reports.append(report)
        self._journal.append(f"report-start:{self.name}:{tagged_report.report_tag}")
        return False


class SequenceFenceStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    # @typing.override
    def __init__(self, *, name: str, journal: list[str]) -> None:
        self._name = name
        self._journal = journal

    # @typing.override
    @property
    def name(self) -> str:
        return self._name

    # @typing.override
    def check_pre_trade_start(
        self,
        *,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyReject | None:
        self._journal.append(f"start:{self.name}:{order.order_tag}")
        return None

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        tagged_report = typing.cast(TaggedReport, report)
        self._journal.append(f"report-start:{self.name}:{tagged_report.report_tag}")
        return False


class CaptureMainPolicy(openpit.pretrade.Policy):
    # @typing.override
    def __init__(self, *, name: str, journal: list[str]) -> None:
        self._name = name
        self._journal = journal
        self.orders: list[openpit.Order] = []
        self.reports: list[openpit.ExecutionReport] = []

    # @typing.override
    @property
    def name(self) -> str:
        return self._name

    # @typing.override
    def perform_pre_trade_check(
        self,
        *,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        tagged_order = typing.cast(TaggedOrder, context.order)
        self.orders.append(context.order)
        self._journal.append(f"execute:{self.name}:{tagged_order.order_tag}")
        return openpit.pretrade.PolicyDecision.accept()

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        tagged_report = typing.cast(TaggedReport, report)
        self.reports.append(report)
        self._journal.append(f"report-main:{self.name}:{tagged_report.report_tag}")
        return False


class SequenceFenceMainPolicy(openpit.pretrade.Policy):
    # @typing.override
    def __init__(self, *, name: str, journal: list[str]) -> None:
        self._name = name
        self._journal = journal

    # @typing.override
    @property
    def name(self) -> str:
        return self._name

    # @typing.override
    def perform_pre_trade_check(
        self,
        *,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        tagged_order = typing.cast(TaggedOrder, context.order)
        self._journal.append(f"execute:{self.name}:{tagged_order.order_tag}")
        return openpit.pretrade.PolicyDecision.accept()

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        tagged_report = typing.cast(TaggedReport, report)
        self._journal.append(f"report-main:{self.name}:{tagged_report.report_tag}")
        return False


def make_tagged_order(order_tag: str) -> TaggedOrder:
    return TaggedOrder(order_tag=order_tag)


def make_tagged_report(report_tag: str) -> TaggedReport:
    return TaggedReport(report_tag=report_tag)


def expected_journal(case: InterleavingCase) -> list[str]:
    order_tags = ("ord-a", "ord-b", "ord-c")
    report_tags = ("rep-a", "rep-b", "rep-c")
    expected: list[str] = []

    for order_tag in order_tags:
        expected.extend(
            (
                f"start:capture-start:{order_tag}",
                f"start:sequence-start:{order_tag}",
            )
        )

    for request_index, action in zip(
        case.execute_order,
        case.finalize_actions,
        strict=True,
    ):
        order_tag = order_tags[request_index]
        expected.extend(
            (
                f"execute:capture-main:{order_tag}",
                f"execute:sequence-main:{order_tag}",
                f"finalize:{action}:{order_tag}",
            )
        )

    for report_index in case.report_order:
        report_tag = report_tags[report_index]
        expected.extend(
            (
                f"report-start:capture-start:{report_tag}",
                f"report-start:sequence-start:{report_tag}",
                f"report-main:capture-main:{report_tag}",
                f"report-main:sequence-main:{report_tag}",
            )
        )

    return expected


def assert_same_objects(actual: list[object], expected: list[object]) -> None:
    assert actual == expected
    for actual_obj, expected_obj in zip(actual, expected, strict=True):
        assert actual_obj is expected_obj


@pytest.mark.unit
@pytest.mark.parametrize(
    "case",
    [
        InterleavingCase(
            name="reverse_then_mixed_finalize",
            execute_order=(2, 0, 1),
            finalize_actions=("rollback", "commit", "rollback"),
            report_order=(1, 2, 0),
        ),
        InterleavingCase(
            name="middle_first_with_tail_commit",
            execute_order=(1, 2, 0),
            finalize_actions=("commit", "rollback", "commit"),
            report_order=(2, 0, 1),
        ),
        InterleavingCase(
            name="front_back_middle",
            execute_order=(0, 2, 1),
            finalize_actions=("commit", "commit", "rollback"),
            report_order=(0, 1, 2),
        ),
    ],
    ids=lambda case: case.name,
)
def test_policy_callbacks_preserve_original_objects_across_interleaving(
    case: InterleavingCase,
) -> None:
    journal: list[str] = []
    capture_start = CaptureStartPolicy(name="capture-start", journal=journal)
    sequence_start = SequenceFenceStartPolicy(name="sequence-start", journal=journal)
    capture_main = CaptureMainPolicy(name="capture-main", journal=journal)
    sequence_main = SequenceFenceMainPolicy(name="sequence-main", journal=journal)
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=capture_start)
        .check_pre_trade_start_policy(policy=sequence_start)
        .pre_trade_policy(policy=capture_main)
        .pre_trade_policy(policy=sequence_main)
        .build()
    )

    orders = [
        make_tagged_order("ord-a"),
        make_tagged_order("ord-b"),
        make_tagged_order("ord-c"),
    ]
    reports = [
        make_tagged_report("rep-a"),
        make_tagged_report("rep-b"),
        make_tagged_report("rep-c"),
    ]
    requests = []

    for order in orders:
        start_result = engine.start_pre_trade(order=order)
        assert start_result.ok
        request = start_result.request
        assert request is not None
        requests.append(request)

    for request_index, action in zip(
        case.execute_order,
        case.finalize_actions,
        strict=True,
    ):
        execute_result = requests[request_index].execute()
        assert execute_result.ok
        reservation = execute_result.reservation
        assert reservation is not None
        if action == "commit":
            reservation.commit()
        else:
            reservation.rollback()
        journal.append(f"finalize:{action}:{orders[request_index].order_tag}")

    for report_index in case.report_order:
        post_trade = engine.apply_execution_report(report=reports[report_index])
        assert post_trade.kill_switch_triggered is False

    expected_execute_orders = [orders[index] for index in case.execute_order]
    expected_reports = [reports[index] for index in case.report_order]

    assert_same_objects(capture_start.orders, orders)
    assert_same_objects(capture_main.orders, expected_execute_orders)
    assert_same_objects(capture_start.reports, expected_reports)
    assert_same_objects(capture_main.reports, expected_reports)
    assert journal == expected_journal(case)
