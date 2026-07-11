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
# Please see https://openpit.dev and the OWNERS file for details.

import datetime

import conftest
import openpit
import pytest


def _rate_limit(max_orders: int) -> openpit.pretrade.policies.RateLimit:
    return openpit.pretrade.policies.RateLimit(
        max_orders=max_orders,
        window=datetime.timedelta(minutes=1),
    )


def _order_size_limit(
    max_quantity: str,
) -> openpit.pretrade.policies.OrderSizeLimit:
    return openpit.pretrade.policies.OrderSizeLimit(
        max_quantity=openpit.param.Quantity(max_quantity),
        max_notional=openpit.param.Volume("1000000"),
    )


def _market_data() -> tuple[
    openpit.marketdata.MarketDataService,
    openpit.marketdata.InstrumentId,
]:
    service = (
        openpit.Engine.builder()
        .full_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    instrument_id = service.register(openpit.Instrument("AAPL", "USD"))
    service.push(instrument_id, openpit.marketdata.Quote(mark="100"))
    return service, instrument_id


def _spot_funds_fee_fill_report(
    account_id: openpit.param.AccountId,
    fee: str,
) -> openpit.ExecutionReport:
    price = openpit.param.Price("100")
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            side=openpit.param.Side.BUY,
            account_id=account_id,
        ),
        fill=openpit.ExecutionReportFillDetails(
            last_trade=openpit.param.Trade(
                price=price,
                quantity=openpit.param.Quantity("1"),
            ),
            fee=openpit.param.MonetaryAmount(
                amount=openpit.param.Fee(fee),
                currency="USD",
            ),
            leaves_quantity=openpit.param.Quantity(0),
            lock=openpit.pretrade.Lock(
                entries=[(openpit.pretrade.DEFAULT_POLICY_GROUP_ID, price)],
            ),
            is_final=True,
        ),
    )


@pytest.mark.unit
def test_rate_limit_configuration_uses_named_entities() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(limit=_rate_limit(1))
            )
        )
        .build()
    )

    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok

    engine.configure().rate_limit(
        policies.RateLimitBuilder.NAME,
        broker=policies.RateLimitBrokerBarrier(limit=_rate_limit(2)),
        asset_barriers=[
            policies.RateLimitAssetBarrier(
                limit=_rate_limit(10),
                settlement_asset=openpit.param.Asset("USD"),
            )
        ],
        account_barriers=[
            policies.RateLimitAccountBarrier(
                limit=_rate_limit(10),
                account_id=account_id,
            )
        ],
        account_asset_barriers=[
            policies.RateLimitAccountAssetBarrier(
                limit=_rate_limit(10),
                account_id=account_id,
                settlement_asset=openpit.param.Asset("USD"),
            )
        ],
    )

    second = engine.start_pre_trade(order=conftest.make_order())
    assert second.ok
    third = engine.start_pre_trade(order=conftest.make_order())
    assert not third.ok
    assert third.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_order_size_configuration_uses_named_entities() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit().broker_barrier(
                policies.OrderSizeBrokerBarrier(
                    limit=_order_size_limit("100"),
                )
            )
        )
        .build()
    )

    engine.configure().order_size_limit(
        policies.OrderSizeLimitBuilder.NAME,
        broker=policies.OrderSizeBrokerBarrier(
            limit=_order_size_limit("1"),
        ),
        asset_barriers=[
            policies.OrderSizeAssetBarrier(
                limit=_order_size_limit("10"),
                settlement_asset=openpit.param.Asset("USD"),
            )
        ],
        account_asset_barriers=[
            policies.OrderSizeAccountAssetBarrier(
                limit=_order_size_limit("10"),
                account_id=account_id,
                settlement_asset=openpit.param.Asset("USD"),
            )
        ],
    )

    result = engine.start_pre_trade(
        order=conftest.make_order(
            trade_amount=openpit.param.TradeAmount.quantity("2"),
        )
    )
    assert not result.ok
    assert result.rejects[0].code == openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT


