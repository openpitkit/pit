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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use openpit::param::{AccountGroupId, AccountId, Asset};
use openpit::pretrade::{
    AccountBlock, PolicyPreTradeResult, PostTradeContext, PostTradeResult, PreTradeContext,
    PreTradePolicy, RejectCode, Rejects,
};
use openpit::{Engine, HasAccountId, RequestFieldAccessError, SyncMode};

#[test]
fn operation_contexts_preserve_send_in_full_and_account_sync() {
    fn send<T: Send>() {}
    fn contexts<F>()
    where
        F: openpit::storage::LockingPolicyFactory
            + openpit::storage::CreateStorageFor<AccountId>
            + 'static,
        openpit::OperationSnapshot<F>: Send,
        openpit::SharedOperationSnapshot<F>: Send,
        PostTradeContext<F>: Send,
        openpit::AccountAdjustmentContext<F>: Send,
    {
        send::<openpit::OperationSnapshot<F>>();
        send::<openpit::SharedOperationSnapshot<F>>();
        send::<PostTradeContext<F>>();
        send::<openpit::AccountAdjustmentContext<F>>();
    }
    contexts::<openpit::storage::FullLocking>();
    contexts::<openpit::storage::IndexLocking<openpit::AccountKeyConstraint>>();
    fn sync<T: Sync>() {}
    sync::<openpit::SharedOperationSnapshot<openpit::storage::FullLocking>>();
}

struct AccountPayload {
    reads: Arc<AtomicUsize>,
    first: Result<AccountId, RequestFieldAccessError>,
    second: Result<AccountId, RequestFieldAccessError>,
}

impl HasAccountId for AccountPayload {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        if self.reads.fetch_add(1, Ordering::SeqCst) == 0 {
            self.first.clone()
        } else {
            self.second.clone()
        }
    }
}

struct AccountCapture {
    seen: Arc<Mutex<Vec<Result<AccountId, RequestFieldAccessError>>>>,
    block: bool,
}

impl<Sync: SyncMode> PreTradePolicy<AccountPayload, AccountPayload, (), Sync> for AccountCapture {
    fn name(&self) -> &str {
        "account-capture"
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<Sync::StorageLockingPolicyFactory>,
        _: &AccountPayload,
    ) -> Option<PostTradeResult> {
        self.seen
            .lock()
            .unwrap()
            .extend([ctx.account_id(), ctx.account_id()]);
        self.block.then(|| {
            PostTradeResult::blocks_only(vec![AccountBlock::new(
                "account-capture",
                RejectCode::PnlKillSwitchTriggered,
                "stopped",
                "",
            )])
        })
    }
}

#[test]
fn report_account_is_read_once_and_blocks_only_its_operation_account() {
    let account = AccountId::from_u64(7);
    let other = AccountId::from_u64(9);
    for second in [Ok(other), Err(RequestFieldAccessError::new("account_id"))] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::builder::<AccountPayload, AccountPayload, ()>()
            .no_sync()
            .pre_trade(AccountCapture {
                seen: seen.clone(),
                block: true,
            })
            .build()
            .unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        engine.apply_execution_report(&AccountPayload {
            reads: reads.clone(),
            first: Ok(account),
            second,
        });
        assert_eq!(
            reads.load(Ordering::SeqCst),
            1,
            "report account must be read exactly once"
        );
        assert_eq!(*seen.lock().unwrap(), vec![Ok(account), Ok(account)]);
        assert!(engine
            .start_pre_trade(AccountPayload {
                reads: Arc::new(AtomicUsize::new(0)),
                first: Ok(account),
                second: Ok(other)
            })
            .is_err());
        assert!(engine
            .start_pre_trade(AccountPayload {
                reads: Arc::new(AtomicUsize::new(0)),
                first: Ok(other),
                second: Ok(account)
            })
            .is_ok());
    }
}

#[test]
fn standalone_post_context_captures_valid_or_unreadable_account_once() {
    for first in [
        Ok(AccountId::from_u64(7)),
        Err(RequestFieldAccessError::new("account_id")),
    ] {
        let reads = Arc::new(AtomicUsize::new(0));
        let report = AccountPayload {
            reads: reads.clone(),
            first: first.clone(),
            second: Ok(AccountId::from_u64(9)),
        };
        let ctx = PostTradeContext::<openpit::storage::NoLocking>::new(&report);
        assert_eq!(ctx.account_id(), first);
        assert_eq!(ctx.account_id(), first);
        assert_eq!(
            reads.load(Ordering::SeqCst),
            1,
            "standalone contexts must capture the report account exactly once"
        );
    }
}

#[test]
fn unreadable_report_account_blocks_globally_only_when_a_policy_blocks() {
    for block in [false, true] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::builder::<AccountPayload, AccountPayload, ()>()
            .no_sync()
            .pre_trade(AccountCapture {
                seen: seen.clone(),
                block,
            })
            .build()
            .unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        let error = RequestFieldAccessError::new("account_id");
        engine.apply_execution_report(&AccountPayload {
            reads: reads.clone(),
            first: Err(error.clone()),
            second: Ok(AccountId::from_u64(1)),
        });
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(*seen.lock().unwrap(), vec![Err(error.clone()), Err(error)]);
        let result = engine.start_pre_trade(AccountPayload {
            reads: Arc::new(AtomicUsize::new(0)),
            first: Ok(AccountId::from_u64(1)),
            second: Ok(AccountId::from_u64(2)),
        });
        assert_eq!(
            result.is_err(),
            block,
            "unreadable account alone must not arm a global block"
        );
    }
}

