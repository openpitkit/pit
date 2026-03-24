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
use std::time::Duration;

use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::{
    OrderSizeLimit, OrderSizeLimitPolicy, OrderValidationPolicy, PnlKillSwitchPolicy,
    RateLimitPolicy,
};
use openpit::pretrade::{
    AccountAdjustmentPolicy, Context, Mutation, Mutations, Policy, Reject, RejectCode, RejectScope,
};
use openpit::{
    Engine, ExecutionReportOperation, FinancialImpact, HasOrderPrice, HasTradeAmount, Instrument,
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

struct ReservePolicy {
    reserved: Rc<RefCell<Volume>>,
    next: Volume,
}

impl<O, R> Policy<O, R> for ReservePolicy {
    fn name(&self) -> &'static str {
        "ReservePolicy"
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &Context<'_, O>,
        mutations: &mut Mutations,
        _rejects: &mut Vec<Reject>,
    ) {
        let prev = *self.reserved.borrow();
        let commit_reserved = Rc::clone(&self.reserved);
        let rollback_reserved = Rc::clone(&self.reserved);
        let next = self.next;

        mutations.push(Mutation::new(
            move || {
                *commit_reserved.borrow_mut() = next;
            },
            move || {
                *rollback_reserved.borrow_mut() = prev;
            },
        ));
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

struct RejectingPolicy;

impl<O, R> Policy<O, R> for RejectingPolicy {
    fn name(&self) -> &'static str {
        "RejectingPolicy"
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &Context<'_, O>,
        _mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        rejects.push(Reject::new(
            "RejectingPolicy",
            RejectScope::Order,
            RejectCode::RiskLimitExceeded,
            "forced reject",
            "demonstrates rollback when a later policy fails",
        ));
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

// --- Policy-API: Custom Main-Stage Policy ---

struct NotionalCapPolicy {
    max_abs_notional: Volume,
}

impl<O, R> Policy<O, R> for NotionalCapPolicy
where
    O: HasTradeAmount + HasOrderPrice,
{
    fn name(&self) -> &'static str {
        "NotionalCapPolicy"
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &Context<'_, O>,
        _mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        let order = ctx.order();
        let trade_amount = match order.trade_amount() {
            Ok(trade_amount) => trade_amount,
            Err(error) => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::MissingRequiredField,
                    "required order field missing",
                    error.to_string(),
                ));
                return;
            }
        };
        let price = match order.price() {
            Ok(price) => price,
            Err(error) => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::MissingRequiredField,
                    "required order field missing",
                    error.to_string(),
                ));
                return;
            }
        };
        let requested_notional = match (trade_amount, price) {
            (TradeAmount::Volume(volume), _) => volume,
            (TradeAmount::Quantity(quantity), Some(price)) => {
                match price.calculate_volume(quantity) {
                    Ok(v) => v,
                    Err(_) => {
                        rejects.push(Reject::new(
                            <Self as Policy<O, R>>::name(self),
                            RejectScope::Order,
                            RejectCode::OrderValueCalculationFailed,
                            "order value calculation failed",
                            "price and quantity could not be used to evaluate notional",
                        ));
                        return;
                    }
                }
            }
            (TradeAmount::Quantity(_), None) => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::OrderValueCalculationFailed,
                    "order value calculation failed",
                    "price not provided for evaluating cash flow/notional/volume",
                ));
                return;
            }
            _ => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::UnsupportedOrderType,
                    "unsupported order type",
                    "custom trade amount variant is not supported by this policy",
                ));
                return;
            }
        };

        if requested_notional > self.max_abs_notional {
            rejects.push(Reject::new(
                <Self as Policy<O, R>>::name(self),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                format!(
                    "requested notional {}, max allowed: {}",
                    requested_notional, self.max_abs_notional
                ),
            ));
        }
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

