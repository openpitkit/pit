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

use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::{
    AccountBlock, PolicyAccountAdjustmentResult, PolicyPreTradeResult, PostTradeContext,
    PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects,
};
use openpit::{
    AccountAdjustmentContext, Engine, ExecutionReportOperation, FinancialImpact, HasOrderPrice,
    HasTradeAmount, Instrument, Mutation, Mutations, OrderOperation, WithExecutionReportOperation,
    WithFinancialImpact,
};

// Mirrors public Rust examples from:
// - ../pit.wiki/Account-Adjustments.md
// - ../pit.wiki/Account-Blocking.md
// - ../pit.wiki/Account-Groups.md
// - ../pit.wiki/Balance-Reconciliation.md
// - ../pit.wiki/Domain-Types.md
// - ../pit.wiki/Dynamic-Policy-Reconfiguration.md
// - ../pit.wiki/Getting-Started.md
// - ../pit.wiki/Non-Mutating-Dry-Run.md
// - ../pit.wiki/Policies.md
// - ../pit.wiki/Policy-API.md
// - ../pit.wiki/Pre-trade-Pipeline.md
// - ../pit.wiki/Pre-Trade-Lock.md
// - ../pit.wiki/Spot-Funds.md
// - ../pit.wiki/Storage.md
// If this file changes, update every linked documentation snippet.

type PitExecutionReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

fn aapl_usd_order(quantity: &str, price: &str) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("AAPL must be valid"),
            Asset::new("USD").expect("USD must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str(quantity).expect("quantity must be valid"),
        ),
        price: Some(Price::from_str(price).expect("price must be valid")),
    }
}

#[allow(dead_code)]
fn aapl_usd_report(pnl: &str, fee: &str) -> PitExecutionReport {
    PitExecutionReport {
        inner: WithFinancialImpact {
            inner: (),
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str(pnl).expect("pnl must be valid"),
                fee: Fee::from_str(fee).expect("fee must be valid"),
            },
        },
        operation: ExecutionReportOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("AAPL must be valid"),
                Asset::new("USD").expect("USD must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
        },
    }
}

// --- Policy-API: Rollback Safety Pattern ---

struct ReserveThenValidatePolicy {
    reserved: Rc<RefCell<Volume>>,
    next: Volume,
    limit: Volume,
}

impl<O, R, A, Sync> PreTradePolicy<O, R, A, Sync> for ReserveThenValidatePolicy
where
    Sync: openpit::SyncMode,
{
    fn name(&self) -> &str {
        "ReserveThenValidatePolicy"
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _order: &O,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        let prev = *self.reserved.borrow();
        let rollback_reserved = Rc::clone(&self.reserved);
        let next = self.next;
        *self.reserved.borrow_mut() = next;

        mutations.push(Mutation::new(
            || {
                // Commit is empty: state was applied eagerly.
            },
            move || {
                *rollback_reserved.borrow_mut() = prev;
            },
        ));

        if next > self.limit {
            return Err(Rejects::from(Reject::new(
                <Self as PreTradePolicy<O, R, A, Sync>>::name(self),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "temporary reservation exceeds limit",
                format!("reserved {}, limit: {}", next, self.limit),
            )));
        }
        Ok(None)
    }

    fn apply_execution_report(
        &self,
        _ctx: &PostTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _report: &R,
    ) -> Option<openpit::PostTradeResult> {
        None
    }
}

// --- Policy-API: Custom Main-Stage Policy ---

struct NotionalCapPolicy {
    // Policy-local config: reject any order above this absolute notional.
    max_abs_notional: Volume,
}

impl<O, R, A, Sync> PreTradePolicy<O, R, A, Sync> for NotionalCapPolicy
where
    O: HasTradeAmount + HasOrderPrice,
    Sync: openpit::SyncMode,
{
    fn name(&self) -> &str {
        "NotionalCapPolicy"
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        order: &O,
        _mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        // Translate the public order surface into one number that this policy
        // can reason about: requested notional.
        let trade_amount = match order.trade_amount() {
            Ok(trade_amount) => trade_amount,
            Err(error) => {
                return Err(Rejects::from(Reject::new(
                    <Self as PreTradePolicy<O, R, A, Sync>>::name(self),
                    RejectScope::Order,
                    RejectCode::MissingRequiredField,
                    "required order field missing",
                    error.to_string(),
                )));
            }
        };
        let price = match order.price() {
            Ok(price) => price,
            Err(error) => {
                return Err(Rejects::from(Reject::new(
                    <Self as PreTradePolicy<O, R, A, Sync>>::name(self),
                    RejectScope::Order,
                    RejectCode::MissingRequiredField,
                    "required order field missing",
                    error.to_string(),
                )));
            }
        };
        let requested_notional = match (trade_amount, price) {
            (TradeAmount::Volume(volume), _) => volume,
            (TradeAmount::Quantity(quantity), Some(price)) => {
                match price.calculate_volume(quantity) {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(Rejects::from(Reject::new(
                            <Self as PreTradePolicy<O, R, A, Sync>>::name(self),
                            RejectScope::Order,
                            RejectCode::OrderValueCalculationFailed,
                            "order value calculation failed",
                            "price and quantity could not be used to evaluate notional",
                        )));
                    }
                }
            }
            (TradeAmount::Quantity(_), None) => {
                return Err(Rejects::from(Reject::new(
                    <Self as PreTradePolicy<O, R, A, Sync>>::name(self),
                    RejectScope::Order,
                    RejectCode::OrderValueCalculationFailed,
                    "order value calculation failed",
                    "price not provided for evaluating cash flow/notional/volume",
                )));
            }
            _ => {
                return Err(Rejects::from(Reject::new(
                    <Self as PreTradePolicy<O, R, A, Sync>>::name(self),
                    RejectScope::Order,
                    RejectCode::UnsupportedOrderType,
                    "unsupported order type",
                    "custom trade amount variant is not supported by this policy",
                )));
            }
        };

        if requested_notional > self.max_abs_notional {
            // Business validation failures should become explicit rejects.
            return Err(Rejects::from(Reject::new(
                <Self as PreTradePolicy<O, R, A, Sync>>::name(self),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                format!(
                    "requested notional {}, max allowed: {}",
                    requested_notional, self.max_abs_notional
                ),
            )));
        }
        Ok(None)
    }

    fn apply_execution_report(
        &self,
        _ctx: &PostTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _report: &R,
    ) -> Option<openpit::PostTradeResult> {
        None
    }
}

// --- Policy-API: Block an Account from an Adjustment Callback ---

struct BlockOnAdjustmentPolicy;

impl<Order, ExecutionReport, AccountAdjustment, Sync>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync> for BlockOnAdjustmentPolicy
where
    Sync: openpit::SyncMode,
{
    fn name(&self) -> &str {
        "BlockOnAdjustmentPolicy"
    }

    fn apply_account_adjustment(
        &self,
        _ctx: &AccountAdjustmentContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _account_id: AccountId,
        _adjustment: &AccountAdjustment,
        _mutations: &mut Mutations,
    ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
        Ok(PolicyAccountAdjustmentResult {
            account_adjustments: Vec::new(),
            account_blocks: vec![AccountBlock::new(
                "BlockOnAdjustmentPolicy",
                RejectCode::AccountBlocked,
                "blocked by account-adjustment policy",
                "custom policy reported an account block from a callback",
            )],
        })
    }
}

// --- Tests ---

