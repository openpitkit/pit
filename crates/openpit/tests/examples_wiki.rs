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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::{
    PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects,
};
use openpit::{
    AccountAdjustmentContext, AccountAdjustmentPolicy, Engine, ExecutionReportOperation,
    FinancialImpact, HasOrderPrice, HasTradeAmount, Instrument, Mutation, Mutations,
    OrderOperation, WithExecutionReportOperation, WithFinancialImpact,
};

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

impl<O, R> PreTradePolicy<O, R> for ReserveThenValidatePolicy {
    fn name(&self) -> &str {
        "ReserveThenValidatePolicy"
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext,
        _order: &O,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
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
                <Self as PreTradePolicy<O, R>>::name(self),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "temporary reservation exceeds limit",
                format!("reserved {}, limit: {}", next, self.limit),
            )));
        }
        Ok(())
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

// --- Policy-API: Custom Main-Stage Policy ---

struct NotionalCapPolicy {
    // Policy-local config: reject any order above this absolute notional.
    max_abs_notional: Volume,
}

impl<O, R> PreTradePolicy<O, R> for NotionalCapPolicy
where
    O: HasTradeAmount + HasOrderPrice,
{
    fn name(&self) -> &str {
        "NotionalCapPolicy"
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext,
        order: &O,
        _mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        // Translate the public order surface into one number that this policy
        // can reason about: requested notional.
        let trade_amount = match order.trade_amount() {
            Ok(trade_amount) => trade_amount,
            Err(error) => {
                return Err(Rejects::from(Reject::new(
                    <Self as PreTradePolicy<O, R>>::name(self),
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
                    <Self as PreTradePolicy<O, R>>::name(self),
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
                            <Self as PreTradePolicy<O, R>>::name(self),
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
                    <Self as PreTradePolicy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::OrderValueCalculationFailed,
                    "order value calculation failed",
                    "price not provided for evaluating cash flow/notional/volume",
                )));
            }
            _ => {
                return Err(Rejects::from(Reject::new(
                    <Self as PreTradePolicy<O, R>>::name(self),
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
                <Self as PreTradePolicy<O, R>>::name(self),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                format!(
                    "requested notional {}, max allowed: {}",
                    requested_notional, self.max_abs_notional
                ),
            )));
        }
        Ok(())
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

// --- Tests ---

#[test]
fn example_wiki_domain_types_create_validated_values() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Domain-Types.md — Create Validated Values
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
    // Wiki example: pit.wiki/Domain-Types.md — Work With Directional Types
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
    // Wiki example: pit.wiki/Domain-Types.md — Create Leverage
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
fn example_wiki_pipeline_start_stage_reject() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md — Handle a Start-Stage Reservation
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
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
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md — Execute the Main Stage and Finalize the Reservation
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
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
fn example_wiki_pipeline_shortcut_start_and_main() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md — Shortcut for Start + Main Stages
    // Wiki example: pit.wiki/Getting-Started.md — Shortcut for Start + Main Stages
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
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
    // Wiki example: pit.wiki/Pre-trade-Pipeline.md — Apply Post-Trade Feedback
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .build()?;
    let report = aapl_usd_report("-50", "3.4");

    // Execution reports feed realized outcomes back into cumulative policy state.
    let result = engine.apply_execution_report(&report);
    if result.kill_switch_triggered {
        eprintln!("halt new orders until the blocked state is cleared");
    }
    assert!(!result.kill_switch_triggered);

    Ok(())
}

#[test]
fn example_wiki_account_adjustments() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Account-Adjustments.md — Examples → Rust
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
                total: Some(AdjustmentAmount::Absolute(PositionSize::from_f64(10000.0)?)),
                reserved: None,
                pending: None,
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
                total: Some(AdjustmentAmount::Absolute(PositionSize::from_f64(-3.0)?)),
                reserved: None,
                pending: None,
            },
        },
    ];

    // The engine validates the whole batch atomically.
    let engine = Engine::<(), (), AccountAdjustment>::builder().build()?;
    let result = engine.apply_account_adjustment(account_id, &adjustments);
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn example_wiki_account_adjustments_balance_limit_policy() -> Result<(), Box<dyn std::error::Error>>
{
    // Wiki example: pit.wiki/Account-Adjustments.md — Balance Limit Policy → Rust
    // Keep this example in sync with the matching wiki example.

    /// Adjustment type must expose an asset and a delta amount.
    trait HasAssetDelta {
        fn asset_id(&self) -> &str;
        fn delta(&self) -> Volume;
    }

    struct BalanceLimitPolicy {
        max_total: Volume,
        totals: Rc<RefCell<HashMap<String, Volume>>>,
    }

    impl BalanceLimitPolicy {
        fn new(max_total: Volume) -> Self {
            Self {
                max_total,
                totals: Rc::new(RefCell::new(HashMap::new())),
            }
        }
    }

    impl<A: HasAssetDelta> AccountAdjustmentPolicy<A> for BalanceLimitPolicy {
        fn name(&self) -> &str {
            "BalanceLimitPolicy"
        }

        fn apply_account_adjustment(
            &self,
            _ctx: &AccountAdjustmentContext,
            _account_id: AccountId,
            adjustment: &A,
            mutations: &mut Mutations,
        ) -> Result<(), Rejects> {
            // Use the asset as the aggregation key for the cumulative limit.
            let asset_id = adjustment.asset_id().to_owned();
            let delta = adjustment.delta();

            let prev_total = {
                let totals = self.totals.borrow();
                totals
                    .get(&asset_id)
                    .copied()
                    .unwrap_or(Volume::from_str("0").unwrap())
            };

            let new_total = prev_total; // simplified: prev_total + delta

            if new_total > self.max_total {
                return Err(Rejects::from(Reject::new(
                    <Self as AccountAdjustmentPolicy<A>>::name(self),
                    RejectScope::Account,
                    RejectCode::RiskLimitExceeded,
                    "cumulative adjustment exceeds limit",
                    format!("asset {asset_id}: {new_total} > {}", self.max_total),
                )));
            }

            // Apply immediately so later adjustments in the same batch see the updated total.
            self.totals.borrow_mut().insert(asset_id.clone(), new_total);

            // Register rollback: restore previous absolute value.
            // Safe because account adjustment batches are fully internal.
            let rollback_totals = Rc::clone(&self.totals);
            let commit_totals = Rc::clone(&self.totals);
            let rollback_asset = asset_id.clone();
            let commit_asset = asset_id;
            let _ = delta;

            mutations.push(Mutation::new(
                move || {
                    // Commit is empty: state was applied eagerly.
                    let _ = commit_totals;
                    let _ = commit_asset;
                },
                move || {
                    // Rollback: restore absolute value captured before modification.
                    rollback_totals
                        .borrow_mut()
                        .insert(rollback_asset, prev_total);
                },
            ));

            Ok(())
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

    let policy = BalanceLimitPolicy::new(Volume::from_str("1000000")?);
    let engine = Engine::<(), (), SimpleAdjustment>::builder()
        .account_adjustment_policy(policy)
        .build()?;

    let result = engine.apply_account_adjustment(
        AccountId::from_u64(99224416),
        &[SimpleAdjustment {
            asset: "USD".to_string(),
            delta: Volume::from_str("100")?,
        }],
    );
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn example_wiki_policy_rollback_safety() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Policy-API.md — Rollback Safety Pattern → Rust
    // Keep this example in sync with the matching wiki example.
    let reserved = Rc::new(RefCell::new(Volume::from_str("0")?));

    let reserve_policy = ReserveThenValidatePolicy {
        reserved: Rc::clone(&reserved),
        next: Volume::from_str("100")?,
        limit: Volume::from_str("50")?,
    };

    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .pre_trade_policy(reserve_policy)
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
    // Wiki example: pit.wiki/Policy-API.md — Custom Main-Stage Policy → Rust
    // Keep this example in sync with the matching wiki example.
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .pre_trade_policy(NotionalCapPolicy {
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
fn example_wiki_custom_types_manual() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Custom-Rust-Types.md — Manual Field Implementations
    // Keep this example in sync with the matching wiki example.
    use openpit::{HasInstrument, RequestFieldAccessError};

    struct MyOrder {
        instrument: Instrument,
    }

    impl HasInstrument for MyOrder {
        fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
            // Expose the project field through the capability trait expected by policies.
            Ok(&self.instrument)
        }
    }

    let order = MyOrder {
        instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
    };
    let instrument = order.instrument()?;
    assert_eq!(instrument.settlement_asset(), &Asset::new("USD")?);
    Ok(())
}

#[cfg(feature = "derive")]
#[test]
fn example_wiki_custom_types_derive() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Custom-Rust-Types.md — Derive-Based Wrapper Composition
    // Keep this example in sync with the matching wiki example.
    use openpit::{
        HasAccountId, HasInstrument, HasOrderPrice, HasTradeAmount, RequestFieldAccessError,
        RequestFields,
    };

    #[derive(RequestFields)]
    #[allow(dead_code)]
    struct WithMyOperation<T> {
        // Preserve capabilities already provided by outer wrappers.
        inner: T,
        // Map SDK capability traits onto the embedded standard operation record.
        #[openpit(
            HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>),
            HasAccountId(account_id -> Result<AccountId, RequestFieldAccessError>),
            HasTradeAmount(trade_amount -> Result<TradeAmount, RequestFieldAccessError>),
            HasOrderPrice(price -> Result<Option<Price>, RequestFieldAccessError>)
        )]
        operation: openpit::OrderOperation,
    }

    let order = WithMyOperation {
        inner: (),
        operation: aapl_usd_order("10", "25"),
    };
    let instrument = order.instrument()?;
    assert_eq!(instrument.underlying_asset(), &Asset::new("AAPL")?);
    assert_eq!(order.account_id()?, AccountId::from_u64(99224416));
    assert_eq!(
        order.trade_amount()?,
        TradeAmount::Quantity(Quantity::from_str("10")?)
    );
    assert_eq!(order.price()?, Some(Price::from_str("25")?));
    Ok(())
}

