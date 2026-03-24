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

if not hasattr(typing, "override"):

    def _override(method: typing.Any) -> typing.Any:
        return method

    typing.override = _override  # type: ignore[attr-defined]


class NotionalCapPolicy(openpit.pretrade.Policy):
    @typing.override
    def __init__(self, max_abs_notional: openpit.param.Volume) -> None:
        self._max_abs_notional = max_abs_notional

    @property
    @typing.override
    def name(self) -> str:
        return "NotionalCapPolicy"

    @typing.override
    def perform_pre_trade_check(
        self,
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

    @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


class StrategyOrder(openpit.Order):
    @typing.override
    def __init__(self, *, strategy_tag: str) -> None:
        super().__init__(operation=aapl_usd_order("10", "25").operation)
        self.strategy_tag = strategy_tag


class StrategyReport(openpit.ExecutionReport):
    @typing.override
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


class StrategyTagPolicy(openpit.pretrade.Policy):
    @property
    @typing.override
    def name(self) -> str:
        return "StrategyTagPolicy"

    @typing.override
    def perform_pre_trade_check(
        self,
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

    @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        strategy_report = typing.cast(StrategyReport, report)
        assert strategy_report.report_tag == "fill-1"
        return False


def main() -> None:
    run_domain_types_examples()
    run_getting_started_examples()
    run_pre_trade_pipeline_examples()
    run_notional_cap_policy_example()
    run_strategy_tag_policy_example()


def run_domain_types_examples() -> None:
    asset = openpit.param.Asset("AAPL")
    quantity = openpit.param.Quantity("10.5")
    price = openpit.param.Price(185)
    pnl = openpit.param.Pnl(-12.5)

    assert asset.value == "AAPL"
    assert quantity.value == "10.5"
    assert price.value == "185"
    assert pnl.value == "-12.5"

    side = openpit.param.Side.BUY
    position_side = openpit.param.PositionSide.LONG
    assert side.opposite().value == "sell"
    assert side.sign() == 1
    assert position_side.opposite().value == "short"

    from_multiplier = openpit.param.Leverage.from_u16(100)
    from_float = openpit.param.Leverage.from_f64(100.5)
    assert from_multiplier.value == 100.0
    assert from_float.value == 100.5


def run_getting_started_examples() -> None:
    pnl_policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset=openpit.param.Asset("USD"),
        barrier=openpit.param.Pnl("1000"),
    )
    rate_limit_policy = openpit.pretrade.policies.RateLimitPolicy(
        max_orders=100,
        window_seconds=1,
    )
    order_size_policy = openpit.pretrade.policies.OrderSizeLimitPolicy(
        limit=openpit.pretrade.policies.OrderSizeLimit(
            settlement_asset=openpit.param.Asset("USD"),
            max_quantity=openpit.param.Quantity("500"),
            max_notional=openpit.param.Volume("100000"),
        )
    )

    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .check_pre_trade_start_policy(policy=pnl_policy)
        .check_pre_trade_start_policy(policy=rate_limit_policy)
        .check_pre_trade_start_policy(policy=order_size_policy)
        .build()
    )

    start_result = engine.start_pre_trade(order=aapl_usd_order("100", "185"))
    assert start_result.ok

    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.commit()

    result = engine.apply_execution_report(report=aapl_usd_report("-50", "3"))
    assert result.kill_switch_triggered is False


def run_pre_trade_pipeline_examples() -> None:
    start_engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy()
        )
        .build()
    )

    start_result = start_engine.start_pre_trade(order=aapl_usd_order("0", "185"))
    assert start_result.ok is False
    assert start_result.reject.code == openpit.pretrade.RejectCode.INVALID_FIELD_VALUE

    main_engine = (
        openpit.Engine.builder()
        .pre_trade_policy(
            policy=NotionalCapPolicy(
                max_abs_notional=openpit.param.Volume("1000"),
            )
        )
        .build()
    )

    execute_result = main_engine.start_pre_trade(order=aapl_usd_order("10", "25"))
    assert execute_result.ok
    reservation_result = execute_result.request.execute()
    assert reservation_result.ok
    reservation_result.reservation.commit()

    blocked_start = main_engine.start_pre_trade(order=aapl_usd_order("100", "25"))
    assert blocked_start.ok
    blocked_execute = blocked_start.request.execute()
    assert blocked_execute.ok is False
    assert (
        blocked_execute.rejects[0].code
        == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    )


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

    start_result = engine.start_pre_trade(order=aapl_usd_order("10", "25"))
    assert start_result.ok

    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.commit()

    blocked_result = engine.start_pre_trade(order=aapl_usd_order("100", "25"))
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

    start_result = engine.start_pre_trade(order=StrategyOrder(strategy_tag="allowed"))
    assert start_result.ok

    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.commit()

    post_trade = engine.apply_execution_report(
        report=StrategyReport(report_tag="fill-1")
    )
    assert post_trade.kill_switch_triggered is False

    blocked_engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=BlockedStrategyStartPolicy())
        .build()
    )
    blocked = blocked_engine.start_pre_trade(
        order=StrategyOrder(strategy_tag="blocked")
    )
    assert blocked.ok is False
    assert blocked.reject.code == openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION


class BlockedStrategyStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    @property
    @typing.override
    def name(self) -> str:
        return "StrategyTagStartPolicy"

    @typing.override
    def check_pre_trade_start(
        self,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyReject | None:
        strategy_order = typing.cast(StrategyOrder, order)
        if strategy_order.strategy_tag == "blocked":
            return openpit.pretrade.PolicyReject(
                code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
                reason="strategy blocked",
                details="project strategy tag blocked",
                scope=openpit.pretrade.RejectScope.ORDER,
            )
        return None

    @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


def aapl_usd_order(quantity: str, price: str) -> openpit.Order:
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Quantity(quantity),
            price=openpit.param.Price(price),
        ),
    )


def aapl_usd_report(pnl: str, fee: str) -> openpit.ExecutionReport:
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl(pnl),
            fee=openpit.param.Fee(fee),
        ),
    )


if __name__ == "__main__":
    main()
