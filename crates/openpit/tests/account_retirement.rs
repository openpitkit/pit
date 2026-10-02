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

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use openpit::param::{
    AccountGroupId, AccountId, AdjustmentAmount, Asset, Fee, Pnl, PositionSize, Price, Quantity,
    Side, TradeAmount,
};
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitSettings,
    PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings, RateLimit,
    RateLimitAccountAssetBarrier, RateLimitAccountBarrier, RateLimitBrokerBarrier, RateLimitPolicy,
    RateLimitSettings, SpotFundsPolicy, SpotFundsPricingSource, SpotFundsSettings,
};
use openpit::pretrade::{PreTradeLock, PreTradePolicy, SpotFundsLimitMode};
use openpit::storage::{FullLocking, NoLocking};
use openpit::{
    AccountRetirementError, AccountRetirementPolicyRefusal, AccountRetirementRefusal, Engine,
    FullSync, HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
    HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceUpperBound,
    HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
    HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
    HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound,
    HasAccountAdjustmentPnlOperation, HasAccountId, HasBalanceAsset, HasExecutionReportFillFee,
    HasExecutionReportIsFinal, HasExecutionReportLastTrade, HasFee, HasInstrument, HasPnl,
    HasPreTradeLock, HasRemainingReservedQuantity, HasSide, Instrument, LocalSync, Mutation,
    Mutations, OrderOperation, PnlHaltReason, PnlState, RequestFieldAccessError,
    SpotFundsMarketData, SyncMode,
};

const ACCOUNT: u64 = 99224416;
const OTHER: u64 = 11223344;

fn account() -> AccountId {
    AccountId::from_u64(ACCOUNT)
}

fn asset(code: &str) -> Asset {
    Asset::new(code).expect("valid asset")
}

fn pnl(value: &str) -> Pnl {
    Pnl::from_str(value).expect("valid PnL")
}

fn order(id: AccountId) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(asset("AAPL"), asset("USD")),
        account_id: id,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("1").expect("valid quantity")),
        price: Some(Price::from_str("1").expect("valid price")),
    }
}

struct TestReport;

impl HasInstrument for TestReport {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new("unused report instrument"))
    }
}
impl HasAccountId for TestReport {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new("unused report account"))
    }
}
impl HasSide for TestReport {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new("unused report side"))
    }
}
impl HasExecutionReportLastTrade for TestReport {
    fn last_trade(&self) -> Result<Option<openpit::param::Trade>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasExecutionReportFillFee for TestReport {
    fn fill_fee(&self) -> Result<Option<openpit::param::MonetaryAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasRemainingReservedQuantity for TestReport {
    fn remaining_reserved_quantity(&self) -> Result<Quantity, RequestFieldAccessError> {
        Ok(Quantity::ZERO)
    }
}
impl HasExecutionReportIsFinal for TestReport {
    fn is_final(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(true)
    }
}
impl HasPreTradeLock for TestReport {
    fn lock(&self) -> Result<PreTradeLock, RequestFieldAccessError> {
        Ok(PreTradeLock::default())
    }
}
impl HasPnl for TestReport {
    fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
        Ok(Pnl::ZERO)
    }
}
impl HasFee for TestReport {
    fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
        Ok(Fee::ZERO)
    }
}

struct TestAdjustment {
    asset: Asset,
    balance: Option<AdjustmentAmount>,
    position_pnl: Option<PnlState>,
    account_pnl: Option<PnlState>,
}

