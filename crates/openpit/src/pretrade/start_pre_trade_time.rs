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
// Please see https://openpit.dev and the OWNERS file for details.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Deref;

use crate::core::{Instrument, InstrumentId};
use crate::marketdata::{MarketDataError, Quote, QuoteResolution};
use crate::param::AccountId;
use crate::time::Instant;

thread_local! {
    static START_PRE_TRADE_NOW: Cell<Option<Instant>> = const { Cell::new(None) };
    static CALL_QUOTES: RefCell<Option<CallQuotes>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct CallQuotes {
    services: HashMap<usize, ServiceQuotes>,
}

struct ServiceQuotes {
    _owner: Box<dyn Any>,
    instruments: HashMap<Instrument, Option<InstrumentId>>,
    quotes: HashMap<(InstrumentId, AccountId, QuoteResolution), Result<Quote, MarketDataError>>,
}

impl CallQuotes {
    fn service<Owner: Clone + Deref<Target = u8> + 'static>(
        &mut self,
        owner: &Owner,
    ) -> &mut ServiceQuotes {
        self.services
            .entry(&**owner as *const u8 as usize)
            .or_insert_with(|| ServiceQuotes {
                _owner: Box::new(owner.clone()),
                instruments: HashMap::new(),
                quotes: HashMap::new(),
            })
    }
}

/// A fresh quote cache for one engine entry. Nested calls to a different engine
/// restore the outer cache, including when a callback unwinds.
pub(crate) struct CallQuotesGuard(Option<CallQuotes>);

pub(crate) fn call_quotes() -> CallQuotesGuard {
    CallQuotesGuard(CALL_QUOTES.with(|slot| slot.replace(Some(CallQuotes::default()))))
}

impl Drop for CallQuotesGuard {
    fn drop(&mut self) {
        CALL_QUOTES.with(|slot| slot.replace(self.0.take()));
    }
}

pub(crate) fn cached_instrument<Owner: Clone + Deref<Target = u8> + 'static>(
    owner: &Owner,
    instrument: &Instrument,
    read: impl FnOnce() -> Option<InstrumentId>,
) -> Option<InstrumentId> {
    let identity = &**owner as *const u8 as usize;
    let cached = CALL_QUOTES.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|cache| cache.services.get(&identity))
            .and_then(|service| service.instruments.get(instrument).copied())
    });
    if let Some(value) = cached {
        return value;
    }
    let value = read();
    CALL_QUOTES.with(|slot| {
        if let Some(cache) = slot.borrow_mut().as_mut() {
            cache
                .service(owner)
                .instruments
                .insert(instrument.clone(), value);
        }
    });
    value
}

pub(crate) fn cached_quote<Owner: Clone + Deref<Target = u8> + 'static>(
    owner: &Owner,
    instrument: InstrumentId,
    account: AccountId,
    resolution: QuoteResolution,
    read: impl FnOnce() -> Result<Quote, MarketDataError>,
) -> Result<Quote, MarketDataError> {
    let identity = &**owner as *const u8 as usize;
    let key = (instrument, account, resolution);
    let cached = CALL_QUOTES.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|cache| cache.services.get(&identity))
            .and_then(|service| service.quotes.get(&key).copied())
    });
    if let Some(value) = cached {
        return value;
    }
    let value = read();
    CALL_QUOTES.with(|slot| {
        if let Some(cache) = slot.borrow_mut().as_mut() {
            cache.service(owner).quotes.insert(key, value);
        }
    });
    value
}

struct StartPreTradeNowGuard<'a> {
    slot: &'a Cell<Option<Instant>>,
    previous: Option<Instant>,
}

impl Drop for StartPreTradeNowGuard<'_> {
    fn drop(&mut self) {
        self.slot.set(self.previous);
    }
}

pub(crate) fn with_start_pre_trade_now<Output>(now: Instant, f: impl FnOnce() -> Output) -> Output {
    START_PRE_TRADE_NOW.with(|slot| {
        let previous = slot.replace(Some(now));
        let _guard = StartPreTradeNowGuard { slot, previous };
        f()
    })
}

