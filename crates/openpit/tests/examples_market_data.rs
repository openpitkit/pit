// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://github.com/openpitkit and the OWNERS file for details.

// Mirrors public Rust examples from:
// - ../pit.wiki/Market-Data.md
// - ../pit.wiki/Market-Data-TTL.md
// - ../pit.wiki/Market-Data-Pricing.md
// If this file changes, update every linked documentation snippet.

#[test]
fn example_wiki_market_data_register_push_get() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Market-Data.md - Pushing and Reading Quotes
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountGroupId, AccountId, Asset, Price};
    use openpit::{Engine, Instrument, Quote, QuoteResolution, QuoteTtl};

    let service = Engine::builder::<(), (), ()>()
        .no_sync()
        .market_data(QuoteTtl::Infinite)
        .build();

    let aapl = Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?);
    let aapl_id = service.register(aapl.clone())?;

    // Publish a full snapshot into the default ("everyone-else") bucket.
    service.push(
        aapl_id,
        Quote::new()
            .with_mark(Price::from_str("150")?)
            .with_bid(Price::from_str("149.5")?)
            .with_ask(Price::from_str("150.5")?),
    )?;

    // Read for an account with no group: the lookup falls through to the
    // default bucket.
    let account = AccountId::from_u64(1);
    let quote = service
        .get(
            aapl_id,
            account,
            &None::<AccountGroupId>,
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .expect("quote must be present");
    assert_eq!(quote.mark, Some(Price::from_str("150")?));
    assert_eq!(quote.bid, Some(Price::from_str("149.5")?));

    // resolve recovers the id from the instrument name.
    assert_eq!(service.resolve(&aapl), Some(aapl_id));
    Ok(())
}

#[test]
fn example_wiki_market_data_replace_vs_patch() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Market-Data.md - Replace Versus Patch
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountGroupId, AccountId, Asset, Price};
    use openpit::{Engine, Instrument, Quote, QuoteResolution, QuoteTtl};

    let service = Engine::builder::<(), (), ()>()
        .no_sync()
        .market_data(QuoteTtl::Infinite)
        .build();
    let aapl_id = service.register(Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?))?;

    service.push(
        aapl_id,
        Quote::new()
            .with_mark(Price::from_str("100")?)
            .with_bid(Price::from_str("99")?)
            .with_ask(Price::from_str("101")?),
    )?;

    // Patch only the mark; bid and ask are preserved.
    service.push_patch(aapl_id, Quote::new().with_mark(Price::from_str("105")?))?;

    let account = AccountId::from_u64(1);
    let quote = service
        .get(
            aapl_id,
            account,
            &None::<AccountGroupId>,
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .expect("quote must be present");
    assert_eq!(quote.mark, Some(Price::from_str("105")?));
    assert_eq!(quote.bid, Some(Price::from_str("99")?));
    assert_eq!(quote.ask, Some(Price::from_str("101")?));
    Ok(())
}

#[test]
fn example_wiki_market_data_finite_ttl_hides_stale_quote() -> Result<(), Box<dyn std::error::Error>>
{
    // Wiki example: pit.wiki/Market-Data-TTL.md - Quote Freshness
    // Keep this example in sync with the matching wiki example.
    use std::time::Duration;

    use openpit::param::{AccountGroupId, AccountId, Asset, Price};
    use openpit::{Engine, Instrument, Quote, QuoteResolution, QuoteTtl};

    // A 50 ms service-wide lifetime: quotes older than that read as absent.
    let service = Engine::builder::<(), (), ()>()
        .no_sync()
        .market_data(QuoteTtl::Within(Duration::from_millis(50)))
        .build();
    let aapl_id = service.register(Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?))?;

    let account = AccountId::from_u64(1);
    let read = |id| {
        service
            .get(
                id,
                account,
                &None::<AccountGroupId>,
                QuoteResolution::AccountThenGroupThenDefault,
            )
            .ok()
    };

    service.push(aapl_id, Quote::new().with_mark(Price::from_str("200")?))?;
    assert!(read(aapl_id).is_some());

    // After the lifetime elapses the quote is hidden until the next push.
    std::thread::sleep(Duration::from_millis(80));
    assert!(read(aapl_id).is_none());

    // A fresh push restores visibility.
    service.push(aapl_id, Quote::new().with_mark(Price::from_str("205")?))?;
    let quote = read(aapl_id).expect("quote must be present");
    assert_eq!(quote.mark, Some(Price::from_str("205")?));
    Ok(())
}

#[test]
fn example_wiki_market_data_clear_then_recover() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Market-Data.md - Clearing a Quote
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountGroupId, AccountId, Asset, Price};
    use openpit::{Engine, Instrument, Quote, QuoteResolution, QuoteTtl};

    let service = Engine::builder::<(), (), ()>()
        .no_sync()
        .market_data(QuoteTtl::Infinite)
        .build();
    let aapl_id = service.register(Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?))?;

    let account = AccountId::from_u64(1);
    let read = |id| {
        service
            .get(
                id,
                account,
                &None::<AccountGroupId>,
                QuoteResolution::AccountThenGroupThenDefault,
            )
            .ok()
    };

    service.push(aapl_id, Quote::new().with_mark(Price::from_str("200")?))?;
    assert!(read(aapl_id).is_some());

    // clear hides the quote but keeps the instrument registered.
    service.clear(aapl_id);
    assert!(read(aapl_id).is_none());

    // Pushing again restores a quote for the same id.
    service.push(aapl_id, Quote::new().with_mark(Price::from_str("210")?))?;
    let quote = read(aapl_id).expect("quote must be present");
    assert_eq!(quote.mark, Some(Price::from_str("210")?));
    Ok(())
}

