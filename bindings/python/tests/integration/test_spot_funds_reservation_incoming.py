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

"""Prove that the incoming bucket on reservation outcomes surfaces through
the Python binding's Reservation.account_adjustments.
Also verify that a confirmed review with an existing hold reserves funds once.
"""

import conftest
import openpit
import pytest


def _build_spot_funds_engine() -> openpit.Engine:
    return (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
    )


def _spot_funds_amounts(
    engine: openpit.Engine, account: openpit.param.AccountId, asset: str
) -> tuple[openpit.param.PositionSize, ...]:
    zero = openpit.param.AdjustmentAmount.delta(openpit.param.PositionSize(0))
    result = engine.apply_account_adjustment(
        account,
        [
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset=asset),
                amount=openpit.AccountAdjustmentAmount(
                    balance=zero, held=zero, incoming=zero
                ),
            )
        ],
    )
    assert result.ok
    assert len(result.outcomes) == 1
    entry = result.outcomes[0].entry
    assert entry.asset == asset
    assert entry.balance is not None
    assert entry.held is not None
    assert entry.incoming is not None
    return entry.balance.absolute, entry.held.absolute, entry.incoming.absolute


def _cancel_report(account, lock, remaining: int) -> openpit.ExecutionReport:
    report = conftest.make_report(pnl=openpit.param.Pnl(0), account_id=account)
    return openpit.ExecutionReport(
        operation=report.operation,
        fill=openpit.ExecutionReportFillDetails(
            lock=lock,
            remaining_reserved_quantity=openpit.param.Quantity(remaining),
            is_final=True,
        ),
    )


def _seed_balance(
    engine: openpit.Engine,
    account_id: openpit.param.AccountId,
    asset: str,
    amount: int,
) -> None:
    adj = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset=asset),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(amount)
            )
        ),
    )
    result = engine.apply_account_adjustment(account_id=account_id, adjustments=[adj])
    assert result.ok, f"seed {asset} balance failed: {result}"


@pytest.mark.integration
def test_spot_funds_buy_reservation_base_incoming_surfaces() -> None:
    """A buy reservation emits a base-asset entry carrying incoming (delta and
    absolute equal to the ordered quantity).  The settlement-asset entry carries
    held only; the base-asset entry carries incoming only."""
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()

    # Seed 5000 USD so the buy reserve can proceed.
    _seed_balance(engine, account_id, "USD", 5000)

    # Buy 3 AAPL @ 100: holds 300 USD and reserves 3 AAPL incoming.
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity("3"),
            price=openpit.param.Price("100"),
        ),
    )
    result = engine.execute_pre_trade(order=order)
    assert result.ok, f"ExecutePreTrade failed: {result.rejects}"
    reservation = result.reservation

    # Read adjustments before finalizing; rollback releases the held state.
    adjustments = reservation.account_adjustments
    reservation.rollback()

    # Two entries: [0] settlement (USD) held, [1] base (AAPL) incoming.
    assert (
        len(adjustments) == 2
    ), f"account_adjustments len = {len(adjustments)}, want 2"

    # Entry 0: settlement asset USD with held set, incoming absent.
    usd_entry = adjustments[0].entry
    assert (
        usd_entry.asset == "USD"
    ), f"adjustments[0].asset = {usd_entry.asset!r}, want 'USD'"
    assert (
        usd_entry.held is not None
    ), "adjustments[0].held is None, want set for settlement leg"
    # Held delta = +300 (price 100 * qty 3).
    assert usd_entry.held.delta == openpit.param.PositionSize(
        "300"
    ), f"USD held delta = {usd_entry.held.delta}, want 300"
    assert (
        usd_entry.incoming is None
    ), "adjustments[0].incoming is set, want None for settlement entry"

    # Entry 1: base asset AAPL with incoming set, held absent.
    aapl_entry = adjustments[1].entry
    assert (
        aapl_entry.asset == "AAPL"
    ), f"adjustments[1].asset = {aapl_entry.asset!r}, want 'AAPL'"
    assert (
        aapl_entry.incoming is not None
    ), "adjustments[1].incoming is None, want set for base leg of buy"
    # Incoming delta = +3 (the ordered quantity).
    assert aapl_entry.incoming.delta == openpit.param.PositionSize(
        "3"
    ), f"AAPL incoming delta = {aapl_entry.incoming.delta}, want 3"
    assert aapl_entry.incoming.absolute == openpit.param.PositionSize(
        "3"
    ), f"AAPL incoming absolute = {aapl_entry.incoming.absolute}, want 3"
    assert (
        aapl_entry.held is None
    ), "adjustments[1].held is set, want None for base entry"


