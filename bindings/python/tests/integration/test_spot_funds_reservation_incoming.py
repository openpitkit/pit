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
the Python binding's Reservation.account_adjustments without any new
binding code - only engine-side behavior is under test."""

import openpit
import pytest


def _build_spot_funds_engine() -> openpit.Engine:
    return (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
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

    # Sell 2 AAPL @ 150: holds 2 AAPL (underlying) and reserves 300 USD incoming.
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