#[test]
fn example_wiki_domain_types_create_validated_values() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Domain-Types.md - Create Validated Values
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{Asset, Pnl, Price, Quantity};

    // Build validated value objects at the integration boundary.
    let asset = Asset::new("AAPL").expect("asset code must be valid");
    let quantity = Quantity::from_str("10.5").expect("quantity must be valid");
    let price = Price::from_str("185").expect("price must be valid");
    let pnl = Pnl::from_str("-12.5").expect("pnl must be valid");

    // The wrappers normalize formatting while preserving domain meaning.
    assert_eq!(asset.as_ref(), "AAPL");
    assert_eq!(quantity.to_string(), "10.5");
    assert_eq!(price.to_string(), "185");
    assert_eq!(pnl.to_string(), "-12.5");
    Ok(())
}

#[test]
fn example_wiki_domain_types_directional_types() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Domain-Types.md - Work With Directional Types
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{PositionSide, Side};

    // Directional helpers keep side logic explicit instead of comparing raw strings.
    assert_eq!(Side::Buy.opposite(), Side::Sell);
    assert_eq!(Side::Sell.sign(), -1);
    assert_eq!(PositionSide::Long.opposite(), PositionSide::Short);
    Ok(())
}

#[test]
fn example_wiki_domain_types_leverage() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Domain-Types.md - Create Leverage
    // Keep this example in sync with the matching wiki example.
    use openpit::param::Leverage;

    // Pick the constructor that matches the upstream representation you receive.
    let from_multiplier = Leverage::from_u16(100).expect("valid leverage");
    let from_float = Leverage::from_f64(100.5).expect("valid leverage");

    // Both constructors end up with the same strongly typed leverage wrapper.
    assert_eq!(from_multiplier.value(), 100.0);
    assert_eq!(from_float.value(), 100.5);
    Ok(())
}

#[test]
fn example_wiki_getting_started_build_engine() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Getting-Started.md - Build an Engine
    // Keep this example in sync with the matching wiki example.
    use std::time::Duration;

    use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
    use openpit::pretrade::policies::{
        OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy,
        OrderSizeLimitSettings, OrderValidationPolicy, PnlBoundsBrokerBarrier,
        PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings, RateLimit, RateLimitBrokerBarrier,
        RateLimitPolicy, RateLimitSettings,
    };
    use openpit::storage::NoLocking;
    use openpit::{
        Engine, ExecutionReportOperation, FinancialImpact, Instrument, OrderOperation,
        WithExecutionReportOperation, WithFinancialImpact,
    };

    let usd = Asset::new("USD")?;

    // 1. Build the engine builder.
    type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;
    let builder = Engine::builder::<OrderOperation, Report, ()>().no_sync();

    // 2. Configure policies.
    let pnl_policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: usd.clone(),
                lower_bound: Some(Pnl::from_str("-1000")?),
                upper_bound: None,
            }],
            [],
        )?,
        builder.storage_builder(),
    );

    let rate_limit_policy = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 100,
                    window: Duration::from_secs(1),
                },
            }),
            [],
            [],
            [],
        )?,
        builder.storage_builder(),
    );

    // 3. Build the engine (one time at the platform initialization).
    let engine = builder
        .pre_trade(OrderValidationPolicy::new())
        .pre_trade(pnl_policy)
        .pre_trade(rate_limit_policy)
        .pre_trade(OrderSizeLimitPolicy::<NoLocking>::new(
            OrderSizeLimitSettings::new(
                Some(OrderSizeBrokerBarrier {
                    limit: OrderSizeLimit {
                        max_quantity: Quantity::from_str("500")?,
                        max_notional: Volume::from_str("100000")?,
                    },
                }),
                [OrderSizeAssetBarrier {
                    limit: OrderSizeLimit {
                        max_quantity: Quantity::from_str("500")?,
                        max_notional: Volume::from_str("100000")?,
                    },
                    settlement_asset: usd.clone(),
                }],
                [],
            )?,
        ))
        .build()?;

    // 4. Check an order.
    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, usd.clone()),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_f64(100.0)?),
        price: Some(Price::from_str("185")?),
    };

    let request = engine.start_pre_trade(order).map_err(|rejects| {
        let message = rejects
            .iter()
            .map(|r| {
                format!(
                    "rejected by {} [{}]: {} ({})",
                    r.policy, r.code, r.reason, r.details,
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        std::io::Error::other(message)
    })?;

    // 5. Quick, lightweight checks, such as fat-finger scope or enabled killswitch,
    // were performed during pre-trade request creation. The system state has not
    // yet changed, except in cases where each request, even rejected ones, must be
    // considered (for example, to prevent frequent transfers). Before the
    // heavy-duty checks, other work on the request can be performed simply by
    // holding the request object.

    // 6. Real pre-trade and risk control.
    let mut reservation = request.execute().map_err(|rejects| {
        let message = rejects
            .iter()
            .map(|r| {
                format!(
                    "rejected by {} [{}]: {} ({})",
                    r.policy, r.code, r.reason, r.details,
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        std::io::Error::other(message)
    })?;

    // Optional shortcut for the same two-stage flow:
    // let reservation = engine.execute_pre_trade(order)?;

    // 7. If the request is successfully sent to the venue, it must be committed.
    // The rollback must be called otherwise to revert all performed reservations.
    reservation.commit();

    // 8. The order goes to the venue and returns with an execution report.
    let report = WithExecutionReportOperation {
        inner: WithFinancialImpact {
            inner: (),
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str("-50")?,
                fee: Fee::from_str("3.4")?,
            },
        },
        operation: ExecutionReportOperation {
            instrument: Instrument::new(Asset::new("AAPL")?, usd),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
        },
    };

    let result = engine.apply_execution_report(&report);
    for outcome in &result.account_pnls {
        eprintln!("account P&L outcome for {}", outcome.account_id);
    }
    for outcome in &result.account_adjustments {
        eprintln!(
            "account adjustment from group {}",
            outcome.policy_group_id.value()
        );
    }

    // 9. After each execution report is applied, the system may report that it has
    // been determined in advance that all subsequent requests will be rejected if
    // the account status does not change.
    assert!(result.account_blocks.is_empty());
    Ok(())
}

#[test]
fn example_wiki_pipeline_start_stage_reject() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md - Handle a Start-Stage Reservation
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;
    let order = aapl_usd_order("100", "185");

    // Start stage returns either a reject or a deferred request handle.
    match engine.start_pre_trade(order) {
        Ok(request) => {
            // Keep the request object if later code wants to enter the main stage.
            let _request = request;
        }
        Err(rejects) => {
            for reject in rejects.iter() {
                eprintln!(
                    "rejected by {} [{}]: {} ({})",
                    reject.policy, reject.code, reject.reason, reject.details
                );
            }
        }
    }
    Ok(())
}

#[test]
fn example_wiki_pipeline_main_stage_finalize() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md - Execute the Main Stage and Finalize the Reservation
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;
    let order = aapl_usd_order("100", "185");

    let request = engine
        .start_pre_trade(order)
        .expect("start stage must pass");

    // Main stage consumes the deferred request and returns reservation or rejects.
    match request.execute() {
        Ok(mut reservation) => {
            // Commit only after the caller knows the reservation should become durable.
            reservation.commit()
        }
        Err(rejects) => {
            for reject in rejects.iter() {
                eprintln!(
                    "rejected by {} [{}]: {} ({})",
                    reject.policy, reject.code, reject.reason, reject.details
                );
            }
        }
    }
    Ok(())
}

#[test]
fn example_wiki_getting_started_run_order() {
    // Wiki example: pit.wiki/Getting-Started.md
    // - Run an Order Through the Engine
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()
        .expect("engine must build");
    let order = aapl_usd_order("100", "185");

    let request = match engine.start_pre_trade(order) {
        Ok(request) => request,
        Err(rejects) => {
            for reject in rejects.iter() {
                eprintln!(
                    "rejected by {} [{}]: {} ({})",
                    reject.policy, reject.code, reject.reason, reject.details
                );
            }
            return;
        }
    };

    let mut reservation = match request.execute() {
        Ok(reservation) => reservation,
        Err(rejects) => {
            for reject in rejects.iter() {
                eprintln!(
                    "rejected by {} [{}]: {} ({})",
                    reject.policy, reject.code, reject.reason, reject.details
                );
            }
            return;
        }
    };

    reservation.commit();
}

#[test]
fn example_wiki_pipeline_shortcut_start_and_main() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md - Shortcut for Start + Main Stages
    // Wiki example: pit.wiki/Getting-Started.md - Shortcut for Start + Main Stages
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;
    let order = aapl_usd_order("100", "185");

    // The shortcut runs start stage and main stage as one convenience call.
    match engine.execute_pre_trade(order) {
        Ok(mut reservation) => {
            // Finalization is still explicit even when the two stages are composed.
            reservation.commit()
        }
        Err(rejects) => {
            for reject in rejects.iter() {
                eprintln!(
                    "rejected by {} [{}]: {} ({})",
                    reject.policy, reject.code, reject.reason, reject.details
                );
            }
        }
    }
    Ok(())
}

#[test]
fn example_wiki_pipeline_apply_post_trade_feedback() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md - Apply Post-Trade Feedback
    // Wiki example: pit.wiki/Getting-Started.md - Apply Post-Trade Feedback
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;
    let report = aapl_usd_report("-50", "3.4");

    // Execution reports feed realized outcomes back into cumulative policy state.
    let result = engine.apply_execution_report(&report);
    for outcome in &result.account_pnls {
        eprintln!("account P&L outcome for {}", outcome.account_id);
    }
    for outcome in &result.account_adjustments {
        eprintln!(
            "account adjustment from group {}",
            outcome.policy_group_id.value()
        );
    }
    if !result.account_blocks.is_empty() {
        eprintln!("halt new orders until the blocked state is cleared");
    }
    assert!(result.account_blocks.is_empty());

    Ok(())
}