@pytest.mark.integration
def test_spot_funds_sell_reservation_quote_incoming_surfaces() -> None:
    """A priced sell reservation emits a settlement-asset entry carrying
    incoming (expected proceeds = price * qty).  The underlying-asset entry
    carries held only; the settlement-asset entry carries incoming only
    (positive price case)."""
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()

    # Seed 5 AAPL so the sell can proceed.
    _seed_balance(engine, account_id, "AAPL", 5)

    # Sell 2 AAPL @ 150: holds 2 AAPL (underlying) and reserves 300 USD
    # incoming.
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.SELL,
            trade_amount=openpit.param.TradeAmount.quantity("2"),
            price=openpit.param.Price("150"),
        ),
    )
    result = engine.execute_pre_trade(order=order)
    assert result.ok, f"ExecutePreTrade failed: {result.rejects}"
    reservation = result.reservation

    # Read adjustments before finalizing; rollback releases the held state.
    adjustments = reservation.account_adjustments
    reservation.rollback()

    # Two entries: [0] underlying (AAPL) held, [1] settlement (USD) incoming.
    assert (
        len(adjustments) == 2
    ), f"account_adjustments len = {len(adjustments)}, want 2"

    # Entry 0: underlying asset AAPL with held set, incoming absent.
    aapl_entry = adjustments[0].entry
    assert (
        aapl_entry.asset == "AAPL"
    ), f"adjustments[0].asset = {aapl_entry.asset!r}, want 'AAPL'"
    assert (
        aapl_entry.held is not None
    ), "adjustments[0].held is None, want set for underlying sell leg"
    assert aapl_entry.held.delta == openpit.param.PositionSize(
        "2"
    ), f"AAPL held delta = {aapl_entry.held.delta}, want 2 (qty sold)"
    assert (
        aapl_entry.incoming is None
    ), "adjustments[0].incoming is set, want None for underlying entry"

    # Entry 1: settlement asset USD with incoming set, held absent (price > 0).
    usd_entry = adjustments[1].entry
    assert (
        usd_entry.asset == "USD"
    ), f"adjustments[1].asset = {usd_entry.asset!r}, want 'USD'"
    assert (
        usd_entry.incoming is not None
    ), "adjustments[1].incoming is None, want set for settlement leg of priced sell"
    # Expected proceeds: price 150 * qty 2 = 300 USD.
    assert usd_entry.incoming.delta == openpit.param.PositionSize(
        "300"
    ), f"USD incoming delta = {usd_entry.incoming.delta}, want 300 (150 * 2)"
    assert usd_entry.incoming.absolute == openpit.param.PositionSize(
        "300"
    ), f"USD incoming absolute = {usd_entry.incoming.absolute}, want 300"
    assert (
        usd_entry.held is None
    ), "adjustments[1].held is set, want None for settlement incoming entry (price > 0)"


@pytest.mark.integration
def test_spot_funds_priceless_sell_without_market_data_bundle_rejects() -> None:
    """A price-less sell in limit-only mode rejects as an unsupported market
    order and leaves the seeded balance available for a later priced sell."""
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()

    # Seed 5 AAPL.
    _seed_balance(engine, account_id, "AAPL", 5)

    # Sell 2 AAPL with no price in limit-only mode: the order is a market
    # order, and no market-data bundle is configured to price it.
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.SELL,
            trade_amount=openpit.param.TradeAmount.quantity("2"),
            price=None,
        ),
    )
    result = engine.execute_pre_trade(order=order)
    assert not result.ok
    assert result.reservation is None
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.UNSUPPORTED_ORDER_TYPE

    # The rejected market order must not consume the seeded AAPL balance.
    priced_order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.SELL,
            trade_amount=openpit.param.TradeAmount.quantity("5"),
            price=openpit.param.Price("100"),
        ),
    )
    priced_result = engine.execute_pre_trade(order=priced_order)
    assert priced_result.ok, f"priced sell failed after reject: {priced_result.rejects}"
    priced_result.reservation.rollback()