pub(crate) fn start_pre_trade_now() -> Instant {
    START_PRE_TRADE_NOW
        .with(|slot| slot.get())
        .unwrap_or_else(Instant::now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param::Asset;

    #[test]
    fn quote_results_and_errors_are_pinned_only_for_one_call() {
        let owner = std::rc::Rc::new(0_u8);
        let account = AccountId::from_u64(7);
        let instrument = InstrumentId::new(1);
        let resolution = QuoteResolution::AccountThenGroupThenDefault;
        let reads = Cell::new(0);
        for error in [
            MarketDataError::QuoteUnavailable,
            MarketDataError::QuoteExpired(Quote::default()),
        ] {
            {
                let _scope = call_quotes();
                let first = cached_quote(&owner, instrument, account, resolution, || {
                    reads.set(reads.get() + 1);
                    Err(error)
                });
                let second = cached_quote(&owner, instrument, account, resolution, || {
                    reads.set(reads.get() + 1);
                    Ok(Quote::default())
                });
                assert_eq!(first, Err(error));
                assert_eq!(
                    second, first,
                    "the first quote result, including stale payload or error, must be retained"
                );
            }
            let _execute = call_quotes();
            assert_eq!(
                cached_quote(&owner, instrument, account, resolution, || {
                    reads.set(reads.get() + 1);
                    Ok(Quote::default())
                }),
                Ok(Quote::default()),
                "execute must use a fresh quote cache"
            );
        }
        assert_eq!(reads.get(), 4, "each call reads a pair at most once");
    }

    #[test]
    fn instrument_resolution_is_shared_by_pair_and_missing_results_are_cached() {
        let owner = std::rc::Rc::new(0_u8);
        let pair = Instrument::new(Asset::new("EUR").unwrap(), Asset::new("USD").unwrap());
        let reads = Cell::new(0);
        let _scope = call_quotes();
        for _ in 0..2 {
            assert_eq!(
                cached_instrument(&owner, &pair, || {
                    reads.set(reads.get() + 1);
                    None
                }),
                None
            );
        }
        assert_eq!(
            reads.get(),
            1,
            "execution and settlement-currency fee must resolve one pair once"
        );
    }

    #[test]
    fn quote_resolution_remains_part_of_the_query_and_repeats_stay_pinned() {
        use crate::marketdata::{MarketDataBuilder, QuoteTtl};
        use crate::param::Price;
        use std::time::Duration;
        let service = MarketDataBuilder::<crate::FullSync>::new(QuoteTtl::Infinite).build();
        let id = service
            .register(Instrument::new(
                Asset::new("EUR").unwrap(),
                Asset::new("USD").unwrap(),
            ))
            .unwrap();
        let account = AccountId::from_u64(7);
        let original = Quote::new().with_mark(Price::from_str("100").unwrap());
        let account_quote = Quote::new().with_mark(Price::from_str("200").unwrap());
        service.push(id, original, Duration::ZERO).unwrap();
        {
            let _call = call_quotes();
            assert_eq!(
                service.get(id, account, &None, QuoteResolution::AccountOnly),
                Err(MarketDataError::QuoteUnavailable)
            );
            assert_eq!(
                service.get(
                    id,
                    account,
                    &None,
                    QuoteResolution::AccountThenGroupThenDefault
                ),
                Ok(original),
                "different resolutions must retain their bucket-selection semantics"
            );
            service
                .push_for(id, account_quote, Duration::ZERO, &[account], &[])
                .unwrap();
            service.push(id, Quote::default(), Duration::ZERO).unwrap();
            assert_eq!(
                service.get(id, account, &None, QuoteResolution::AccountOnly),
                Err(MarketDataError::QuoteUnavailable),
                "repeat of a query must retain its first error"
            );
            assert_eq!(
                service.get(
                    id,
                    account,
                    &None,
                    QuoteResolution::AccountThenGroupThenDefault
                ),
                Ok(original),
                "repeat of a query must retain its first quote"
            );
        }
        let _call = call_quotes();
        assert_eq!(
            service.get(id, account, &None, QuoteResolution::AccountOnly),
            Ok(account_quote)
        );
        assert_eq!(
            service.get(
                id,
                account,
                &None,
                QuoteResolution::AccountThenGroupThenDefault
            ),
            Ok(account_quote)
        );
    }

    #[test]
    fn service_identity_survives_moves_and_replacements_until_scope_end() {
        use crate::marketdata::{MarketDataBuilder, MarketDataService, QuoteTtl};
        use crate::param::Price;
        use std::rc::Rc;
        use std::time::Duration;
        let pair = Instrument::new(Asset::new("EUR").unwrap(), Asset::new("USD").unwrap());
        let account = AccountId::from_u64(7);
        let resolution = QuoteResolution::AccountThenGroupThenDefault;
        let quote = |value| Quote::new().with_mark(Price::from_str(value).unwrap());
        let make = |id, value: Option<Quote>| -> MarketDataService<crate::LocalSync> {
            let service = MarketDataBuilder::<crate::LocalSync>::new(QuoteTtl::Infinite).build();
            service
                .register_with_id(pair.clone(), InstrumentId::new(id))
                .unwrap();
            if let Some(value) = value {
                service
                    .push(InstrumentId::new(id), value, Duration::ZERO)
                    .unwrap();
            }
            Rc::try_unwrap(service).ok().unwrap()
        };
        let mut slots = [make(1, Some(quote("100"))), make(77, Some(quote("200")))];
        let retired;
        {
            let _scope = call_quotes();
            assert_eq!(slots[0].resolve(&pair), Some(InstrumentId::new(1)));
            assert_eq!(
                slots[0].get(InstrumentId::new(1), account, &None, resolution),
                Ok(quote("100"))
            );
            slots.swap(0, 1);
            assert_eq!(
                slots[0].resolve(&pair),
                Some(InstrumentId::new(77)),
                "a swapped service must keep its own instrument id"
            );
            assert_eq!(
                slots[0].get(InstrumentId::new(77), account, &None, resolution),
                Ok(quote("200"))
            );
            slots[1]
                .push(InstrumentId::new(1), quote("300"), Duration::ZERO)
                .unwrap();
            assert_eq!(
                slots[1].get(InstrumentId::new(1), account, &None, resolution),
                Ok(quote("100")),
                "moving a service must not escape its first quote"
            );
            retired = Rc::downgrade(&slots[0].cache_identity);
            slots[0] = make(77, None);
            assert!(
                retired.upgrade().is_some(),
                "cached entries must retain the retired service identity"
            );
            assert_eq!(
                slots[0].get(InstrumentId::new(77), account, &None, resolution),
                Err(MarketDataError::QuoteUnavailable),
                "a replacement must not inherit another service's quote"
            );
        }
        assert!(
            retired.upgrade().is_none(),
            "the identity lease ends with its call"
        );
        let _next = call_quotes();
        assert_eq!(
            slots[1].get(InstrumentId::new(1), account, &None, resolution),
            Ok(quote("300"))
        );
    }

    #[test]
    fn many_distinct_queries_keep_their_own_first_results() {
        let owner = std::rc::Rc::new(0_u8);
        let _scope = call_quotes();
        for index in 1..=2048 {
            let pair = Instrument::new(
                Asset::new(format!("A{index}")).unwrap(),
                Asset::new("USD").unwrap(),
            );
            let id = InstrumentId::new(index);
            assert_eq!(cached_instrument(&owner, &pair, || Some(id)), Some(id));
            assert_eq!(cached_instrument(&owner, &pair, || None), Some(id));
            let error = if index % 2 == 0 {
                MarketDataError::QuoteUnavailable
            } else {
                MarketDataError::QuoteExpired(Quote::default())
            };
            assert_eq!(
                cached_quote(
                    &owner,
                    id,
                    AccountId::from_u64(index),
                    QuoteResolution::AccountOnly,
                    || Err(error)
                ),
                Err(error)
            );
        }
        for index in (1..=2048).rev() {
            let error = if index % 2 == 0 {
                MarketDataError::QuoteUnavailable
            } else {
                MarketDataError::QuoteExpired(Quote::default())
            };
            assert_eq!(
                cached_quote(
                    &owner,
                    InstrumentId::new(index),
                    AccountId::from_u64(index),
                    QuoteResolution::AccountOnly,
                    || Ok(Quote::default())
                ),
                Err(error)
            );
        }
    }

    #[test]
    fn cache_hits_do_not_resolve_unused_group_metadata() {
        use crate::marketdata::{AccountInfo, MarketDataBuilder, QuoteTtl};
        use crate::param::{AccountGroupId, Price};
        use std::time::Duration;
        struct NoGroup;
        impl AccountInfo for NoGroup {
            fn group(&self) -> Option<AccountGroupId> {
                panic!("this account-only query must not resolve group metadata")
            }
        }
        let service = MarketDataBuilder::<crate::LocalSync>::new(QuoteTtl::Infinite).build();
        let pair = Instrument::new(Asset::new("EUR").unwrap(), Asset::new("USD").unwrap());
        let id = service.register(pair).unwrap();
        let account = AccountId::from_u64(7);
        service
            .set_instrument_account_ttl(id, account, QuoteTtl::Infinite)
            .unwrap();
        let quote = Quote::new().with_mark(Price::from_str("100").unwrap());
        service
            .push_for(id, quote, Duration::ZERO, &[account], &[])
            .unwrap();
        let _scope = call_quotes();
        assert_eq!(
            service.get(id, account, &NoGroup, QuoteResolution::AccountOnly),
            Ok(quote)
        );
        service
            .push_for(id, Quote::default(), Duration::ZERO, &[account], &[])
            .unwrap();
        assert_eq!(
            service.get(id, account, &NoGroup, QuoteResolution::AccountOnly),
            Ok(quote)
        );
    }
}