// --- Account-Adjustments: Balance Limit Policy ---

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
    fn name(&self) -> &'static str {
        "BalanceLimitPolicy"
    }

    fn apply_account_adjustment(
        &self,
        _account_id: AccountId,
        adjustment: &A,
        mutations: &mut Mutations,
    ) -> Result<(), Reject> {
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
            return Err(Reject::new(
                "BalanceLimitPolicy",
                RejectScope::Account,
                RejectCode::RiskLimitExceeded,
                "cumulative adjustment exceeds limit",
                format!("asset {asset_id}: {new_total} > {}", self.max_total),
            ));
        }

        // Apply the new total immediately.
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
                // Commit: state is already applied, nothing extra needed.
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

// --- Tests ---

#[test]
fn example_wiki_domain_types_create_validated_values() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Domain-Types.md — Create Validated Values
    use openpit::param::{Asset, Pnl, Price, Quantity};

    let asset = Asset::new("AAPL").expect("asset code must be valid");
    let quantity = Quantity::from_str("10.5").expect("quantity must be valid");
    let price = Price::from_str("185").expect("price must be valid");
    let pnl = Pnl::from_str("-12.5").expect("pnl must be valid");

    assert_eq!(asset.as_ref(), "AAPL");
    assert_eq!(quantity.to_string(), "10.5");
    assert_eq!(price.to_string(), "185");
    assert_eq!(pnl.to_string(), "-12.5");
    Ok(())
}

#[test]
fn example_wiki_domain_types_directional_types() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Domain-Types.md — Work With Directional Types
    use openpit::param::{PositionSide, Side};

    assert_eq!(Side::Buy.opposite(), Side::Sell);
    assert_eq!(Side::Sell.sign(), -1);
    assert_eq!(PositionSide::Long.opposite(), PositionSide::Short);
    Ok(())
}

#[test]
fn example_wiki_domain_types_leverage() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Domain-Types.md — Create Leverage
    use openpit::param::Leverage;

    let from_multiplier = Leverage::from_u16(100).expect("valid leverage");
    let from_float = Leverage::from_f64(100.5).expect("valid leverage");

    assert_eq!(from_multiplier.value(), 100.0);
    assert_eq!(from_float.value(), 100.5);
    Ok(())
}

#[test]
fn example_wiki_getting_started() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Getting-Started.md — Build an Engine + Run an Order Through the Engine + Apply Post-Trade Feedback
    type ExecutionReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

    let usd = Asset::new("USD")?;

    let pnl_policy = PnlKillSwitchPolicy::new((usd.clone(), Pnl::from_str("1000")?), [])?;

    let rate_limit_policy = RateLimitPolicy::new(100, Duration::from_secs(1));

    let size_policy = OrderSizeLimitPolicy::new(
        OrderSizeLimit {
            settlement_asset: usd.clone(),
            max_quantity: Quantity::from_str("500")?,
            max_notional: Volume::from_str("100000")?,
        },
        [],
    );

    let engine = Engine::<OrderOperation, ExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .check_pre_trade_start_policy(pnl_policy)
        .check_pre_trade_start_policy(rate_limit_policy)
        .check_pre_trade_start_policy(size_policy)
        .build()?;

    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("100")?),
        price: Some(Price::from_str("185")?),
    };

    let request = match engine.start_pre_trade(order) {
        Ok(request) => request,
        Err(reject) => {
            eprintln!(
                "rejected by {} [{}]: {} ({})",
                reject.policy, reject.code, reject.reason, reject.details
            );
            return Ok(());
        }
    };

    let reservation = match request.execute() {
        Ok(reservation) => reservation,
        Err(rejects) => {
            for reject in rejects.iter() {
                eprintln!(
                    "rejected by {} [{}]: {} ({})",
                    reject.policy, reject.code, reject.reason, reject.details
                );
            }
            return Ok(());
        }
    };

    reservation.commit();

    let report = WithExecutionReportOperation {
        inner: WithFinancialImpact {
            inner: (),
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str("-50")?,
                fee: Fee::from_str("3")?,
            },
        },
        operation: ExecutionReportOperation {
            instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
        },
    };

    let result = engine.apply_execution_report(&report);
    if result.kill_switch_triggered {
        eprintln!("halt new orders until the blocked state is cleared");
    }
    Ok(())
}