@pytest.mark.integration
def test_spot_funds_already_held_does_not_reserve_again() -> None:
    """Keep the first review hold through rerun, then release its exact legs."""
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()
    policies = openpit.pretrade.policies
    engine.configure().spot_funds(
        policies.SpotFundsBuilder.NAME,
        global_limit_mode=policies.SpotFundsLimitMode.REVIEW_WITH_HOLD,
    )
    _seed_balance(engine, account_id, "USD", 500)

    price = openpit.param.Price("100")
    order = conftest.make_order(
        account_id=account_id,
        trade_amount=openpit.param.TradeAmount.quantity("6"),
        price=price,
    )
    operation = order.operation
    assert operation is not None, "expected the initial buy order's operation"
    initial = engine.execute_pre_trade(order=order)
    assert initial.ok, f"expected initial 600 USD order to pass: {initial.rejects}"
    reservation = initial.reservation
    assert reservation is not None, "expected the initial 600 USD reservation"
    assert [(mark.code, mark.mode) for mark in initial.reviews] == [
        (
            openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS,
            openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
        )
    ]
    first_lock = reservation.lock
    assert len(first_lock) == 1
    assert [px for _, px in first_lock.entries()] == [price]
    reservation.commit()
    usd = _spot_funds_amounts(engine, account_id, "USD")
    aapl = _spot_funds_amounts(engine, account_id, "AAPL")
    assert usd == tuple(openpit.param.PositionSize(v) for v in (-100, 600, 0))
    assert aapl == tuple(openpit.param.PositionSize(v) for v in (0, 0, 6))

    held = engine.execute_pre_trade(
        order=openpit.Order(
            operation=operation,
            review=openpit.OrderReview(confirmed=True, already_held=True),
        )
    )
    assert held.ok, f"expected already-held order to pass: {held.rejects}"
    reservation = held.reservation
    assert reservation is not None, "expected the already-held order's reservation"
    assert len(reservation.lock) == 0
    assert held.reviews == reservation.reviews == []
    assert reservation.account_adjustments == []
    reservation.commit()
    assert _spot_funds_amounts(engine, account_id, "USD") == usd
    assert _spot_funds_amounts(engine, account_id, "AAPL") == aapl

    report = _cancel_report(account_id, first_lock, 6)
    post = engine.apply_execution_report(report=report)
    assert post.account_blocks == []
    assert _spot_funds_amounts(engine, account_id, "USD") == tuple(
        openpit.param.PositionSize(v) for v in (500, 0, 0)
    )
    assert _spot_funds_amounts(engine, account_id, "AAPL") == tuple(
        openpit.param.PositionSize(0) for _ in range(3)
    )


