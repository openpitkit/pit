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
# Please see https://github.com/openpitkit and the OWNERS file for details.


import time
import types
from datetime import timedelta

import openpit
import pytest

# Mirrors public Python examples from:
# - ../pit.wiki/Market-Data.md
# - ../pit.wiki/Market-Data-TTL.md
# - ../pit.wiki/Market-Data-Pricing.md
# If this file changes, update every linked documentation snippet.

# Convenience aliases used throughout this file. account_info is the no-group
# stand-in passed to reads; in policy code this is usually the pre-trade context.
_DEFAULT = openpit.marketdata.QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT
_NO_GROUP = types.SimpleNamespace(account_group=None)


def _seed_usd_balance(amount: int) -> openpit.AccountAdjustment:
    return openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(amount)
            )
        ),
    )


@pytest.mark.integration
def test_example_wiki_market_data_register_push_get() -> None:
    # Used in: pit.wiki/Market-Data.md - Pushing and Reading Quotes
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )

    account_id = openpit.param.AccountId.from_int(1)
    aapl = openpit.Instrument("AAPL", "USD")
    aapl_id = service.register(aapl)

    # Publish a full snapshot into the default ("everyone-else") bucket.
    service.push(
        aapl_id,
        openpit.marketdata.Quote(mark="150", bid="149.5", ask="150.5"),
    )

    # Read for an account with no group: the lookup falls through to the
    # default bucket.
    quote = service.get(aapl_id, account_id, _NO_GROUP, _DEFAULT)
    assert quote is not None
    assert quote.mark == openpit.param.Price("150")
    assert quote.bid == openpit.param.Price("149.5")

    # resolve recovers the id from the instrument name.
    assert service.resolve(aapl) == aapl_id


@pytest.mark.integration
def test_example_wiki_market_data_replace_vs_patch() -> None:
    # Used in: pit.wiki/Market-Data.md - Replace Versus Patch
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    account_id = openpit.param.AccountId.from_int(1)
    aapl_id = service.register(openpit.Instrument("AAPL", "USD"))

    service.push(
        aapl_id,
        openpit.marketdata.Quote(mark="100", bid="99", ask="101"),
    )

    # Patch only the mark; bid and ask are preserved.
    service.push_patch(aapl_id, openpit.marketdata.Quote(mark="105"))

    quote = service.get(aapl_id, account_id, _NO_GROUP, _DEFAULT)
    assert quote.mark == openpit.param.Price("105")
    assert quote.bid == openpit.param.Price("99")
    assert quote.ask == openpit.param.Price("101")


@pytest.mark.integration
def test_example_wiki_market_data_finite_ttl_hides_stale_quote() -> None:
    # Used in: pit.wiki/Market-Data-TTL.md - Quote Freshness
    # A 50 ms service-wide lifetime: quotes older than that read as absent.
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.within(timedelta(milliseconds=50)))
        .build()
    )
    aapl_id = service.register(openpit.Instrument("AAPL", "USD"))

    account_id = openpit.param.AccountId.from_int(1)
    account_info = types.SimpleNamespace(account_group=None)

    def read():
        return service.get(
            aapl_id,
            account_id,
            account_info,
            openpit.marketdata.QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT,
        )

    service.push(aapl_id, openpit.marketdata.Quote(mark="200"))
    assert read() is not None

    # After the lifetime elapses the quote reads as absent.
    time.sleep(0.08)
    assert read() is None

    # A fresh push restores visibility.
    service.push(aapl_id, openpit.marketdata.Quote(mark="205"))
    assert read() is not None


@pytest.mark.integration
def test_example_wiki_market_data_clear_then_recover() -> None:
    # Used in: pit.wiki/Market-Data.md - Clearing a Quote
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    aapl_id = service.register(openpit.Instrument("AAPL", "USD"))

    account_id = openpit.param.AccountId.from_int(1)
    account_info = types.SimpleNamespace(account_group=None)

    service.push(aapl_id, openpit.marketdata.Quote(mark="200"))

    # clear hides the quote but keeps the instrument registered.
    service.clear(aapl_id)
    assert (
        service.get(
            aapl_id,
            account_id,
            account_info,
            openpit.marketdata.QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT,
        )
        is None
    )

    # Pushing again restores a quote for the same id.
    service.push(aapl_id, openpit.marketdata.Quote(mark="210"))
    assert (
        service.get(
            aapl_id,
            account_id,
            account_info,
            openpit.marketdata.QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT,
        )
        is not None
    )