@pytest.mark.unit
def test_rate_limit_configuration_can_clear_broker_barrier() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit()
            .broker_barrier(policies.RateLimitBrokerBarrier(limit=_rate_limit(1)))
            .account_barriers(
                policies.RateLimitAccountBarrier(
                    limit=_rate_limit(10),
                    account_id=account_id,
                )
            )
        )
        .build()
    )

    assert engine.start_pre_trade(order=conftest.make_order()).ok
    blocked = engine.start_pre_trade(order=conftest.make_order())
    assert not blocked.ok
    assert blocked.rejects[0].reason == "rate limit exceeded: broker barrier"

    engine.configure().rate_limit(
        policies.RateLimitBuilder.NAME,
        clear_broker=True,
    )

    assert engine.start_pre_trade(order=conftest.make_order()).ok


@pytest.mark.unit
def test_order_size_configuration_can_clear_broker_barrier() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(
                policies.OrderSizeBrokerBarrier(limit=_order_size_limit("1"))
            )
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=_order_size_limit("10"),
                    settlement_asset=openpit.param.Asset("USD"),
                )
            )
        )
        .build()
    )

    oversized = engine.start_pre_trade(
        order=conftest.make_order(
            trade_amount=openpit.param.TradeAmount.quantity("2"),
        )
    )
    assert not oversized.ok
    assert (
        oversized.rejects[0].code == openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT
    )

    engine.configure().order_size_limit(
        policies.OrderSizeLimitBuilder.NAME,
        clear_broker=True,
    )

    allowed = engine.start_pre_trade(
        order=conftest.make_order(
            trade_amount=openpit.param.TradeAmount.quantity("2"),
        )
    )
    assert allowed.ok


@pytest.mark.unit
def test_spot_funds_pnl_bounds_builder_and_configurator_use_named_entities() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    account_group_id = openpit.param.AccountGroupId.from_int(7)
    usd = openpit.param.Asset("USD")

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds_pnl_bounds_killswitch().account_barriers(
                policies.SpotFundsPnlBoundsAccountBarrier(
                    barrier=policies.SpotFundsPnlBoundsBarrier(
                        account_currency=usd,
                        lower_bound=openpit.param.Pnl("-10"),
                        upper_bound=openpit.param.Pnl("10"),
                    ),
                    account_id=account_id,
                    initial_pnl=openpit.param.Pnl("0"),
                )
            )
        )
        .build()
    )

    engine.configure().spot_funds_pnl_bounds_killswitch(
        policies.SpotFundsPnlBoundsKillswitchBuilder.NAME,
        global_barriers=[
            policies.SpotFundsPnlBoundsBarrier(
                account_currency=usd,
                lower_bound=openpit.param.Pnl("-100"),
            )
        ],
        account_group_barriers=[
            policies.SpotFundsPnlBoundsAccountGroupBarrier(
                barrier=policies.SpotFundsPnlBoundsBarrier(
                    account_currency=usd,
                    upper_bound=openpit.param.Pnl("100"),
                ),
                account_group_id=account_group_id,
            )
        ],
        account_barriers=[
            policies.SpotFundsPnlBoundsAccountBarrierUpdate(
                barrier=policies.SpotFundsPnlBoundsBarrier(
                    account_currency=usd,
                    lower_bound=openpit.param.Pnl("-20"),
                    upper_bound=openpit.param.Pnl("20"),
                ),
                account_id=account_id,
            )
        ],
    )
    engine.configure().set_spot_funds_account_pnl(
        policies.SpotFundsPnlBoundsKillswitchBuilder.NAME,
        account=account_id,
        account_currency=usd,
        pnl=openpit.param.Pnl("2.5"),
    )


