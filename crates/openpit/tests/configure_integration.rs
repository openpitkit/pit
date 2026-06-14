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

// Engine-side runtime reconfiguration: builds a FullSync engine with various
// built-in policies, then drives each through `Engine::configure()`.
// Covers rate-limit retune, P&L-bounds kill-switch (including pre-barrier
// accumulation guarantee), order-size-limit retune, spot-funds retune, the
// three `ConfigureError` paths, and the per-mode `Send`/`Sync` contract of
// `Configurator`.

use std::time::Duration;

use openpit::param::{
    AccountId, AdjustmentAmount, Asset, Fee, Pnl, PositionSize, Price, Quantity, Side, TradeAmount,
    Volume,
};
use openpit::pretrade::policies::pnl_bounds_killswitch::PnlBoundsAccountAssetBarrierUpdate;
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitPolicyError,
    OrderSizeLimitSettings, PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier,
    PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchPolicyError, PnlBoundsKillSwitchSettings,
    RateLimit, RateLimitAssetBarrier, RateLimitBrokerBarrier, RateLimitPolicy,
    RateLimitPolicyError, RateLimitSettings, SpotFundsPolicy, SpotFundsPricingSource,
    SpotFundsSettings,
};
use openpit::pretrade::PreTradePolicy;
use openpit::storage::{FullLocking, IndexLocking, NoLocking};
use openpit::{
    AccountKeyConstraint, AccountSync, AccountSyncEngine, Configurator, ConfigureError, Engine,
    ExecutionReportOperation, FinancialImpact, FullSync, FullSyncEngine,
    HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceLowerBound,
    HasAccountAdjustmentBalanceUpperBound, HasAccountAdjustmentHeld,
    HasAccountAdjustmentHeldLowerBound, HasAccountAdjustmentHeldUpperBound,
    HasAccountAdjustmentIncoming, HasAccountAdjustmentIncomingLowerBound,
    HasAccountAdjustmentIncomingUpperBound, HasBalanceAsset, Instrument, LocalEngine, LocalSync,
    OrderOperation, RequestFieldAccessError, SpotFundsConfigError, SpotFundsMarketData,
    WithExecutionReportFillDetails, WithExecutionReportOperation, WithFinancialImpact,
};

type AccountLocking = IndexLocking<AccountKeyConstraint>;

struct CustomPolicy;

impl PreTradePolicy<OrderOperation, (), (), LocalSync> for CustomPolicy {
    fn name(&self) -> &str {
        "CustomPolicy"
    }
}

fn order(account: u64) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(account),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("1").expect("quantity literal must be valid"),
        ),
        price: None,
    }
}

fn broker_settings(max_orders: usize) -> RateLimitSettings {
    RateLimitSettings::new(
        Some(RateLimitBrokerBarrier {
            limit: RateLimit {
                max_orders,
                // Wide window so every order in the test shares one window.
                window: Duration::from_secs(60),
            },
        }),
        [],
        [],
        [],
    )
    .expect("broker barrier is a valid configuration")
}

fn build_engine(max_orders: usize) -> FullSyncEngine<OrderOperation> {
    let builder = Engine::builder::<OrderOperation, (), ()>().full_sync();
    let policy =
        RateLimitPolicy::<FullLocking>::new(broker_settings(max_orders), builder.storage_builder());
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

fn build_local_engine(max_orders: usize) -> LocalEngine<OrderOperation> {
    let builder = Engine::builder::<OrderOperation, (), ()>().no_sync();
    let policy =
        RateLimitPolicy::<NoLocking>::new(broker_settings(max_orders), builder.storage_builder());
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

fn build_account_engine(max_orders: usize) -> AccountSyncEngine<OrderOperation> {
    let builder = Engine::builder::<OrderOperation, (), ()>().account_sync();
    let policy = RateLimitPolicy::<AccountLocking>::new(
        broker_settings(max_orders),
        builder.storage_builder(),
    );
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

#[test]
fn configure_retunes_live_policy_behavior() {
    let engine = build_engine(5);
    let name = RateLimitPolicy::<FullLocking>::NAME;

    // Generous limit of 5: the first three orders pass (broker count 1..=3).
    for account in 0..3 {
        engine
            .execute_pre_trade(order(account))
            .expect("order within the generous limit must pass");
    }

    // Tighten the broker limit to 2 through the engine. The fourth order
    // (count 4) would have passed under the old limit of 5, so its rejection
    // proves the live policy now reads the new value.
    engine
        .configure()
        .rate_limit::<RateLimitPolicyError>(name, |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 2,
                    window: Duration::from_secs(60),
                },
            }))
        })
        .expect("retune must publish");

    let rejects = match engine.execute_pre_trade(order(3)) {
        Ok(_) => panic!("order beyond the tightened limit must be rejected"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].reason, "rate limit exceeded: broker barrier");
}