@pytest.mark.integration
def test_example_wiki_market_data_market_orders_book_top_override() -> None:
    # Used in: pit.wiki/Market-Data-Pricing.md - Pricing Market Orders → Python
    builder = openpit.Engine.builder().no_sync()

    # A shared market-data service feeds the policy's market-order pricing.
    market_data = builder.market_data(openpit.marketdata.QuoteTtl.infinite()).build()
    aapl = openpit.Instrument("AAPL", "USD")
    aapl_id = market_data.register(aapl)
    market_data.push(
        aapl_id,
        openpit.marketdata.Quote(mark="200", bid="199.5", ask="200.5"),
    )

    # Price market orders from the top of book (ask for buys, bid for sells).
    # The global slippage is 100 bps, but AAPL overrides it to zero, so a buy
    # is priced exactly at the ask.
    engine = builder.builtin(
        openpit.pretrade.policies.build_spot_funds().market_data(
            market_data,
            global_slippage_bps=100,
            pricing_source=openpit.pretrade.policies.SpotFundsPricingSource.BOOK_TOP,
            overrides=[
                openpit.pretrade.policies.SpotFundsOverrideEntry(
                    target=(
                        openpit.pretrade.policies.SpotFundsOverrideTargetInstrument(
                            instrument=aapl_id
                        )
                    ),
                    override=openpit.pretrade.policies.SpotFundsOverride(
                        slippage_bps=0
                    ),
                )
            ],
        )
    ).build()

    account_id = openpit.param.AccountId.from_int(99224416)
    engine.apply_account_adjustment(
        account_id=account_id, adjustments=[_seed_usd_balance(1000)]
    )

    def market_buy(quantity: str) -> openpit.Order:
        return openpit.Order(
            operation=openpit.OrderOperation(
                instrument=aapl,
                account_id=account_id,
                side=openpit.param.Side.BUY,
                trade_amount=openpit.param.TradeAmount.quantity(quantity),
                price=None,
            ),
        )

    # Market buy (no price): priced at the ask 200.5 because the override pins
    # slippage to zero. The seeded balance covers it, so it passes.
    passed = engine.execute_pre_trade(order=market_buy("1"))
    assert passed.ok
    passed.reservation.commit()

    # A full replace that carries only the mark drops bid and ask. With the
    # BookTop source there is no ask to price a buy, so it is rejected.
    market_data.push(aapl_id, openpit.marketdata.Quote(mark="215"))
    rejected = engine.execute_pre_trade(order=market_buy("1"))
    assert not rejected.ok
    assert (
        rejected.rejects[0].code == openpit.pretrade.RejectCode.MARK_PRICE_UNAVAILABLE
    )


@pytest.mark.integration
def test_example_wiki_market_data_push_for_fan_out() -> None:
    # Used in: pit.wiki/Market-Data.md - Targeted Fan-Out: push for
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    aapl_id = service.register(openpit.Instrument("AAPL", "USD"))

    group_id = openpit.param.AccountGroupId.from_int(7)

    # Fan out to two accounts and one group simultaneously.
    service.push_for(
        aapl_id,
        openpit.marketdata.Quote(mark="150"),
        [
            openpit.param.AccountId.from_int(10),
            openpit.param.AccountId.from_int(11),
        ],
        [group_id],
    )

    # Read back for account 10 under AccountOnly - hits the per-account bucket.
    account_info = types.SimpleNamespace(account_group=None)
    quote = service.get(
        aapl_id,
        openpit.param.AccountId.from_int(10),
        account_info,
        openpit.marketdata.QuoteResolution.ACCOUNT_ONLY,
    )
    assert quote is not None
    assert quote.mark == openpit.param.Price("150")