#[test]
fn example_wiki_account_adjustments() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Account-Adjustments.md - Examples → Rust
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AdjustmentAmount, PositionMode, PositionSize};
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation,
        AccountAdjustmentPositionOperation, Engine, Instrument,
    };

    #[derive(Clone)]
    #[allow(dead_code)]
    enum AccountAdjustmentOperation {
        Balance(AccountAdjustmentBalanceOperation),
        Position(AccountAdjustmentPositionOperation),
    }

    #[derive(Clone)]
    #[allow(dead_code)]
    struct AccountAdjustment {
        operation: AccountAdjustmentOperation,
        amount: AccountAdjustmentAmount,
    }

    // Build one batch that mixes balance and position adjustments.
    let account_id = AccountId::from_u64(99224416);

    let adjustments = vec![
        AccountAdjustment {
            operation: AccountAdjustmentOperation::Balance(AccountAdjustmentBalanceOperation {
                asset: Asset::new("USD")?,
                average_entry_price: None,
            }),
            amount: AccountAdjustmentAmount {
                balance: Some(AdjustmentAmount::Absolute(PositionSize::from_f64(10000.0)?)),
                held: None,
                incoming: None,
            },
        },
        AccountAdjustment {
            operation: AccountAdjustmentOperation::Position(AccountAdjustmentPositionOperation {
                instrument: Instrument::new(Asset::new("SPX")?, Asset::new("USD")?),
                collateral_asset: Asset::new("USD")?,
                average_entry_price: Price::from_f64(95000.0)?,
                mode: PositionMode::Hedged,
                leverage: None,
            }),
            amount: AccountAdjustmentAmount {
                balance: Some(AdjustmentAmount::Absolute(PositionSize::from_f64(-3.0)?)),
                held: None,
                incoming: None,
            },
        },
    ];

    struct AcceptAllAdjustments;

    impl<Sync> openpit::pretrade::PreTradePolicy<(), (), AccountAdjustment, Sync>
        for AcceptAllAdjustments
    where
        Sync: openpit::SyncMode,
    {
        fn name(&self) -> &'static str {
            "AcceptAllAdjustments"
        }

        fn apply_account_adjustment(
            &self,
            _ctx: &openpit::AccountAdjustmentContext<
                <Sync as openpit::SyncMode>::StorageLockingPolicyFactory,
            >,
            _account_id: openpit::param::AccountId,
            _adjustment: &AccountAdjustment,
            _mutations: &mut openpit::Mutations,
        ) -> Result<openpit::pretrade::PolicyAccountAdjustmentResult, openpit::pretrade::Rejects>
        {
            Ok(openpit::pretrade::PolicyAccountAdjustmentResult {
                account_adjustments: Vec::new(),
                account_blocks: Vec::new(),
            })
        }
    }

    // The engine validates the whole batch atomically.
    let engine = Engine::builder::<(), (), AccountAdjustment>()
        .no_sync()
        .pre_trade(AcceptAllAdjustments)
        .build()?;
    let result = engine.apply_account_adjustment(account_id, &adjustments)?;
    assert!(result.account_blocks.is_empty());
    Ok(())
}