def test_spot_funds_pnl_axes_can_be_enabled_replaced_and_cleared() -> None:
    policies = openpit.pretrade.policies
    survivor = openpit.param.AccountId.from_int(99224416)
    account_override = openpit.param.AccountId.from_int(99224417)
    cleared = openpit.param.AccountId.from_int(99224418)
    usd = openpit.param.Asset("USD")
    engine = (
        openpit.Engine.builder().no_sync().builtin(policies.build_spot_funds()).build()
    )

    for account_id in (survivor, account_override, cleared):
        engine.accounts().set_currency(account_id, usd)
    engine.apply_account_adjustment(
        account_id=cleared,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
                amount=openpit.AccountAdjustmentAmount(
                    balance=openpit.param.AdjustmentAmount.absolute(
                        openpit.param.PositionSize("1000")
                    )
                ),
            )
        ],
    )

    # Ordinary Spot Funds has no P&L axis, but a funded order still runs.
    ordinary = engine.execute_pre_trade(order=conftest.make_order(account_id=cleared))
    assert ordinary.ok
    ordinary.reservation.rollback()

    engine.configure().spot_funds_pnl_bounds_killswitch(
        policies.SpotFundsBuilder.NAME,
        global_barriers=[
            policies.SpotFundsPnlBoundsBarrier(
                account_currency=usd,
                lower_bound=openpit.param.Pnl("-10"),
            )
        ],
    )

    first_survivor_fill = engine.apply_execution_report(
        report=_spot_funds_fee_fill_report(survivor, "9")
    )
    assert not first_survivor_fill.account_blocks

    # Adding an account barrier leaves the global axis and its live P&L intact.
    engine.configure().spot_funds_pnl_bounds_killswitch(
        policies.SpotFundsBuilder.NAME,
        account_barriers=[
            policies.SpotFundsPnlBoundsAccountBarrierUpdate(
                barrier=policies.SpotFundsPnlBoundsBarrier(
                    account_currency=usd,
                    lower_bound=openpit.param.Pnl("-20"),
                ),
                account_id=account_override,
            )
        ],
    )

    survivor_breach = engine.apply_execution_report(
        report=_spot_funds_fee_fill_report(survivor, "2")
    )
    assert survivor_breach.account_blocks
    assert (
        survivor_breach.account_blocks[0].code
        == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )

    override_fill = engine.apply_execution_report(
        report=_spot_funds_fee_fill_report(account_override, "15")
    )
    assert not override_fill.account_blocks

    # Omitted global barriers remain in place while the account axis is cleared.
    engine.configure().spot_funds_pnl_bounds_killswitch(
        policies.SpotFundsBuilder.NAME,
        account_barriers=[],
    )
    override_recheck = engine.apply_execution_report(
        report=_spot_funds_fee_fill_report(account_override, "0")
    )
    assert override_recheck.account_blocks
    assert (
        override_recheck.account_blocks[0].code
        == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )

    engine.configure().spot_funds_pnl_bounds_killswitch(
        policies.SpotFundsBuilder.NAME,
        global_barriers=[],
        account_group_barriers=[],
        account_barriers=[],
    )
    cleared_fill = engine.apply_execution_report(
        report=_spot_funds_fee_fill_report(cleared, "15")
    )
    assert not cleared_fill.account_blocks

    after_clear = engine.execute_pre_trade(
        order=conftest.make_order(account_id=cleared)
    )
    assert after_clear.ok
    after_clear.reservation.rollback()


@pytest.mark.unit
def test_broker_clear_and_replacement_are_mutually_exclusive() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(limit=_rate_limit(1))
            )
        )
        .build()
    )

    with pytest.raises(ValueError, match="clear_broker"):
        engine.configure().rate_limit(
            policies.RateLimitBuilder.NAME,
            broker=policies.RateLimitBrokerBarrier(limit=_rate_limit(2)),
            clear_broker=True,
        )


@pytest.mark.unit
def test_order_size_broker_clear_and_replacement_are_mutually_exclusive() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit().broker_barrier(
                policies.OrderSizeBrokerBarrier(
                    limit=_order_size_limit("100"),
                )
            )
        )
        .build()
    )

    with pytest.raises(ValueError, match="clear_broker"):
        engine.configure().order_size_limit(
            policies.OrderSizeLimitBuilder.NAME,
            broker=policies.OrderSizeBrokerBarrier(
                limit=_order_size_limit("10"),
            ),
            clear_broker=True,
        )


@pytest.mark.unit
def test_pnl_bounds_configuration_uses_named_entities() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    # Use loose broker bounds so only the account barrier is under test;
    # the broker barrier must not mask the account+asset barrier behavior.
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-1000"),
                )
            )
        )
        .build()
    )

    engine.configure().pnl_bounds_killswitch(
        policies.PnlBoundsKillswitchBuilder.NAME,
        broker_barriers=[
            policies.PnlBoundsBrokerBarrier(
                settlement_asset=openpit.param.Asset("USD"),
                lower_bound=openpit.param.Pnl("-1000"),
            )
        ],
        account_barriers=[
            policies.PnlBoundsAccountAssetBarrierUpdate(
                barrier=policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-10"),
                ),
                account_id=account_id,
            )
        ],
    )

    result = engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("-20"))
    )
    assert result.account_blocks