#[test]
fn configure_adds_asset_barrier_at_runtime() {
    // Headline capability: a barrier on a previously-unconfigured axis is
    // added through the engine without a rebuild and enforced immediately.
    let engine = build_engine(100);
    let name = RateLimitPolicy::<FullLocking>::NAME;

    engine
        .configure()
        .rate_limit::<RateLimitPolicyError>(name, |settings| {
            settings.set_asset_barriers([RateLimitAssetBarrier {
                settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }])
        })
        .expect("adding an asset barrier must publish");

    // The new USD barrier admits one order, then rejects the next.
    engine
        .execute_pre_trade(order(0))
        .expect("first USD order under the new barrier must pass");
    let rejects = match engine.execute_pre_trade(order(1)) {
        Ok(_) => panic!("second USD order must breach the new asset barrier"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].reason, "rate limit exceeded: asset barrier");
}

#[test]
fn configure_unknown_policy_name_is_reported() {
    let engine = build_engine(5);

    let error = engine
        .configure()
        .rate_limit::<RateLimitPolicyError>("does-not-exist", |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }))
        })
        .expect_err("an unknown policy name must be rejected");
    assert_eq!(
        error,
        ConfigureError::UnknownPolicy {
            name: "does-not-exist".to_owned(),
        }
    );
}

#[test]
fn configure_type_mismatch_is_reported() {
    let engine = build_engine(5);
    let name = RateLimitPolicy::<FullLocking>::NAME;

    // The policy under `name` is a RateLimitPolicy, but `spot_funds` targets
    // SpotFundsSettings, so the downcast must fail with a type mismatch.
    let error = engine
        .configure()
        .spot_funds::<std::convert::Infallible>(name, |_settings| Ok(()))
        .expect_err("a typed method against the wrong settings type must fail");
    assert!(
        matches!(&error, ConfigureError::PolicyTypeMismatch { name: n, .. } if n.as_str() == name),
        "expected PolicyTypeMismatch, got {error:?}"
    );
}

#[test]
fn configure_validation_failure_keeps_prior_value() {
    let engine = build_engine(2);
    let name = RateLimitPolicy::<FullLocking>::NAME;

    // A zero window is rejected by the setter; the update must not publish.
    let error = engine
        .configure()
        .rate_limit::<RateLimitPolicyError>(name, |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1_000,
                    window: Duration::ZERO,
                },
            }))
        })
        .expect_err("an invalid window must be rejected");
    assert!(
        matches!(&error, ConfigureError::Validation { name: n, .. } if n.as_str() == name),
        "expected Validation, got {error:?}"
    );

    // The prior limit of 2 still applies: two orders pass, the third is
    // rejected, proving the failed update left the live value intact.
    engine
        .execute_pre_trade(order(0))
        .expect("first order under the retained limit must pass");
    engine
        .execute_pre_trade(order(0))
        .expect("second order under the retained limit must pass");
    assert!(
        engine.execute_pre_trade(order(0)).is_err(),
        "third order must still breach the retained limit of 2"
    );
}

#[test]
fn configurator_send_sync_per_mode() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    // FullSync: shareable across threads.
    assert_send::<Configurator<FullSync>>();
    assert_sync::<Configurator<FullSync>>();
    // AccountSync: movable between threads, not shareable.
    assert_send::<Configurator<AccountSync>>();
}