#[test]
fn example_wiki_account_adjustments_balance_limit_policy() -> Result<(), Box<dyn std::error::Error>>
{
    // Wiki example: pit.wiki/Account-Adjustments.md - Balance Limit Policy → Rust
    // Keep this example in sync with the matching wiki example.
    use std::sync::Arc;

    use openpit::storage::{CreateStorageFor, LockingPolicyFactory, Storage, StorageBuilder};

    /// Adjustment type must expose an asset and a delta amount.
    trait HasAssetDelta {
        fn asset_id(&self) -> &str;
        fn delta(&self) -> Volume;
    }

    struct BalanceLimitPolicy<StorageLockingPolicyFactory>
    where
        StorageLockingPolicyFactory: LockingPolicyFactory,
    {
        max_total: Volume,
        totals: Arc<Storage<String, Volume, StorageLockingPolicyFactory::Policy>>,
    }

    impl<StorageLockingPolicyFactory> BalanceLimitPolicy<StorageLockingPolicyFactory>
    where
        StorageLockingPolicyFactory: LockingPolicyFactory + CreateStorageFor<String>,
    {
        fn new(
            max_total: Volume,
            storage_builder: &StorageBuilder<StorageLockingPolicyFactory>,
        ) -> Self {
            Self {
                max_total,
                totals: Arc::new(storage_builder.create_for_bound_key()),
            }
        }
    }

    impl<Order, ExecutionReport, A, Sync, StorageLockingPolicyFactory>
        PreTradePolicy<Order, ExecutionReport, A, Sync>
        for BalanceLimitPolicy<StorageLockingPolicyFactory>
    where
        A: HasAssetDelta,
        Sync: openpit::SyncMode,
        StorageLockingPolicyFactory: LockingPolicyFactory + CreateStorageFor<String>,
        StorageLockingPolicyFactory::Policy: 'static,
    {
        fn name(&self) -> &str {
            "BalanceLimitPolicy"
        }

        fn apply_account_adjustment(
            &self,
            _ctx: &AccountAdjustmentContext<
                <Sync as openpit::SyncMode>::StorageLockingPolicyFactory,
            >,
            _account_id: AccountId,
            adjustment: &A,
            mutations: &mut Mutations,
        ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
            let asset_id = adjustment.asset_id().to_owned();
            let delta = adjustment.delta();

            let prev_total = self
                .totals
                .with(&asset_id, |total| *total)
                .unwrap_or(Volume::ZERO);

            let new_total = prev_total.checked_add(delta).map_err(|error| {
                Rejects::from(Reject::new(
                    "BalanceLimitPolicy",
                    RejectScope::Account,
                    RejectCode::RiskLimitExceeded,
                    "invalid adjustment total",
                    error.to_string(),
                ))
            })?;

            if new_total > self.max_total {
                return Err(Rejects::from(Reject::new(
                    "BalanceLimitPolicy",
                    RejectScope::Account,
                    RejectCode::RiskLimitExceeded,
                    "cumulative adjustment exceeds limit",
                    format!("asset {asset_id}: {new_total} > {}", self.max_total),
                )));
            }

            // Apply immediately so later adjustments in the same batch see the updated total.
            self.totals.with_mut(
                asset_id.clone(),
                || Volume::ZERO,
                |entry, _is_new| {
                    *entry = new_total;
                },
            );

            // Register rollback: restore previous absolute value.
            // Safe because account adjustment batches are fully internal.
            let rollback_totals = Arc::clone(&self.totals);
            let rollback_asset = asset_id;

            mutations.push(Mutation::new(
                || {
                    // Commit is empty: state was applied eagerly.
                },
                move || {
                    // Rollback: restore absolute value captured before modification.
                    rollback_totals.with_mut(
                        rollback_asset,
                        || Volume::ZERO,
                        |entry, _is_new| {
                            *entry = prev_total;
                        },
                    );
                },
            ));

            Ok(PolicyAccountAdjustmentResult {
                account_adjustments: Vec::new(),
                account_blocks: Vec::new(),
            })
        }
    }

    struct SimpleAdjustment {
        asset: String,
        delta: Volume,
    }

    impl HasAssetDelta for SimpleAdjustment {
        fn asset_id(&self) -> &str {
            &self.asset
        }
        fn delta(&self) -> Volume {
            self.delta
        }
    }

    let builder = Engine::builder::<(), (), SimpleAdjustment>().no_sync();
    let policy = BalanceLimitPolicy::new(Volume::from_str("1000000")?, builder.storage_builder());
    let engine = builder.pre_trade(policy).build()?;

    let result = engine.apply_account_adjustment(
        AccountId::from_u64(99224416),
        &[SimpleAdjustment {
            asset: "USD".to_string(),
            delta: Volume::from_str("100")?,
        }],
    )?;
    assert!(result.account_blocks.is_empty());
    Ok(())
}

#[test]
fn example_wiki_policy_rollback_safety() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policy-API.md - Rollback Safety Pattern → Rust
    // Keep this example in sync with the matching wiki example.
    let reserved = Rc::new(RefCell::new(Volume::from_str("0")?));

    let reserve_policy = ReserveThenValidatePolicy {
        reserved: Rc::clone(&reserved),
        next: Volume::from_str("100")?,
        limit: Volume::from_str("50")?,
    };

    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(reserve_policy)
        .build()?;

    let request = engine.start_pre_trade(aapl_usd_order("10", "25"))?;
    let rejects = match request.execute() {
        Ok(_) => panic!("main stage must reject"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::RiskLimitExceeded);
    assert_eq!(reserved.borrow().to_string(), "0");
    Ok(())
}

#[test]
fn example_wiki_policy_notional_cap() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policy-API.md - Custom Main-Stage Policy → Rust
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(NotionalCapPolicy {
            max_abs_notional: Volume::from_str("1000")?,
        })
        .build()?;

    let request = engine.start_pre_trade(aapl_usd_order("10", "25"))?;
    request.execute()?.commit();

    let request = engine.start_pre_trade(aapl_usd_order("100", "25"))?;
    let rejects = match request.execute() {
        Ok(_) => panic!("main stage must reject"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::RiskLimitExceeded);
    Ok(())
}

#[test]
fn example_wiki_policy_blocks_account_from_adjustment() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policy-API.md - Block an Account from an Adjustment Callback
    let engine = Engine::builder::<OrderOperation, (), ()>()
        .no_sync()
        .pre_trade(BlockOnAdjustmentPolicy)
        .build()?;
    let account = AccountId::from_u64(99224416);

    let adjustment_result = engine.apply_account_adjustment(account, &[()])?;
    assert_eq!(adjustment_result.account_blocks.len(), 1);
    let rejects = match engine.start_pre_trade(aapl_usd_order("10", "25")) {
        Ok(_) => panic!("blocked account must fail the start stage"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
    Ok(())
}

#[test]
fn example_wiki_policy_api_custom_rust_models() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policy-API.md - Rust Custom Models
    // Keep the model definitions in sync with the matching wiki example.
    use std::ops::Deref;

    use openpit::{ExecutionReportOperation, OrderOperation};

    struct StrategyOrder {
        inner: OrderOperation,
        strategy_tag: String,
    }

    impl Deref for StrategyOrder {
        type Target = OrderOperation;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    struct StrategyReport {
        inner: ExecutionReportOperation,
        venue_exec_id: String,
    }

    impl Deref for StrategyReport {
        type Target = ExecutionReportOperation;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    let order = StrategyOrder {
        inner: aapl_usd_order("10", "25"),
        strategy_tag: "alpha".to_owned(),
    };
    let report = StrategyReport {
        inner: ExecutionReportOperation {
            instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
        },
        venue_exec_id: "venue-42".to_owned(),
    };

    assert_eq!(order.strategy_tag, "alpha");
    assert_eq!(order.account_id, AccountId::from_u64(99224416));
    assert_eq!(report.venue_exec_id, "venue-42");
    assert_eq!(report.side, Side::Buy);
    Ok(())
}

#[test]
fn example_wiki_policies_order_validation() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policies.md - OrderValidationPolicy
    // Keep this example in sync with the matching wiki example.
    use openpit::pretrade::policies::OrderValidationPolicy;

    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;

    let order = aapl_usd_order("100", "185");
    engine.start_pre_trade(order)?.execute()?.commit();
    Ok(())
}

#[test]
fn example_wiki_policies_rate_limit() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policies.md - RateLimitPolicy
    // Keep this example in sync with the matching wiki example.
    use std::time::Duration;

    use openpit::pretrade::policies::{
        RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
    };

    let builder = Engine::builder::<OrderOperation, PitExecutionReport, ()>().no_sync();
    let policy = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 100,
                    window: Duration::from_secs(1),
                },
            }),
            [],
            [],
            [],
        )?,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let order = aapl_usd_order("1", "100");
    engine.start_pre_trade(order)?.execute()?.commit();
    Ok(())
}

#[test]
fn example_wiki_policies_order_size_limit() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policies.md - OrderSizeLimitPolicy
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{Asset, Quantity, Volume};
    use openpit::pretrade::policies::{
        OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy,
        OrderSizeLimitSettings,
    };
    use openpit::storage::NoLocking;

    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderSizeLimitPolicy::<NoLocking>::new(
            OrderSizeLimitSettings::new(
                Some(OrderSizeBrokerBarrier {
                    limit: OrderSizeLimit {
                        max_quantity: Quantity::from_str("100")?,
                        max_notional: Volume::from_str("50000")?,
                    },
                }),
                [OrderSizeAssetBarrier {
                    limit: OrderSizeLimit {
                        max_quantity: Quantity::from_str("100")?,
                        max_notional: Volume::from_str("50000")?,
                    },
                    settlement_asset: Asset::new("USD")?,
                }],
                [],
            )?,
        ))
        .build()?;

    let order = aapl_usd_order("10", "100");
    engine.start_pre_trade(order)?.execute()?.commit();
    Ok(())
}