@pytest.mark.integration
@pytest.mark.parametrize("cause", ["funds", "pnl"])
def test_spot_funds_already_held_reject_keeps_first_hold(cause: str) -> None:
    policies = openpit.pretrade.policies
    account = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()
    engine.accounts().set_currency(account, openpit.param.Asset("USD"))
    _seed_balance(engine, account, "USD", 1000)
    order = conftest.make_order(
        account_id=account,
        price=openpit.param.Price(100),
        trade_amount=openpit.param.TradeAmount.quantity(6),
    )
    first = engine.execute_pre_trade(order=order)
    assert first.ok and first.reservation is not None
    first.reservation.commit()

    rejected = engine.execute_pre_trade(
        order=openpit.Order(
            operation=order.operation, review=openpit.OrderReview(confirmed=True)
        )
    )
    assert rejected.reservation is None
    assert [(r.code, r.mode) for r in rejected.rejects] == [
        (
            openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS,
            openpit.pretrade.RuleMode.REJECT,
        )
    ]
    rerun = openpit.Order(
        operation=order.operation,
        review=openpit.OrderReview(confirmed=True, already_held=True),
    )
    held = engine.execute_pre_trade(order=rerun)
    assert held.ok and held.reservation is not None
    assert len(held.reservation.lock) == 0
    assert held.reviews == held.reservation.reviews == []
    assert held.reservation.account_adjustments == []
    held.reservation.commit()
    assert _spot_funds_amounts(engine, account, "USD") == tuple(
        openpit.param.PositionSize(v) for v in (400, 600, 0)
    )
    remaining = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account,
            price=openpit.param.Price(100),
            trade_amount=openpit.param.TradeAmount.quantity(4),
        )
    )
    assert remaining.ok and remaining.reservation is not None
    remaining.reservation.rollback()

    if cause == "funds":
        _seed_balance(engine, account, "USD", -1)
        expected = openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS
    else:
        engine.configure().spot_funds_pnl_bounds_killswitch(
            policies.SpotFundsBuilder.NAME,
            account_barriers=[
                policies.SpotFundsPnlBoundsAccountBarrier(
                    policies.SpotFundsPnlBoundsBarrier(
                        currency=openpit.param.Asset("USD"),
                        lower_bound=openpit.param.Pnl(-10),
                    ),
                    account,
                )
            ],
        )
        configured = engine.configure().set_spot_funds_account_pnl(
            policies.SpotFundsBuilder.NAME,
            account=account,
            state=openpit.param.Pnl(-20),
        )
        assert len(configured.account_blocks) == 1
        engine.accounts().unblock(account)
        expected = openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    before = _spot_funds_amounts(engine, account, "USD")
    expected_balance = -1 if cause == "funds" else 400
    assert before == tuple(
        openpit.param.PositionSize(v) for v in (expected_balance, 600, 0)
    )
    rejected = engine.execute_pre_trade(order=rerun)
    assert rejected.reservation is None
    assert [(r.code, r.mode) for r in rejected.rejects] == [
        (expected, openpit.pretrade.RuleMode.REJECT)
    ]
    assert _spot_funds_amounts(engine, account, "USD") == before


@pytest.mark.integration
def test_spot_funds_cancel_with_rerun_lock_blocks_without_releasing_usd() -> None:
    policies = openpit.pretrade.policies
    account = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()
    engine.configure().spot_funds(
        policies.SpotFundsBuilder.NAME,
        global_limit_mode=policies.SpotFundsLimitMode.REVIEW_WITH_HOLD,
    )
    _seed_balance(engine, account, "USD", 500)
    order = conftest.make_order(
        account_id=account,
        price=openpit.param.Price(100),
        trade_amount=openpit.param.TradeAmount.quantity(6),
    )
    first = engine.execute_pre_trade(order=order)
    assert first.ok and first.reservation is not None
    first.reservation.commit()
    rerun = engine.execute_pre_trade(
        order=openpit.Order(
            operation=order.operation,
            review=openpit.OrderReview(confirmed=True, already_held=True),
        )
    )
    assert rerun.ok and rerun.reservation is not None
    lock = rerun.reservation.lock
    assert len(lock) == 0
    assert rerun.reviews == rerun.reservation.reviews == []
    assert rerun.reservation.account_adjustments == []
    rerun.reservation.commit()
    before = _spot_funds_amounts(engine, account, "USD")
    assert before == tuple(openpit.param.PositionSize(v) for v in (-100, 600, 0))
    report = _cancel_report(account, lock, 6)
    post = engine.apply_execution_report(report=report)
    assert [block.code for block in post.account_blocks] == [
        openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD
    ]
    assert _spot_funds_amounts(engine, account, "USD") == before