#[test]
fn built_in_configuration_works_in_every_sync_mode() {
    let local = build_local_engine(5);
    local
        .configure()
        .rate_limit::<RateLimitPolicyError>(RateLimitPolicy::<NoLocking>::NAME, |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }))
        })
        .expect("LocalSync retune must publish");
    local
        .execute_pre_trade(order(0))
        .expect("first LocalSync order must pass");
    assert!(local.execute_pre_trade(order(1)).is_err());

    let account = build_account_engine(5);
    account
        .configure()
        .rate_limit::<RateLimitPolicyError>(RateLimitPolicy::<AccountLocking>::NAME, |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }))
        })
        .expect("AccountSync retune must publish");
    account
        .execute_pre_trade(order(0))
        .expect("first AccountSync order must pass");
    assert!(account.execute_pre_trade(order(1)).is_err());

    let full = build_engine(5);
    full.configure()
        .rate_limit::<RateLimitPolicyError>(RateLimitPolicy::<FullLocking>::NAME, |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }))
        })
        .expect("FullSync retune must publish");
    full.execute_pre_trade(order(0))
        .expect("first FullSync order must pass");
    assert!(full.execute_pre_trade(order(1)).is_err());
}

#[test]
fn custom_policy_needs_no_configuration_plumbing() {
    Engine::builder::<OrderOperation, (), ()>()
        .no_sync()
        .pre_trade(CustomPolicy)
        .build()
        .expect("custom policy must build without configuration hooks");
}

// ─── P&L-bounds kill-switch ───────────────────────────────────────────────────

// Execution-report type that carries `(instrument, account_id, pnl, fee)`.
// Used only for the P&L-bounds configure tests.
type PnlReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

fn pnl_report(account: u64, settlement: &str, pnl: &str, fee: &str) -> PnlReport {
    WithExecutionReportOperation {
        inner: WithFinancialImpact {
            inner: (),
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str(pnl).expect("pnl literal must be valid"),
                fee: Fee::from_str(fee).expect("fee literal must be valid"),
            },
        },
        operation: ExecutionReportOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new(settlement).expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(account),
            side: Side::Buy,
        },
    }
}