#[test]
fn example_wiki_policies_pnl_bounds_killswitch() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policies.md - PnlBoundsKillSwitchPolicy
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{Asset, Pnl};
    use openpit::pretrade::policies::{
        PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings,
    };

    let builder = Engine::builder::<OrderOperation, PitExecutionReport, ()>().no_sync();
    let policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: Asset::new("USD")?,
                lower_bound: Some(Pnl::from_str("-1000")?),
                upper_bound: Some(Pnl::from_str("500")?),
            }],
            [],
        )?,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let order = aapl_usd_order("1", "100");
    engine.start_pre_trade(order)?.execute()?.commit();
    Ok(())
}

#[test]
fn example_wiki_storage_custom_policy() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Storage.md - Custom Policy with Storage
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountId, Asset, Pnl};
    use openpit::storage::{LockingPolicyFactory, Storage, StorageBuilder};

    pub struct MyPolicy<StorageLockingPolicyFactory>
    where
        StorageLockingPolicyFactory: LockingPolicyFactory,
    {
        realized: Storage<(AccountId, Asset), Pnl, StorageLockingPolicyFactory::Policy>,
    }

    impl<StorageLockingPolicyFactory> MyPolicy<StorageLockingPolicyFactory>
    where
        StorageLockingPolicyFactory:
            LockingPolicyFactory + openpit::storage::CreateStorageFor<(AccountId, Asset)>,
    {
        pub fn new(storage_builder: &StorageBuilder<StorageLockingPolicyFactory>) -> Self {
            Self {
                realized: storage_builder.create_for_bound_key(),
            }
        }

        pub fn record_pnl(&self, account: AccountId, settlement: Asset, delta: Pnl) {
            self.realized.with_mut(
                (account, settlement),
                || Pnl::ZERO,
                |entry, _is_new| {
                    if let Ok(updated) = entry.checked_add(delta) {
                        *entry = updated;
                    }
                },
            );
        }

        pub fn current_pnl(&self, account: AccountId, settlement: &Asset) -> Pnl {
            let key = (account, settlement.clone());
            self.realized
                .with(&key, |entry| *entry)
                .unwrap_or(Pnl::ZERO)
        }
    }

    let builder = Engine::builder::<(), (), ()>().no_sync();
    let policy = MyPolicy::new(builder.storage_builder());

    let account = AccountId::from_u64(1);
    let usd = Asset::new("USD")?;
    policy.record_pnl(account, usd.clone(), Pnl::from_str("-50")?);
    assert_eq!(policy.current_pnl(account, &usd), Pnl::from_str("-50")?);
    Ok(())
}

#[test]
fn example_wiki_storage_engine_builder() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Storage.md - Engine-Owned Builder Use
    // Keep this example in sync with the matching wiki example.
    let builder = Engine::builder::<(), (), ()>().full_sync();
    let counters = builder
        .storage_builder()
        .create_for_bound_key::<&'static str, u64>();

    counters.with_mut(
        "ticks",
        || 0,
        |value, _is_new| {
            *value += 1;
        },
    );

    assert_eq!(counters.with(&"ticks", |value| *value), Some(1));
    assert!(counters.remove(&"ticks"));
    Ok(())
}

#[test]
fn example_wiki_policies_spot_funds() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policies.md - SpotFundsPolicy
    // Keep this example in sync with the matching wiki example.
    use openpit::pretrade::policies::{SpotFundsPolicy, SpotFundsSettings};
    use openpit::{
        Engine, FullSync, OrderOperation, SpotFundsMarketData, SpotFundsPricingSource,
        WithAccountAdjustmentAmount, WithAccountAdjustmentBalanceOperation,
        WithAccountAdjustmentBounds, WithExecutionReportFillDetails, WithExecutionReportOperation,
    };

    // Report and account-adjustment shapes composed from public SDK wrappers.
    type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
    type SpotAdjustment = WithAccountAdjustmentAmount<
        WithAccountAdjustmentBounds<
            WithAccountAdjustmentBalanceOperation<openpit::AccountAdjustmentAmount>,
        >,
    >;

    let builder = Engine::builder::<OrderOperation, SpotReport, SpotAdjustment>().full_sync();
    // Limit-only mode: no market-data bundle.
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])?,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let _engine = builder.pre_trade(policy).build()?;
    Ok(())
}

#[test]
fn example_wiki_spot_funds_limit_only() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Spot-Funds.md - Limit-Only Mode
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{
        AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side, TradeAmount,
    };
    use openpit::pretrade::policies::{SpotFundsPolicy, SpotFundsSettings};
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        Engine, FullSync, Instrument, OrderOperation, SpotFundsMarketData, SpotFundsPricingSource,
        WithAccountAdjustmentAmount, WithAccountAdjustmentBalanceOperation,
        WithAccountAdjustmentBounds, WithExecutionReportFillDetails, WithExecutionReportOperation,
    };

    // Report and account-adjustment shapes composed from public SDK wrappers.
    type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
    type SpotAdjustment = WithAccountAdjustmentAmount<
        WithAccountAdjustmentBounds<
            WithAccountAdjustmentBalanceOperation<openpit::AccountAdjustmentAmount>,
        >,
    >;

    let builder = Engine::builder::<OrderOperation, SpotReport, SpotAdjustment>().full_sync();
    // Limit-only mode: no market-data bundle.
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])?,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let account = AccountId::from_u64(99224416);

    // Seed 10000 USD of available funds through the account-adjustment pipeline.
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
            balance: Some(AdjustmentAmount::Absolute(PositionSize::from_str("10000")?)),
            held: None,
            incoming: None,
        },
    };
    engine.apply_account_adjustment(account, &[seed])?;

    // Buy 10 AAPL @ 200 holds 2000 USD; available drops to 8000.
    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
        account_id: account,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("10")?),
        price: Some(Price::from_str("200")?),
    };
    engine.execute_pre_trade(order)?.commit();
    Ok(())
}

#[test]
fn example_wiki_spot_funds_market_orders() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Spot-Funds.md - Market Orders
    // Keep this example in sync with the matching wiki example.
    use std::sync::Arc;

    use openpit::param::{
        AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side, TradeAmount,
    };
    use openpit::pretrade::policies::{SpotFundsPolicy, SpotFundsSettings};
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        Engine, FullSync, Instrument, OrderOperation, Quote, QuoteTtl, SpotFundsMarketData,
        SpotFundsPricingSource, WithAccountAdjustmentAmount, WithAccountAdjustmentBalanceOperation,
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
    market_data.push(aapl_id, Quote::new().with_mark(Price::from_str("200")?))?;

    // Worst-case slippage of 1500 bps, priced from the quote mark.
    let settings = SpotFundsSettings::new(1500, SpotFundsPricingSource::Mark, [])?;
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
            balance: Some(AdjustmentAmount::Absolute(PositionSize::from_str("10000")?)),
            held: None,
            incoming: None,
        },
    };
    engine.apply_account_adjustment(account, &[seed])?;

    // Market buy (no price): priced at mark 200 + 15% = 230 per unit worst case.
    let order = OrderOperation {
        instrument: aapl,
        account_id: account,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("5")?),
        price: None,
    };
    engine.execute_pre_trade(order)?.commit();
    Ok(())
}

