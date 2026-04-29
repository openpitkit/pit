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

import openpit
import pytest


def send_order_to_venue(order: openpit.Order) -> None:
    _ = order


def _aapl_usd_order(
    *,
    quantity: str = "100",
    price: str = "185",
    account_id: openpit.param.AccountId | None = None,
) -> openpit.Order:
    if account_id is None:
        account_id = openpit.param.AccountId.from_u64(99224416)
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(quantity),
            price=openpit.param.Price(price),
        ),
    )


def _aapl_usd_report() -> openpit.ExecutionReport:
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl("-50"),
            fee=openpit.param.Fee("3.4"),
        ),
    )


@pytest.mark.integration
def test_readme_quickstart() -> None:
    # Source: bindings/python/README.md — Usage
    # Shared with: pit.wiki/Getting-Started.md
    # Keep README and wiki versions of this example in sync.

    # 1. Configure policies.
    pnl_policy = openpit.pretrade.policies.PnlBoundsKillSwitchPolicy(
        settlement_asset="USD",
        lower_bound=openpit.param.Pnl("-1000"),
        initial_pnl=openpit.param.Pnl("0"),
    )

    rate_limit_policy = openpit.pretrade.policies.RateLimitPolicy(
        max_orders=100,
        window_seconds=1,
    )

    order_size_policy = openpit.pretrade.policies.OrderSizeLimitPolicy(
        limit=openpit.pretrade.policies.OrderSizeLimit(
            settlement_asset="USD",
            max_quantity=openpit.param.Quantity("500"),
            max_notional=openpit.param.Volume("100000"),
        ),
    )

    # 2. Build the engine (one time at the platform initialization).
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

    # 3. Check an order.
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(100.0),
            price=openpit.param.Price(185.0),
        ),
    )

    start_result = engine.start_pre_trade(order=order)

    if not start_result:
        messages = ", ".join(
            f"{r.policy} [{r.code}]: {r.reason}: {r.details}"
            for r in start_result.rejects
        )
        raise RuntimeError(messages)

    request = start_result.request

    # 4. Quick, lightweight checks, such as fat-finger scope or enabled kill
    # switch, were performed during pre-trade request creation. The system state
    # has not yet changed, except in cases where each request, even rejected ones,
    # must be considered. Before the heavy-duty checks, other work on the request
    # can be performed simply by holding the request object.

    # 5. Real pre-trade and risk control.
    execute_result = request.execute()

    # Optional shortcut for the same two-stage flow:
    # execute_result = engine.execute_pre_trade(order=order)

    if not execute_result:
        messages = ", ".join(
            f"{reject.policy} [{reject.code}]: {reject.reason}: {reject.details}"
            for reject in execute_result.rejects
        )
        raise RuntimeError(messages)

    reservation = execute_result.reservation

    # 6. If the request is successfully sent to the venue, it must be committed.
    # The rollback must be called otherwise to revert all performed reservations.
    try:
        send_order_to_venue(order)
    except Exception:
        reservation.rollback()
        raise

    reservation.commit()

    # 7. The order goes to the venue and returns with an execution report.
    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl("-50"),
            fee=openpit.param.Fee("3.4"),
        ),
    )

    result = engine.apply_execution_report(report=report)

    # 8. After each execution report is applied, the system may report that it has
    # been determined in advance that all subsequent requests will be rejected if
    # the account status does not change.
    assert result.kill_switch_triggered is False


class BlockedAccountPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    @property
    def name(self) -> str:
        return "BlockedAccountPolicy"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx
        assert order.operation is not None
        if order.operation.account_id == openpit.param.AccountId.from_u64(1):
            return (
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.ACCOUNT_BLOCKED,
                    reason="account is blocked",
                    details="account 1 cannot send new orders",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                ),
            )
        return ()

    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        del report
        return False


class DocsNotionalCapPolicy(openpit.pretrade.PreTradePolicy):
    def __init__(self, max_notional: openpit.param.Volume) -> None:
        self._max_notional = max_notional

    @property
    def name(self) -> str:
        return "NotionalCapPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx
        assert order.operation is not None
        amount = order.operation.trade_amount
        if amount.is_volume:
            requested = amount.as_volume
        else:
            assert order.operation.price is not None
            requested = order.operation.price.calculate_volume(amount.as_quantity)

        if requested > self._max_notional:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="notional cap exceeded",
                        details=f"requested {requested}, max {self._max_notional}",
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )
        return openpit.pretrade.PolicyDecision.accept()

    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        del report
        return False