fn build_pnl_engine(
    account_barriers: impl IntoIterator<Item = PnlBoundsAccountAssetBarrier>,
) -> FullSyncEngine<OrderOperation, PnlReport> {
    let builder = Engine::builder::<OrderOperation, PnlReport, ()>().full_sync();
    let settings =
        PnlBoundsKillSwitchSettings::new([], account_barriers).expect("settings must be valid");
    let policy = PnlBoundsKillSwitchPolicy::<FullLocking>::new(settings, builder.storage_builder());
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

// Adds a broker barrier via configure and asserts an order that was passing
// before is now rejected with the verbatim broker-axis reason.
// The accumulation is seeded BEFORE the barrier is installed so that
// `apply_execution_report` does not trigger an account block (which would
// mask the pre-trade reject reason with "account blocked").
#[test]
fn configure_pnl_bounds_broker_barrier_added_at_runtime_triggers_reject() {
    // Start with an account+asset barrier on account 2 to satisfy the
    // constructor's "at least one barrier" requirement, while account 1 has no
    // coverage and its orders pass freely.
    let engine = build_pnl_engine([PnlBoundsAccountAssetBarrier {
        barrier: PnlBoundsBrokerBarrier {
            settlement_asset: Asset::new("USD").expect("asset code must be valid"),
            lower_bound: Some(Pnl::from_str("-500").expect("pnl literal must be valid")),
            upper_bound: None,
        },
        account_id: AccountId::from_u64(2),
        initial_pnl: Pnl::ZERO,
    }]);
    let name = PnlBoundsKillSwitchPolicy::<FullLocking>::NAME;

    // Accumulate a loss for account 1 while no broker barrier is present.
    // apply_execution_report sees no barrier for (account 1, USD) and stores
    // the running total without issuing an AccountBlock.
    engine.apply_execution_report(&pnl_report(1, "USD", "-10", "0"));

    // Account 1 order still passes before the broker barrier is installed.
    engine
        .execute_pre_trade(order(1))
        .expect("order with no barrier must pass before configure");

    // Add a broker barrier on USD whose lower bound is already breached by
    // the accumulated -10.
    engine
        .configure()
        .pnl_bounds_killswitch::<PnlBoundsKillSwitchPolicyError>(name, |settings| {
            settings.set_broker_barriers([PnlBoundsBrokerBarrier {
                settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                lower_bound: Some(Pnl::from_str("-1").expect("pnl literal must be valid")),
                upper_bound: None,
            }])
        })
        .expect("adding broker barrier must publish");

    // The pre-trade check now sees the broker barrier and the accumulated -10
    // breaches lower bound -1 → reject with the verbatim broker-axis reason.
    let rejects = engine
        .execute_pre_trade(order(1))
        .err()
        .expect("order past broker barrier must be rejected");
    assert_eq!(
        rejects[0].reason,
        "pnl kill switch triggered: broker barrier"
    );
}

#[test]
fn configure_pnl_bounds_pre_barrier_accumulation_seen_by_new_account_asset_barrier() {
    let engine = build_pnl_engine([PnlBoundsAccountAssetBarrier {
        barrier: PnlBoundsBrokerBarrier {
            settlement_asset: Asset::new("USD").expect("asset code must be valid"),
            lower_bound: Some(Pnl::from_str("-500").expect("pnl literal must be valid")),
            upper_bound: None,
        },
        account_id: AccountId::from_u64(2),
        initial_pnl: Pnl::ZERO,
    }]);
    let name = PnlBoundsKillSwitchPolicy::<FullLocking>::NAME;

    engine.apply_execution_report(&pnl_report(1, "USD", "-80", "0"));
    engine.apply_execution_report(&pnl_report(1, "USD", "-40", "0"));
    engine
        .execute_pre_trade(order(1))
        .expect("no barrier yet: order must pass");

    engine
        .configure()
        .pnl_bounds_killswitch::<PnlBoundsKillSwitchPolicyError>(name, |settings| {
            settings.set_account_barriers([
                PnlBoundsAccountAssetBarrierUpdate {
                    barrier: PnlBoundsBrokerBarrier {
                        settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                        lower_bound: Some(
                            Pnl::from_str("-500").expect("pnl literal must be valid"),
                        ),
                        upper_bound: None,
                    },
                    account_id: AccountId::from_u64(2),
                },
                PnlBoundsAccountAssetBarrierUpdate {
                    barrier: PnlBoundsBrokerBarrier {
                        settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                        lower_bound: Some(
                            Pnl::from_str("-100").expect("pnl literal must be valid"),
                        ),
                        upper_bound: None,
                    },
                    account_id: AccountId::from_u64(1),
                },
            ])
        })
        .expect("barrier add must publish");

    let rejects = engine
        .execute_pre_trade(order(1))
        .err()
        .expect("order must be rejected by the newly-configured barrier");
    assert_eq!(
        rejects[0].reason,
        "pnl kill switch triggered: account + asset barrier"
    );
}

// `set_account_pnl` force-sets the live accumulator through the engine, and the
// override is observed on the next hot-path check: forcing one account past the
// bound rejects its next order, while forcing a different (never-breached)
// account to a value inside the bound lets its order pass. This is the
// deliberate force-set path, separate from the bounds retune above.
//
// The two directions use different accounts on purpose: a P&L-bounds breach is
// a kill switch, so once account 1's order is rejected the engine latches the
// account-level block. Force-setting the ledger back inside the bound does not
// clear that latch (only an explicit admin unblock does), so the "inside the
// bound passes" direction is exercised on account 2, which was never latched.
#[test]
fn configure_set_account_pnl_overrides_accumulated_pnl() {
    let engine = build_pnl_engine([
        PnlBoundsAccountAssetBarrier {
            barrier: PnlBoundsBrokerBarrier {
                settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                lower_bound: Some(Pnl::from_str("-100").expect("pnl literal must be valid")),
                upper_bound: None,
            },
            account_id: AccountId::from_u64(1),
            initial_pnl: Pnl::ZERO,
        },
        PnlBoundsAccountAssetBarrier {
            barrier: PnlBoundsBrokerBarrier {
                settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                lower_bound: Some(Pnl::from_str("-100").expect("pnl literal must be valid")),
                upper_bound: None,
            },
            account_id: AccountId::from_u64(2),
            initial_pnl: Pnl::ZERO,
        },
    ]);
    let name = PnlBoundsKillSwitchPolicy::<FullLocking>::NAME;
    let usd = Asset::new("USD").expect("asset code must be valid");

    // Zero accumulated P&L is within [-100, ∞): the order passes.
    engine
        .execute_pre_trade(order(1))
        .expect("order must pass at zero P&L")
        .rollback();

    // Force account 1's accumulator below the lower bound; its next order is
    // rejected and the engine latches the account-level block.
    engine
        .configure()
        .set_account_pnl(
            name,
            AccountId::from_u64(1),
            usd.clone(),
            Pnl::from_str("-150").expect("pnl literal must be valid"),
        )
        .expect("force-set must publish");
    let rejects = engine
        .execute_pre_trade(order(1))
        .err()
        .expect("order must be rejected after the override breaches the bound");
    assert_eq!(
        rejects[0].reason,
        "pnl kill switch triggered: account + asset barrier"
    );

    // Force account 2's accumulator inside the bound; its order passes, proving
    // the override is observed in the passing direction too.
    engine
        .configure()
        .set_account_pnl(
            name,
            AccountId::from_u64(2),
            usd,
            Pnl::from_str("-10").expect("pnl literal must be valid"),
        )
        .expect("force-set inside bounds must publish");
    engine
        .execute_pre_trade(order(2))
        .expect("order must pass after the accumulator is set inside bounds")
        .rollback();
}

// `set_account_pnl` creates the entry on demand for a previously-untracked
// `(account, settlement_asset)` pair, mirroring the construction-time seed.
#[test]
fn configure_set_account_pnl_creates_absent_entry() {
    let engine = build_pnl_engine([PnlBoundsAccountAssetBarrier {
        barrier: PnlBoundsBrokerBarrier {
            settlement_asset: Asset::new("USD").expect("asset code must be valid"),
            lower_bound: Some(Pnl::from_str("-100").expect("pnl literal must be valid")),
            upper_bound: None,
        },
        account_id: AccountId::from_u64(2),
        initial_pnl: Pnl::ZERO,
    }]);
    let name = PnlBoundsKillSwitchPolicy::<FullLocking>::NAME;

    // Account 2 has never traded and has no stored entry; force it past the
    // bound and the next order is rejected, proving the upsert created it.
    engine
        .configure()
        .set_account_pnl(
            name,
            AccountId::from_u64(2),
            Asset::new("USD").expect("asset code must be valid"),
            Pnl::from_str("-150").expect("pnl literal must be valid"),
        )
        .expect("force-set on an absent entry must publish");
    let rejects = engine
        .execute_pre_trade(order(2))
        .err()
        .expect("order must be rejected after the created entry breaches the bound");
    assert_eq!(
        rejects[0].reason,
        "pnl kill switch triggered: account + asset barrier"
    );
}

#[test]
fn configure_set_account_pnl_unknown_policy_name_is_reported() {
    let engine = build_pnl_engine([PnlBoundsAccountAssetBarrier {
        barrier: PnlBoundsBrokerBarrier {
            settlement_asset: Asset::new("USD").expect("asset code must be valid"),
            lower_bound: Some(Pnl::from_str("-100").expect("pnl literal must be valid")),
            upper_bound: None,
        },
        account_id: AccountId::from_u64(1),
        initial_pnl: Pnl::ZERO,
    }]);

    let error = engine
        .configure()
        .set_account_pnl(
            "does-not-exist",
            AccountId::from_u64(1),
            Asset::new("USD").expect("asset code must be valid"),
            Pnl::ZERO,
        )
        .expect_err("an unknown policy name must be rejected");
    assert_eq!(
        error,
        ConfigureError::UnknownPolicy {
            name: "does-not-exist".to_owned(),
        }
    );
}

#[test]
fn configure_set_account_pnl_type_mismatch_is_reported() {
    // The engine here carries a RateLimitPolicy, so targeting its name with the
    // P&L force-set must fail with a type mismatch.
    let engine = build_engine(5);
    let name = RateLimitPolicy::<FullLocking>::NAME;

    let error = engine
        .configure()
        .set_account_pnl(
            name,
            AccountId::from_u64(1),
            Asset::new("USD").expect("asset code must be valid"),
            Pnl::ZERO,
        )
        .expect_err("targeting the wrong policy type must fail");
    assert!(
        matches!(&error, ConfigureError::PolicyTypeMismatch { name: n, .. } if n.as_str() == name),
        "expected PolicyTypeMismatch, got {error:?}"
    );
}

// ─── Order-size-limit ─────────────────────────────────────────────────────────

fn order_with_price(account: u64, quantity: &str, price: &str) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(account),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str(quantity).expect("quantity literal must be valid"),
        ),
        price: Some(Price::from_str(price).expect("price literal must be valid")),
    }
}

