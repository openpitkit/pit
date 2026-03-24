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
                    openpit.param.Asset("AAPL"),
                    openpit.param.Asset("USD"),
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.Quantity("10"),
                price=openpit.param.Price("25"),
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


class CaptureStrategyOrderStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    # @typing.override
    def __init__(self) -> None:
        self.orders: list[openpit.Order] = []
        self.strategy_tags: list[str] = []
        self.reports: list[openpit.ExecutionReport] = []
        self.report_tags: list[str] = []

    # @typing.override
    @property
    def name(self) -> str:
        return "CaptureStrategyOrderStartPolicy"

    # @typing.override
    def check_pre_trade_start(
        self,
        *,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyReject | None:
        strategy_order = typing.cast(StrategyOrder, order)
        self.orders.append(order)
        self.strategy_tags.append(strategy_order.strategy_tag)
        return None

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        strategy_report = typing.cast(StrategyReport, report)
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
        *,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        strategy_order = typing.cast(StrategyOrder, context.order)
        self.orders.append(context.order)
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
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        strategy_report = typing.cast(StrategyReport, report)
        self.reports.append(report)
        self.report_tags.append(strategy_report.report_tag)
        return False


def make_strategy_order(strategy_tag: str) -> StrategyOrder:
    return StrategyOrder(strategy_tag=strategy_tag)


def make_strategy_report(report_tag: str) -> StrategyReport:
    return StrategyReport(report_tag=report_tag)


@pytest.mark.unit
def test_custom_order_model_reaches_start_and_main_policy_callbacks() -> None:
    start_policy = CaptureStrategyOrderStartPolicy()
    main_policy = StrategyTagPolicy()
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=start_policy)
        .pre_trade_policy(policy=main_policy)
        .build()
    )
    order = make_strategy_order("allowed")

    start_result = engine.start_pre_trade(order=order)

    assert start_result.ok
    assert start_policy.orders == [order]
    assert start_policy.orders[0] is order
    assert start_policy.strategy_tags == ["allowed"]

    execute_result = start_result.request.execute()

    assert execute_result.ok
    assert main_policy.orders == [order]
    assert main_policy.orders[0] is order
    assert main_policy.strategy_tags == ["allowed"]
    execute_result.reservation.rollback()


@pytest.mark.unit
def test_custom_order_model_rejects_blocked_strategy_tag() -> None:
    start_policy = CaptureStrategyOrderStartPolicy()
    main_policy = StrategyTagPolicy()
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=start_policy)
        .pre_trade_policy(policy=main_policy)
        .build()
    )
    order = make_strategy_order("blocked")

    start_result = engine.start_pre_trade(order=order)

    assert start_result.ok
    assert start_policy.orders[0] is order
    assert start_policy.strategy_tags == ["blocked"]

    execute_result = start_result.request.execute()

    assert not execute_result.ok
    assert execute_result.reservation is None
    assert len(execute_result.rejects) == 1
    assert execute_result.rejects[0].policy == "StrategyTagPolicy"
    assert execute_result.rejects[0].code == (
        openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION
    )
    assert main_policy.orders[0] is order
    assert main_policy.strategy_tags == ["blocked"]


@pytest.mark.unit
def test_custom_execution_report_model_reaches_start_and_main_callbacks() -> None:
    start_policy = CaptureStrategyOrderStartPolicy()
    main_policy = StrategyTagPolicy()
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=start_policy)
        .pre_trade_policy(policy=main_policy)
        .build()
    )
    report = make_strategy_report("fill-1")

    post_trade = engine.apply_execution_report(report=report)

    assert post_trade.kill_switch_triggered is False
    assert start_policy.reports == [report]
    assert start_policy.reports[0] is report
    assert start_policy.report_tags == ["fill-1"]
    assert main_policy.reports == [report]
    assert main_policy.reports[0] is report
    assert main_policy.report_tags == ["fill-1"]