#[test]
fn example_wiki_spot_funds_pnl_kill_switch_builder() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Spot-Funds.md - Self-Computed PnL Kill Switch / Configuring Barriers
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountId, Pnl};
    use openpit::pretrade::policies::{
        SpotFundsPnlBoundsAccountBarrier, SpotFundsPnlBoundsBarrier, SpotFundsPolicy,
    };
    use openpit::{
        Engine, FullSync, OrderOperation, SpotFundsMarketData, WithAccountAdjustmentAmount,
        WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBounds,
        WithExecutionReportFillDetails, WithExecutionReportOperation,
    };

    type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
    type SpotAdjustment = WithAccountAdjustmentAmount<
        WithAccountAdjustmentBounds<
            WithAccountAdjustmentBalanceOperation<openpit::AccountAdjustmentAmount>,
        >,
    >;

    let account = AccountId::from_u64(99224416);

    // A global loss barrier of -1000, plus a tighter per-account barrier.
    let global_barrier = SpotFundsPnlBoundsBarrier {
        lower_bound: Some(Pnl::from_str("-1000")?),
        upper_bound: None,
    };
    let account_barrier = SpotFundsPnlBoundsAccountBarrier {
        barrier: SpotFundsPnlBoundsBarrier {
            lower_bound: Some(Pnl::from_str("-250")?),
            upper_bound: None,
        },
        account_id: account,
    };

    // The PnL kill switch is a distinct spot-funds builder entry point; it
    // produces the same SpotFundsPolicy, registered under the same name.
    let builder = Engine::builder::<OrderOperation, SpotReport, SpotAdjustment>().full_sync();
    let policy = SpotFundsPolicy::<FullSync, FullSync>::pnl_bounds_kill_switch(
        Some(global_barrier),
        [],
        [account_barrier],
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    )?;
    let engine = builder.pre_trade(policy).build()?;

    // Harness only: silence the unused-engine warning; the wiki snippet stops
    // at the built engine.
    let _ = &engine;
    Ok(())
}

#[test]
fn example_wiki_spot_funds_pnl_kill_switch_reconfigure() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Spot-Funds.md - Self-Computed PnL Kill Switch / Runtime Reconfiguration
    // Keep this example in sync with the matching wiki example. The engine and
    // account below are harness scaffolding built to match the snippet's world.
    use openpit::param::{AccountId, Pnl};
    use openpit::pretrade::policies::{
        SpotFundsConfigError, SpotFundsPnlBoundsBarrier, SpotFundsPolicy, SpotFundsPricingSource,
        SpotFundsSettings,
    };
    use openpit::{
        Engine, FullSync, OrderOperation, SpotFundsMarketData, WithAccountAdjustmentAmount,
        WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBounds,
        WithExecutionReportFillDetails, WithExecutionReportOperation,
    };

    type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
    type SpotAdjustment = WithAccountAdjustmentAmount<
        WithAccountAdjustmentBounds<
            WithAccountAdjustmentBalanceOperation<openpit::AccountAdjustmentAmount>,
        >,
    >;

    let account = AccountId::from_u64(99224416);
    let builder = Engine::builder::<OrderOperation, SpotReport, SpotAdjustment>().full_sync();
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])?,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let name = SpotFundsPolicy::<FullSync, FullSync>::NAME;
    let new_lower = Pnl::from_str("-500")?;

    // Retune the account PnL barrier; live accumulated PnL is untouched.
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
                lower_bound: Some(new_lower),
                upper_bound: None,
            }))
        })?;

    // Force-set the live accumulated PnL for one account.
    let result = engine.configure().set_spot_funds_account_pnl(
        name,
        account,
        openpit::PnlState::Value(Pnl::from_str("-600")?),
    )?;
    assert_eq!(result.account_blocks.len(), 1);
    Ok(())
}

#[test]
fn example_wiki_balance_reconciliation_delta_absolute() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Balance-Reconciliation.md - Delta Versus Absolute
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountId, AdjustmentAmount, Asset, PositionSize};
    use openpit::pretrade::policies::{SpotFundsPolicy, SpotFundsSettings};
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        Engine, FullSync, OrderOperation, SpotFundsMarketData, SpotFundsPricingSource,
        WithAccountAdjustmentAmount, WithAccountAdjustmentBalanceOperation,
        WithAccountAdjustmentBounds, WithExecutionReportFillDetails, WithExecutionReportOperation,
    };

    // Report and account-adjustment shapes composed from public SDK wrappers.
    type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
    type SpotAdjustment = WithAccountAdjustmentAmount<
        WithAccountAdjustmentBounds<
            WithAccountAdjustmentBalanceOperation<openpit::AccountAdjustmentAmount>,
        >,
    >;

    let builder = Engine::builder::<OrderOperation, SpotReport, SpotAdjustment>().full_sync();
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])?,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let account = AccountId::from_u64(99224416);

    let seed = |amount: &str| -> Result<SpotAdjustment, Box<dyn std::error::Error>> {
        Ok(WithAccountAdjustmentAmount {
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
                balance: Some(AdjustmentAmount::Absolute(PositionSize::from_str(amount)?)),
                held: None,
                incoming: None,
            },
        })
    };

    // First seed: available USD goes from 0 to 10000.
    let first = engine.apply_account_adjustment(account, &[seed("10000")?])?;
    let usd = first.outcomes[0].entry.balance.expect("balance changed");
    assert_eq!(usd.delta, PositionSize::from_str("10000")?);
    assert_eq!(usd.absolute, PositionSize::from_str("10000")?);

    // Second seed: available USD goes from 10000 to 15000.
    let second = engine.apply_account_adjustment(account, &[seed("15000")?])?;
    let usd = second.outcomes[0].entry.balance.expect("balance changed");
    // delta is the change to add to your own ledger; absolute is just a snapshot.
    assert_eq!(usd.delta, PositionSize::from_str("5000")?);
    assert_eq!(usd.absolute, PositionSize::from_str("15000")?);
    Ok(())
}