fn build_order_size_engine(
    account_id: u64,
    max_quantity: &str,
    max_notional: &str,
) -> FullSyncEngine<OrderOperation> {
    let builder = Engine::builder::<OrderOperation, (), ()>().full_sync();
    let settings = OrderSizeLimitSettings::new(
        None,
        [],
        [OrderSizeAccountAssetBarrier {
            limit: OrderSizeLimit {
                max_quantity: Quantity::from_str(max_quantity)
                    .expect("max_quantity literal must be valid"),
                max_notional: Volume::from_str(max_notional)
                    .expect("max_notional literal must be valid"),
            },
            account_id: AccountId::from_u64(account_id),
            settlement_asset: Asset::new("USD").expect("asset code must be valid"),
        }],
    )
    .expect("settings must be valid");
    let policy = OrderSizeLimitPolicy::<FullLocking>::new(settings);
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

// Tighten the account+asset barrier for account 1 via configure.  An order
// that the generous initial limit admitted is rejected under the tightened one.
// The broker axis is absent so only the account+asset axis triggers.
#[test]
fn configure_order_size_limit_account_asset_barrier_tightened_rejects_previously_admitted_order() {
    let engine = build_order_size_engine(1, "20", "100000");
    let name = OrderSizeLimitPolicy::<FullLocking>::NAME;

    // qty=15 is within the generous limit of 20: passes.
    engine
        .execute_pre_trade(order_with_price(1, "15", "100"))
        .expect("order within the generous limit must pass");

    // Tighten the account+asset barrier for account 1 to qty=10 via configure.
    engine
        .configure()
        .order_size_limit::<OrderSizeLimitPolicyError>(name, |settings| {
            settings.set_account_asset_barriers([OrderSizeAccountAssetBarrier {
                limit: OrderSizeLimit {
                    max_quantity: Quantity::from_str("10")
                        .expect("max_quantity literal must be valid"),
                    max_notional: Volume::from_str("100000")
                        .expect("max_notional literal must be valid"),
                },
                account_id: AccountId::from_u64(1),
                settlement_asset: Asset::new("USD").expect("asset code must be valid"),
            }])
        })
        .expect("tightening barrier must publish");

    // qty=15 now exceeds the tightened limit of 10.
    let rejects = engine
        .execute_pre_trade(order_with_price(1, "15", "100"))
        .err()
        .expect("order beyond the tightened limit must be rejected");
    assert_eq!(rejects[0].reason, "order quantity exceeded");
    assert!(rejects[0].details.contains("max allowed: 10"));

    // Account 2 has no account+asset barrier and no broker barrier: passes.
    engine
        .execute_pre_trade(order_with_price(2, "15", "100"))
        .expect("account 2 with no barrier must still pass");
}

// ─── Spot funds ───────────────────────────────────────────────────────────────

// Minimal account-adjustment stub for SpotFunds seeding.
struct SpotAdjustment {
    asset: Asset,
    balance: Option<AdjustmentAmount>,
}

impl HasBalanceAsset for SpotAdjustment {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        Ok(&self.asset)
    }
}

