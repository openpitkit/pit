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
    assert len(second.rejects) == 1
    assert second.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_pnl_kill_switch_can_be_reset_after_trigger() -> None:
    policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset="USD",
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
    assert len(blocked.rejects) == 1
    assert (
        blocked.rejects[0].code == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )
    assert blocked.rejects[0].scope == "account"

    policy.reset_pnl(settlement_asset="USD")
    resumed = engine.start_pre_trade(order=conftest.make_order())
    assert resumed.ok


@pytest.mark.unit
@pytest.mark.parametrize(
    ("limit_asset", "quantity", "volume", "price", "expected_code"),
    [
        (
            "EUR",
            openpit.param.Quantity("1"),
            None,
            openpit.param.Price("100"),
            openpit.pretrade.RejectCode.RISK_CONFIGURATION_MISSING,
        ),
        (
            "USD",
            openpit.param.Quantity("11"),
            None,
            openpit.param.Price("90"),
            openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT,
        ),
        (
            "USD",
            openpit.param.Quantity("10"),
            None,
            openpit.param.Price("101"),
            openpit.pretrade.RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT,
        ),
        (
            "USD",
            openpit.param.Quantity("10"),
            None,
            openpit.param.Price("100"),
            None,
        ),
        (
            "USD",
            None,
            openpit.param.Volume("100"),
            openpit.param.Price("100"),
            None,
        ),
        (
            "USD",
            openpit.param.Quantity("10"),
            None,
            None,
            openpit.pretrade.RejectCode.ORDER_VALUE_CALCULATION_FAILED,
        ),
    ],
)
def test_order_size_limit_paths(
    limit_asset: str,
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
    trade_amount: openpit.param.TradeAmount | None
    if quantity is not None:
        trade_amount = openpit.param.TradeAmount.quantity(quantity)
    elif volume is not None:
        trade_amount = openpit.param.TradeAmount.volume(volume)
    else:
        trade_amount = None
    if price is None:
        order = openpit.Order(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    "AAPL",
                    "USD",
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
        assert len(start_result.rejects) == 1
        assert start_result.rejects[0].code == expected_code


@pytest.mark.unit
def test_order_size_limit_rejects_positional_args_for_keyword_only_constructor() -> (
    None
):
    with pytest.raises(TypeError):
        openpit.pretrade.policies.OrderSizeLimit("USD", "10", "1000")


@pytest.mark.unit
def test_order_size_limit_requires_asset_string() -> None:
    with pytest.raises(TypeError, match="asset must be a str"):
        openpit.pretrade.policies.OrderSizeLimit(
            settlement_asset=123,  # type: ignore[arg-type]
            max_quantity=openpit.param.Quantity(10),
            max_notional=openpit.param.Volume(1000),
        )


@pytest.mark.unit
def test_pnl_kill_switch_requires_asset_string() -> None:
    with pytest.raises(TypeError, match="asset must be a str"):
        openpit.pretrade.policies.PnlKillSwitchPolicy(
            settlement_asset=123,  # type: ignore[arg-type]
            barrier=openpit.param.Pnl(100),
        )


@pytest.mark.unit
def test_pnl_kill_switch_set_barrier_requires_asset_string() -> None:
    policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset="USD",
        barrier=openpit.param.Pnl(100),
    )

    with pytest.raises(TypeError, match="asset must be a str"):
        policy.set_barrier(
            settlement_asset=123,  # type: ignore[arg-type]
            barrier=openpit.param.Pnl(200),
        )