#[test]
fn example_wiki_pipeline_start_stage_reject() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Pre-trade-Pipeline.md — Handle a Start-Stage Reject
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .build()?;
    let order = aapl_usd_order("100", "185");

    match engine.start_pre_trade(order) {
        Ok(request) => {
            let _request = request;
        }
        Err(reject) => {
            eprintln!(
                "rejected by {} [{}]: {} ({})",
                reject.policy, reject.code, reject.reason, reject.details
            );
        }
    }
    Ok(())
}

#[test]
fn example_wiki_pipeline_main_stage_finalize() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Pre-trade-Pipeline.md — Execute the Main Stage and Finalize the Reservation
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .build()?;
    let order = aapl_usd_order("100", "185");

    let request = engine
        .start_pre_trade(order)
        .expect("start stage must pass");

    match request.execute() {
        Ok(reservation) => reservation.commit(),
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
fn example_wiki_account_adjustments() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Account-Adjustments.md — Examples → Rust
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

    let engine = Engine::<(), (), AccountAdjustment>::builder().build()?;
    let result = engine.apply_account_adjustment(account_id, &adjustments);
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn example_wiki_account_adjustments_balance_limit_policy() -> Result<(), Box<dyn std::error::Error>>
{
    // Used in: pit.wiki/Account-Adjustments.md — Balance Limit Policy → Rust
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
    // Used in: pit.wiki/Policy-API.md — Rollback Safety Pattern → Rust
    let reserved = Rc::new(RefCell::new(Volume::from_str("0")?));

    let reserve_policy = ReservePolicy {
        reserved: Rc::clone(&reserved),
        next: Volume::from_str("100")?,
    };

    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .pre_trade_policy(reserve_policy)
        .pre_trade_policy(RejectingPolicy)
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
    // Used in: pit.wiki/Policy-API.md — Custom Main-Stage Policy → Rust
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
    // Used in: pit.wiki/Custom-Rust-Types.md — Manual Field Implementations
    use openpit::{HasInstrument, Instrument, RequestFieldAccessError};

    struct MyOrder {
        instrument: Instrument,
    }

    impl HasInstrument for MyOrder {
        fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
            Ok(&self.instrument)
        }
    }

    let order = MyOrder {
        instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
    };
    let _instrument = order.instrument()?;
    Ok(())
}

#[cfg(feature = "derive")]
#[test]
fn example_wiki_custom_types_derive() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Custom-Rust-Types.md — Derive-Based Wrapper Composition
    use openpit::param::{AccountId, TradeAmount};
    use openpit::{
        HasAccountId, HasInstrument, HasOrderPrice, HasTradeAmount, Instrument,
        RequestFieldAccessError, RequestFields,
    };

    #[derive(RequestFields)]
    #[allow(dead_code)]
    struct WithMyOperation<T> {
        inner: T,
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
    let _instrument = order.instrument()?;
    Ok(())
}

#[cfg(feature = "derive")]
#[test]
fn example_wiki_custom_types_inner_field() -> Result<(), Box<dyn std::error::Error>> {
    // Used in: pit.wiki/Custom-Rust-Types.md — Selecting the Inner Field
    use openpit::{HasInstrument, Instrument, RequestFieldAccessError, RequestFields};

    #[derive(RequestFields)]
    #[allow(dead_code)]
    struct WithMyOperation<T> {
        #[openpit(inner)]
        base: T,
        #[openpit(HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>))]
        operation: openpit::OrderOperation,
    }

    let order = WithMyOperation {
        base: (),
        operation: aapl_usd_order("10", "25"),
    };
    let _instrument = order.instrument()?;
    Ok(())
}