@pytest.mark.integration
def test_spot_funds_already_held_omits_breached_review_rules() -> None:
    policies = openpit.pretrade.policies
    account = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()
    engine.accounts().set_currency(account, openpit.param.Asset("USD"))
    _seed_balance(engine, account, "USD", 1000)
    order = conftest.make_order(account_id=account)
    first = engine.execute_pre_trade(order=order)
    assert first.ok and first.reservation is not None
    first.reservation.commit()
    engine.configure().spot_funds_pnl_bounds_killswitch(
        policies.SpotFundsBuilder.NAME,
        global_barriers=[
            policies.SpotFundsPnlBoundsBarrier(
                currency=openpit.param.Asset("USD"),
                lower_bound=openpit.param.Pnl(-10),
                mode=openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
            )
        ],
    )
    assert (
        not engine.configure()
        .set_spot_funds_account_pnl(
            policies.SpotFundsBuilder.NAME,
            account=account,
            state=openpit.param.Pnl(-20),
        )
        .account_blocks
    )
    engine.configure().spot_funds(
        policies.SpotFundsBuilder.NAME,
        position_limits=[
            policies.SpotFundsPositionLimitEntry(
                account,
                openpit.param.Asset("AAPL"),
                openpit.param.Quantity(0),
                mode=openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD,
            )
        ],
    )
    dry = engine.execute_pre_trade_dry_run(order=order)
    assert dry
    assert {(m.code, m.mode) for m in dry.reviews} == {
        (
            openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED,
            openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
        ),
        (
            openpit.pretrade.RejectCode.POSITION_LIMIT_EXCEEDED,
            openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD,
        ),
    }
    assert len(dry.reviews) == 2
    before = {
        asset: _spot_funds_amounts(engine, account, asset) for asset in ("USD", "AAPL")
    }
    rerun = engine.execute_pre_trade(
        order=openpit.Order(
            operation=order.operation,
            review=openpit.OrderReview(confirmed=True, already_held=True),
        )
    )
    assert rerun.ok and rerun.reservation is not None
    assert len(rerun.reservation.lock) == 0
    assert rerun.reviews == rerun.reservation.reviews == []
    assert rerun.reservation.account_adjustments == []
    rerun.reservation.commit()
    for asset, amounts in before.items():
        assert _spot_funds_amounts(engine, account, asset) == amounts


@pytest.mark.integration
@pytest.mark.parametrize(
    "mode",
    [
        openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITH_HOLD,
        openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITHOUT_HOLD,
    ],
)
@pytest.mark.parametrize("drop_copy", [False, True])
def test_spot_funds_confirmed_and_drop_copy_track_shortfall(
    mode: openpit.pretrade.policies.SpotFundsLimitMode, drop_copy: bool
) -> None:
    policies = openpit.pretrade.policies
    account = openpit.param.AccountId.from_int(99224416)
    engine = _build_spot_funds_engine()
    engine.configure().spot_funds(
        policies.SpotFundsBuilder.NAME,
        global_limit_mode=mode,
        position_limits=(
            [
                policies.SpotFundsPositionLimitEntry(
                    account, openpit.param.Asset("AAPL"), openpit.param.Quantity(0)
                )
            ]
            if drop_copy
            else []
        ),
    )
    _seed_balance(engine, account, "USD", 500)
    order = conftest.make_order(
        account_id=account,
        price=openpit.param.Price(100),
        trade_amount=openpit.param.TradeAmount.quantity(6),
    )
    if drop_copy:
        dry = engine.execute_pre_trade_dry_run(order=order)
        assert [(r.code, r.mode) for r in dry.rejects] == [
            (
                openpit.pretrade.RejectCode.POSITION_LIMIT_EXCEEDED,
                openpit.pretrade.RuleMode.REJECT,
            )
        ]
        result = engine.apply_drop_copy(order=order)
        assert result.ok and result.rejects == []
        handle = result.operation
    else:
        order = openpit.Order(
            operation=order.operation, review=openpit.OrderReview(confirmed=True)
        )
        dry = engine.execute_pre_trade_dry_run(order=order)
        assert dry and dry.reviews == []
        result = engine.execute_pre_trade(order=order)
        assert result.ok and result.rejects == result.reviews == []
        handle = result.reservation
    assert handle is not None
    assert len(handle.lock) == 1
    assert [px for _, px in handle.lock.entries()] == [openpit.param.Price(100)]
    assert len(handle.account_adjustments) == 2
    entries = {
        outcome.entry.asset: outcome.entry for outcome in handle.account_adjustments
    }
    assert set(entries) == {"USD", "AAPL"}
    assert entries["USD"].balance.delta == openpit.param.PositionSize(-600)
    assert entries["USD"].balance.absolute == openpit.param.PositionSize(-100)
    assert entries["USD"].held.delta == openpit.param.PositionSize(600)
    assert entries["USD"].held.absolute == openpit.param.PositionSize(600)
    assert entries["AAPL"].incoming.delta == openpit.param.PositionSize(6)
    assert entries["AAPL"].incoming.absolute == openpit.param.PositionSize(6)
    handle.commit()
    assert _spot_funds_amounts(engine, account, "USD") == tuple(
        openpit.param.PositionSize(v) for v in (-100, 600, 0)
    )
    assert _spot_funds_amounts(engine, account, "AAPL") == tuple(
        openpit.param.PositionSize(v) for v in (0, 0, 6)
    )
    if drop_copy:
        engine.configure().spot_funds(
            policies.SpotFundsBuilder.NAME,
            position_limits=[
                policies.SpotFundsPositionLimitEntry(
                    account, openpit.param.Asset("AAPL"), None
                )
            ],
        )
    next_order = engine.execute_pre_trade(order=conftest.make_order(account_id=account))
    assert next_order.ok and next_order.reservation is not None
    assert [(m.code, m.mode) for m in next_order.reviews] == [
        (openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS, mode.value)
    ]
    next_order.reservation.rollback()