impl HasAccountAdjustmentBalance for SpotAdjustment {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.balance)
    }
}

impl HasAccountAdjustmentBalanceLowerBound for SpotAdjustment {
    fn balance_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentBalanceUpperBound for SpotAdjustment {
    fn balance_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentHeld for SpotAdjustment {
    fn held(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentHeldLowerBound for SpotAdjustment {
    fn held_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentHeldUpperBound for SpotAdjustment {
    fn held_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncoming for SpotAdjustment {
    fn incoming(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncomingLowerBound for SpotAdjustment {
    fn incoming_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncomingUpperBound for SpotAdjustment {
    fn incoming_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

fn spot_balance(asset_code: &str, amount: &str) -> SpotAdjustment {
    SpotAdjustment {
        asset: Asset::new(asset_code).expect("asset code must be valid"),
        balance: Some(AdjustmentAmount::Absolute(
            PositionSize::from_str(amount).expect("position size literal must be valid"),
        )),
    }
}

type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
type SpotEngine = FullSyncEngine<OrderOperation, SpotReport, SpotAdjustment>;

fn build_spot_engine(slippage_bps: u16) -> SpotEngine {
    let builder = Engine::builder::<OrderOperation, SpotReport, SpotAdjustment>().full_sync();
    let settings = SpotFundsSettings::new(slippage_bps, SpotFundsPricingSource::Mark, [])
        .expect("settings must be valid");
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        settings,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

fn seed_spot(engine: &SpotEngine, account: u64, asset_code: &str, amount: &str) {
    let adj = spot_balance(asset_code, amount);
    engine
        .apply_account_adjustment(AccountId::from_u64(account), &[adj])
        .expect("seed must succeed");
}

// Build a SpotFunds engine, seed a balance, confirm a limit order passes, then
// retune the settings via configure (changing the pricing source, which is
// observable via the settings cell update path) and confirm that:
// 1. The configure call succeeds, proving the cell is properly wired.
// 2. A subsequent limit order with sufficient balance still passes, proving
//    the live policy reads from the updated cell without corruption.
// 3. A validation rejection from configure (out-of-range slippage) leaves
//    the engine functional.
#[test]
fn configure_spot_funds_settings_retune_takes_effect() {
    let engine = build_spot_engine(0);
    let name = SpotFundsPolicy::<FullSync, FullSync>::NAME;
    let account_id = 1u64;

    // Seed 1000 USD for account 1.
    seed_spot(&engine, account_id, "USD", "1000");

    // A limit buy of 5 AAPL @ 100 USD (cost = 500 USD) must pass.
    engine
        .execute_pre_trade(order_with_price(account_id, "5", "100"))
        .expect("order within funded balance must pass")
        .rollback();

    // Retune: change the pricing source to BookTop. The engine must accept
    // the update without error and without resetting holdings.
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_pricing_source(SpotFundsPricingSource::BookTop);
            Ok(())
        })
        .expect("pricing-source retune must publish");

    // After the retune the limit order (unaffected by pricing source) still
    // passes — the holdings were not touched by the settings update.
    engine
        .execute_pre_trade(order_with_price(account_id, "5", "100"))
        .expect("limit order must pass after pricing-source retune")
        .rollback();

    // A validation failure (out-of-range slippage) must not corrupt the cell.
    let validation_err = engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_global_slippage_bps(20_000)
        })
        .expect_err("out-of-range slippage must be rejected");
    assert!(
        matches!(&validation_err, ConfigureError::Validation { .. }),
        "expected Validation error, got {validation_err:?}"
    );

    // Engine must still be functional after the failed retune.
    engine
        .execute_pre_trade(order_with_price(account_id, "5", "100"))
        .expect("engine must remain functional after failed retune")
        .rollback();
}