#[test]
fn example_wiki_pre_trade_lock_persistence() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Pre-Trade-Lock.md - Persisting and Restoring a Lock
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{
        AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side, Trade, TradeAmount,
    };
    use openpit::pretrade::policies::{SpotFundsPolicy, SpotFundsSettings};
    use openpit::pretrade::PreTradeLock;
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        Engine, ExecutionReportFillDetails, ExecutionReportOperation, FullSync, Instrument,
        OrderOperation, PolicyGroupId, SpotFundsMarketData, SpotFundsPricingSource,
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
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])?,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let account = AccountId::from_u64(99224416);
    let instrument = Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?);

    // Seed 10000 USD so the buy can be reserved.
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
            balance: Some(AdjustmentAmount::Absolute(PositionSize::from_str("10000")?)),
            held: None,
            incoming: None,
        },
    };
    engine.apply_account_adjustment(account, &[seed])?;

    // Buy 10 AAPL @ 200 holds 2000 USD and records the lock price (200).
    let order = OrderOperation {
        instrument: instrument.clone(),
        account_id: account,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("10")?),
        price: Some(Price::from_str("200")?),
    };
    let mut reservation = engine.execute_pre_trade(order)?;

    // Persist the lock in whatever format your store prefers. Here we walk the
    // entries and keep `(group, price-as-string)` pairs; the built-in serde
    // (`serde_json::to_string(reservation.lock())`) is an alternative.
    let persisted: Vec<(u16, String)> = reservation
        .lock()
        .entries()
        .map(|(group, price)| (group.value(), price.to_string()))
        .collect();

    reservation.commit();

    // --- After a process restart, rebuild the lock from your store. ---
    let restored = persisted
        .iter()
        .map(|(group, price)| {
            Ok::<_, Box<dyn std::error::Error>>((
                PolicyGroupId::new(*group),
                Price::from_str(price)?,
            ))
        })
        .collect::<Result<PreTradeLock, _>>()?;

    // The final fill must carry the restored lock so the policy reconciles the
    // 2000 USD it held against the real fill instead of blocking the account.
    let report = WithExecutionReportOperation {
        inner: WithExecutionReportFillDetails {
            inner: (),
            fill: ExecutionReportFillDetails {
                last_trade: Some(Trade {
                    price: Price::from_str("200")?,
                    quantity: Quantity::from_str("10")?,
                }),
                fee: None,
                leaves_quantity: Quantity::from_str("0")?,
                lock: restored,
                is_final: true,
            },
        },
        operation: ExecutionReportOperation {
            instrument,
            account_id: account,
            side: Side::Buy,
        },
    };
    let result = engine.apply_execution_report(&report);
    assert!(result.account_blocks.is_empty());
    Ok(())
}

#[test]
fn example_wiki_account_groups_register_and_read() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Account-Groups.md - Examples → Rust
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountGroupId, AccountId};
    use openpit::pretrade::policies::OrderValidationPolicy;
    use openpit::{Engine, OrderOperation};

    let engine: openpit::LocalEngine<OrderOperation> = Engine::builder()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;

    // Group two accounts under one compact identifier.
    let accounts = engine.accounts();
    let hedge_book = AccountGroupId::from_u32(7)?;
    accounts.register_group(
        &[AccountId::from_u64(10), AccountId::from_u64(11)],
        hedge_book,
    )?;

    // Membership is readable by id, without enumerating the accounts.
    assert_eq!(accounts.group_of(AccountId::from_u64(10)), Some(hedge_book));
    assert_eq!(accounts.group_of(AccountId::from_u64(99)), None);

    // Removing the group is atomic too: every listed account must be a member.
    accounts.unregister_group(
        &[AccountId::from_u64(10), AccountId::from_u64(11)],
        hedge_book,
    )?;
    assert_eq!(accounts.group_of(AccountId::from_u64(10)), None);
    Ok(())
}

#[test]
fn example_wiki_account_block_unblock() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Account-Blocking.md - Examples → Rust
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AccountGroupId, AccountId};
    use openpit::pretrade::policies::OrderValidationPolicy;
    use openpit::{Engine, OrderOperation};

    let engine: openpit::LocalEngine<OrderOperation> = Engine::builder()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;

    let accounts = engine.accounts();

    // Block account 99224416 - all subsequent pre-trade orders are rejected.
    accounts.block(AccountId::from_u64(99224416), "compliance hold".to_string());

    // Unblock account 99224416 - pre-trade orders are allowed again.
    accounts.unblock(AccountId::from_u64(99224416));

    // Block every current and future member of a group in one call.
    let desk = AccountGroupId::from_u32(7)?;
    accounts.block_group(desk, "desk suspended".to_string())?;
    accounts.unblock_group(desk)?;
    Ok(())
}

#[test]
fn example_wiki_spot_funds_global_limit_mode() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Dynamic-Policy-Reconfiguration.md - Spot Funds: Global Limit Mode
    // This mirror is intentionally wider than the wiki snippet: it adds the test
    // harness (the fn -> Result wrapper and seed/order helpers) so the example runs.
    // Keep the shared user-code flow in sync with the wiki.
    use openpit::param::{
        AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side, TradeAmount,
    };
    use openpit::pretrade::policies::{SpotFundsConfigError, SpotFundsPolicy, SpotFundsSettings};
    use openpit::pretrade::SpotFundsLimitMode;
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        Engine, FullSync, Instrument, OrderOperation, SpotFundsMarketData, SpotFundsPricingSource,
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
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])?,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let account = AccountId::from_u64(99224416);
    // Harness: seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
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

    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
        account_id: account,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("10")?),
        price: Some(Price::from_str("200")?),
    };

    // Default Enforce: 2 000 notional exceeds 1 000 available - rejected.
    let rejects = engine
        .execute_pre_trade(order.clone())
        .err()
        .expect("enforce must reject insufficient funds");
    assert_eq!(rejects[0].reason, "spot funds insufficient");

    // Switch to TrackOnly: the same order now passes and reserves against deficit.
    let name = SpotFundsPolicy::<FullSync, FullSync>::NAME;
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
            Ok(())
        })?;
    engine.execute_pre_trade(order.clone())?.commit(); // available: -1 000

    // Restore Enforce: available is negative - still rejected.
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_global_limit_mode(SpotFundsLimitMode::Enforce);
            Ok(())
        })?;
    let rejects = engine
        .execute_pre_trade(order)
        .err()
        .expect("enforce must reject negative available");
    assert_eq!(rejects[0].reason, "spot funds insufficient");
    Ok(())
}

#[test]
fn example_wiki_spot_funds_per_account_limit_mode() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Dynamic-Policy-Reconfiguration.md - Spot Funds: Per-Account Limit Mode
    // This mirror is intentionally wider than the wiki snippet: it adds the test
    // harness (the fn -> Result wrapper and seed/order helpers) so the example runs.
    // Keep the shared user-code flow in sync with the wiki.
    use openpit::param::{
        AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side, TradeAmount,
    };
    use openpit::pretrade::policies::{SpotFundsConfigError, SpotFundsPolicy, SpotFundsSettings};
    use openpit::pretrade::SpotFundsLimitMode;
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        Engine, FullSync, Instrument, OrderOperation, SpotFundsMarketData, SpotFundsPricingSource,
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
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, [])?,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    let account = AccountId::from_u64(99224416);
    // Harness: seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
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

    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
        account_id: account,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("10")?),
        price: Some(Price::from_str("200")?),
    };

    // Global Enforce: under-funded buy is rejected.
    let rejects = engine
        .execute_pre_trade(order.clone())
        .err()
        .expect("enforce must reject insufficient funds");
    assert_eq!(rejects[0].reason, "spot funds insufficient");

    // Pin this account to TrackOnly: per-account override wins over global Enforce.
    let name = SpotFundsPolicy::<FullSync, FullSync>::NAME;
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_account_limit_mode(account, Some(SpotFundsLimitMode::TrackOnly));
            Ok(())
        })?;
    engine.execute_pre_trade(order.clone())?.commit(); // reservation recorded

    // Clear the per-account override: cascade falls back to global Enforce.
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_account_limit_mode(account, None);
            Ok(())
        })?;
    let rejects = engine
        .execute_pre_trade(order)
        .err()
        .expect("enforce must reject after pin is cleared");
    assert_eq!(rejects[0].reason, "spot funds insufficient");
    Ok(())
}

