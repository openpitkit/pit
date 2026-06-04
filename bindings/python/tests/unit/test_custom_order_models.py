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

import typing

import openpit
import pytest


class StrategyOrder(openpit.Order):
    # @typing.override
    def __init__(
        self,
        *,
        strategy_tag: str,
    ) -> None:
        super().__init__(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    "AAPL",
                    "USD",
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_int(99224416),
                trade_amount=openpit.param.TradeAmount.quantity(10),
                price=openpit.param.Price(25),
            ),
        )
        self.strategy_tag = strategy_tag


class StrategyReport(openpit.ExecutionReport):
    # @typing.override
    def __init__(
        self,
        *,
        report_tag: str,
    ) -> None:
        super().__init__(
            operation=openpit.ExecutionReportOperation(
                instrument=openpit.Instrument(
                    "AAPL",
                    "USD",
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_int(99224416),
            ),
            financial_impact=openpit.FinancialImpact(
                pnl=openpit.param.Pnl(5),
                fee=openpit.param.Fee(1),
            ),
        )
        self.report_tag = report_tag


class CaptureStrategyOrderStartCheck(openpit.pretrade.Policy):
    # @typing.override
    def __init__(self) -> None:
        self.orders: list[openpit.Order] = []
        self.strategy_tags: list[str] = []
        self.reports: list[openpit.ExecutionReport] = []
        self.report_tags: list[str] = []

    # @typing.override
    @property
    def name(self) -> str:
        return "CaptureStrategyOrderStartCheck"

    # @typing.override
    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        strategy_order = typing.cast(StrategyOrder, order)
        self.orders.append(order)
        self.strategy_tags.append(strategy_order.strategy_tag)
        return ()

    # @typing.override
    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> bool:
        strategy_report = typing.cast(StrategyReport, report)
        _ = ctx
        self.reports.append(report)
        self.report_tags.append(strategy_report.report_tag)
        return False


class StrategyTagPolicy(openpit.pretrade.Policy):
    # @typing.override
    def __init__(self) -> None:
        self.orders: list[openpit.Order] = []
        self.strategy_tags: list[str] = []
        self.reports: list[openpit.ExecutionReport] = []
        self.report_tags: list[str] = []

    # @typing.override
    @property
    def name(self) -> str:
        return "StrategyTagPolicy"

    # @typing.override
    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        strategy_order = typing.cast(StrategyOrder, order)
        self.orders.append(order)
        self.strategy_tags.append(strategy_order.strategy_tag)

        if strategy_order.strategy_tag == "blocked":
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
                        reason="strategy blocked",
                        details="project strategy tag blocked",
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )

        return openpit.pretrade.PolicyDecision.accept()

    # @typing.override
    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> bool:
        strategy_report = typing.cast(StrategyReport, report)
        _ = ctx
        self.reports.append(report)
        self.report_tags.append(strategy_report.report_tag)
        return False


def make_strategy_order(strategy_tag: str) -> StrategyOrder:
    return StrategyOrder(strategy_tag=strategy_tag)


def make_strategy_report(report_tag: str) -> StrategyReport:
    return StrategyReport(report_tag=report_tag)


@pytest.mark.unit
def test_custom_order_model_reaches_start_and_execution_check_callbacks() -> None:
    start_check = CaptureStrategyOrderStartCheck()
    execution_check = StrategyTagPolicy()
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=start_check)
        .pre_trade(policy=execution_check)
        .build()
    )
    order = make_strategy_order("allowed")

    start_result = engine.start_pre_trade(order=order)

    assert start_result.ok
    assert start_check.orders == [order]
    assert start_check.orders[0] is order
    assert start_check.strategy_tags == ["allowed"]

    execute_result = start_result.request.execute()

    assert execute_result.ok
    assert execution_check.orders == [order]
    assert execution_check.orders[0] is order
    assert execution_check.strategy_tags == ["allowed"]
    execute_result.reservation.rollback()


@pytest.mark.unit
def test_custom_order_model_rejects_blocked_strategy_tag() -> None:
    start_check = CaptureStrategyOrderStartCheck()
    execution_check = StrategyTagPolicy()
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=start_check)
        .pre_trade(policy=execution_check)
        .build()
    )
    order = make_strategy_order("blocked")

    start_result = engine.start_pre_trade(order=order)

    assert start_result.ok
    assert start_check.orders[0] is order
    assert start_check.strategy_tags == ["blocked"]

    execute_result = start_result.request.execute()

    assert not execute_result.ok
    assert execute_result.reservation is None
    assert len(execute_result.rejects) == 1
    assert execute_result.rejects[0].policy == "StrategyTagPolicy"
    assert execute_result.rejects[0].code == (
        openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION
    )
    assert execution_check.orders[0] is order
    assert execution_check.strategy_tags == ["blocked"]


@pytest.mark.unit
def test_custom_execution_report_model_reaches_start_and_main_callbacks() -> None:
    start_check = CaptureStrategyOrderStartCheck()
    execution_check = StrategyTagPolicy()
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=start_check)
        .pre_trade(policy=execution_check)
        .build()
    )
    report = make_strategy_report("fill-1")

    post_trade = engine.apply_execution_report(report=report)

    assert not post_trade.account_blocks
    assert start_check.reports == [report]
    assert start_check.reports[0] is report
    assert start_check.report_tags == ["fill-1"]
    assert execution_check.reports == [report]
    assert execution_check.reports[0] is report
    assert execution_check.report_tags == ["fill-1"]
