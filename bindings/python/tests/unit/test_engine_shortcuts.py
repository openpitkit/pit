import conftest
import openpit
import pytest


class DualRejectPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "DualRejectPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT,
                    reason="quantity exceeded",
                    details="first reject",
                    scope=openpit.pretrade.RejectScope.ORDER,
                ),
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="risk exceeded",
                    details="second reject",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                ),
            ]
        )

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = ctx, report
        return False


@pytest.mark.unit
def test_execute_pre_trade_returns_ok_result_with_reservation() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    result = engine.execute_pre_trade(order=conftest.make_order())

    assert result.ok
    assert result.reservation is not None
    assert result.rejects == []
    result.reservation.rollback()


@pytest.mark.unit
def test_execute_pre_trade_returns_single_reject_for_start_stage_failure() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    result = engine.execute_pre_trade(
        order=conftest.make_order(trade_amount=openpit.param.TradeAmount.quantity(0))
    )

    assert not result.ok
    assert result.reservation is None
    assert len(result.rejects) == 1
    reject = result.rejects[0]
    assert reject.policy == "OrderValidationPolicy"


@pytest.mark.unit
def test_execute_pre_trade_preserves_main_stage_reject_list_order() -> None:
    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=DualRejectPolicy()).build()
    )

    result = engine.execute_pre_trade(order=conftest.make_order())

    assert not result.ok
    assert result.reservation is None
    assert [reject.policy for reject in result.rejects] == [
        "DualRejectPolicy",
        "DualRejectPolicy",
    ]
    assert [reject.details for reject in result.rejects] == [
        "first reject",
        "second reject",
    ]
    assert [reject.scope for reject in result.rejects] == ["order", "account"]