#[test]
fn example_wiki_dynamic_policy_reconfiguration_rate_limit() -> Result<(), Box<dyn std::error::Error>>
{
    // Wiki example: pit.wiki/Dynamic-Policy-Reconfiguration.md - Retune a Built-in Policy
    // This mirror is intentionally wider than the wiki snippet: it adds the test
    // harness (the fn -> Result wrapper and `order` helper) so the example runs.
    // Keep this example in sync with the wiki.
    use std::time::Duration;

    use openpit::pretrade::policies::{
        RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitPolicyError, RateLimitSettings,
    };
    use openpit::storage::NoLocking;
    use openpit::{Engine, OrderOperation, WithExecutionReportOperation, WithFinancialImpact};

    type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;

    // Harness helper: the AAPL/USD order from Getting Started.
    fn order() -> OrderOperation {
        aapl_usd_order("1", "100")
    }

    // Register the rate-limit policy so the engine keeps a handle to its
    // settings cell; built-in policies are configurable by name.
    let builder = Engine::builder::<OrderOperation, Report, ()>().no_sync();
    let policy = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 5,
                    window: Duration::from_secs(60),
                },
            }),
            [],
            [],
            [],
        )?,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    // The generous limit of 5 admits the first three orders. `order` is the
    // AAPL/USD order built in Getting Started.
    for _ in 0..3 {
        engine.execute_pre_trade(order())?.commit();
    }

    // Tighten the broker limit to 2 at runtime, without rebuilding the engine.
    // Built-in policies register under their own name (RateLimitPolicy::NAME).
    let name = RateLimitPolicy::<NoLocking>::NAME;
    engine
        .configure()
        .rate_limit::<RateLimitPolicyError>(name, |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 2,
                    window: Duration::from_secs(60),
                },
            }))
        })?;

    // The next order would have passed under the old limit of 5; the new limit
    // of 2 rejects it, proving the live policy reads the retuned value.
    let rejects = engine
        .execute_pre_trade(order())
        .err()
        .expect("order beyond the tightened limit must be rejected");
    assert_eq!(rejects[0].reason, "rate limit exceeded: broker barrier");
    Ok(())
}

#[test]
fn example_wiki_dynamic_policy_reconfiguration_set_account_pnl(
) -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Dynamic-Policy-Reconfiguration.md - Force-set Accumulated P&L
    // This mirror is intentionally wider than the wiki snippet: it adds the test
    // harness (the fn -> Result wrapper, the `order` helper, and the `account`
    // binding) so the example runs. Keep this example in sync with the wiki.
    use openpit::param::{AccountId, Asset, Pnl};
    use openpit::pretrade::policies::{
        PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings,
    };
    use openpit::storage::NoLocking;
    use openpit::{Engine, OrderOperation, WithExecutionReportOperation, WithFinancialImpact};

    type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;

    // Harness: the AAPL/USD order from Getting Started and its account.
    let account = AccountId::from_u64(99224416);
    fn order() -> OrderOperation {
        aapl_usd_order("1", "100")
    }

    // Register the kill-switch policy so the engine keeps a handle to its
    // accumulator; built-in policies are configurable by name.
    let builder = Engine::builder::<OrderOperation, Report, ()>().no_sync();
    let policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: Asset::new("USD")?,
                lower_bound: Some(Pnl::from_str("-100")?),
                upper_bound: None,
            }],
            [],
        )?,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    // With no P&L history the order passes against the lower bound of -100.
    // `order` is the AAPL/USD order built in Getting Started.
    engine.execute_pre_trade(order())?.commit();

    // Force-set the account's accumulated P&L to -150 USD, below the bound.
    // Built-in policies register under their own name (PnlBoundsKillSwitchPolicy::NAME).
    let name = PnlBoundsKillSwitchPolicy::<NoLocking>::NAME;
    engine.configure().set_account_pnl(
        name,
        account,
        Asset::new("USD")?,
        Pnl::from_str("-150")?,
    )?;

    // The next order for that account breaches the lower bound and is rejected;
    // the breach also latches an engine-level block on the account.
    let rejects = engine
        .execute_pre_trade(order())
        .err()
        .expect("order beyond the breached bound must be rejected");
    assert_eq!(
        rejects[0].reason,
        "pnl kill switch triggered: broker barrier"
    );
    Ok(())
}

#[test]
fn example_wiki_dry_run_verdict() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Non-Mutating-Dry-Run.md - Read the Dry-Run Verdict
    // This mirror is intentionally wider than the wiki snippet: it adds the test
    // harness (the fn -> Result wrapper and assertions) so the example runs.
    // Keep the shared user-code flow in sync with the wiki.
    use openpit::pretrade::policies::OrderValidationPolicy;

    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;
    let order = aapl_usd_order("100", "185");

    let report = engine.execute_pre_trade_dry_run(order);
    if report.is_pass() {
        println!("order would be admitted");
    } else {
        for reject in report.rejects().unwrap().iter() {
            eprintln!(
                "would reject by {} [{}]: {} ({})",
                reject.policy, reject.code, reject.reason, reject.details
            );
        }
    }
    assert!(report.is_pass());
    Ok(())
}

#[test]
fn example_wiki_dry_run_before_real_call() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Non-Mutating-Dry-Run.md - Use the Dry-Run Before a Real Call
    // This mirror is intentionally wider than the wiki snippet: it adds the test
    // harness (the fn -> Result wrapper and assertions) so the example runs.
    // Keep the shared user-code flow in sync with the wiki.
    use openpit::pretrade::policies::OrderValidationPolicy;

    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()?;
    let order = aapl_usd_order("100", "185");

    // Probe without spending any budget or creating any reservation.
    let probe = engine.execute_pre_trade_dry_run(order.clone());
    if probe.is_pass() {
        // The real call now runs with fresh state; the probe had no effect.
        engine.execute_pre_trade(order)?.commit();
    }
    Ok(())
}

// --- Non-Mutating-Dry-Run: Custom Policy with Dry-Run Hook ---

struct MyCountingPolicy {
    count: std::cell::Cell<u64>,
}

impl<O, R, A, Sync> PreTradePolicy<O, R, A, Sync> for MyCountingPolicy
where
    Sync: openpit::SyncMode,
{
    fn name(&self) -> &str {
        "MyCountingPolicy"
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _order: &O,
    ) -> Result<(), Rejects> {
        self.count.set(self.count.get() + 1); // side effect
        Ok(())
    }

    // Dry-run variant: read-only, no counter increment.
    fn check_pre_trade_start_dry_run(
        &self,
        _ctx: &PreTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _order: &O,
    ) -> Result<(), Rejects> {
        Ok(()) // inspect state without spending budget
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _order: &O,
        _mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        Ok(None)
    }

    fn apply_execution_report(
        &self,
        _ctx: &PostTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
        _report: &R,
    ) -> Option<openpit::PostTradeResult> {
        None
    }
}

#[test]
fn example_wiki_dry_run_custom_policy_hook() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Non-Mutating-Dry-Run.md - Read-Only Custom Start-Stage Hook
    // This mirror is intentionally wider than the wiki snippet: it defines the full
    // MyCountingPolicy struct above (the snippet shows only the hook bodies) and
    // exercises the dry-run and real-call paths to confirm compilation and correct
    // routing. Keep the shared user-code flow in sync with the wiki.
    let engine = Engine::builder::<OrderOperation, PitExecutionReport, ()>()
        .no_sync()
        .pre_trade(MyCountingPolicy {
            count: std::cell::Cell::new(0),
        })
        .build()?;
    let order = aapl_usd_order("10", "100");

    // Dry-run path routes through check_pre_trade_start_dry_run.
    let probe = engine.execute_pre_trade_dry_run(order.clone());
    assert!(probe.is_pass());
    // Real call path routes through check_pre_trade_start.
    engine.execute_pre_trade(order)?.commit();
    Ok(())
}
