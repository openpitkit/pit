import conftest
import openpit
import pytest


class IntegrationStrategy(openpit.pretrade.PreTradePolicy):
    # @typing.override
    def __init__(self, *, max_abs_notional: str) -> None:
        self._max_abs_notional = openpit.param.Volume(max_abs_notional)
        self.journal: list[tuple[str, str, str]] = []

    # @typing.override
    @property
    def name(self) -> str:
        return "IntegrationStrategy"

    # @typing.override
    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        assert order.operation is not None
        assert order.operation.trade_amount is not None
        assert order.operation.price is not None

        assert order.operation.trade_amount.is_quantity
        quantity = order.operation.trade_amount.as_quantity
        price = order.operation.price
        requested_notional = price.calculate_volume(quantity)
        signed_notional_text = (
            str(requested_notional.to_cash_flow_outflow())
            if order.operation.side == openpit.param.Side.BUY
            else str(requested_notional.to_cash_flow_inflow())
        )
        self.journal.append(
            (
                order.operation.instrument.underlying_asset,
                order.operation.instrument.settlement_asset,
                signed_notional_text,
            )
        )

        if requested_notional > self._max_abs_notional:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="strategy cap exceeded",
                        details=(
                            "requested notional "
                            f"{requested_notional}, "
                            f"max allowed: {self._max_abs_notional}"
                        ),
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )

        return openpit.pretrade.PolicyDecision.accept(
            mutations=[
                openpit.Mutation(
                    commit=lambda: None,
                    rollback=lambda: None,
                )
            ]
        )

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        assert report.operation is not None
        assert report.financial_impact is not None
        assert report.financial_impact.pnl is not None

        self.journal.append(
            (
                report.operation.instrument.underlying_asset,
                report.operation.instrument.settlement_asset,
                str(report.financial_impact.pnl),
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
            order=conftest.make_order(
                side=openpit.param.Side.SELL,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.TradeAmount.quantity(5),
                price=openpit.param.Price("100"),
            )
        )
        assert start.ok
        execute = start.request.execute()
        assert execute.ok
        execute.reservation.commit()

        assert strategy.journal[0] == ("AAPL", "USD", "500")

        post_trade = engine.apply_execution_report(
            report=conftest.make_report(pnl=openpit.param.Pnl("4.5"))
        )
        assert post_trade.kill_switch_triggered is False
        assert strategy.journal[-1] == ("AAPL", "USD", "4.5")
        return

    if case == "rollback_on_exception":
        strategy = IntegrationStrategy(max_abs_notional="1000")
        engine = openpit.Engine.builder().pre_trade_policy(policy=strategy).build()

        start = engine.start_pre_trade(
            order=conftest.make_order(
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.TradeAmount.quantity(3),
                price=openpit.param.Price("100"),
            )
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
            order=conftest.make_order(
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.TradeAmount.quantity(7),
                price=openpit.param.Price("100"),
            )
        )
        resumed_execute = resumed.request.execute()
        assert resumed_execute.ok
        resumed_execute.reservation.rollback()
        return

    if case == "main_stage_reject":
        strategy = IntegrationStrategy(max_abs_notional="700")
        engine = openpit.Engine.builder().pre_trade_policy(policy=strategy).build()

        start = engine.start_pre_trade(
            order=conftest.make_order(
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.TradeAmount.quantity(8),
                price=openpit.param.Price("100"),
            )
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

        assert engine.start_pre_trade(order=conftest.make_order()).ok
        blocked = engine.start_pre_trade(order=conftest.make_order())
        assert not blocked.ok
        assert len(blocked.rejects) == 1
        assert blocked.rejects[0].code == expected_code
        assert blocked.rejects[0].scope == "order"
        return

    if case == "kill_switch_reset_resume":
        pnl_policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
            settlement_asset="USD",
            barrier=openpit.param.Pnl("500"),
        )
        engine = (
            openpit.Engine.builder()
            .check_pre_trade_start_policy(policy=pnl_policy)
            .build()
        )

        post_trade = engine.apply_execution_report(
            report=conftest.make_report(pnl=openpit.param.Pnl("-600"))
        )
        assert post_trade.kill_switch_triggered

        blocked = engine.start_pre_trade(
            order=conftest.make_order(price=openpit.param.Price("99.5"))
        )
        assert not blocked.ok
        assert len(blocked.rejects) == 1
        assert blocked.rejects[0].code == expected_code
        assert blocked.rejects[0].scope == "account"

        pnl_policy.reset_pnl(settlement_asset="USD")
        resumed = engine.start_pre_trade(
            order=conftest.make_order(
                price=openpit.param.Price("101"),
                trade_amount=openpit.param.TradeAmount.quantity(2),
            )
        )
        assert resumed.ok
        resumed_execute = resumed.request.execute()
        assert resumed_execute.ok
        resumed_execute.reservation.commit()
        return

    if case.startswith("order_size_"):
        if case == "order_size_missing":
            limit_asset = "EUR"
            quantity = openpit.param.Quantity("1")
            price = openpit.param.Price("100")
        elif case == "order_size_quantity":
            limit_asset = "USD"
            quantity = openpit.param.Quantity("11")
            price = openpit.param.Price("90")
        elif case == "order_size_notional":
            limit_asset = "USD"
            quantity = openpit.param.Quantity("10")
            price = openpit.param.Price("101")
        else:
            limit_asset = "USD"
            quantity = openpit.param.Quantity("10")
            price = openpit.param.Price("100")

        engine = (
            openpit.Engine.builder()
            .check_pre_trade_start_policy(
                policy=openpit.pretrade.policies.OrderSizeLimitPolicy(
                    limit=openpit.pretrade.policies.OrderSizeLimit(
                        settlement_asset=limit_asset,
                        max_quantity=openpit.param.Quantity("10"),
                        max_notional=openpit.param.Volume("1000"),
                    )
                )
            )
            .build()
        )
        result = engine.start_pre_trade(
            order=conftest.make_order(
                trade_amount=openpit.param.TradeAmount.quantity(quantity),
                price=price,
            )
        )
        if expected_code is None:
            assert result.ok
            result.request.execute().reservation.rollback()
        else:
            assert not result.ok
            assert len(result.rejects) == 1
            assert result.rejects[0].code == expected_code
            assert result.rejects[0].scope == "order"
        return

    raise AssertionError(f"unknown test case: {case}")
