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

use std::cell::RefCell;
use std::rc::Rc;

use openpit::param::{AccountGroupId, AccountId, Asset, Quantity, Side, TradeAmount};
use openpit::pretrade::{PolicyPreTradeResult, PreTradeContext, PreTradePolicy, Rejects};
use openpit::storage::NoLocking;
use openpit::{
    AccountRetirementRefusal, Accounts, Engine, LocalEngine, LocalSync, Mutation, Mutations,
    OrderOperation,
};

fn order() -> OrderOperation {
    OrderOperation {
        instrument: openpit::Instrument::new(
            Asset::new("AAPL").unwrap(),
            Asset::new("USD").unwrap(),
        ),
        account_id: AccountId::from_u64(1),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("1").unwrap()),
        price: None,
    }
}

struct CallbackPolicy {
    callback: Rc<dyn Fn()>,
    on_start: bool,
    refuse_retirement: bool,
}

impl CallbackPolicy {
    fn append_mutation(&self, mutations: &mut Mutations) {
        let commit = self.callback.clone();
        let rollback = self.callback.clone();
        mutations.push(Mutation::new(move || commit(), move || rollback()));
    }
}

impl PreTradePolicy<OrderOperation, (), (), LocalSync> for CallbackPolicy {
    fn name(&self) -> &str {
        "CallbackPolicy"
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<NoLocking>,
        _order: &OrderOperation,
    ) -> Result<(), Rejects> {
        if self.on_start {
            (self.callback)();
        }
        Ok(())
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext<NoLocking>,
        _order: &OrderOperation,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        if !self.on_start {
            self.append_mutation(mutations);
        }
        Ok(None)
    }

    fn retire_account(
        &self,
        _account: AccountId,
        mutations: &mut Mutations,
    ) -> Result<(), AccountRetirementRefusal> {
        self.append_mutation(mutations);
        if self.refuse_retirement {
            Err(AccountRetirementRefusal::NonZeroState)
        } else {
            Ok(())
        }
    }
}

fn callback_engine(
    on_start: bool,
    refuse_retirement: bool,
    callback: impl Fn(&Accounts<NoLocking>) + 'static,
) -> LocalEngine<OrderOperation> {
    let accounts = Rc::new(RefCell::new(None::<Accounts<NoLocking>>));
    let callback_accounts = accounts.clone();
    let engine = Engine::builder::<OrderOperation, (), ()>()
        .no_sync()
        .pre_trade(CallbackPolicy {
            callback: Rc::new(move || {
                callback(callback_accounts.borrow().as_ref().unwrap());
            }),
            on_start,
            refuse_retirement,
        })
        .build()
        .unwrap();
    *accounts.borrow_mut() = Some(engine.accounts());
    engine
}

#[test]
fn policy_callback_allows_lease_free_calls() {
    let engine = callback_engine(true, false, |accounts| {
        let account = AccountId::from_u64(1);
        let group = AccountGroupId::from_u32(1).unwrap();
        assert_eq!(accounts.group_of(account), Some(group));
        accounts.block(account, "callback".to_owned());
        accounts.unblock(account);
        accounts.block_group(group, "callback".to_owned()).unwrap();
        accounts.unblock_group(group).unwrap();
        accounts.unblock_all();
    });
    engine
        .accounts()
        .register_group(
            &[AccountId::from_u64(1)],
            AccountGroupId::from_u32(1).unwrap(),
        )
        .unwrap();
    engine.start_pre_trade(order()).unwrap();
}

#[test]
fn policy_callback_allows_an_independent_engine() {
    let other = callback_engine(false, false, |_| {});
    let engine = callback_engine(true, false, move |_| {
        let mut reservation = other.execute_pre_trade(order()).unwrap();
        reservation.commit();
        other.configure();
        other.accounts().set_group_currency(
            AccountGroupId::from_u32(1).unwrap(),
            Asset::new("USD").unwrap(),
        );
    });
    engine.start_pre_trade(order()).unwrap();
}

#[cfg(debug_assertions)]
mod debug {
    use super::*;
    use std::convert::Infallible;
    use std::time::Duration;