#[test]
fn example_wiki_market_data_market_orders_book_top_override(
) -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Market-Data-Pricing.md - Pricing Market Orders
    // Keep this example in sync with the matching wiki example.
    use std::sync::Arc;

    use openpit::param::{
        AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side, TradeAmount,
    };
    use openpit::pretrade::policies::{SpotFundsPolicy, SpotFundsSettings};
    use openpit::pretrade::RejectCode;
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        Engine, FullSync, Instrument, OrderOperation, Quote, QuoteTtl, SpotFundsMarketData,
        SpotFundsOverride, SpotFundsOverrideTarget, SpotFundsPricingSource,
        WithAccountAdjustmentAmount, WithAccountAdjustmentBalanceOperation,
        WithAccountAdjustmentBounds, WithExecutionReportFillDetails, WithExecutionReportOperation,
    };

    type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
    type SpotAdjustment = WithAccountAdjustmentAmount<
        WithAccountAdjustmentBounds<
            WithAccountAdjustmentBalanceOperation<openpit::AccountAdjustmentAmount>,
        >,
    >;

    let builder = Engine::builder::<OrderOperation, SpotReport, SpotAdjustment>().full_sync();

    // A shared market-data service feeds the policy's market-order pricing.
    let market_data = builder.market_data(QuoteTtl::Infinite).build();
    let aapl = Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?);
    let aapl_id = market_data.register(aapl.clone())?;
    market_data.push(
        aapl_id,
        Quote::new()
            .with_mark(Price::from_str("200")?)
            .with_bid(Price::from_str("199.5")?)
            .with_ask(Price::from_str("200.5")?),
    )?;

    // Price market orders from the top of book (ask for buys, bid for sells).
    // The global slippage is 100 bps, but AAPL overrides it to zero, so a buy
    // is priced exactly at the ask.
    let settings = SpotFundsSettings::new(
        100,
        SpotFundsPricingSource::BookTop,
        [(
            SpotFundsOverrideTarget::Instrument(aapl_id),
            SpotFundsOverride {
                slippage_bps: Some(0),
            },
        )],
    )?;
    let bundle = SpotFundsMarketData::new(Arc::clone(&market_data));
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        settings,
        Some(bundle),
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let account = AccountId::from_u64(99224416);
    let seed = WithAccountAdjustmentAmount {
        inner: WithAccountAdjustmentBounds {
            inner: WithAccountAdjustmentBalanceOperation {
                inner: AccountAdjustmentAmount::default(),
                operation: AccountAdjustmentBalanceOperation {
                    asset: Asset::new("USD")?,
                    average_entry_price: None,
                },
            },
            bounds: AccountAdjustmentBounds::default(),
        },
        amount: AccountAdjustmentAmount {
            balance: Some(AdjustmentAmount::Absolute(PositionSize::from_str("1000")?)),
            held: None,
            incoming: None,
        },
    };
    engine.apply_account_adjustment(account, &[seed])?;

    // Market buy (no price): priced at the ask 200.5 because the override
    // pins slippage to zero. The seeded balance covers it, so it passes.
    let buy = |quantity: &str| OrderOperation {
        instrument: aapl.clone(),
        account_id: account,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str(quantity).expect("valid quantity")),
        price: None,
    };
    engine.execute_pre_trade(buy("1"))?.commit();

    // A full replace that carries only the mark drops bid and ask. With the
    // BookTop source there is no ask to price a buy, so it is rejected.
    market_data.push(aapl_id, Quote::new().with_mark(Price::from_str("215")?))?;
    let rejects = match engine.execute_pre_trade(buy("1")) {
        Ok(_) => panic!("market buy must reject when the ask is missing"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::MarkPriceUnavailable);
    Ok(())
}

#[test]
fn example_wiki_market_data_push_for_fan_out() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Market-Data.md - Targeted Fan-Out: push for
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountGroupId, AccountId, Asset, Price};
    use openpit::{Engine, Instrument, Quote, QuoteResolution, QuoteTtl};

    let service = Engine::builder::<(), (), ()>()
        .no_sync()
        .market_data(QuoteTtl::Infinite)
        .build();
    let aapl_id = service.register(Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?))?;

    let group_id = AccountGroupId::from_u32(7)?;

    // Fan out to two accounts and one group simultaneously.
    service.push_for(
        aapl_id,
        Quote::new().with_mark(Price::from_str("150")?),
        &[AccountId::from_u64(10), AccountId::from_u64(11)],
        &[group_id],
    )?;

    // Read back for account 10 under AccountOnly - hits the per-account bucket.
    let quote = service
        .get(
            aapl_id,
            AccountId::from_u64(10),
            &None::<AccountGroupId>,
            QuoteResolution::AccountOnly,
        )
        .expect("quote must be present");
    assert_eq!(quote.mark, Some(Price::from_str("150")?));
    Ok(())
}