@pytest.mark.integration
@pytest.mark.parametrize(
    "global_mode,group_mode,account_mode",
    [
        (
            openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITH_HOLD,
            openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITHOUT_HOLD,
            openpit.pretrade.policies.SpotFundsLimitMode.ENFORCE,
        ),
        (
            openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITHOUT_HOLD,
            openpit.pretrade.policies.SpotFundsLimitMode.ENFORCE,
            openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITH_HOLD,
        ),
        (
            openpit.pretrade.policies.SpotFundsLimitMode.ENFORCE,
            openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITH_HOLD,
            openpit.pretrade.policies.SpotFundsLimitMode.REVIEW_WITHOUT_HOLD,
        ),
    ],
)
def test_spot_funds_review_limit_mode_cascade(
    global_mode: openpit.pretrade.policies.SpotFundsLimitMode,
    group_mode: openpit.pretrade.policies.SpotFundsLimitMode,
    account_mode: openpit.pretrade.policies.SpotFundsLimitMode,
) -> None:
    policies = openpit.pretrade.policies
    account = openpit.param.AccountId.from_int(99224416)
    group = openpit.param.AccountGroupId.from_int(7)
    engine = _build_spot_funds_engine()
    engine.accounts().register_group([account], group)
    engine.configure().spot_funds(
        policies.SpotFundsBuilder.NAME,
        global_limit_mode=global_mode,
        account_group_limit_modes=[
            policies.SpotFundsLimitModeAccountGroupEntry(group, group_mode)
        ],
        account_limit_modes=[
            policies.SpotFundsLimitModeAccountEntry(account, account_mode)
        ],
    )
    order = conftest.make_order(account_id=account)
    for tier, mode in (
        ("account", account_mode),
        ("group", group_mode),
        ("global", global_mode),
    ):
        result = engine.execute_pre_trade(order=order)
        if mode is policies.SpotFundsLimitMode.ENFORCE:
            assert result.reservation is None
            assert [(r.code, r.mode) for r in result.rejects] == [
                (
                    openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS,
                    openpit.pretrade.RuleMode.REJECT,
                )
            ]
        else:
            assert result.ok and result.reservation is not None
            assert [(m.code, m.mode) for m in result.reviews] == [
                (openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS, mode.value)
            ]
            expected = (
                (-10, 10, 0)
                if mode is policies.SpotFundsLimitMode.REVIEW_WITH_HOLD
                else (0, 0, 0)
            )
            assert _spot_funds_amounts(engine, account, "USD") == tuple(
                openpit.param.PositionSize(v) for v in expected
            )
            result.reservation.rollback()
        if tier == "account":
            engine.configure().spot_funds(
                policies.SpotFundsBuilder.NAME,
                account_limit_modes=[
                    policies.SpotFundsLimitModeAccountEntry(account, None)
                ],
            )
        elif tier == "group":
            engine.configure().spot_funds(
                policies.SpotFundsBuilder.NAME,
                account_group_limit_modes=[
                    policies.SpotFundsLimitModeAccountGroupEntry(group, None)
                ],
            )
