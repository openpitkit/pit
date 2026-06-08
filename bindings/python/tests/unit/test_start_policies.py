import datetime
import threading

import conftest
import openpit
import pytest


@pytest.mark.unit
def test_rate_limit_rejects_second_order_in_window() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    )
                )
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
def test_pnl_kill_switch_triggers_when_pnl_outside_bounds() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch()
            .broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset="USD",
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
            .account_barriers(
                policies.PnlBoundsAccountAssetBarrier(
                    account_id=openpit.param.AccountId.from_int(99224416),
                    settlement_asset="USD",
                    lower_bound=openpit.param.Pnl("-100"),
                    initial_pnl=openpit.param.Pnl("0"),
                )
            )
        )
        .build()
    )

    post_trade = engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("-120"))
    )
    assert post_trade.account_blocks

    blocked = engine.start_pre_trade(order=conftest.make_order())
    assert not blocked.ok
    assert len(blocked.rejects) == 1
    assert (
        blocked.rejects[0].code == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )
    assert blocked.rejects[0].scope == "account"


def _huge_broker_barrier() -> openpit.pretrade.policies.OrderSizeBrokerBarrier:
    policies = openpit.pretrade.policies
    return policies.OrderSizeBrokerBarrier(
        limit=policies.OrderSizeLimit(
            max_quantity=openpit.param.Quantity("1000000"),
            max_notional=openpit.param.Volume("1000000000"),
        )
    )


@pytest.mark.unit
@pytest.mark.parametrize(
    ("limit_asset", "quantity", "volume", "price", "expected_code"),
    [
        (
            "EUR",
            openpit.param.Quantity("1"),
            None,
            openpit.param.Price("100"),
            None,
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
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("10"),
                        max_notional=openpit.param.Volume("1000"),
                    ),
                    settlement_asset=limit_asset,
                )
            )
        )
        .build()
    )
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
                account_id=openpit.param.AccountId.from_int(99224416),
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
def test_order_size_limit_policy_asset_barrier_requires_asset_string() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises((TypeError, ValueError)):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity(10),
                        max_notional=openpit.param.Volume(1000),
                    ),
                    settlement_asset=123,  # type: ignore[arg-type]
                )
            )
        ).build()


@pytest.mark.unit
def test_pnl_kill_switch_requires_asset_string() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(TypeError, match="asset must be a str"):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=123,  # type: ignore[arg-type]
                    lower_bound=openpit.param.Pnl(-100),
                )
            )
        ).build()


@pytest.mark.unit
def test_pnl_kill_switch_requires_at_least_one_bound() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset="USD",
                )
            )
        ).build()


@pytest.mark.unit
def test_rate_limit_asset_barrier_rejects_when_limit_is_exceeded() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().asset_barriers(
                policies.RateLimitAssetBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    ),
                    settlement_asset="USD",
                )
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
def test_rate_limit_account_barrier_independent_of_asset() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().account_barriers(
                policies.RateLimitAccountBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                )
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
def test_rate_limit_account_asset_barrier_specific_to_pair() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().account_asset_barriers(
                policies.RateLimitAccountAssetBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                    settlement_asset="USD",
                )
            )
        )
        .build()
    )

    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok

    second = engine.start_pre_trade(order=conftest.make_order())
    assert not second.ok
    assert second.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED

    third = engine.start_pre_trade(
        order=conftest.make_order(
            account_id=openpit.param.AccountId.from_int(11111111),
        )
    )
    assert third.ok


@pytest.mark.unit
def test_order_size_limit_account_asset_overrides_asset_baseline() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("5"),
                        max_notional=openpit.param.Volume("10000"),
                    ),
                    settlement_asset="USD",
                )
            )
            .account_asset_barriers(
                policies.OrderSizeAccountAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("100"),
                        max_notional=openpit.param.Volume("10000"),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                    settlement_asset="USD",
                )
            )
        )
        .build()
    )

    result = engine.start_pre_trade(
        order=conftest.make_order(
            trade_amount=openpit.param.TradeAmount.quantity(
                openpit.param.Quantity("10")
            ),
            price=openpit.param.Price("100"),
        )
    )
    assert result.ok
    result.request.execute().reservation.rollback()


@pytest.mark.unit
def test_order_size_limit_unknown_settlement_passes() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("1"),
                        max_notional=openpit.param.Volume("100"),
                    ),
                    settlement_asset="EUR",
                )
            )
        )
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert result.ok
    result.request.execute().reservation.rollback()


@pytest.mark.unit
def test_with_full_sync_uses_full_locking_for_builtin_policies() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .full_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=100,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert result.ok


