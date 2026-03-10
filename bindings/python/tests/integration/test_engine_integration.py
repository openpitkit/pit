from decimal import Decimal

import openpit
import pytest
from conftest import make_order, make_report


def _format_decimal(value: Decimal) -> str:
    text = format(value, "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text or "0"


class IntegrationStrategy(openpit.pretrade.Policy):
    def __init__(self, *, max_abs_notional: str) -> None:
        self._max_abs_notional = Decimal(max_abs_notional)
        self.journal: list[tuple[str, str, str]] = []

    @property
    def name(self) -> str:
        return "IntegrationStrategy"

    def perform_pre_trade_check(
        self,
        *,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        self.journal.append(
            (
                context.order.underlying_asset,
                context.order.settlement_asset,
                context.notional,
            )
        )

        requested = Decimal(context.notional).copy_abs()
        if requested > self._max_abs_notional:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="strategy cap exceeded",
                        details=(
                            "requested notional "
                            f"{_format_decimal(requested)}, "
                            f"max allowed: {_format_decimal(self._max_abs_notional)}"
                        ),
                        scope="order",
                    )
                ]
            )

        return openpit.pretrade.PolicyDecision.accept(
            mutations=[
                openpit.pretrade.Mutation.kill_switch(
                    kill_switch_id="integration.noop",
                    commit_enabled=False,
                    rollback_enabled=False,
                )
            ]
        )

    def apply_execution_report(
        self,
        *,
        report: openpit.pretrade.ExecutionReport,
    ) -> bool:
        self.journal.append(
            (
                report.underlying_asset,
                report.settlement_asset,
                report.pnl,
            )
        )
        return False


@pytest.mark.integration
@pytest.mark.parametrize(
    ("case", "expected_code"),
    [
        ("commit_success", None),
        ("rollback_on_exception", None),
        ("main_stage_reject", openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED),
        ("rate_limit_reject", openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED),
        (
            "kill_switch_reset_resume",
            openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED,
        ),
        ("order_size_missing", openpit.pretrade.RejectCode.RISK_CONFIGURATION_MISSING),
        ("order_size_quantity", openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT),
        (
            "order_size_notional",
            openpit.pretrade.RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT,
        ),
        ("order_size_boundary", None),
    ],
)
def test_engine_end_to_end_table(case: str, expected_code: str | None) -> None:
    if case == "commit_success":
        strategy = IntegrationStrategy(max_abs_notional="700")
        engine = (
            openpit.Engine.builder()
            .check_pre_trade_start_policy(
                policy=openpit.pretrade.policies.OrderValidationPolicy(),
            )
            .pre_trade_policy(policy=strategy)
            .build()
        )

        start = engine.start_pre_trade(
            order=make_order(side="sell", quantity=5.0, price=100.0)
        )
        assert start.ok
        execute = start.request.execute()
        assert execute.ok
        execute.reservation.commit()

        assert strategy.journal[0] == ("AAPL", "USD", "500")

        post_trade = engine.apply_execution_report(report=make_report(pnl=4.5))
        assert post_trade.kill_switch_triggered is False
        assert strategy.journal[-1] == ("AAPL", "USD", "4.5")
        return

    if case == "rollback_on_exception":
        strategy = IntegrationStrategy(max_abs_notional="1000")
        engine = openpit.Engine.builder().pre_trade_policy(policy=strategy).build()

        start = engine.start_pre_trade(
            order=make_order(side="buy", quantity=3.0, price=100.0)
        )
        assert start.ok
        execute = start.request.execute()
        assert execute.ok
        reservation = execute.reservation

        with pytest.raises(RuntimeError, match="venue send failed"):
            try:
                raise RuntimeError("venue send failed")
            except Exception:
                reservation.rollback()
                raise

        resumed = engine.start_pre_trade(
            order=make_order(side="buy", quantity=7.0, price=100.0)
        )
        resumed_execute = resumed.request.execute()
        assert resumed_execute.ok
        resumed_execute.reservation.rollback()
        return

    if case == "main_stage_reject":
        strategy = IntegrationStrategy(max_abs_notional="700")
        engine = openpit.Engine.builder().pre_trade_policy(policy=strategy).build()

        start = engine.start_pre_trade(
            order=make_order(side="buy", quantity=8.0, price=100.0)
        )
        assert start.ok
        execute = start.request.execute()
        assert not execute.ok
        assert execute.reservation is None
        assert len(execute.rejects) == 1
        reject = execute.rejects[0]
        assert reject.code == expected_code
        assert reject.policy == "IntegrationStrategy"
        assert reject.scope == "order"
        assert reject.details == "requested notional 800, max allowed: 700"
        assert strategy.journal[0] == ("AAPL", "USD", "-800")
        return

    if case == "rate_limit_reject":
        engine = (
            openpit.Engine.builder()
            .check_pre_trade_start_policy(
                policy=openpit.pretrade.policies.RateLimitPolicy(
                    max_orders=1,
                    window_seconds=60,
                )
            )
            .build()
        )

        assert engine.start_pre_trade(order=make_order()).ok
        blocked = engine.start_pre_trade(order=make_order())
        assert not blocked.ok
        assert blocked.reject.code == expected_code
        assert blocked.reject.scope == "order"
        return

    if case == "kill_switch_reset_resume":
        pnl_policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
            settlement_asset="USD",
            barrier="500",
        )
        engine = (
            openpit.Engine.builder()
            .check_pre_trade_start_policy(policy=pnl_policy)
            .build()
        )

        post_trade = engine.apply_execution_report(report=make_report(pnl=-600.0))
        assert post_trade.kill_switch_triggered

        blocked = engine.start_pre_trade(order=make_order(price=99.5))
        assert not blocked.ok
        assert blocked.reject.code == expected_code
        assert blocked.reject.scope == "account"

        pnl_policy.reset_pnl(settlement_asset="USD")
        resumed = engine.start_pre_trade(order=make_order(price=101.0, quantity=2.0))
        assert resumed.ok
        resumed_execute = resumed.request.execute()
        assert resumed_execute.ok
        resumed_execute.reservation.commit()
        return

    if case.startswith("order_size_"):
        if case == "order_size_missing":
            limit_asset = "EUR"
            quantity = 1.0
            price = 100.0
        elif case == "order_size_quantity":
            limit_asset = "USD"
            quantity = 11.0
            price = 90.0
        elif case == "order_size_notional":
            limit_asset = "USD"
            quantity = 10.0
            price = 101.0
        else:
            limit_asset = "USD"
            quantity = 10.0
            price = 100.0

        engine = (
            openpit.Engine.builder()
            .check_pre_trade_start_policy(
                policy=openpit.pretrade.policies.OrderSizeLimitPolicy(
                    limit=openpit.pretrade.policies.OrderSizeLimit(
                        settlement_asset=limit_asset,
                        max_quantity="10",
                        max_notional="1000",
                    )
                )
            )
            .build()
        )
        result = engine.start_pre_trade(
            order=make_order(quantity=quantity, price=price)
        )
        if expected_code is None:
            assert result.ok
            result.request.execute().reservation.rollback()
        else:
            assert not result.ok
            assert result.reject.code == expected_code
            assert result.reject.scope == "order"
        return

    raise AssertionError(f"unknown test case: {case}")
