import openpit
import pytest
from conftest import make_order, make_report
from openpit.pretrade import policies


@pytest.mark.unit
def test_rate_limit_rejects_second_order_in_window() -> None:
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=policies.RateLimitPolicy(max_orders=1, window_seconds=60)
        )
        .build()
    )

    first = engine.start_pre_trade(order=make_order())
    assert first.ok
    second = engine.start_pre_trade(order=make_order())
    assert not second.ok
    assert second.reject.code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_pnl_kill_switch_can_be_reset_after_trigger() -> None:
    policy = policies.PnlKillSwitchPolicy(settlement_asset="USD", barrier="100")
    engine = (
        openpit.Engine.builder().check_pre_trade_start_policy(policy=policy).build()
    )

    post_trade = engine.apply_execution_report(report=make_report(pnl=-120.0))
    assert post_trade.kill_switch_triggered

    blocked = engine.start_pre_trade(order=make_order())
    assert not blocked.ok
    assert blocked.reject.code == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    assert blocked.reject.scope == "account"

    policy.reset_pnl(settlement_asset="USD")
    resumed = engine.start_pre_trade(order=make_order())
    assert resumed.ok


@pytest.mark.unit
@pytest.mark.parametrize(
    ("limit_asset", "quantity", "price", "expected_code"),
    [
        ("EUR", 1.0, 100.0, openpit.pretrade.RejectCode.RISK_CONFIGURATION_MISSING),
        ("USD", 11.0, 90.0, openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT),
        ("USD", 10.0, 101.0, openpit.pretrade.RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT),
        ("USD", 10.0, 100.0, None),
    ],
)
def test_order_size_limit_paths(
    limit_asset: str, quantity: float, price: float, expected_code: str | None
) -> None:
    size = policies.OrderSizeLimitPolicy(
        limit=policies.OrderSizeLimit(
            settlement_asset=limit_asset,
            max_quantity="10",
            max_notional="1000",
        )
    )
    engine = openpit.Engine.builder().check_pre_trade_start_policy(policy=size).build()
    order = make_order(quantity=quantity, price=price)
    start_result = engine.start_pre_trade(order=order)

    if expected_code is None:
        assert start_result.ok
        start_result.request.execute().reservation.rollback()
    else:
        assert not start_result.ok
        assert start_result.reject.code == expected_code