def _run_concurrent_start_pre_trade(
    engine: openpit.Engine,
    orders: list[openpit.Order],
    per_thread: int,
) -> None:
    errors: list[BaseException] = []

    def worker(order: openpit.Order) -> None:
        try:
            for _ in range(per_thread):
                result = engine.start_pre_trade(order=order)
                if not result.ok:
                    raise AssertionError(f"unexpected rejects: {result.rejects}")
                del result
        except BaseException as error:
            errors.append(error)

    threads = [threading.Thread(target=worker, args=(order,)) for order in orders]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    assert not errors


@pytest.mark.unit
def test_engine_full_sync_concurrent_start_pre_trade_is_safe() -> None:
    policies = openpit.pretrade.policies
    total_threads = 4
    per_thread = 500
    account_ids = [
        openpit.param.AccountId.from_int(index) for index in range(total_threads)
    ]

    engine = (
        openpit.Engine.builder()
        .full_sync()
        .builtin(
            policies.build_rate_limit().account_barriers(
                *[
                    policies.RateLimitAccountBarrier(
                        limit=policies.RateLimit(
                            max_orders=per_thread,
                            window=datetime.timedelta(seconds=60),
                        ),
                        account_id=account_id,
                    )
                    for account_id in account_ids
                ]
            )
        )
        .build()
    )

    orders = [conftest.make_order(account_id=account_id) for account_id in account_ids]
    _run_concurrent_start_pre_trade(engine, orders, per_thread)

    result = engine.start_pre_trade(
        order=conftest.make_order(account_id=account_ids[0])
    )
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_engine_full_sync_concurrent_broker_rate_limit_is_safe() -> None:
    policies = openpit.pretrade.policies
    total_threads = 4
    per_thread = 500
    total_calls = total_threads * per_thread

    engine = (
        openpit.Engine.builder()
        .full_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=total_calls,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .build()
    )

    orders = [conftest.make_order() for _ in range(total_threads)]
    _run_concurrent_start_pre_trade(engine, orders, per_thread)

    result = engine.start_pre_trade(order=conftest.make_order())
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_spot_funds_builder_limit_only_mode() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder().no_sync().builtin(policies.build_spot_funds()).build()
    )
    assert engine is not None


@pytest.mark.unit
def test_builtin_policy_builders_reject_duplicate_non_default_group_id() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError, match="duplicate non-default policy_group_id: 7"):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(policies.build_order_validation().with_policy_group_id(7))
            .builtin(policies.build_spot_funds().with_policy_group_id(7))
            .build()
        )


@pytest.mark.unit
def test_builtin_policy_builders_accept_non_default_group_id() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit()
            .with_policy_group_id(1)
            .broker_barrier(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=10,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .builtin(
            policies.build_order_size_limit()
            .with_policy_group_id(2)
            .broker_barrier(_huge_broker_barrier())
        )
        .builtin(
            policies.build_pnl_bounds_killswitch()
            .with_policy_group_id(3)
            .broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset="USD",
                    lower_bound=openpit.param.Pnl("-1000"),
                )
            )
        )
        .builtin(policies.build_order_validation().with_policy_group_id(4))
        .builtin(policies.build_spot_funds().with_policy_group_id(5))
        .build()
    )

    assert engine is not None


@pytest.mark.unit
def test_spot_funds_policy_group_id_tags_outcomes_and_lock_prices() -> None:
    policies = openpit.pretrade.policies
    policy_group_id = 7
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(policies.build_spot_funds().with_policy_group_id(policy_group_id))
        .build()
    )

    seed_result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
                amount=openpit.AccountAdjustmentAmount(
                    balance=openpit.param.AdjustmentAmount.absolute(
                        openpit.param.PositionSize("10000")
                    )
                ),
            )
        ],
    )

    assert seed_result.ok
    assert seed_result.outcomes
    assert {outcome.policy_group_id for outcome in seed_result.outcomes} == {
        policy_group_id
    }

    result = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account_id,
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("200"),
        )
    )

    assert result.ok
    assert result.reservation is not None
    assert result.reservation.lock().entries() == [
        (policy_group_id, openpit.param.Price("200"))
    ]
    assert result.reservation.account_adjustments()
    assert {
        outcome.policy_group_id for outcome in result.reservation.account_adjustments()
    } == {policy_group_id}