type State = Option<AccountGroupId>;
struct SnapshotCapture {
    seen: Arc<Mutex<Vec<State>>>,
    resolve_at_start: bool,
}

impl<Sync: SyncMode> PreTradePolicy<AccountPayload, (), (), Sync> for SnapshotCapture {
    fn name(&self) -> &str {
        "snapshot-capture"
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<Sync::StorageLockingPolicyFactory>,
        _: &AccountPayload,
    ) -> Result<(), Rejects> {
        if self.resolve_at_start {
            self.seen.lock().unwrap().push(ctx.account_group());
        }
        Ok(())
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<Sync::StorageLockingPolicyFactory>,
        _: &AccountPayload,
        _: &mut openpit::Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.seen.lock().unwrap().push(ctx.account_group());
        Ok(None)
    }
}

#[test]
fn pre_trade_snapshot_retains_resolved_values_and_leaves_unrequested_values_lazy() {
    let account = AccountId::from_u64(7);
    let before = AccountGroupId::from_u32(3).unwrap();
    let after = AccountGroupId::from_u32(4).unwrap();
    let usd = Asset::new("USD").unwrap();
    let eur = Asset::new("EUR").unwrap();
    for resolve_at_start in [false, true] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::builder::<AccountPayload, (), ()>()
            .no_sync()
            .pre_trade(SnapshotCapture {
                seen: seen.clone(),
                resolve_at_start,
            })
            .build()
            .unwrap();
        let accounts = engine.accounts();
        accounts.register_group(&[account], before).unwrap();
        accounts.set_group_currency(before, usd.clone());
        accounts.set_group_currency(after, eur.clone());
        let reads = Arc::new(AtomicUsize::new(0));
        let request = engine
            .start_pre_trade(AccountPayload {
                reads: reads.clone(),
                first: Ok(account),
                second: Err(RequestFieldAccessError::new("account_id")),
            })
            .unwrap();
        accounts.unregister_group(&[account], before).unwrap();
        accounts.register_group(&[account], after).unwrap();
        request.execute().unwrap().rollback();
        let expected = if resolve_at_start {
            Some(before)
        } else {
            Some(after)
        };
        assert_eq!(
            seen.lock().unwrap().last(),
            Some(&expected),
            "optional values must be pinned only on first use"
        );
        assert_eq!(
            reads.load(Ordering::SeqCst),
            1,
            "execution must retain the start account"
        );
    }
}

struct QuoteCapture {
    service: Arc<openpit::marketdata::MarketDataService<openpit::FullSync>>,
    instrument: openpit::InstrumentId,
    seen: Arc<Mutex<Vec<openpit::param::Price>>>,
    next: openpit::param::Price,
    name: &'static str,
}

impl QuoteCapture {
    fn read(&self, ctx: &PreTradeContext<openpit::storage::FullLocking>) {
        let quote = self
            .service
            .get(
                self.instrument,
                ctx.account_id().unwrap(),
                ctx,
                openpit::marketdata::QuoteResolution::AccountThenGroupThenDefault,
            )
            .unwrap();
        self.seen.lock().unwrap().push(quote.mark.unwrap());
        self.service
            .push(
                self.instrument,
                openpit::marketdata::Quote::new().with_mark(self.next),
                std::time::Duration::ZERO,
            )
            .unwrap();
    }
}

impl PreTradePolicy<AccountPayload, (), (), openpit::FullSync> for QuoteCapture {
    fn name(&self) -> &str {
        self.name
    }
    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<openpit::storage::FullLocking>,
        _: &AccountPayload,
    ) -> Result<(), Rejects> {
        self.read(ctx);
        Ok(())
    }
    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<openpit::storage::FullLocking>,
        _: &AccountPayload,
        _: &mut openpit::Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.read(ctx);
        Ok(None)
    }
}

#[test]
fn policies_share_one_quote_and_execute_gets_a_fresh_quote_cache() {
    use openpit::param::Price;
    let builder = Engine::builder::<AccountPayload, (), ()>().full_sync();
    let service = builder
        .market_data(openpit::marketdata::QuoteTtl::Infinite)
        .build();
    let instrument = service
        .register(openpit::Instrument::new(
            Asset::new("AAPL").unwrap(),
            Asset::new("USD").unwrap(),
        ))
        .unwrap();
    let price = |value: &str| Price::from_str(value).unwrap();
    service
        .push(
            instrument,
            openpit::marketdata::Quote::new().with_mark(price("100")),
            std::time::Duration::ZERO,
        )
        .unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let engine = builder
        .pre_trade(QuoteCapture {
            service: service.clone(),
            instrument,
            seen: seen.clone(),
            next: price("200"),
            name: "first",
        })
        .pre_trade(QuoteCapture {
            service,
            instrument,
            seen: seen.clone(),
            next: price("300"),
            name: "second",
        })
        .build()
        .unwrap();
    let request = engine
        .start_pre_trade(AccountPayload {
            reads: Arc::new(AtomicUsize::new(0)),
            first: Ok(AccountId::from_u64(7)),
            second: Err(RequestFieldAccessError::new("account_id")),
        })
        .unwrap();
    request.execute().unwrap().rollback();
    assert_eq!(
        *seen.lock().unwrap(),
        vec![price("100"), price("100"), price("300"), price("300")],
        "policies share each call's first quote, and execute resets the start cache"
    );
}