class MyMainStagePolicy(openpit.pretrade.PreTradePolicy):
    @property
    def name(self) -> str:
        return "MyMainStagePolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.accept()

    def apply_execution_report(self, report: openpit.ExecutionReport) -> bool:
        del report
        return False


class MyAccountAdjustmentPolicy(openpit.AccountAdjustmentPolicy):
    @property
    def name(self) -> str:
        return "MyAccountAdjustmentPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> None:
        del ctx, account_id, adjustment


@pytest.mark.integration
def test_docs_guides_policies_start_stage_policy() -> None:
    # Source: bindings/python/docs/guides/policies.md — Start-stage policies
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=BlockedAccountPolicy())
        .build()
    )

    allowed = engine.start_pre_trade(order=_aapl_usd_order())
    assert allowed.ok

    blocked = engine.start_pre_trade(
        order=_aapl_usd_order(
            account_id=openpit.param.AccountId.from_u64(1),
        )
    )
    assert not blocked.ok
    assert blocked.rejects[0].code == openpit.pretrade.RejectCode.ACCOUNT_BLOCKED


@pytest.mark.integration
def test_docs_guides_policies_main_stage_policy() -> None:
    # Source: bindings/python/docs/guides/policies.md — Main-stage policies
    engine = (
        openpit.Engine.builder()
        .pre_trade_policy(
            policy=DocsNotionalCapPolicy(
                max_notional=openpit.param.Volume("1000"),
            )
        )
        .build()
    )

    accepted = engine.execute_pre_trade(
        order=_aapl_usd_order(quantity="10", price="25")
    )
    assert accepted.ok
    accepted.reservation.commit()

    rejected = engine.execute_pre_trade(
        order=_aapl_usd_order(quantity="100", price="25")
    )
    assert not rejected.ok
    assert rejected.rejects[0].code == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED


@pytest.mark.integration
def test_docs_guides_engine_build_engine() -> None:
    # Source: bindings/python/docs/guides/engine.md — Build an engine
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .pre_trade_policy(policy=MyMainStagePolicy())
        .account_adjustment_policy(policy=MyAccountAdjustmentPolicy())
        .build()
    )

    assert engine is not None


@pytest.mark.integration
def test_docs_guides_engine_explicit_two_stage_flow() -> None:
    # Source: bindings/python/docs/guides/engine.md — Run the explicit two-stage flow
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .build()
    )
    order = _aapl_usd_order()

    start_result = engine.start_pre_trade(order=order)
    if not start_result:
        messages = ", ".join(
            f"{reject.policy} [{reject.code}]: {reject.reason}"
            for reject in start_result.rejects
        )
        raise RuntimeError(messages)
    else:
        execute_result = start_result.request.execute()

    assert execute_result.ok
    execute_result.reservation.commit()


@pytest.mark.integration
def test_docs_guides_engine_shortcut_flow() -> None:
    # Source: bindings/python/docs/guides/engine.md — Run the shortcut flow
    engine = openpit.Engine.builder().build()
    order = _aapl_usd_order()

    execute_result = engine.execute_pre_trade(order=order)

    assert execute_result.ok
    execute_result.reservation.commit()


@pytest.mark.integration
def test_docs_guides_engine_finalize_reservations() -> None:
    # Source: bindings/python/docs/guides/engine.md — Finalize reservations
    engine = openpit.Engine.builder().build()
    order = _aapl_usd_order()
    execute_result = engine.execute_pre_trade(order=order)

    def send_order(order: openpit.Order) -> None:
        _ = order

    if execute_result:
        reservation = execute_result.reservation
        try:
            send_order(order)
        except Exception:
            reservation.rollback()
            raise
        else:
            reservation.commit()

    assert execute_result.ok


@pytest.mark.integration
def test_docs_guides_engine_apply_post_trade_reports() -> None:
    # Source: bindings/python/docs/guides/engine.md — Apply post-trade reports
    engine = openpit.Engine.builder().build()
    report = _aapl_usd_report()

    post_trade = engine.apply_execution_report(report=report)
    if post_trade.kill_switch_triggered:
        print("halt new orders until the blocked state is cleared")

    assert post_trade.kill_switch_triggered is False