@pytest.mark.unit
def test_set_account_pnl_force_sets_live_accumulator() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        )
        .build()
    )

    # With no P&L history the order passes against the lower bound of -100.
    first = engine.start_pre_trade(order=conftest.make_order(account_id=account_id))
    assert first.ok

    # Force the accumulator below the lower bound; the next order is rejected.
    engine.configure().set_account_pnl(
        policies.PnlBoundsKillswitchBuilder.NAME,
        account=account_id,
        settlement_asset=openpit.param.Asset("USD"),
        pnl=openpit.param.Pnl("-150"),
    )

    second = engine.start_pre_trade(order=conftest.make_order(account_id=account_id))
    assert not second.ok
    assert (
        second.rejects[0].code == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )


@pytest.mark.unit
def test_set_account_pnl_unknown_policy() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        )
        .build()
    )

    with pytest.raises(openpit.PolicyConfigureError) as caught:
        engine.configure().set_account_pnl(
            "NoSuchPolicy",
            account=openpit.param.AccountId.from_int(99224416),
            settlement_asset=openpit.param.Asset("USD"),
            pnl=openpit.param.Pnl("-150"),
        )
    assert caught.value.kind == openpit.ConfigureErrorKind.UNKNOWN


@pytest.mark.unit
def test_set_account_pnl_type_mismatch() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(limit=_rate_limit(1))
            )
        )
        .build()
    )

    # The name resolves to a rate-limit policy, not a P&L kill-switch policy.
    with pytest.raises(openpit.PolicyConfigureError) as caught:
        engine.configure().set_account_pnl(
            policies.RateLimitBuilder.NAME,
            account=openpit.param.AccountId.from_int(99224416),
            settlement_asset=openpit.param.Asset("USD"),
            pnl=openpit.param.Pnl("-150"),
        )
    assert caught.value.kind == openpit.ConfigureErrorKind.TYPE_MISMATCH


@pytest.mark.unit
def test_pnl_account_barrier_update_rejects_initial_pnl() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(TypeError, match="initial_pnl"):
        policies.PnlBoundsAccountAssetBarrierUpdate(
            barrier=policies.PnlBoundsBrokerBarrier(
                settlement_asset=openpit.param.Asset("USD"),
                lower_bound=openpit.param.Pnl("-10"),
            ),
            account_id=openpit.param.AccountId.from_int(99224416),
            initial_pnl=openpit.param.Pnl("0"),  # type: ignore[call-arg]
        )


@pytest.mark.unit
def test_set_spot_funds_account_pnl_force_sets_live_accumulator() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    usd = openpit.param.Asset("USD")
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds_pnl_bounds_killswitch().global_barriers(
                policies.SpotFundsPnlBoundsBarrier(
                    account_currency=usd,
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        )
        .build()
    )
    engine.accounts().set_currency(account_id, usd)

    # A 10 USD fee accrues P&L -10, inside the -100 bound: no block.
    first = engine.apply_execution_report(
        report=_spot_funds_fee_fill_report(account_id, "10")
    )
    assert not first.account_blocks

    # Force the accumulator near the bound. The next identical fee fill takes
    # it to -105 and breaches; without the force-set it would sit at -20.
    engine.configure().set_spot_funds_account_pnl(
        policies.SpotFundsPnlBoundsKillswitchBuilder.NAME,
        account=account_id,
        account_currency=usd,
        pnl=openpit.param.Pnl("-95"),
    )

    second = engine.apply_execution_report(
        report=_spot_funds_fee_fill_report(account_id, "10")
    )
    assert second.account_blocks
    assert (
        second.account_blocks[0].code
        == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )

    # The engine has latched the account block: the next order rejects.
    blocked = engine.start_pre_trade(order=conftest.make_order(account_id=account_id))
    assert not blocked.ok
    assert (
        blocked.rejects[0].code == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )


@pytest.mark.unit
def test_set_spot_funds_account_pnl_unknown_policy() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds_pnl_bounds_killswitch().global_barriers(
                policies.SpotFundsPnlBoundsBarrier(
                    account_currency=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        )
        .build()
    )

    with pytest.raises(openpit.PolicyConfigureError) as caught:
        engine.configure().set_spot_funds_account_pnl(
            "NoSuchPolicy",
            account=openpit.param.AccountId.from_int(99224416),
            account_currency=openpit.param.Asset("USD"),
            pnl=openpit.param.Pnl("-150"),
        )
    assert caught.value.kind == openpit.ConfigureErrorKind.UNKNOWN