#[cfg(feature = "derive")]
#[test]
fn example_wiki_custom_types_inner_field() -> Result<(), Box<dyn std::error::Error>> {
    // Wiki example: pit.wiki/Custom-Rust-Types.md — Selecting the Inner Field
    // Keep this example in sync with the matching wiki example.
    use openpit::{HasInstrument, RequestFieldAccessError, RequestFields};

    struct Base {
        instrument: Instrument,
    }

    impl HasInstrument for Base {
        fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
            Ok(&self.instrument)
        }
    }

    #[derive(RequestFields)]
    #[allow(dead_code)]
    struct WithMyOperation<T> {
        // Explicitly declare which traits should passthrough to the non-standard inner field.
        #[openpit(inner, HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>))]
        base: T,
    }

    let order = WithMyOperation {
        base: Base {
            instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
        },
    };
    let instrument = order.instrument()?;
    assert_eq!(instrument.underlying_asset(), &Asset::new("AAPL")?);
    Ok(())
}

#[cfg(feature = "derive")]
#[test]
fn example_wiki_custom_types_account_adjustment_wrapper() -> Result<(), Box<dyn std::error::Error>>
{
    // Wiki example: pit.wiki/Custom-Rust-Types.md — Derive-Based Wrapper Composition / account adjustments
    // Keep this example in sync with the matching wiki example.
    use openpit::param::{AdjustmentAmount, PositionSize};
    use openpit::{
        HasAccountAdjustmentPending, HasAccountAdjustmentReserved, HasAccountAdjustmentTotal,
        HasBalanceAsset, RequestFieldAccessError, RequestFields,
    };

    struct BalanceContext {
        asset: Asset,
    }

    impl HasBalanceAsset for BalanceContext {
        fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
            Ok(&self.asset)
        }
    }

    #[derive(RequestFields)]
    #[allow(dead_code)]
    struct WithAccountAdjustmentAmount<T> {
        // Keep balance-level capabilities from the outer context available.
        #[openpit(inner, HasBalanceAsset(balance_asset -> Result<&Asset, RequestFieldAccessError>))]
        inner: T,
        // Expose the standard account-adjustment amount fields through capability traits.
        #[openpit(
            HasAccountAdjustmentTotal(total -> Result<Option<AdjustmentAmount>, RequestFieldAccessError>),
            HasAccountAdjustmentReserved(reserved -> Result<Option<AdjustmentAmount>, RequestFieldAccessError>),
            HasAccountAdjustmentPending(pending -> Result<Option<AdjustmentAmount>, RequestFieldAccessError>)
        )]
        amount: openpit::AccountAdjustmentAmount,
    }

    let wrapper = WithAccountAdjustmentAmount {
        inner: BalanceContext {
            asset: Asset::new("USD")?,
        },
        amount: openpit::AccountAdjustmentAmount {
            total: Some(AdjustmentAmount::Absolute(PositionSize::from_str("100")?)),
            reserved: Some(AdjustmentAmount::Delta(PositionSize::from_str("-20")?)),
            pending: Some(AdjustmentAmount::Delta(PositionSize::from_str("5")?)),
        },
    };

    assert_eq!(wrapper.balance_asset()?, &Asset::new("USD")?);
    assert_eq!(
        wrapper.total()?,
        Some(AdjustmentAmount::Absolute(PositionSize::from_str("100")?))
    );
    assert_eq!(
        wrapper.reserved()?,
        Some(AdjustmentAmount::Delta(PositionSize::from_str("-20")?))
    );
    assert_eq!(
        wrapper.pending()?,
        Some(AdjustmentAmount::Delta(PositionSize::from_str("5")?))
    );
    Ok(())
}
