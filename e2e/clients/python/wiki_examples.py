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
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.`
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://github.com/openpitkit and the OWNERS file for details.

import typing

import openpit

# Source: pit.wiki/Policies.md


class NotionalCapPolicy(openpit.pretrade.Policy):
    # @typing.override
    def __init__(self, max_abs_notional: openpit.param.Volume) -> None:
        self._max_abs_notional = max_abs_notional

    # @typing.override
    @property
    def name(self) -> str:
        return "NotionalCapPolicy"

    # @typing.override
    def perform_pre_trade_check(
        self,
        *,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        assert context.order.operation is not None
        trade_amount = context.order.operation.trade_amount
        if isinstance(trade_amount, openpit.param.Volume):
            requested_notional = trade_amount
        else:
            assert isinstance(trade_amount, openpit.param.Quantity)
            assert context.order.operation.price is not None
            requested_notional = context.order.operation.price.calculate_volume(
                trade_amount
            )

        if requested_notional > self._max_abs_notional:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="strategy cap exceeded",
                        details=(
                            "requested notional "
                            f"{requested_notional.value}, "
                            f"max allowed: {self._max_abs_notional.value}"
                        ),
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )

        return openpit.pretrade.PolicyDecision.accept(
            mutations=[
                openpit.pretrade.Mutation.reserve_notional(
                    settlement_asset=context.order.operation.instrument.settlement_asset,
                    commit_amount=requested_notional,
                    rollback_amount=openpit.param.Volume("0"),
                )
            ]
        )

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


class StrategyOrder(openpit.Order):
    # @typing.override
    def __init__(self, *, strategy_tag: str) -> None:
        super().__init__(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    openpit.param.Asset("AAPL"),
                    openpit.param.Asset("USD"),
                ),
                side=openpit.param.Side.BUY,
                trade_amount=openpit.param.Quantity("10"),
                price=openpit.param.Price("25"),
            ),
        )
        # Project field: this field is added by the host application,
        # not by the SDK.
        self.strategy_tag = strategy_tag


class StrategyReport(openpit.ExecutionReport):
    # @typing.override
    def __init__(self, *, report_tag: str) -> None:
        super().__init__(
            operation=openpit.ExecutionReportOperation(
                instrument=openpit.Instrument(
                    openpit.param.Asset("AAPL"),
                    openpit.param.Asset("USD"),
                ),
                side=openpit.param.Side.BUY,
            ),
            financial_impact=openpit.FinancialImpact(
                pnl=openpit.param.Pnl("5"),
                fee=openpit.param.Fee("1"),
            ),
        )
        # Project field: this field is added by the host application,
        # not by the SDK.
        self.report_tag = report_tag


class StrategyTagPolicy(openpit.pretrade.Policy):
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
        order = typing.cast(StrategyOrder, context.order)
        if order.strategy_tag == "blocked":
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
        _ = strategy_report
        return False


class StrategyTagStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    # @typing.override
    @property
    def name(self) -> str:
        return "StrategyTagStartPolicy"

    # @typing.override
    def check_pre_trade_start(
        self,
        *,
        order: openpit.Order,
    ) -> typing.Optional[openpit.pretrade.PolicyReject]:
        strategy_order = typing.cast(StrategyOrder, order)
        if strategy_order.strategy_tag == "blocked":
            return openpit.pretrade.PolicyReject(
                code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
                reason="strategy blocked",
                details="project strategy tag blocked",
                scope=openpit.pretrade.RejectScope.ORDER,
            )
        return None

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


def run_notional_cap_policy_example() -> None:
    engine = (
        openpit.Engine.builder()
        .pre_trade_policy(
            policy=NotionalCapPolicy(
                max_abs_notional=openpit.param.Volume("1000"),
            )
        )
        .build()
    )

    order_within_limit = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Quantity("10"),
            price=openpit.param.Price("25"),
        ),
    )
    start_result = engine.start_pre_trade(order=order_within_limit)
    assert start_result.ok

    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.commit()

    order_above_limit = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Quantity("100"),
            price=openpit.param.Price("25"),
        ),
    )
    blocked_result = engine.start_pre_trade(order=order_above_limit)
    assert blocked_result.ok

    blocked_execute_result = blocked_result.request.execute()
    assert blocked_execute_result.ok is False
    assert (
        blocked_execute_result.rejects[0].code
        == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    )


def run_strategy_tag_policy_example() -> None:
    engine = (
        openpit.Engine.builder().pre_trade_policy(policy=StrategyTagPolicy()).build()
    )

    order = StrategyOrder(strategy_tag="allowed")

    start_result = engine.start_pre_trade(order=order)
    if not start_result.ok:
        raise RuntimeError(start_result.reject.reason)

    execute_result = start_result.request.execute()
    if not execute_result.ok:
        raise RuntimeError(execute_result.rejects[0].reason)

    execute_result.reservation.commit()

    report = StrategyReport(report_tag="fill-1")
    post_trade = engine.apply_execution_report(report=report)
    assert post_trade.kill_switch_triggered is False

    blocked_engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=StrategyTagStartPolicy())
        .build()
    )
    blocked_order = StrategyOrder(strategy_tag="blocked")
    blocked_start_result = blocked_engine.start_pre_trade(order=blocked_order)
    assert blocked_start_result.ok is False
    assert (
        blocked_start_result.reject.code
        == openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION
    )


def main() -> None:
    run_notional_cap_policy_example()
    run_strategy_tag_policy_example()


if __name__ == "__main__":
    main()