impl HasBalanceAsset for TestAdjustment {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        Ok(&self.asset)
    }
}
impl HasAccountAdjustmentBalance for TestAdjustment {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.balance)
    }
    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        Ok(self.position_pnl)
    }
}
impl HasAccountAdjustmentPnlOperation for TestAdjustment {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, RequestFieldAccessError> {
        Ok(self.account_pnl)
    }
}
impl HasAccountAdjustmentBalanceAverageEntryPrice for TestAdjustment {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentHeld for TestAdjustment {
    fn held(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentIncoming for TestAdjustment {
    fn incoming(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}
macro_rules! no_bound {
    ($trait_name:ident, $method:ident) => {
        impl $trait_name for TestAdjustment {
            fn $method(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
                Ok(None)
            }
        }
    };
}
no_bound!(HasAccountAdjustmentBalanceLowerBound, balance_lower);
no_bound!(HasAccountAdjustmentBalanceUpperBound, balance_upper);
no_bound!(HasAccountAdjustmentHeldLowerBound, held_lower);
no_bound!(HasAccountAdjustmentHeldUpperBound, held_upper);
no_bound!(HasAccountAdjustmentIncomingLowerBound, incoming_lower);
no_bound!(HasAccountAdjustmentIncomingUpperBound, incoming_upper);

fn balance(value: &str) -> TestAdjustment {
    TestAdjustment {
        asset: asset("USD"),
        balance: Some(AdjustmentAmount::Absolute(
            PositionSize::from_str(value).expect("valid position size"),
        )),
        position_pnl: None,
        account_pnl: None,
    }
}

fn balance_delta(value: &str) -> TestAdjustment {
    TestAdjustment {
        asset: asset("USD"),
        balance: Some(AdjustmentAmount::Delta(
            PositionSize::from_str(value).expect("valid position size"),
        )),
        position_pnl: None,
        account_pnl: None,
    }
}

fn empty_adjustment() -> TestAdjustment {
    TestAdjustment {
        asset: asset("USD"),
        balance: None,
        position_pnl: None,
        account_pnl: None,
    }
}

fn spot_settings() -> SpotFundsSettings {
    SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).expect("valid Spot Funds settings")
}

fn rate_limit() -> RateLimit {
    RateLimit {
        max_orders: 100,
        window: Duration::from_secs(3600),
    }
}

fn broker_rate_settings() -> RateLimitSettings {
    RateLimitSettings::new(
        Some(RateLimitBrokerBarrier {
            limit: rate_limit(),
        }),
        [],
        [],
        [],
    )
    .expect("valid rate settings")
}

fn pnl_broker_settings() -> PnlBoundsKillSwitchSettings {
    PnlBoundsKillSwitchSettings::new(
        [PnlBoundsBrokerBarrier {
            settlement_asset: asset("USD"),
            lower_bound: Some(pnl("-100")),
            upper_bound: None,
        }],
        [],
    )
    .expect("valid PnL settings")
}

fn assert_refusal(error: AccountRetirementError, policy: &str, refusal: AccountRetirementRefusal) {
    assert_eq!(
        error,
        AccountRetirementError::Refused {
            refusals: vec![AccountRetirementPolicyRefusal {
                policy: policy.to_owned(),
                refusal,
            }],
        }
    );
}

type TestEngine = openpit::FullSyncEngine<OrderOperation, TestReport, TestAdjustment>;

fn spot_engine() -> TestEngine {
    let builder = Engine::builder::<OrderOperation, TestReport, TestAdjustment>().full_sync();
    let spot = SpotFundsPolicy::<FullSync, FullSync>::new(
        spot_settings(),
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    builder.pre_trade(spot).build().expect("engine builds")
}

macro_rules! retire_happy_mode {
    ($mode:ident, $sync:ty, $market_sync:ty, $locking:ty) => {{
        let builder = Engine::builder::<OrderOperation, TestReport, TestAdjustment>().$mode();
        let spot = SpotFundsPolicy::<$sync, $market_sync>::new(
            spot_settings(),
            None::<SpotFundsMarketData<$market_sync>>,
            builder.storage_builder(),
        );
        let rate =
            RateLimitPolicy::<$locking>::new(broker_rate_settings(), builder.storage_builder());
        let size = OrderSizeLimitPolicy::<$locking>::new(
            OrderSizeLimitSettings::new(None, [], []).expect("empty limits are valid"),
        );
        let kill = PnlBoundsKillSwitchPolicy::<$locking>::new(
            pnl_broker_settings(),
            builder.storage_builder(),
        );
        let engine = builder
            .pre_trade(spot)
            .pre_trade(rate)
            .pre_trade(size)
            .pre_trade(kill)
            .build()
            .expect("engine builds");
        let group = AccountGroupId::from_u32(123).expect("valid group");
        engine
            .accounts()
            .register_group(&[account()], group)
            .expect("register group");
        engine.accounts().block(account(), "admin block".to_owned());
        engine
            .apply_account_adjustment(account(), &[balance("10")])
            .expect("credit");
        engine
            .apply_account_adjustment(account(), &[balance("0")])
            .expect("zero");
        assert_eq!(engine.retire_account(account()), Ok(()));
        assert_eq!(engine.accounts().group_of(account()), None);
        assert!(
            engine.start_pre_trade(order(account())).is_ok(),
            "retired account must not remain blocked"
        );
        assert_eq!(engine.retire_account(account()), Ok(()));
        assert_eq!(engine.retire_account(AccountId::from_u64(987654)), Ok(()));
    }};
}

#[test]
fn retire_happy_path_and_clean_reuse_in_every_sync_mode() {
    retire_happy_mode!(no_sync, LocalSync, LocalSync, NoLocking);
    retire_happy_mode!(full_sync, FullSync, FullSync, FullLocking);
    retire_happy_mode!(
        account_sync,
        openpit::AccountSync,
        FullSync,
        openpit::storage::IndexLocking<openpit::AccountKeyConstraint>
    );
}

#[test]
fn retire_refusal_leaves_group_block_and_holdings_usable() {
    let engine = spot_engine();
    let group = AccountGroupId::from_u32(123).expect("valid group");
    engine
        .accounts()
        .register_group(&[account()], group)
        .expect("register group");
    engine.accounts().block(account(), "admin block".to_owned());
    engine
        .apply_account_adjustment(account(), &[balance("10")])
        .expect("credit");

    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("nonzero holdings refuse"),
        SpotFundsPolicy::<FullSync, FullSync>::NAME,
        AccountRetirementRefusal::NonZeroState,
    );
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("nonzero holdings still refuse"),
        SpotFundsPolicy::<FullSync, FullSync>::NAME,
        AccountRetirementRefusal::NonZeroState,
    );
    assert_eq!(
        engine.accounts().group_of(account()),
        Some(group),
        "refusal must keep membership"
    );
    let rejects = match engine.start_pre_trade(order(account())) {
        Ok(_) => panic!("refusal must keep admin block"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].reason, "admin block");
    // A delta of exactly -10 makes the slot retirable only if the refusals
    // left the original balance of 10 untouched.
    engine
        .apply_account_adjustment(account(), &[balance_delta("-10")])
        .expect("holdings remain usable");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_reservation_in_flight_then_rollback() {
    let engine = spot_engine();
    engine
        .apply_account_adjustment(account(), &[balance("10")])
        .expect("credit");
    let mut reservation = engine.execute_pre_trade(order(account())).expect("reserve");
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("reservation refuses"),
        SpotFundsPolicy::<FullSync, FullSync>::NAME,
        AccountRetirementRefusal::OperationInProgress,
    );
    reservation.rollback();
    engine
        .apply_account_adjustment(account(), &[balance("0")])
        .expect("zero balance");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_spot_account_pnl_value_and_halt_require_zero() {
    let engine = spot_engine();
    let name = SpotFundsPolicy::<FullSync, FullSync>::NAME;
    engine
        .configure()
        .set_spot_funds_account_pnl(name, account(), PnlState::Value(pnl("7")))
        .expect("set PnL");
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("nonzero PnL refuses"),
        name,
        AccountRetirementRefusal::NonZeroState,
    );
    engine
        .configure()
        .set_spot_funds_account_pnl(name, account(), PnlState::Halted(PnlHaltReason::MissingFx))
        .expect("halt PnL");
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("halted PnL refuses"),
        name,
        AccountRetirementRefusal::NonZeroState,
    );
    engine
        .configure()
        .set_spot_funds_account_pnl(name, account(), PnlState::Value(Pnl::ZERO))
        .expect("zero PnL");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_published_zero_position_pnl_slot() {
    let engine = spot_engine();
    let mut adjustment = empty_adjustment();
    adjustment.position_pnl = Some(PnlState::Value(Pnl::ZERO));
    engine
        .apply_account_adjustment(account(), &[adjustment])
        .expect("publish zero position PnL");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_published_nonzero_position_pnl_slot_refuses() {
    let engine = spot_engine();
    let mut adjustment = empty_adjustment();
    adjustment.position_pnl = Some(PnlState::Value(pnl("7")));
    engine
        .apply_account_adjustment(account(), &[adjustment])
        .expect("publish nonzero position PnL");
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("nonzero position PnL refuses"),
        SpotFundsPolicy::<FullSync, FullSync>::NAME,
        AccountRetirementRefusal::NonZeroState,
    );
}

#[test]
fn retire_kill_switch_realized_pnl_requires_zero() {
    let builder = Engine::builder::<OrderOperation, TestReport, TestAdjustment>().full_sync();
    let kill = PnlBoundsKillSwitchPolicy::<FullLocking>::new(
        pnl_broker_settings(),
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(kill).build().expect("engine builds");
    let name = PnlBoundsKillSwitchPolicy::<FullLocking>::NAME;
    engine
        .configure()
        .set_account_pnl(name, account(), asset("USD"), pnl("7"))
        .expect("set PnL");
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("nonzero realized PnL refuses"),
        name,
        AccountRetirementRefusal::NonZeroState,
    );
    engine
        .configure()
        .set_account_pnl(name, account(), asset("USD"), Pnl::ZERO)
        .expect("zero PnL");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

fn rate_engine(settings: RateLimitSettings) -> TestEngine {
    let builder = Engine::builder::<OrderOperation, TestReport, TestAdjustment>().full_sync();
    let rate = RateLimitPolicy::<FullLocking>::new(settings, builder.storage_builder());
    builder.pre_trade(rate).build().expect("engine builds")
}

#[test]
fn retire_rate_account_barrier_refuses_until_configuration_cleared() {
    let engine = rate_engine(
        RateLimitSettings::new(
            None,
            [],
            [RateLimitAccountBarrier {
                limit: rate_limit(),
                account_id: account(),
            }],
            [],
        )
        .expect("valid account barrier"),
    );
    let name = RateLimitPolicy::<FullLocking>::NAME;
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration still refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    engine
        .configure()
        .rate_limit(name, |settings| settings.set_account_barriers([]))
        .expect("clear account barrier");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_rate_account_asset_barrier_refuses_until_configuration_cleared() {
    let engine = rate_engine(
        RateLimitSettings::new(
            None,
            [],
            [],
            [RateLimitAccountAssetBarrier {
                limit: rate_limit(),
                account_id: account(),
                settlement_asset: asset("USD"),
            }],
        )
        .expect("valid account asset barrier"),
    );
    let name = RateLimitPolicy::<FullLocking>::NAME;
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration still refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    engine
        .configure()
        .rate_limit(name, |settings| settings.set_account_asset_barriers([]))
        .expect("clear account asset barrier");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

fn size_barrier() -> OrderSizeAccountAssetBarrier {
    OrderSizeAccountAssetBarrier {
        limit: OrderSizeLimit {
            max_quantity: Some(Quantity::from_str("10").expect("valid quantity")),
            max_notional: None,
        },
        account_id: account(),
        asset: asset("AAPL"),
    }
}

#[test]
fn retire_order_size_account_asset_limit_refuses_until_configuration_cleared() {
    let builder = Engine::builder::<OrderOperation, TestReport, TestAdjustment>().full_sync();
    let size = OrderSizeLimitPolicy::<FullLocking>::new(
        OrderSizeLimitSettings::new(None, [], [size_barrier()]).expect("valid limit"),
    );
    let engine = builder.pre_trade(size).build().expect("engine builds");
    let name = OrderSizeLimitPolicy::<FullLocking>::NAME;
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration still refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    engine
        .configure()
        .order_size_limit(name, |settings| settings.set_account_asset_barriers([]))
        .expect("clear account asset limit");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_kill_switch_account_barrier_refuses_until_configuration_cleared() {
    let builder = Engine::builder::<OrderOperation, TestReport, TestAdjustment>().full_sync();
    let broker = PnlBoundsBrokerBarrier {
        settlement_asset: asset("USD"),
        lower_bound: Some(pnl("-100")),
        upper_bound: None,
    };
    let settings = PnlBoundsKillSwitchSettings::new(
        [broker.clone()],
        [openpit::pretrade::policies::PnlBoundsAccountAssetBarrier {
            barrier: broker,
            account_id: account(),
            initial_pnl: Pnl::ZERO,
        }],
    )
    .expect("valid barriers");
    let kill = PnlBoundsKillSwitchPolicy::<FullLocking>::new(settings, builder.storage_builder());
    let engine = builder.pre_trade(kill).build().expect("engine builds");
    let name = PnlBoundsKillSwitchPolicy::<FullLocking>::NAME;
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration still refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    engine
        .configure()
        .pnl_bounds_killswitch(name, |settings| settings.set_account_barriers([]))
        .expect("clear account barrier");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_spot_account_setting_refuses_until_configuration_cleared() {
    let engine = spot_engine();
    let name = SpotFundsPolicy::<FullSync, FullSync>::NAME;
    engine
        .configure()
        .spot_funds(name, |settings| {
            settings.set_account_limit_mode(account(), Some(SpotFundsLimitMode::TrackOnly));
            Ok::<(), openpit::SpotFundsConfigError>(())
        })
        .expect("configure account override");
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("configuration still refuses"),
        name,
        AccountRetirementRefusal::ConfigurationReferencesAccount,
    );
    engine
        .configure()
        .spot_funds(name, |settings| {
            settings.set_account_limit_mode(account(), None);
            Ok::<(), openpit::SpotFundsConfigError>(())
        })
        .expect("clear account override");
    assert_eq!(engine.retire_account(account()), Ok(()));
}

#[test]
fn retire_collects_multiple_refusals_in_registration_order() {
    let builder = Engine::builder::<OrderOperation, TestReport, TestAdjustment>().full_sync();
    let rate = RateLimitPolicy::<FullLocking>::new(
        RateLimitSettings::new(
            None,
            [],
            [RateLimitAccountBarrier {
                limit: rate_limit(),
                account_id: account(),
            }],
            [],
        )
        .expect("valid rate barrier"),
        builder.storage_builder(),
    );
    let size = OrderSizeLimitPolicy::<FullLocking>::new(
        OrderSizeLimitSettings::new(None, [], [size_barrier()]).expect("valid limit"),
    );
    let engine = builder
        .pre_trade(rate)
        .pre_trade(size)
        .build()
        .expect("engine builds");
    for _ in 0..2 {
        assert_eq!(
            engine.retire_account(account()),
            Err(AccountRetirementError::Refused {
                refusals: vec![
                    AccountRetirementPolicyRefusal {
                        policy: RateLimitPolicy::<FullLocking>::NAME.to_owned(),
                        refusal: AccountRetirementRefusal::ConfigurationReferencesAccount,
                    },
                    AccountRetirementPolicyRefusal {
                        policy: OrderSizeLimitPolicy::<FullLocking>::NAME.to_owned(),
                        refusal: AccountRetirementRefusal::ConfigurationReferencesAccount,
                    },
                ],
            })
        );
    }
}

struct RetirementPolicy {
    name: &'static str,
    committed: Rc<Cell<bool>>,
    rolled_back: Rc<Cell<bool>>,
    refusal: Option<AccountRetirementRefusal>,
    commit_succeeds: bool,
}

impl<Order, ExecutionReport, AccountAdjustment, Sync: SyncMode>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync> for RetirementPolicy
{
    fn name(&self) -> &str {
        self.name
    }

    fn retire_account(
        &self,
        _account: AccountId,
        mutations: &mut Mutations,
    ) -> Result<(), AccountRetirementRefusal> {
        let committed = Rc::clone(&self.committed);
        let rolled_back = Rc::clone(&self.rolled_back);
        let commit_succeeds = self.commit_succeeds;
        mutations.push(Mutation::new_fallible(
            move || {
                committed.set(true);
                commit_succeeds
            },
            move || {
                rolled_back.set(true);
                true
            },
        ));
        self.refusal.map_or(Ok(()), Err)
    }
}

fn custom_policy(
    name: &'static str,
    refusal: Option<AccountRetirementRefusal>,
    commit_succeeds: bool,
) -> (RetirementPolicy, Rc<Cell<bool>>, Rc<Cell<bool>>) {
    let committed = Rc::new(Cell::new(false));
    let rolled_back = Rc::new(Cell::new(false));
    (
        RetirementPolicy {
            name,
            refusal,
            commit_succeeds,
            committed: Rc::clone(&committed),
            rolled_back: Rc::clone(&rolled_back),
        },
        committed,
        rolled_back,
    )
}

#[test]
fn retire_custom_policy_refusal_rolls_back_earlier_acceptance() {
    let (accepting, committed, rolled_back) = custom_policy("accepting", None, true);
    let (refusing, _, _) = custom_policy(
        "refusing",
        Some(AccountRetirementRefusal::NonZeroState),
        true,
    );
    let engine = Engine::builder::<OrderOperation, (), ()>()
        .no_sync()
        .pre_trade(accepting)
        .pre_trade(refusing)
        .build()
        .expect("engine builds");
    assert_refusal(
        engine
            .retire_account(account())
            .expect_err("custom refusal"),
        "refusing",
        AccountRetirementRefusal::NonZeroState,
    );
    assert!(
        !committed.get(),
        "accepting callback must not commit on refusal"
    );
    assert!(
        rolled_back.get(),
        "accepting callback must roll back on refusal"
    );
}

#[test]
fn retire_custom_policy_commit_callback_runs_on_success() {
    let (policy, committed, rolled_back) = custom_policy("custom", None, true);
    let engine = Engine::builder::<OrderOperation, (), ()>()
        .no_sync()
        .pre_trade(policy)
        .build()
        .expect("engine builds");
    assert_eq!(engine.retire_account(account()), Ok(()));
    assert!(committed.get(), "successful retirement must commit");
    assert!(
        !rolled_back.get(),
        "successful retirement must not roll back"
    );
}

#[test]
fn retire_custom_commit_failure_keeps_group_and_arms_global_block() {
    let (policy, _, _) = custom_policy("custom", None, false);
    let engine = Engine::builder::<OrderOperation, (), ()>()
        .no_sync()
        .pre_trade(policy)
        .build()
        .expect("engine builds");
    let group = AccountGroupId::from_u32(123).expect("valid group");
    engine
        .accounts()
        .register_group(&[account()], group)
        .expect("register group");
    assert_eq!(
        engine.retire_account(account()),
        Err(AccountRetirementError::FinalizerFailed)
    );
    assert_eq!(engine.accounts().group_of(account()), Some(group));
    assert!(
        engine
            .start_pre_trade(order(AccountId::from_u64(OTHER)))
            .is_err(),
        "custom finalizer failure must block unrelated account"
    );
}