@pytest.mark.unit
def test_set_spot_funds_account_pnl_type_mismatch() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(limit=_rate_limit(1))
            )
        )
        .build()
    )

    # The name resolves to a rate-limit policy, not a spot-funds policy.
    with pytest.raises(openpit.PolicyConfigureError) as caught:
        engine.configure().set_spot_funds_account_pnl(
            policies.RateLimitBuilder.NAME,
            account=openpit.param.AccountId.from_int(99224416),
            account_currency=openpit.param.Asset("USD"),
            pnl=openpit.param.Pnl("-150"),
        )
    assert caught.value.kind == openpit.ConfigureErrorKind.TYPE_MISMATCH


@pytest.mark.unit
def test_spot_funds_configuration_uses_named_entities() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    group_id = openpit.param.AccountGroupId.from_int(7)
    service, instrument_id = _market_data()
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                global_slippage_bps=100,
            )
        )
        .build()
    )

    # Seed a balance so the pre-trade check can succeed.
    engine.apply_account_adjustment(
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
    engine.accounts().register_group([account_id], group_id)

    engine.configure().spot_funds(
        policies.SpotFundsBuilder.NAME,
        global_slippage_bps=200,
        pricing_source=policies.SpotFundsPricingSource.MARK,
        overrides=[
            policies.SpotFundsOverrideEntry(
                target=(
                    policies.SpotFundsOverrideTargetInstrument(instrument=instrument_id)
                ),
                override=policies.SpotFundsOverride(slippage_bps=50),
            ),
            policies.SpotFundsOverrideEntry(
                target=(
                    policies.SpotFundsOverrideTargetInstrumentAccountGroup(
                        instrument=instrument_id,
                        account_group_id=group_id,
                    )
                ),
                override=policies.SpotFundsOverride(slippage_bps=25),
            ),
            policies.SpotFundsOverrideEntry(
                target=(
                    policies.SpotFundsOverrideTargetInstrumentAccount(
                        instrument=instrument_id,
                        account_id=account_id,
                    )
                ),
                override=policies.SpotFundsOverride(slippage_bps=0),
            ),
        ],
    )

    # The account override has precedence over the group, instrument, and
    # global values, so a BUY market order locks exactly at mark=100.
    result = engine.execute_pre_trade(
        order=openpit.Order(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument("AAPL", "USD"),
                side=openpit.param.Side.BUY,
                trade_amount=openpit.param.TradeAmount.quantity("1"),
                account_id=account_id,
            )
        )
    )
    assert result.ok
    assert result.reservation is not None
    locked_prices = result.reservation.lock().entries()
    assert locked_prices, "expected at least one locked price"
    _, locked_price = locked_prices[0]
    assert locked_price == openpit.param.Price("100")
    result.reservation.rollback()