    use openpit::param::Pnl;
    use openpit::pretrade::policies::{
        RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
    };
    use openpit::storage::FullLocking;
    use openpit::{ExecutionReportOperation, FullSyncEngine};

    fn engine() -> FullSyncEngine<OrderOperation, ExecutionReportOperation> {
        let builder = Engine::builder::<OrderOperation, ExecutionReportOperation, ()>().full_sync();
        let settings = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 100,
                    window: Duration::from_secs(60),
                },
            }),
            [],
            [],
            [],
        )
        .unwrap();
        let policy = RateLimitPolicy::<FullLocking>::new(settings, builder.storage_builder());
        builder.pre_trade(policy).build().unwrap()
    }

    fn during_configuration<T>(
        engine: &FullSyncEngine<OrderOperation, ExecutionReportOperation>,
        callback: impl FnOnce() -> T,
    ) -> T {
        let mut value = None;
        engine
            .configure()
            .rate_limit::<Infallible>(RateLimitPolicy::<FullLocking>::NAME, |_| {
                value = Some(callback());
                Ok(())
            })
            .unwrap();
        value.expect("configuration must run its callback")
    }

    macro_rules! reentry_test {
        ($name:ident, |$engine:ident| $call:expr) => {
            reentry_test!($name, |$engine| (), |_| $call);
        };
        ($name:ident, |$engine:ident| $prepare:expr, |$value:pat_param| $call:expr) => {
            #[test]
            #[should_panic(expected = "callback re-entry contract violated")]
            fn $name() {
                let $engine = engine();
                let $value = $prepare;
                // Kept past the callback: dropping a returned handle inside it
                // would roll back and panic on its own, hiding a missing check
                // on the call itself.
                let _kept = during_configuration(&$engine, || $call);
            }
        };
    }

    reentry_test!(start, |engine| engine.start_pre_trade(order()));
    reentry_test!(execute, |engine| engine.execute_pre_trade(order()));
    reentry_test!(start_dry_run, |engine| engine
        .start_pre_trade_dry_run(order()));
    reentry_test!(execute_dry_run, |engine| engine
        .execute_pre_trade_dry_run(order()));
    reentry_test!(drop_copy, |engine| engine.apply_drop_copy(order()));
    reentry_test!(execution_report, |engine| engine.apply_execution_report(
        &ExecutionReportOperation {
            instrument: order().instrument,
            account_id: AccountId::from_u64(1),
            side: Side::Buy,
        }
    ));
    reentry_test!(adjustment, |engine| engine
        .apply_account_adjustment(AccountId::from_u64(1), &[()]));
    reentry_test!(retirement, |engine| engine
        .retire_account(AccountId::from_u64(1)));
    reentry_test!(register_group, |engine| engine
        .accounts()
        .register_group(&[], AccountGroupId::from_u32(1).unwrap()));
    reentry_test!(unregister_group, |engine| engine
        .accounts()
        .unregister_group(&[], AccountGroupId::from_u32(1).unwrap()));
    reentry_test!(set_currency, |engine| engine
        .accounts()
        .set_currency(AccountId::from_u64(1), Asset::new("USD").unwrap()));
    reentry_test!(clear_currency, |engine| engine
        .accounts()
        .clear_currency(AccountId::from_u64(1)));
    reentry_test!(set_group_currency, |engine| engine
        .accounts()
        .set_group_currency(
            AccountGroupId::from_u32(1).unwrap(),
            Asset::new("USD").unwrap()
        ));
    reentry_test!(clear_group_currency, |engine| engine
        .accounts()
        .clear_group_currency(AccountGroupId::from_u32(1).unwrap()));
    reentry_test!(configure_rate, |engine| engine
        .configure()
        .rate_limit::<Infallible>("missing", |_| Ok(())));
    reentry_test!(configure_pnl, |engine| engine
        .configure()
        .pnl_bounds_killswitch::<Infallible>("missing", |_| Ok(())));
    reentry_test!(configure_spot, |engine| engine
        .configure()
        .spot_funds::<Infallible>("missing", |_| Ok(())));
    reentry_test!(configure_size, |engine| engine
        .configure()
        .order_size_limit::<Infallible>("missing", |_| Ok(())));
    reentry_test!(set_pnl, |engine| engine.configure().set_account_pnl(
        "missing",
        AccountId::from_u64(1),
        Asset::new("USD").unwrap(),
        Pnl::ZERO
    ));
    reentry_test!(set_spot_pnl, |engine| engine
        .configure()
        .set_spot_funds_account_pnl(
            "missing",
            AccountId::from_u64(1),
            openpit::PnlState::Value(Pnl::ZERO)
        ));
    reentry_test!(
        request,
        |engine| engine.start_pre_trade(order()).unwrap(),
        |request| request.execute()
    );
    reentry_test!(
        commit,
        |engine| engine.execute_pre_trade(order()).unwrap(),
        |mut reservation| reservation.commit()
    );
    reentry_test!(
        rollback,
        |engine| engine.execute_pre_trade(order()).unwrap(),
        |mut reservation| reservation.rollback()
    );
    reentry_test!(
        implicit_rollback,
        |engine| engine.execute_pre_trade(order()).unwrap(),
        |reservation| drop(reservation)
    );
    reentry_test!(
        drop_copy_commit,
        |engine| engine.apply_drop_copy(order()).unwrap(),
        |mut operation| operation.commit()
    );
    reentry_test!(
        drop_copy_rollback,
        |engine| engine.apply_drop_copy(order()).unwrap(),
        |mut operation| operation.rollback()
    );
    reentry_test!(
        drop_copy_implicit_rollback,
        |engine| engine.apply_drop_copy(order()).unwrap(),
        |operation| drop(operation)
    );

    macro_rules! callback_test {
        ($name:ident, $start:expr, $refuse:expr, |$engine:ident| $call:expr) => {
            #[test]
            #[should_panic(expected = "callback re-entry contract violated")]
            fn $name() {
                let $engine = callback_engine($start, $refuse, |accounts| {
                    accounts
                        .register_group(&[], AccountGroupId::from_u32(1).unwrap())
                        .unwrap();
                });
                $call;
            }
        };
    }

    callback_test!(policy_callback, true, false, |engine| engine
        .start_pre_trade(order())
        .unwrap());
    callback_test!(commit_callback, false, false, |engine| engine
        .execute_pre_trade(order())
        .unwrap()
        .commit());
    callback_test!(rollback_callback, false, false, |engine| engine
        .execute_pre_trade(order())
        .unwrap()
        .rollback());
    callback_test!(implicit_rollback_callback, false, false, |engine| drop(
        engine.execute_pre_trade(order()).unwrap()
    ));
    callback_test!(drop_copy_commit_callback, false, false, |engine| engine
        .apply_drop_copy(order())
        .unwrap()
        .commit());
    callback_test!(drop_copy_rollback_callback, false, false, |engine| engine
        .apply_drop_copy(order())
        .unwrap()
        .rollback());
    callback_test!(
        drop_copy_implicit_rollback_callback,
        false,
        false,
        |engine| drop(engine.apply_drop_copy(order()).unwrap())
    );
    callback_test!(retirement_release_callback, false, false, |engine| engine
        .retire_account(AccountId::from_u64(1))
        .unwrap());
    callback_test!(retirement_restore_callback, false, true, |engine| engine
        .retire_account(AccountId::from_u64(1))
        .unwrap());

    #[test]
    fn allowed_calls_and_unwind_leave_the_detector_clean() {
        let engine = engine();
        during_configuration(&engine, || {
            engine.configure();
            engine.accounts().group_of(AccountId::from_u64(1));
            engine
                .accounts()
                .block(AccountId::from_u64(1), "callback".to_owned());
            engine.accounts().unblock(AccountId::from_u64(1));
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            during_configuration(&engine, || {
                let _ = engine.start_pre_trade(order());
            });
        }));
        assert!(panic.is_err());
        engine.execute_pre_trade(order()).unwrap().commit();
        during_configuration(&engine, || {});
    }
}
