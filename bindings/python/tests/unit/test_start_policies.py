import conftest
import openpit
import pytest


@pytest.mark.unit
def test_rate_limit_rejects_second_order_in_window() -> None:
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

    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok
    second = engine.start_pre_trade(order=conftest.make_order())
    assert not second.ok
    assert second.reject.code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_pnl_kill_switch_can_be_reset_after_trigger() -> None:
    policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset=openpit.param.Asset("USD"),
        barrier=openpit.param.Pnl("100"),
    )
    engine = (
        openpit.Engine.builder().check_pre_trade_start_policy(policy=policy).build()
    )

    post_trade = engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("-120"))
    )
    assert post_trade.kill_switch_triggered

    blocked = engine.start_pre_trade(order=conftest.make_order())
    assert not blocked.ok
    assert blocked.reject.code == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    assert blocked.reject.scope == "account"

    policy.reset_pnl(settlement_asset=openpit.param.Asset("USD"))
    resumed = engine.start_pre_trade(order=conftest.make_order())
    assert resumed.ok


@pytest.mark.unit
@pytest.mark.parametrize(
    ("limit_asset", "quantity", "volume", "price", "expected_code"),
    [
        (
            openpit.param.Asset("EUR"),
            openpit.param.Quantity("1"),
            None,
            openpit.param.Price("100"),
            openpit.pretrade.RejectCode.RISK_CONFIGURATION_MISSING,
        ),
        (
            openpit.param.Asset("USD"),
            openpit.param.Quantity("11"),
            None,
            openpit.param.Price("90"),
            openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT,
        ),
        (
            openpit.param.Asset("USD"),
            openpit.param.Quantity("10"),
            None,
            openpit.param.Price("101"),
            openpit.pretrade.RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT,
        ),
        (
            openpit.param.Asset("USD"),
            openpit.param.Quantity("10"),
            None,
            openpit.param.Price("100"),
            None,
        ),
        (
            openpit.param.Asset("USD"),
            None,
            openpit.param.Volume("100"),
            openpit.param.Price("100"),
            None,
        ),
        (
            openpit.param.Asset("USD"),
            openpit.param.Quantity("10"),
            None,
            None,
            openpit.pretrade.RejectCode.ORDER_VALUE_CALCULATION_FAILED,
        ),
    ],
)
def test_order_size_limit_paths(
    limit_asset: openpit.param.Asset,
    quantity: openpit.param.Quantity | None,
    volume: openpit.param.Volume | None,
    price: openpit.param.Price | None,
    expected_code: str | None,
) -> None:
    size = openpit.pretrade.policies.OrderSizeLimitPolicy(
        limit=openpit.pretrade.policies.OrderSizeLimit(
            settlement_asset=limit_asset,
            max_quantity=openpit.param.Quantity("10"),
            max_notional=openpit.param.Volume("1000"),
        )
    )
    engine = openpit.Engine.builder().check_pre_trade_start_policy(policy=size).build()
    trade_amount: openpit.param.Quantity | openpit.param.Volume | None
    trade_amount = quantity if quantity is not None else volume
    if price is None:
        order = openpit.Order(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    openpit.param.Asset("AAPL"),
                    openpit.param.Asset("USD"),
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=trade_amount,
            ),
        )
    else:
        order = conftest.make_order(trade_amount=trade_amount, price=price)
    start_result = engine.start_pre_trade(order=order)

    if expected_code is None:
        assert start_result.ok
        start_result.request.execute().reservation.rollback()
    else:
        assert not start_result.ok
        assert start_result.reject.code == expected_code


@pytest.mark.unit
def test_order_size_limit_rejects_positional_args_for_keyword_only_constructor() -> (
    None
):
    with pytest.raises(TypeError):
        openpit.pretrade.policies.OrderSizeLimit("USD", "10", "1000")