@pytest.mark.unit
def test_configurator_rejects_obsolete_tuple_and_string_inputs() -> None:
    policies = openpit.pretrade.policies
    _, instrument_id = _market_data()
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(limit=_rate_limit(1))
            )
        )
        .builtin(
            policies.build_order_size_limit().broker_barrier(
                policies.OrderSizeBrokerBarrier(
                    limit=_order_size_limit("100"),
                )
            )
        )
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        )
        .builtin(policies.build_spot_funds())
        .build()
    )
    configurator = engine.configure()

    with pytest.raises(TypeError, match="RateLimitBrokerBarrier"):
        configurator.rate_limit(
            policies.RateLimitBuilder.NAME, broker=(1, 60_000_000_000)
        )
    with pytest.raises(TypeError, match="OrderSizeBrokerBarrier"):
        configurator.order_size_limit(
            policies.OrderSizeLimitBuilder.NAME,
            broker=_order_size_limit("1"),
        )
    with pytest.raises(TypeError, match="PnlBoundsBrokerBarrier"):
        configurator.pnl_bounds_killswitch(
            policies.PnlBoundsKillswitchBuilder.NAME,
            broker_barriers=[object()],
        )
    with pytest.raises(TypeError, match="PnlBoundsAccountAssetBarrierUpdate"):
        configurator.pnl_bounds_killswitch(
            policies.PnlBoundsKillswitchBuilder.NAME,
            account_barriers=[
                policies.PnlBoundsAccountAssetBarrier(
                    barrier=policies.PnlBoundsBrokerBarrier(
                        settlement_asset=openpit.param.Asset("USD"),
                        lower_bound=openpit.param.Pnl("-10"),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                    initial_pnl=openpit.param.Pnl("0"),
                )
            ],
        )
    with pytest.raises(TypeError, match="SpotFundsPnlBoundsBarrier"):
        configurator.spot_funds_pnl_bounds_killswitch(
            policies.SpotFundsBuilder.NAME,
            global_barriers=[object()],
        )
    with pytest.raises(TypeError, match="SpotFundsPnlBoundsAccountBarrierUpdate"):
        configurator.spot_funds_pnl_bounds_killswitch(
            policies.SpotFundsBuilder.NAME,
            account_barriers=[
                policies.SpotFundsPnlBoundsAccountBarrier(
                    barrier=policies.SpotFundsPnlBoundsBarrier(
                        account_currency=openpit.param.Asset("USD"),
                        lower_bound=openpit.param.Pnl("-10"),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                    initial_pnl=openpit.param.Pnl("0"),
                )
            ],
        )
    with pytest.raises(TypeError, match="SpotFundsPricingSource"):
        configurator.spot_funds(policies.SpotFundsBuilder.NAME, pricing_source="Mark")
    with pytest.raises(TypeError, match="SpotFundsOverrideEntry"):
        configurator.spot_funds(
            policies.SpotFundsBuilder.NAME,
            overrides=[(object(), None, None, 10)],
        )
    with pytest.raises(
        TypeError,
        match=r"expected openpit\.pretrade\.policies\.SpotFundsOverride",
    ):
        configurator.spot_funds(
            policies.SpotFundsBuilder.NAME,
            overrides=[
                policies.SpotFundsOverrideEntry(
                    target=policies.SpotFundsOverrideTargetInstrument(
                        instrument=instrument_id,
                    ),
                    override=object(),  # type: ignore[arg-type]
                )
            ],
        )


@pytest.mark.unit
def test_configurator_reports_domain_validation_errors() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(limit=_rate_limit(1))
            )
        )
        .build()
    )

    with pytest.raises(openpit.PolicyConfigureError) as caught:
        engine.configure().rate_limit(
            policies.RateLimitBuilder.NAME,
            broker=policies.RateLimitBrokerBarrier(
                limit=policies.RateLimit(
                    max_orders=1,
                    window=datetime.timedelta(0),
                )
            ),
        )
    assert caught.value.kind == openpit.ConfigureErrorKind.VALIDATION


@pytest.mark.unit
def test_pnl_bounds_builder_rejects_wrong_broker_barrier_type() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(TypeError, match="PnlBoundsBrokerBarrier"):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(policies.build_pnl_bounds_killswitch().broker_barriers(object()))
            .build()
        )


@pytest.mark.unit
def test_spot_funds_pnl_bounds_builder_rejects_wrong_barrier_types() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(TypeError, match="SpotFundsPnlBoundsBarrier"):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(
                policies.build_spot_funds_pnl_bounds_killswitch().global_barriers(
                    object()
                )
            )
            .build()
        )
    with pytest.raises(TypeError, match="SpotFundsPnlBoundsAccountGroupBarrier"):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(
                policies.build_spot_funds_pnl_bounds_killswitch().account_group_barriers(
                    object()
                )
            )
            .build()
        )
    with pytest.raises(TypeError, match="SpotFundsPnlBoundsAccountBarrier"):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(
                policies.build_spot_funds_pnl_bounds_killswitch().account_barriers(
                    object()
                )
            )
            .build()
        )


@pytest.mark.unit
def test_spot_funds_pnl_bounds_builder_surfaces_core_empty_batch_error() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(
        ValueError, match="^spot funds P&L bounds require at least one barrier$"
    ):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(
                policies.build_spot_funds_pnl_bounds_killswitch().global_barriers()
            )
            .build()
        )


@pytest.mark.unit
def test_spot_funds_override_rejects_unknown_target_entity() -> None:
    policies = openpit.pretrade.policies
    service, _ = _market_data()
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                global_slippage_bps=100,
            )
        )
        .build()
    )

    with pytest.raises(
        TypeError,
        match=r"SpotFundsOverrideEntry\.target must be one of",
    ):
        engine.configure().spot_funds(
            policies.SpotFundsBuilder.NAME,
            overrides=[
                policies.SpotFundsOverrideEntry(
                    target=object(),  # type: ignore[arg-type]
                    override=policies.SpotFundsOverride(slippage_bps=10),
                )
            ],
        )