def _mock_market_data(*quotes: tuple[str, str]) -> openpit.marketdata.MarketDataService:
    service = (
        openpit.Engine.builder()
        .full_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    for underlying, mark in quotes:
        instrument_id = service.register(openpit.Instrument(underlying, "USD"))
        service.push(instrument_id, openpit.marketdata.Quote(mark=mark))
    return service


@pytest.mark.unit
def test_spot_funds_builder_market_data_with_quotes() -> None:
    policies = openpit.pretrade.policies
    service = _mock_market_data(("AAPL", "200"), ("BTC", "50000"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(service, default_slippage_bps=2000)
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_builder_market_data_zero_slippage() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                _mock_market_data(), default_slippage_bps=0
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_builder_market_data_max_slippage_accepted() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                _mock_market_data(), default_slippage_bps=10_000
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_market_data_slippage_out_of_range_rejected() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_spot_funds().market_data(
                _mock_market_data(), default_slippage_bps=10_001
            )
        ).build()


@pytest.mark.unit
def test_spot_funds_full_engine_with_local_md_service_is_rejected() -> None:
    policies = openpit.pretrade.policies
    # No-sync MD service: derived from a no_sync() engine builder (no
    # full_sync upgrade).
    local_service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    with pytest.raises((ValueError, RuntimeError), match="multi-threaded|full_sync"):
        openpit.Engine.builder().full_sync().builtin(
            policies.build_spot_funds().market_data(
                local_service, default_slippage_bps=100
            )
        ).build()


@pytest.mark.unit
def test_spot_funds_local_engine_with_full_md_service_is_accepted() -> None:
    policies = openpit.pretrade.policies
    # Full MD service: derived from a no_sync() builder then upgraded via full_sync().
    full_service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .full_sync()
        .build()
    )
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                full_service, default_slippage_bps=100
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_rate_limit_zero_window_rejected_by_builder() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError, match="rate limit window must be positive"):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=0),
                    )
                )
            )
        ).build()


def _mock_market_data_named(
    *quotes: tuple[str, str],
) -> tuple[
    openpit.marketdata.MarketDataService,
    dict[str, openpit.marketdata.InstrumentId],
]:
    service = (
        openpit.Engine.builder()
        .full_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    ids: dict[str, openpit.marketdata.InstrumentId] = {}
    for underlying, mark in quotes:
        instrument_id = service.register(openpit.Instrument(underlying, "USD"))
        service.push(instrument_id, openpit.marketdata.Quote(mark=mark))
        ids[underlying] = instrument_id
    return service, ids


def _seed_balance(engine: openpit.Engine, account_id: openpit.param.AccountId) -> None:
    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
                amount=openpit.AccountAdjustmentAmount(
                    balance=openpit.param.AdjustmentAmount.absolute(
                        openpit.param.PositionSize("10000")
                    )
                ),
            )
        ],
    )
    assert result.ok


@pytest.mark.unit
def test_spot_funds_override_instrument_only_accepted() -> None:
    """Instrument-only override is accepted and the engine builds."""
    policies = openpit.pretrade.policies
    service, ids = _mock_market_data_named(("AAPL", "200"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                default_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverride(
                        instrument=ids["AAPL"],
                        slippage_bps=0,
                    )
                ],
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_override_account_scoped_accepted() -> None:
    """Account-scoped override is accepted and influences a pre-trade check."""
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    service, ids = _mock_market_data_named(("AAPL", "200"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                default_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverride(
                        instrument=ids["AAPL"],
                        account_id=account_id,
                        slippage_bps=0,
                    )
                ],
            )
        )
        .build()
    )
    _seed_balance(engine, account_id)

    result = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account_id,
            trade_amount=openpit.param.TradeAmount.quantity("1"),
            price=None,
        )
    )
    assert result.ok
    result.reservation.rollback()


@pytest.mark.unit
def test_spot_funds_override_group_scoped_accepted() -> None:
    """Group-scoped override is accepted and influences a pre-trade check."""
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    group_id = openpit.param.AccountGroupId.from_int(7)
    service, ids = _mock_market_data_named(("AAPL", "200"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                default_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverride(
                        instrument=ids["AAPL"],
                        account_group_id=group_id,
                        slippage_bps=0,
                    )
                ],
            )
        )
        .build()
    )
    engine.accounts().register_group([account_id], group_id)
    _seed_balance(engine, account_id)

    result = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account_id,
            trade_amount=openpit.param.TradeAmount.quantity("1"),
            price=None,
        )
    )
    assert result.ok
    result.reservation.rollback()


@pytest.mark.unit
def test_spot_funds_override_both_account_and_group_raises_value_error() -> None:
    """Passing both account_id and account_group_id on one override is an error."""
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    service, ids = _mock_market_data_named(("AAPL", "200"))
    with pytest.raises(ValueError, match="account.*group|group.*account"):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_spot_funds().market_data(
                service,
                default_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverride(
                        instrument=ids["AAPL"],
                        account_id=account_id,
                        account_group_id=openpit.param.AccountGroupId.from_int(7),
                        slippage_bps=0,
                    )
                ],
            )
        ).build()
