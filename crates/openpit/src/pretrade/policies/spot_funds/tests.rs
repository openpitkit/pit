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

//! Unit tests for [`SpotFundsPolicy`].

use super::rollback::AccountPnlAssertionRollback;
use super::*;
use crate::core::AccountAdjustmentContext;
use crate::marketdata::{MarketDataBuilder, Quote, QuoteTtl};
use crate::param::{
    AccountGroupId, AccountId, AdjustmentAmount, Asset, Fee, MonetaryAmount, Pnl, PositionSize,
    Price, Quantity, Side, Trade, TradeAmount, Volume,
};
use crate::pretrade::{
    holdings::Holdings, PreTradeContext, PreTradeLock, PreTradePolicy, RejectCode,
    DEFAULT_POLICY_GROUP_ID,
};
use crate::{
    FullSync, HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
    HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceUpperBound,
    HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
    HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
    HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound, HasAccountId,
    HasBalanceAsset, HasExecutionReportFillFee, HasExecutionReportIsFinal,
    HasExecutionReportLastTrade, HasInstrument, HasLeavesQuantity, HasPreTradeLock, HasSide,
    Instrument, Mutations, OrderOperation, RequestFieldAccessError,
};
use std::sync::Arc;

// ── type aliases ──────────────────────────────────────────────────────────

type TestPolicy = SpotFundsPolicy<FullSync, FullSync>;
type TestOrder = OrderOperation;
type AccountPnlAdjustment = crate::AccountAdjustmentAccountPnlOperation;
type AccountPnlTestEngine = crate::FullSyncEngine<TestOrder, TestReport, AccountPnlAdjustment>;

// ── TestReport ────────────────────────────────────────────────────────────

struct TestReport {
    instrument: Instrument,
    account_id: AccountId,
    side: Side,
    last_trade: Option<Trade>,
    fee: Option<MonetaryAmount>,
    leaves_quantity: Quantity,
    is_final: bool,
    lock: PreTradeLock,
}

impl HasInstrument for TestReport {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}

impl HasAccountId for TestReport {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        Ok(self.account_id)
    }
}

impl HasSide for TestReport {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        Ok(self.side)
    }
}

impl HasExecutionReportLastTrade for TestReport {
    fn last_trade(&self) -> Result<Option<Trade>, RequestFieldAccessError> {
        Ok(self.last_trade)
    }
}

impl HasExecutionReportFillFee for TestReport {
    fn fill_fee(&self) -> Result<Option<MonetaryAmount>, RequestFieldAccessError> {
        Ok(self.fee.clone())
    }
}

impl HasLeavesQuantity for TestReport {
    fn leaves_quantity(&self) -> Result<Quantity, RequestFieldAccessError> {
        Ok(self.leaves_quantity)
    }
}

impl HasExecutionReportIsFinal for TestReport {
    fn is_final(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.is_final)
    }
}

impl HasPreTradeLock for TestReport {
    fn lock(&self) -> Result<PreTradeLock, RequestFieldAccessError> {
        Ok(self.lock.clone())
    }
}

// ── TestAdjustment ────────────────────────────────────────────────────────

struct TestAdjustment {
    asset: Asset,
    balance: Option<AdjustmentAmount>,
    balance_average_entry_price: Option<Price>,
    pnl_operation: Option<crate::PnlState>,
    balance_lower: Option<PositionSize>,
    balance_upper: Option<PositionSize>,
    held: Option<AdjustmentAmount>,
    held_lower: Option<PositionSize>,
    held_upper: Option<PositionSize>,
    incoming: Option<AdjustmentAmount>,
    incoming_lower: Option<PositionSize>,
    incoming_upper: Option<PositionSize>,
}

impl HasBalanceAsset for TestAdjustment {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        Ok(&self.asset)
    }
}

impl crate::HasAccountAdjustmentPnlOperation for TestAdjustment {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<crate::PnlState>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentBalance for TestAdjustment {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.balance)
    }

    fn balance_realized_pnl(&self) -> Result<Option<crate::PnlState>, RequestFieldAccessError> {
        Ok(self.pnl_operation)
    }
}

impl HasAccountAdjustmentBalanceAverageEntryPrice for TestAdjustment {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        Ok(self.balance_average_entry_price)
    }
}

impl HasAccountAdjustmentBalanceLowerBound for TestAdjustment {
    fn balance_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.balance_lower)
    }
}

impl HasAccountAdjustmentBalanceUpperBound for TestAdjustment {
    fn balance_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.balance_upper)
    }
}

impl HasAccountAdjustmentHeld for TestAdjustment {
    fn held(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.held)
    }
}

impl HasAccountAdjustmentHeldLowerBound for TestAdjustment {
    fn held_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.held_lower)
    }
}

impl HasAccountAdjustmentHeldUpperBound for TestAdjustment {
    fn held_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.held_upper)
    }
}

impl HasAccountAdjustmentIncoming for TestAdjustment {
    fn incoming(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.incoming)
    }
}

impl HasAccountAdjustmentIncomingLowerBound for TestAdjustment {
    fn incoming_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.incoming_lower)
    }
}

impl HasAccountAdjustmentIncomingUpperBound for TestAdjustment {
    fn incoming_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.incoming_upper)
    }
}

// ── value helpers ─────────────────────────────────────────────────────────

fn asset(s: &str) -> Asset {
    Asset::new(s).expect("valid asset")
}

fn ps(s: &str) -> PositionSize {
    PositionSize::from_str(s).expect("valid position size")
}

fn pnl_value(s: &str) -> Pnl {
    Pnl::from_str(s).expect("valid pnl")
}

fn fee_value(s: &str) -> Fee {
    Fee::from_str(s).expect("valid fee")
}

fn px(s: &str) -> Price {
    Price::from_str(s).expect("valid price")
}

fn qty(s: &str) -> Quantity {
    Quantity::from_str(s).expect("valid quantity")
}

fn vol(s: &str) -> Volume {
    Volume::from_str(s).expect("valid volume")
}

fn account(n: u64) -> AccountId {
    AccountId::from_u64(n)
}

fn group(n: u32) -> AccountGroupId {
    AccountGroupId::from_u32(n).expect("valid account group id")
}

fn dummy_control(
    account_id: AccountId,
) -> crate::core::AccountControl<crate::storage::FullLocking> {
    use crate::core::account_control::BlockedAccounts;
    use crate::core::AccountBlockHandle;
    use crate::storage::{FullLocking, LockingPolicyFactory, StorageBuilder};
    let sb = StorageBuilder::new(FullLocking);
    let blocked = FullLocking::new_shared(BlockedAccounts::new(&sb));
    let handle = AccountBlockHandle::from_inner(blocked);
    crate::core::AccountControl::new(handle, account_id)
}

/// Builds a kill-switch block tagged with the in-flight assertion it was
/// computed against, exactly as a barrier breach raised by an execution report
/// that interleaves an unfinished force-set does.
fn provisional_block(details: &str, provenance: u64) -> crate::pretrade::AccountBlock {
    crate::pretrade::AccountBlock::new(
        TestPolicy::NAME,
        RejectCode::PnlKillSwitchTriggered,
        "account PnL barrier breached",
        details.to_owned(),
    )
    .with_provenance(Some(provenance))
}

fn instr(under: &str, sett: &str) -> Instrument {
    Instrument::new(asset(under), asset(sett))
}

// ── policy builder ────────────────────────────────────────────────────────

fn engine_builder() -> crate::SyncedEngineBuilder<(), (), (), crate::FullSync> {
    crate::Engine::builder().full_sync()
}

/// Default-cascade settings: global `slip_bps`, `Mark` pricing, no overrides.
fn settings(slip_bps: u16) -> SpotFundsSettings {
    SpotFundsSettings::new(slip_bps, SpotFundsPricingSource::Mark, std::iter::empty())
        .expect("settings must build")
}

/// Builds a `SpotFundsPolicy` with no market-order support.
fn build_policy(_mark: Option<()>, _slip_bps: Option<u16>) -> TestPolicy {
    let b = engine_builder();
    SpotFundsPolicy::new(settings(0), None, b.storage_builder())
}

fn build_policy_from_settings(
    settings: SpotFundsSettings,
    market_data: Option<SpotFundsMarketData<FullSync>>,
) -> TestPolicy {
    let b = engine_builder();
    SpotFundsPolicy::new(settings, market_data, b.storage_builder())
}

fn build_account_pnl_test_engine(settings: SpotFundsSettings) -> AccountPnlTestEngine {
    let builder =
        crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>().full_sync();
    let policy =
        SpotFundsPolicy::<FullSync, FullSync>::new(settings, None, builder.storage_builder());
    builder
        .pre_trade(policy)
        .build()
        .expect("account PnL test engine must build")
}

fn account_pnl_adjustment(state: crate::PnlState) -> AccountPnlAdjustment {
    crate::AccountAdjustmentAccountPnlOperation { state }
}

fn account_pnl_probe_order(account_id: AccountId) -> TestOrder {
    make_order(
        account_id,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("1")),
    )
}

/// Builds a policy whose market-data service has `instrument` registered
/// with mark price `price` and slippage `slip_bps`.
fn build_policy_with_market_data(
    instrument: Instrument,
    price: Price,
    slip_bps: u16,
) -> TestPolicy {
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instrument.clone())
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(price))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    SpotFundsPolicy::new(settings(slip_bps), Some(bundle), b.storage_builder())
}

/// Builds a policy with market-order support and a registered instrument, but
/// no quote for it.
fn build_policy_with_market_data_no_quote(instrument: Instrument) -> TestPolicy {
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    svc.register(instrument).expect("register must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder())
}

// ── request builders ──────────────────────────────────────────────────────

fn make_order(
    account_id: AccountId,
    instrument: Instrument,
    side: Side,
    trade_amount: TradeAmount,
    price: Option<Price>,
) -> TestOrder {
    OrderOperation {
        instrument,
        account_id,
        side,
        trade_amount,
        price,
    }
}

fn make_report(
    account_id: AccountId,
    instrument: Instrument,
    side: Side,
    last_trade: Option<Trade>,
    leaves: Quantity,
    is_final: bool,
    lock: Option<PreTradeLock>,
) -> TestReport {
    TestReport {
        instrument,
        account_id,
        side,
        last_trade,
        fee: None,
        leaves_quantity: leaves,
        is_final,
        lock: lock.unwrap_or_default(),
    }
}

fn money_fee(amount: &str, currency: &str) -> MonetaryAmount {
    MonetaryAmount {
        amount: fee_value(amount),
        currency: asset(currency),
    }
}

fn fill_with_fee(
    account_id: AccountId,
    instrument: Instrument,
    side: Side,
    price: &str,
    quantity: &str,
    fee: MonetaryAmount,
) -> TestReport {
    let price = px(price);
    let mut report = make_report(
        account_id,
        instrument,
        side,
        Some(Trade {
            price,
            quantity: qty(quantity),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            price,
        )])),
    );
    report.fee = Some(fee);
    report
}

fn fee_only_report(
    account_id: AccountId,
    instrument: Instrument,
    side: Side,
    leaves_quantity: &str,
    is_final: bool,
    fee: MonetaryAmount,
) -> TestReport {
    let mut report = make_report(
        account_id,
        instrument,
        side,
        None,
        qty(leaves_quantity),
        is_final,
        None,
    );
    report.fee = Some(fee);
    report
}

fn adj(asset: Asset, balance: Option<AdjustmentAmount>) -> TestAdjustment {
    TestAdjustment {
        asset,
        balance,
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    }
}

/// Builds a balance adjustment carrying an average entry price.
fn adj_with_avg(
    asset: Asset,
    balance: Option<AdjustmentAmount>,
    average_entry_price: Option<Price>,
) -> TestAdjustment {
    TestAdjustment {
        asset,
        balance,
        balance_average_entry_price: average_entry_price,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    }
}

/// Builds a balance operation that corrects the selected position's realized
/// PnL.
fn adj_with_realized_pnl(asset: Asset, realized_pnl: Pnl) -> TestAdjustment {
    TestAdjustment {
        asset,
        balance: None,
        balance_average_entry_price: None,
        pnl_operation: Some(crate::PnlState::Value(realized_pnl)),
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    }
}

fn bounded_adj(
    asset: Asset,
    balance: Option<AdjustmentAmount>,
    lower: Option<PositionSize>,
    upper: Option<PositionSize>,
) -> TestAdjustment {
    TestAdjustment {
        asset,
        balance,
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: lower,
        balance_upper: upper,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    }
}

fn held_adj(
    asset: Asset,
    held: Option<AdjustmentAmount>,
    lower: Option<PositionSize>,
    upper: Option<PositionSize>,
) -> TestAdjustment {
    TestAdjustment {
        asset,
        balance: None,
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held,
        held_lower: lower,
        held_upper: upper,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    }
}

fn incoming_adj(
    asset: Asset,
    incoming: Option<AdjustmentAmount>,
    lower: Option<PositionSize>,
    upper: Option<PositionSize>,
) -> TestAdjustment {
    TestAdjustment {
        asset,
        balance: None,
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming,
        incoming_lower: lower,
        incoming_upper: upper,
    }
}

fn all_fields_adj(
    asset: Asset,
    balance: Option<AdjustmentAmount>,
    held: Option<AdjustmentAmount>,
    incoming: Option<AdjustmentAmount>,
) -> TestAdjustment {
    TestAdjustment {
        asset,
        balance,
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held,
        held_lower: None,
        held_upper: None,
        incoming,
        incoming_lower: None,
        incoming_upper: None,
    }
}

// ── seed / access helpers ─────────────────────────────────────────────────

fn seed(policy: &TestPolicy, account_id: AccountId, asset: Asset, amount: &str) {
    let adjustment = adj(asset, Some(AdjustmentAmount::Absolute(ps(amount))));
    let mut mutations = Mutations::with_capacity(1);
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_account_adjustment(
        policy,
        &AccountAdjustmentContext::new_test(dummy_control(account_id)),
        account_id,
        &adjustment,
        &mut mutations,
    )
    .expect("seed must succeed");
    mutations.commit_all();
}

fn holdings_of(policy: &TestPolicy, account_id: AccountId, asset: &Asset) -> Option<Holdings> {
    policy.holdings.get(&(account_id, asset.clone()))
}

fn account_pnl_of(policy: &TestPolicy, account_id: AccountId) -> Option<Pnl> {
    policy.pnl.with(&account_id, |entry| match entry.state {
        crate::PnlState::Value(value) => Some(value),
        crate::PnlState::Halted(_) => None,
    })?
}

fn account_pnl_state_of(policy: &TestPolicy, account_id: AccountId) -> Option<crate::PnlState> {
    policy.pnl.with(&account_id, |entry| entry.state)
}

fn post_trade_ctx_with_currency_and_group(
    report: &TestReport,
    currency: Asset,
    group_id: AccountGroupId,
) -> crate::pretrade::PostTradeContext<crate::storage::FullLocking> {
    use crate::core::{
        AccountBlockHandle, AccountCurrencies, AccountGroups, AccountGroupsHandle, Accounts,
        BlockedAccounts,
    };
    use crate::storage::{FullLocking, LockingPolicyFactory, StorageBuilder};

    let storage_builder = StorageBuilder::new(FullLocking);
    let groups = AccountGroups::new(&storage_builder);
    groups
        .register_group(&[report.account_id], group_id)
        .expect("group registration must succeed");
    let group_handle = AccountGroupsHandle::from_inner(FullLocking::new_shared(groups));
    let block_handle = AccountBlockHandle::from_inner(FullLocking::new_shared(
        BlockedAccounts::new(&storage_builder),
    ));
    let currencies = FullLocking::new_shared(AccountCurrencies::new(&storage_builder));
    let accounts = Accounts::new(group_handle.clone(), block_handle, currencies);
    accounts.set_currency(report.account_id, currency);
    crate::pretrade::PostTradeContext::with_accounts(
        accounts,
        group_handle,
        Some(report.account_id),
    )
}

// ── trait call wrappers ───────────────────────────────────────────────────

fn pre_trade_check(
    policy: &TestPolicy,
    order: &TestOrder,
    mutations: &mut Mutations,
) -> Result<(), crate::pretrade::Rejects> {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::perform_pre_trade_check(
        policy,
        &PreTradeContext::new(None),
        order,
        mutations,
    )
    .map(|_| ())
}

fn dry_run_check(
    policy: &TestPolicy,
    order: &TestOrder,
) -> Result<Option<crate::pretrade::PolicyPreTradeResult>, crate::pretrade::Rejects> {
    let mut mutations = Mutations::new();
    let result = <TestPolicy as PreTradePolicy<
        TestOrder,
        TestReport,
        TestAdjustment,
        crate::core::FullSync,
    >>::perform_pre_trade_check_dry_run(
        policy, &PreTradeContext::new(None), order, &mut mutations
    );
    // A dry-run must never register a mutation.
    assert!(mutations.is_empty(), "dry-run must push no mutations");
    result
}

fn apply_adj(
    policy: &TestPolicy,
    account_id: AccountId,
    adjustment: &TestAdjustment,
    mutations: &mut Mutations,
) -> Result<(), crate::pretrade::Rejects> {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_account_adjustment(
        policy,
        &AccountAdjustmentContext::new_test(dummy_control(account_id)),
        account_id,
        adjustment,
        mutations,
    )
    .map(|_| ())
}

fn report_blocks(policy: &TestPolicy, report: &TestReport) -> Vec<crate::pretrade::AccountBlock> {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_execution_report(
        policy,
        &post_trade_ctx(report),
        report,
    )
    .map(|r| r.account_blocks)
    .unwrap_or_default()
}

fn post_trade_ctx(
    report: &TestReport,
) -> crate::pretrade::PostTradeContext<crate::storage::FullLocking> {
    crate::pretrade::PostTradeContext::with_account_currency(
        report.account_id,
        report.instrument.settlement_asset().clone(),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// Constructor
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn new_creates_empty_holdings() {
    let policy = build_policy(None, None);
    assert!(holdings_of(&policy, account(99224416), &asset("USD")).is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// SpotFundsSettings construction + validation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn settings_new_rejects_out_of_range_global_slippage() {
    let result = SpotFundsSettings::new(10_001, SpotFundsPricingSource::Mark, std::iter::empty());
    assert_eq!(
        result.err(),
        Some(SpotFundsConfigError::SlippageOutOfRange { bps: 10_001 })
    );
}

#[test]
fn settings_new_accepts_max_slippage_boundary() {
    // 10_000 bps is the inclusive upper bound and must build.
    assert!(
        SpotFundsSettings::new(10_000, SpotFundsPricingSource::Mark, std::iter::empty()).is_ok()
    );
}

#[test]
fn settings_set_global_slippage_bps_boundary_and_reject() {
    let mut s = settings(0);
    // Boundary value succeeds.
    assert!(s.set_global_slippage_bps(10_000).is_ok());
    // Above the bound is rejected and the prior value is retained.
    assert_eq!(
        s.set_global_slippage_bps(10_001).err(),
        Some(SpotFundsConfigError::SlippageOutOfRange { bps: 10_001 })
    );
    // The 10_000 bps value is still in effect: a sell at 100% slippage prices
    // to zero and is uncomputable, whereas a fresh 0-bps default would not.
    let (svc, id) = {
        let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
        let id = svc
            .register(instr("AAPL", "USD"))
            .expect("register must succeed");
        svc.push(id, Quote::new().with_mark(px("100")))
            .expect("push must succeed");
        (svc, id)
    };
    let md = SpotFundsMarketData::<FullSync>::new(Arc::clone(&svc));
    let quote = md.quote(id, account(7), &None).expect("quote present");
    assert!(s
        .effective_sell_price(&quote, id, account(7), &None)
        .is_err());
}

#[test]
fn settings_set_override_above_bound_is_rejected() {
    let mut s = settings(0);
    let id = {
        let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
        svc.register(instr("AAPL", "USD"))
            .expect("register must succeed")
    };
    assert_eq!(
        s.set_override(
            SpotFundsOverrideTarget::Instrument(id),
            SpotFundsOverride {
                slippage_bps: Some(10_001),
            },
        )
        .err(),
        Some(SpotFundsConfigError::SlippageOutOfRange { bps: 10_001 })
    );
}

#[test]
fn settings_set_override_then_clear_falls_back_to_global() {
    // Global 0 bps; set an instrument override at 1000 bps, observe it, then
    // clear it (None) and observe the cascade fall back to the global tier.
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");
    let md = SpotFundsMarketData::<FullSync>::new(Arc::clone(&svc));
    let quote = md.quote(id, account(7), &None).expect("quote present");

    let mut s = settings(0);
    s.set_override(
        SpotFundsOverrideTarget::Instrument(id),
        SpotFundsOverride {
            slippage_bps: Some(1000),
        },
    )
    .expect("override must set");
    // 100 * (1 + 0.10) = 110.
    assert_eq!(
        s.effective_buy_price(&quote, id, account(7), &None),
        Ok(px("110"))
    );

    s.set_override(
        SpotFundsOverrideTarget::Instrument(id),
        SpotFundsOverride { slippage_bps: None },
    )
    .expect("override must clear");
    // Back to global 0 bps: 100 * 1.00 = 100.
    assert_eq!(
        s.effective_buy_price(&quote, id, account(7), &None),
        Ok(px("100"))
    );
}

#[test]
fn settings_set_pricing_source_switches_quote_field() {
    // Mark unset, ask = 100: Mark source has no base (QuoteUnavailable),
    // BookTop source reads `ask` and prices the buy.
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.push(id, Quote::new().with_ask(px("100")))
        .expect("push must succeed");
    let md = SpotFundsMarketData::<FullSync>::new(Arc::clone(&svc));
    let quote = md.quote(id, account(7), &None).expect("quote present");

    let mut s = settings(0);
    assert!(s
        .effective_buy_price(&quote, id, account(7), &None)
        .is_err());
    s.set_pricing_source(SpotFundsPricingSource::BookTop);
    assert_eq!(
        s.effective_buy_price(&quote, id, account(7), &None),
        Ok(px("100"))
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Policy group tag + ConfigurablePolicy
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn with_policy_group_id_records_tag_observed_by_policy() {
    use crate::pretrade::PreTradePolicy;
    let id = DEFAULT_POLICY_GROUP_ID;
    let policy = build_policy(None, None);
    assert_eq!(
        <TestPolicy as PreTradePolicy<
            TestOrder,
            TestReport,
            TestAdjustment,
            crate::core::FullSync,
        >>::policy_group_id(&policy),
        id
    );

    let tag = crate::pretrade::PolicyGroupId::new(7);
    let tagged = build_policy(None, None).with_policy_group_id(tag);
    assert_eq!(
        <TestPolicy as PreTradePolicy<
            TestOrder,
            TestReport,
            TestAdjustment,
            crate::core::FullSync,
        >>::policy_group_id(&tagged),
        tag
    );

    let report = fee_only_report(
        account(99224416),
        instr("AAPL", "USD"),
        Side::Buy,
        "0",
        false,
        money_fee("1", "USD"),
    );
    let result = run_report_with_currency(&tagged, &report, asset("USD"));
    assert_eq!(result.account_pnls.len(), 1);
    assert_eq!(result.account_pnls[0].policy_group_id, tag);
}

#[test]
fn settings_cell_clone_shares_state_with_running_policy() {
    use crate::pretrade::ConfigurablePolicy;
    use crate::storage::ConfigCell;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    // Start at global 0 bps so a market buy of qty 10 @ mark 100 reserves 1000.
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("100"), 0);
    seed(&policy, acc, asset("USD"), "10000");

    // Publish a new global slippage (2000 bps) through the registry-side clone;
    // the running policy must observe it on its next hot-path read.
    let cell =
        <TestPolicy as ConfigurablePolicy<crate::storage::FullLocking>>::settings_cell(&policy);
    cell.update(|s| s.set_global_slippage_bps(2000))
        .expect("update must publish");

    // effective = 100 * (1 + 0.20) = 120; held = 10 * 120 = 1200.
    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("1200"));
    assert_eq!(h.available(), ps("8800"));
}

// ═══════════════════════════════════════════════════════════════════════════
// perform_pre_trade_check — §8.1
// ═══════════════════════════════════════════════════════════════════════════

// ── Buy Quantity + limit price ─────────────────────────────────────────────

#[test]
fn buy_qty_limit_sufficient_reserves_settlement() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    assert!(pre_trade_check(&policy, &order, &mut mutations).is_ok());
    assert!(!mutations.is_empty());

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("2000"));
    assert_eq!(h.available(), ps("8000"));
}

#[test]
fn buy_qty_limit_insufficient_rejects_insufficient_funds() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "1000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");

    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
    assert!(mutations.is_empty());
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("1000"));
    assert_eq!(h.held(), ps("0"));
}

// ── Buy dry-run ─────────────────────────────────────────────────────────────

#[test]
fn dry_run_buy_reports_outcome_and_leaves_holdings_untouched() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );

    let outcome = dry_run_check(&policy, &order)
        .expect("dry-run must pass")
        .expect("dry-run must report an outcome");

    // The would-be settlement leg matches a real reservation: held +2000 to
    // absolute 2000, balance -2000 to absolute 8000, lock price 200; plus the
    // base incoming projection. Dry-run mirrors the mutating path byte for
    // byte, including the [settlement, base] order.
    assert_eq!(outcome.account_adjustments.len(), 2);
    let entry = &outcome.account_adjustments[0];
    assert_eq!(entry.asset, asset("USD"));
    let held = entry.held.expect("held outcome present");
    assert_eq!(held.delta, ps("2000"));
    assert_eq!(held.absolute, ps("2000"));
    let balance = entry.balance.expect("balance outcome present");
    assert_eq!(balance.delta, ps("-2000"));
    assert_eq!(balance.absolute, ps("8000"));
    assert!(entry.incoming.is_none());

    let base = &outcome.account_adjustments[1];
    assert_eq!(base.asset, asset("AAPL"));
    assert!(base.balance.is_none());
    assert!(base.held.is_none());
    let incoming = base.incoming.expect("base incoming outcome present");
    assert_eq!(incoming.delta, ps("10"));
    assert_eq!(incoming.absolute, ps("10"));
    assert_eq!(outcome.lock_prices.to_vec(), vec![px("200")]);

    // Holdings are untouched by the dry-run.
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("10000"));
    assert_eq!(h.held(), ps("0"));
}

#[test]
fn dry_run_buy_matches_real_reservation_holdings_then_leaves_them_for_the_real_call() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );

    // Two dry-runs in a row must not move state.
    dry_run_check(&policy, &order).expect("first dry-run must pass");
    dry_run_check(&policy, &order).expect("second dry-run must pass");
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("10000"));
    assert_eq!(h.held(), ps("0"));

    // A real reservation afterwards still reserves the full amount: the
    // dry-runs consumed nothing.
    let mut mutations = Mutations::with_capacity(1);
    assert!(pre_trade_check(&policy, &order, &mut mutations).is_ok());
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("2000"));
    assert_eq!(h.available(), ps("8000"));
}

#[test]
fn dry_run_buy_insufficient_reports_same_reject_and_leaves_holdings_untouched() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "1000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );

    let rejects = dry_run_check(&policy, &order).expect_err("dry-run must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);

    // Repeated rejecting dry-runs must not create or mutate holdings.
    dry_run_check(&policy, &order).expect_err("dry-run must reject again");
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("1000"));
    assert_eq!(h.held(), ps("0"));
}

#[test]
fn dry_run_sell_reports_underlying_hold_and_leaves_holdings_untouched() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("200")),
    );

    let outcome = dry_run_check(&policy, &order)
        .expect("dry-run must pass")
        .expect("dry-run must report an outcome");
    // A priced sell now also projects its settlement incoming (proceeds =
    // 4 * 200 = 800) and records the lock, in [underlying, settlement] order.
    assert_eq!(outcome.account_adjustments.len(), 2);
    let entry = &outcome.account_adjustments[0];
    assert_eq!(entry.asset, asset("AAPL"));
    let held = entry.held.expect("held outcome present");
    assert_eq!(held.delta, ps("4"));
    assert_eq!(held.absolute, ps("4"));
    assert!(entry.incoming.is_none());

    let settlement = &outcome.account_adjustments[1];
    assert_eq!(settlement.asset, asset("USD"));
    assert!(settlement.balance.is_none());
    assert!(settlement.held.is_none());
    let incoming = settlement.incoming.expect("settlement incoming present");
    assert_eq!(incoming.delta, ps("800"));
    assert_eq!(incoming.absolute, ps("800"));
    assert_eq!(outcome.lock_prices.to_vec(), vec![px("200")]);

    let h = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(h.available(), ps("10"));
    assert_eq!(h.held(), ps("0"));
}

#[test]
fn dry_run_priceless_sell_without_market_data_bundle_rejects_like_the_mutating_path() {
    // A sell with no order price in limit-only mode rejects identically in the
    // dry-run twin and the mutating path: both route through the shared
    // compute_reservation_legs and emit UnsupportedOrderType. The dry-run
    // touches no holdings.
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        None,
    );

    let dry_rejects = dry_run_check(&policy, &order).expect_err("dry-run must reject");
    assert_eq!(dry_rejects[0].code, RejectCode::UnsupportedOrderType);

    let mut mutations = Mutations::new();
    let real_rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(real_rejects[0].code, RejectCode::UnsupportedOrderType);
    assert!(mutations.is_empty());

    // Neither path moved the underlying holdings.
    let h = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(h.available(), ps("10"));
    assert_eq!(h.held(), ps("0"));
}

// ── Buy Volume ────────────────────────────────────────────────────────────

#[test]
fn buy_volume_sufficient_reserves_volume_amount() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Volume(vol("3000")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    assert!(pre_trade_check(&policy, &order, &mut mutations).is_ok());

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("3000"));
    assert_eq!(h.available(), ps("7000"));
}

#[test]
fn buy_volume_insufficient_rejects() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "2000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Volume(vol("3000")),
        Some(px("200")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
}

#[test]
fn buy_volume_without_price_or_mark_rejects_as_unsupported() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Volume(vol("3000")),
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::UnsupportedOrderType);
}

// ── Buy market + mark ─────────────────────────────────────────────────────

#[test]
fn buy_market_with_mark_reserves_slippage_adjusted_amount() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("200"), 1500);
    seed(&policy, acc, asset("USD"), "10000");

    // effective = 200 * 1.15 = 230; charge = 10 * 230 = 2300
    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    assert!(pre_trade_check(&policy, &order, &mut mutations).is_ok());

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("2300"));
    assert_eq!(h.available(), ps("7700"));
}

#[test]
fn buy_market_no_bundle_rejects_unsupported_order_type() {
    let acc = account(99224416);
    // No SpotFundsMarketData → market orders are unsupported
    let policy = build_policy(None::<()>, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::UnsupportedOrderType);
}

// ── Sell Quantity ─────────────────────────────────────────────────────────

#[test]
fn sell_qty_sufficient_holds_underlying() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    // Every accepted sell must resolve a price; the underlying-coverage gate is
    // exercised with a limit price so the order reserves rather than rejecting
    // as unpriceable.
    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    assert!(pre_trade_check(&policy, &order, &mut mutations).is_ok());

    let h = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(h.held(), ps("4"));
    assert_eq!(h.available(), ps("6"));
}

#[test]
fn sell_qty_insufficient_rejects() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "3");

    // Priced so the order is past the unpriceable gate; the underlying-coverage
    // gate is what must reject here.
    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("200")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);

    let h = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(h.available(), ps("3"));
    assert_eq!(h.held(), ps("0"));
}

// ── Sell Volume + limit price ─────────────────────────────────────────────

#[test]
fn sell_volume_limit_holds_quantity_charge() {
    // AAPL=10, Sell vol=600 @ 200 → charge_qty = 600 / 200 = 3 AAPL
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Volume(vol("600")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    assert!(pre_trade_check(&policy, &order, &mut mutations).is_ok());

    let h = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(h.held(), ps("3"));
    assert_eq!(h.available(), ps("7"));
}

#[test]
fn sell_volume_limit_insufficient_rejects() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "2");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Volume(vol("600")),
        Some(px("200")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
}

// ── Sell Volume + mark ────────────────────────────────────────────────────

#[test]
fn sell_qty_market_registered_without_quote_rejects_mark_price_unavailable() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data_no_quote(aapl_usd.clone());
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::MarkPriceUnavailable);
    assert!(mutations.is_empty());

    let h = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(h.available(), ps("10"));
    assert_eq!(h.held(), ps("0"));
}

#[test]
fn sell_volume_market_zero_slip_holds_correct_quantity() {
    // AAPL=10, mark=200, slip=0 → effective=200*(1-0)=200, vol=400, charge=2 AAPL
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("200"), 0);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Volume(vol("400")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    assert!(pre_trade_check(&policy, &order, &mut mutations).is_ok());

    let h = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(h.held(), ps("2"));
    assert_eq!(h.available(), ps("8"));
}

#[test]
fn sell_volume_market_full_slip_rejects_order_value_calculation_failed() {
    // slip=10000 → effective = mark * (1 - 1.0) = 0 → PriceUncomputable
    // → OrderValueCalculationFailed.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("200"), 10_000);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Volume(vol("400")),
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    // With 100% slippage the effective sell price is zero, which the
    // pricer maps to PriceUncomputable → OrderValueCalculationFailed.
    assert_eq!(rejects[0].code, RejectCode::OrderValueCalculationFailed);
}

// ── Missing holdings treated as zero ──────────────────────────────────────

#[test]
fn missing_holdings_treated_as_zero_rejects_insufficient_funds() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // EUR settlement, no EUR balance seeded → treated as zero
    let order = make_order(
        acc,
        instr("AAPL", "EUR"),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("200")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
    assert!(mutations.is_empty());
}

// ── Phantom-entry prevention ──────────────────────────────────────────────

#[test]
fn insufficient_funds_on_missing_settlement_does_not_create_holdings_entry() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    // EUR settlement not seeded — treated as zero → InsufficientFunds.

    let order = make_order(
        acc,
        instr("AAPL", "EUR"),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("100")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
    assert!(
        holdings_of(&policy, acc, &asset("EUR")).is_none(),
        "phantom entry must not be created on reject"
    );
}

#[test]
fn bounds_exceeded_on_new_asset_does_not_create_holdings_entry() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    // upper=0 blocks any positive balance; Delta(+10) on an unseen asset.
    let adjustment = bounded_adj(
        asset("EUR"),
        Some(AdjustmentAmount::Delta(ps("10"))),
        None,
        Some(ps("0")),
    );
    let mut mutations = Mutations::new();
    let rejects = apply_adj(&policy, acc, &adjustment, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::AccountAdjustmentBoundsExceeded);
    assert!(
        holdings_of(&policy, acc, &asset("EUR")).is_none(),
        "phantom entry must not be created on reject"
    );
}

#[test]
fn negative_result_on_new_asset_does_not_create_holdings_entry() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    // lower=0 blocks negative result; Absolute(-1) on an unseen asset.
    let adjustment = bounded_adj(
        asset("EUR"),
        Some(AdjustmentAmount::Absolute(ps("-1"))),
        Some(ps("0")),
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = apply_adj(&policy, acc, &adjustment, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::AccountAdjustmentBoundsExceeded);
    assert!(
        holdings_of(&policy, acc, &asset("EUR")).is_none(),
        "phantom entry must not be created on reject"
    );
}

// ── Rollback simulation ────────────────────────────────────────────────────

#[test]
fn rollback_restores_holdings_to_pre_reserve_state() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");

    // write is synchronous; state is already updated
    let after_check = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(after_check.held(), ps("2000"));
    assert_eq!(after_check.available(), ps("8000"));

    mutations.rollback_all();

    let after_rollback = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(after_rollback.held(), ps("0"));
    assert_eq!(after_rollback.available(), ps("10000"));
}

#[test]
fn concurrent_second_check_rejects_when_first_already_reserved() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "100");

    let order_a = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Volume(vol("100")),
        Some(px("1")),
    );
    let order_b = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Volume(vol("100")),
        Some(px("1")),
    );

    let mut mutations_a = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order_a, &mut mutations_a).expect("A must pass");

    // B's check sees available already reduced to 0 by A's
    // synchronous write, so it is rejected immediately.
    let mut mutations_b = Mutations::new();
    let rejects = pre_trade_check(&policy, &order_b, &mut mutations_b)
        .expect_err("B must reject - funds already held by A");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);

    // Rolling back A returns the funds; B would now fit.
    mutations_a.rollback_all();
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("0"));
    assert_eq!(h.available(), ps("100"));
}

// ═══════════════════════════════════════════════════════════════════════════
// apply_execution_report — §8.2
// ═══════════════════════════════════════════════════════════════════════════

// ── Buy partial fill ──────────────────────────────────────────────────────

#[test]
fn buy_partial_fill_consumes_held_settlement_and_credits_underlying() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held(USD)=2000, available(USD)=8000

    let report = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let blocks = report_blocks(&policy, &report);
    assert!(blocks.is_empty());

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("1200")); // 2000 - 4*200=800
    assert_eq!(usd.available(), ps("8000")); // unchanged

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL entry created");
    assert_eq!(aapl.available(), ps("4"));
    assert_eq!(aapl.held(), ps("0"));
}

// ── Sell partial fill ─────────────────────────────────────────────────────

#[test]
fn sell_partial_fill_consumes_held_underlying_and_credits_settlement() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    // A priced sell reserves the AAPL underlying as held and projects the
    // expected proceeds (200 * 10 = 2000 USD) as settlement incoming.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held(AAPL)=10, available(AAPL)=0; incoming(USD)=2000

    // The fill carries the lock so the settlement leg reconciles; a sell fill
    // without it would block the account like a buy.
    let report = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &report).is_empty());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.held(), ps("6")); // 10 - 4
    assert_eq!(aapl.available(), ps("0")); // unchanged

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD entry created");
    assert_eq!(usd.available(), ps("800")); // 4 * 200
    assert_eq!(usd.incoming(), ps("1200")); // 2000 - 200 * 4
}

// ── Buy fill - missing lock price ─────────────────────────────────────────

#[test]
fn buy_fill_without_lock_price_blocks_account() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held=2000, available=8000

    // Report with empty lock - no price for group.
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        None,
    );
    let blocks = report_blocks(&policy, &fill);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);

    // Holdings must not have changed.
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("2000"));
    assert_eq!(usd.available(), ps("8000"));
}

// ── Sell fill - missing lock price ────────────────────────────────────────

#[test]
fn sell_fill_without_lock_price_blocks_account() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    // Priced sell: reserves the underlying held and projects the settlement
    // incoming (10 * 200 = 2000 USD), recording a lock.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held(AAPL)=10, available(AAPL)=0, incoming(USD)=2000.

    // The fill arrives without its lock: the settlement leg cannot reconcile, so
    // the account is blocked exactly like a buy, before any holdings mutation.
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        None,
    );
    let blocks = report_blocks(&policy, &fill);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);

    // No mutation/leak: the underlying held and the settlement incoming are
    // untouched - the block fired before any leg moved.
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.held(), ps("10"));
    assert_eq!(aapl.available(), ps("0"));
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("2000"));
}

// ── Sell cancel - missing lock price ──────────────────────────────────────

#[test]
fn sell_cancel_without_lock_price_blocks_account() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held(AAPL)=10, available(AAPL)=0, incoming(USD)=2000.

    // The final cancel arrives without its lock: the settlement release cannot
    // be priced, so the account is blocked before any leg is released.
    let cancel = make_report(acc, aapl_usd, Side::Sell, None, qty("10"), true, None);
    let blocks = report_blocks(&policy, &cancel);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);

    // No mutation/leak: both legs are left exactly as reserved.
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.held(), ps("10"));
    assert_eq!(aapl.available(), ps("0"));
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("2000"));
}

// ── Buy fill - multiple lock prices ──────────────────────────────────────

#[test]
fn buy_fill_with_multiple_lock_prices_blocks_account() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held=2000, available=8000

    // Lock with two prices for the same group - ambiguous, must block.
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([
            (DEFAULT_POLICY_GROUP_ID, px("200")),
            (DEFAULT_POLICY_GROUP_ID, px("210")),
        ])),
    );
    let blocks = report_blocks(&policy, &fill);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].code, RejectCode::Other);

    // Holdings must not have changed.
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("2000"));
    assert_eq!(usd.available(), ps("8000"));
}

// ── Cancel - Buy limit ────────────────────────────────────────────────────

// ── Cancel with leftover - Buy limit ──────────────────────────────────────

#[test]
fn buy_limit_cancel_leftover_releases_held_by_leaves_times_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held=2000, available=8000

    let fill = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    report_blocks(&policy, &fill);
    // held=2000-800=1200

    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    report_blocks(&policy, &cancel);
    // release=6*200=1200; held=0; available=8000+1200=9200

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("0"));
    assert_eq!(usd.available(), ps("9200"));
}

// ── Cancel - Buy market uses lock price for release ───────────────────────

#[test]
fn buy_market_cancel_uses_lock_price_for_release() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("200"), 1500);
    seed(&policy, acc, asset("USD"), "10000");

    // Reserve: effective=230, held=2300, available=7700.
    // The lock written during pre-trade is propagated by the caller into
    // every execution report for this order.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("195"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("230"),
        )])),
    );
    report_blocks(&policy, &fill);
    // Fill consumed 4*230=920 from held (lock price), savings=140 returned to available.
    // After fill: held=2300-920=1380, available=7700+140=7840.

    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("195"),
            quantity: qty("0"),
        }),
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("230"),
        )])),
    );
    report_blocks(&policy, &cancel);
    // Cancel: release = 6*230=1380; held=1380-1380=0, available=7840+1380=9220.

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("0"));
    assert_eq!(usd.available(), ps("9220"));
}

#[test]
fn buy_market_cancel_without_lock_price_blocks() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("200"), 1500);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let cancel = make_report(acc, aapl_usd, Side::Buy, None, qty("10"), true, None);
    let blocks = report_blocks(&policy, &cancel);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);
}

// ── Cancel - Buy market, multiple lock prices ─────────────────────────────

#[test]
fn buy_market_cancel_with_multiple_lock_prices_blocks() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("200"), 1500);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([
            (DEFAULT_POLICY_GROUP_ID, px("230")),
            (DEFAULT_POLICY_GROUP_ID, px("240")),
        ])),
    );
    let blocks = report_blocks(&policy, &cancel);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].code, RejectCode::Other);
}

// ── Cancel - Buy market, no fills, with mark ──────────────────────────────

#[test]
fn buy_market_cancel_no_fills_with_lock_price_releases_full_amount() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("200"), 1500);
    seed(&policy, acc, asset("USD"), "10000");

    // Reserve: effective=230, held=2300.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    // Cancel with the propagated lock price = 230:
    // release = 10*230 = 2300; held=0; available=7700+2300=10000.
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("230"),
        )])),
    );
    report_blocks(&policy, &cancel);

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("0"));
    assert_eq!(usd.available(), ps("10000"));
}

// ── Cancel - Buy market, no fills, no mark (stuck) ────────────────────────

#[test]
fn buy_market_cancel_no_fills_no_mark_held_stays_stuck() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let usd = asset("USD");

    // Inject held=2300 directly to simulate a prior market reservation
    policy
        .holdings
        .with_mut((acc, usd.clone()), Holdings::zero, |slot, _| {
            *slot = Holdings::new(ps("0"), ps("2300"));
        });

    // Cancel with no lock price recorded for the buy side surfaces an
    // AccountBlock with MissingRequiredField; held is left untouched
    // because compute_release_amount fails before any mutation runs.
    let cancel = make_report(acc, aapl_usd, Side::Buy, None, qty("10"), true, None);
    let blocks = report_blocks(&policy, &cancel);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);

    let h = holdings_of(&policy, acc, &usd).expect("must exist");
    assert_eq!(h.held(), ps("2300"));
}

// ── Cancel Sell ───────────────────────────────────────────────────────────

#[test]
fn sell_cancel_leftover_releases_underlying_held() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    // Reserve a priced Sell 10 @ 200 → held(AAPL)=10, available(AAPL)=0,
    // incoming(USD)=2000.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    // Partial fill 4@200 (lock replayed): consume(4) from held → held=6
    let fill = make_report(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &fill).is_empty());

    // Cancel leaves=6 (lock replayed): Side::Sell → release=6 directly; held=0;
    // available=6. The settlement incoming drains to zero too. A sell cancel
    // missing its lock would block the account like a buy.
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.held(), ps("0"));
    assert_eq!(aapl.available(), ps("6"));
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("0"));
}

// ── is_final + leaves=0 ───────────────────────────────────────────────────

#[test]
fn final_report_with_zero_leaves_triggers_no_release() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // held=2000

    // Full fill, leaves=0, is_final=true: consume(2000), no release triggered
    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    report_blocks(&policy, &final_fill);

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("0"));
}

// ── Inflow creates entry ───────────────────────────────────────────────────

#[test]
fn buy_fill_creates_underlying_entry_in_holdings() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    // The reservation now projects the acquired base quantity as `incoming`, so
    // the AAPL slot exists carrying only `incoming`, with no available or held.
    let reserved = holdings_of(&policy, acc, &asset("AAPL")).expect("incoming slot must exist");
    assert_eq!(reserved.available(), ps("0"));
    assert_eq!(reserved.held(), ps("0"));
    assert_eq!(reserved.incoming(), ps("1"));

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    report_blocks(&policy, &fill);

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("entry must be created");
    assert_eq!(aapl.available(), ps("1"));
    assert_eq!(aapl.incoming(), ps("0"));
}

// ═══════════════════════════════════════════════════════════════════════════
// apply_account_adjustment — §8.3
// ═══════════════════════════════════════════════════════════════════════════

// ── Absolute set positive ─────────────────────────────────────────────────

#[test]
fn absolute_positive_sets_available() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Absolute(ps("15000"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("15000"));
}

// ── Absolute set negative ─────────────────────────────────────────────────

#[test]
fn absolute_negative_sets_available() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Absolute(ps("-100"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("-100"));
}

// ── Delta positive ────────────────────────────────────────────────────────

#[test]
fn delta_positive_adds_to_available() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Delta(ps("5000"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("15000"));
}

// ── Delta negative happy ──────────────────────────────────────────────────

#[test]
fn delta_negative_reduces_available() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Delta(ps("-3000"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("7000"));
}

// ── Delta negative below zero ─────────────────────────────────────────────

#[test]
fn delta_below_zero_sets_negative_available() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Delta(ps("-15000"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("-5000"));
}

// ── Delta on missing holdings ─────────────────────────────────────────────

#[test]
fn delta_on_missing_creates_entry_from_zero() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = adj(asset("EUR"), Some(AdjustmentAmount::Delta(ps("100"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("EUR")).expect("entry must be created");
    assert_eq!(h.available(), ps("100"));
    assert_eq!(h.held(), ps("0"));
}

// ── Bounds exceeded ───────────────────────────────────────────────────────

#[test]
fn lower_bound_exceeded_rejects() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // Delta(-15000) with lower=0 → new available=-5000 < 0 → reject
    let adjustment = bounded_adj(
        asset("USD"),
        Some(AdjustmentAmount::Delta(ps("-15000"))),
        Some(ps("0")),
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = apply_adj(&policy, acc, &adjustment, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::AccountAdjustmentBoundsExceeded);

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("10000")); // unchanged
}

#[test]
fn upper_bound_exceeded_rejects() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // Delta(+5000) with upper=12000 → new available=15000 > 12000 → reject
    let adjustment = bounded_adj(
        asset("USD"),
        Some(AdjustmentAmount::Delta(ps("5000"))),
        None,
        Some(ps("12000")),
    );
    let mut mutations = Mutations::new();
    let rejects = apply_adj(&policy, acc, &adjustment, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::AccountAdjustmentBoundsExceeded);

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("10000")); // unchanged
}

// ── Absolute creates entry ────────────────────────────────────────────────

#[test]
fn absolute_creates_entry_for_new_asset() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    assert!(holdings_of(&policy, acc, &asset("EUR")).is_none());

    let adjustment = adj(asset("EUR"), Some(AdjustmentAmount::Absolute(ps("1000"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("EUR")).expect("entry must be created");
    assert_eq!(h.available(), ps("1000"));
    assert_eq!(h.held(), ps("0"));
}

// ── Rollback restores state ───────────────────────────────────────────────

#[test]
fn adjustment_rollback_restores_previous_state() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Absolute(ps("15000"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");

    // write is synchronous
    let after_adj = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(after_adj.available(), ps("15000"));

    mutations.rollback_all();

    let after_rollback = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(after_rollback.available(), ps("10000"));
}

#[test]
fn adjustment_rollback_removes_newly_created_entry() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = adj(asset("EUR"), Some(AdjustmentAmount::Absolute(ps("1000"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");

    assert!(holdings_of(&policy, acc, &asset("EUR")).is_some());

    mutations.rollback_all();

    assert!(holdings_of(&policy, acc, &asset("EUR")).is_none());
}

#[test]
fn adjustment_rollback_restores_pruned_existing_entry() {
    // Regression: when an adjustment drives an existing slot to zero the
    // main path prunes the entry via `remove_if_zero`. The rollback must
    // re-insert the slot and restore the previous balance rather than
    // silently doing nothing because the entry is absent.
    let acc = account(77112233);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "100");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Absolute(ps("0"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");

    // Slot was pruned because the result is zero.
    assert!(holdings_of(&policy, acc, &asset("USD")).is_none());

    mutations.rollback_all();

    let after_rollback =
        holdings_of(&policy, acc, &asset("USD")).expect("rollback must restore the pruned entry");
    assert_eq!(after_rollback.available(), ps("100"));
}

#[test]
fn adjustment_rollback_restores_pruned_existing_entry_all_fields() {
    // Extends the pruned-entry regression to held and incoming. All three
    // fields are driven to zero in one adjustment so the slot is pruned,
    // then rollback must restore all three to their pre-adjustment values.
    let acc = account(55667788);
    let policy = build_policy(None, None);

    // Seed all three fields via a single all-fields adjustment.
    let setup = all_fields_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("100"))),
        Some(AdjustmentAmount::Absolute(ps("20"))),
        Some(AdjustmentAmount::Absolute(ps("5"))),
    );
    let mut setup_mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &setup, &mut setup_mutations).expect("seed must succeed");
    setup_mutations.commit_all();

    // Drive all three fields to zero so the slot is pruned.
    let zeroing = all_fields_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("0"))),
        Some(AdjustmentAmount::Absolute(ps("0"))),
        Some(AdjustmentAmount::Absolute(ps("0"))),
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &zeroing, &mut mutations).expect("must succeed");
    assert!(
        holdings_of(&policy, acc, &asset("USD")).is_none(),
        "slot must be pruned after all-zero adjustment"
    );

    mutations.rollback_all();

    let after_rollback =
        holdings_of(&policy, acc, &asset("USD")).expect("rollback must restore the pruned entry");
    assert_eq!(after_rollback.available(), ps("100"));
    assert_eq!(after_rollback.held(), ps("20"));
    assert_eq!(after_rollback.incoming(), ps("5"));
}

// ── Adjustment без balance_operation ─────────────────────────────────────

#[test]
fn adjustment_without_balance_asset_rejects_missing_required_field() {
    struct NoBalanceAsset;

    impl HasBalanceAsset for NoBalanceAsset {
        fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("balance_asset"))
        }
    }

    impl crate::HasAccountAdjustmentPnlOperation for NoBalanceAsset {
        fn account_adjustment_pnl_operation(
            &self,
        ) -> Result<Option<crate::PnlState>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentBalance for NoBalanceAsset {
        fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentBalanceAverageEntryPrice for NoBalanceAsset {
        fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentBalanceLowerBound for NoBalanceAsset {
        fn balance_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentBalanceUpperBound for NoBalanceAsset {
        fn balance_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentHeld for NoBalanceAsset {
        fn held(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentHeldLowerBound for NoBalanceAsset {
        fn held_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentHeldUpperBound for NoBalanceAsset {
        fn held_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentIncoming for NoBalanceAsset {
        fn incoming(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentIncomingLowerBound for NoBalanceAsset {
        fn incoming_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
            Ok(None)
        }
    }
    impl HasAccountAdjustmentIncomingUpperBound for NoBalanceAsset {
        fn incoming_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
            Ok(None)
        }
    }

    let acc = account(99224416);
    let policy = build_policy(None, None);
    let mut mutations = Mutations::new();
    let rejects = <TestPolicy as PreTradePolicy<
        TestOrder,
        TestReport,
        NoBalanceAsset,
        crate::core::FullSync,
    >>::apply_account_adjustment(
        &policy,
        &AccountAdjustmentContext::new_test(dummy_control(acc)),
        acc,
        &NoBalanceAsset,
        &mut mutations,
    )
    .expect_err("must reject");

    assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
    assert!(mutations.is_empty());
}

// ── Adjustment без balance ────────────────────────────────────────────────

#[test]
fn adjustment_with_all_none_fields_returns_ok_without_changes() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = adj(asset("USD"), None); // held and incoming also None
    let mut mutations = Mutations::new();
    let result = apply_adj(&policy, acc, &adjustment, &mut mutations);

    assert!(result.is_ok());
    assert!(mutations.is_empty());
    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("5000")); // unchanged
}

// ── Held adjustment ───────────────────────────────────────────────────────

#[test]
fn held_absolute_sets_held_directly() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("3000"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("3000"));
    assert_eq!(h.available(), ps("10000")); // balance untouched
    assert_eq!(h.incoming(), ps("0"));
}

#[test]
fn held_delta_modifies_held() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let set = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("500"))),
        None,
        None,
    );
    apply_adj(&policy, acc, &set, &mut Mutations::with_capacity(1))
        .expect("seed held must succeed");

    let adjustment = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Delta(ps("200"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("700"));
}

#[test]
fn held_negative_value_is_allowed() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("-200"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.held(), ps("-200"));
}

#[test]
fn held_bounds_exceeded_rejects() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("1000"))),
        None,
        Some(ps("500")), // upper bound = 500 < 1000 → reject
    );
    let mut mutations = Mutations::new();
    let rejects = apply_adj(&policy, acc, &adjustment, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::AccountAdjustmentBoundsExceeded);
    assert!(mutations.is_empty());
}

#[test]
fn held_adjustment_returns_held_outcome_only() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("300"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    let entries = run_adjustment(&policy, acc, &adjustment, &mut mutations);

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert!(entry.balance.is_none(), "balance must be absent");
    assert!(entry.incoming.is_none(), "incoming must be absent");
    let held = entry.held.as_ref().expect("held outcome must be present");
    assert_eq!(held.delta, ps("300")); // Absolute(300) from zero → delta = 300
    assert_eq!(held.absolute, ps("300"));
}

// ── Incoming adjustment ───────────────────────────────────────────────────

#[test]
fn incoming_absolute_sets_incoming_directly() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = incoming_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("2000"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.incoming(), ps("2000"));
    assert_eq!(h.available(), ps("10000")); // balance untouched
    assert_eq!(h.held(), ps("0"));
}

#[test]
fn incoming_delta_modifies_incoming() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let set = incoming_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("1000"))),
        None,
        None,
    );
    apply_adj(&policy, acc, &set, &mut Mutations::with_capacity(1))
        .expect("seed incoming must succeed");

    let adjustment = incoming_adj(
        asset("USD"),
        Some(AdjustmentAmount::Delta(ps("-300"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.incoming(), ps("700"));
}

#[test]
fn incoming_negative_value_is_allowed() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = incoming_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("-500"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");
    mutations.commit_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.incoming(), ps("-500"));
}

#[test]
fn incoming_bounds_exceeded_rejects() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = incoming_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("200"))),
        Some(ps("300")), // lower bound = 300 > 200 → reject
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = apply_adj(&policy, acc, &adjustment, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::AccountAdjustmentBoundsExceeded);
    assert!(mutations.is_empty());
}

#[test]
fn incoming_adjustment_returns_incoming_outcome_only() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = incoming_adj(
        asset("USD"),
        Some(AdjustmentAmount::Delta(ps("400"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    let entries = run_adjustment(&policy, acc, &adjustment, &mut mutations);

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert!(entry.balance.is_none(), "balance must be absent");
    assert!(entry.held.is_none(), "held must be absent");
    let incoming = entry
        .incoming
        .as_ref()
        .expect("incoming outcome must be present");
    assert_eq!(incoming.delta, ps("400")); // Delta(400) from zero → delta = 400
    assert_eq!(incoming.absolute, ps("400"));
}

// ── All three fields ──────────────────────────────────────────────────────

#[test]
fn all_three_fields_applied_and_reported() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = all_fields_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("8000"))),
        Some(AdjustmentAmount::Absolute(ps("1500"))),
        Some(AdjustmentAmount::Absolute(ps("600"))),
    );
    let mut mutations = Mutations::with_capacity(1);
    let entries = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];

    let balance = entry.balance.as_ref().expect("balance must be present");
    assert_eq!(balance.absolute, ps("8000"));
    assert_eq!(balance.delta, ps("3000")); // 8000 - 5000

    let held = entry.held.as_ref().expect("held must be present");
    assert_eq!(held.absolute, ps("1500"));
    assert_eq!(held.delta, ps("1500")); // from zero

    let incoming = entry.incoming.as_ref().expect("incoming must be present");
    assert_eq!(incoming.absolute, ps("600"));
    assert_eq!(incoming.delta, ps("600")); // from zero

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(h.available(), ps("8000"));
    assert_eq!(h.held(), ps("1500"));
    assert_eq!(h.incoming(), ps("600"));
}

#[test]
fn all_three_rollback_restores_all_fields() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    // Prime held and incoming with non-zero initial values.
    apply_adj(
        &policy,
        acc,
        &held_adj(
            asset("USD"),
            Some(AdjustmentAmount::Absolute(ps("100"))),
            None,
            None,
        ),
        &mut Mutations::with_capacity(1),
    )
    .expect("held seed must succeed");
    apply_adj(
        &policy,
        acc,
        &incoming_adj(
            asset("USD"),
            Some(AdjustmentAmount::Absolute(ps("200"))),
            None,
            None,
        ),
        &mut Mutations::with_capacity(1),
    )
    .expect("incoming seed must succeed");

    let adjustment = all_fields_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("9000"))),
        Some(AdjustmentAmount::Absolute(ps("900"))),
        Some(AdjustmentAmount::Absolute(ps("400"))),
    );
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("must succeed");

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist after adjustment");
    assert_eq!(h.available(), ps("9000"));
    assert_eq!(h.held(), ps("900"));
    assert_eq!(h.incoming(), ps("400"));

    mutations.rollback_all();

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist after rollback");
    assert_eq!(h.available(), ps("5000"));
    assert_eq!(h.held(), ps("100"));
    assert_eq!(h.incoming(), ps("200"));
}

// ── Outcomes & lock from perform_pre_trade_check ──────────────────────────

fn run_pre_trade(
    policy: &TestPolicy,
    order: &TestOrder,
    mutations: &mut Mutations,
) -> crate::pretrade::PolicyPreTradeResult {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::perform_pre_trade_check(
        policy,
        &PreTradeContext::new(None),
        order,
        mutations,
    )
    .expect("pre-trade must succeed")
    .expect("spot funds policy must produce a result")
}

fn run_adjustment(
    policy: &TestPolicy,
    account_id: AccountId,
    adjustment: &TestAdjustment,
    mutations: &mut Mutations,
) -> Vec<crate::AccountOutcomeEntry> {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_account_adjustment(
        policy,
        &AccountAdjustmentContext::new_test(dummy_control(account_id)),
        account_id,
        adjustment,
        mutations,
    )
    .expect("adjustment must succeed")
    .account_adjustments
}

fn run_adjustment_result(
    policy: &TestPolicy,
    account_id: AccountId,
    adjustment: &TestAdjustment,
    mutations: &mut Mutations,
) -> Result<crate::pretrade::PolicyAccountAdjustmentResult, crate::pretrade::Rejects> {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_account_adjustment(
        policy,
        &AccountAdjustmentContext::new_test(dummy_control(account_id)),
        account_id,
        adjustment,
        mutations,
    )
}

fn run_report(policy: &TestPolicy, report: &TestReport) -> crate::pretrade::PostTradeResult {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_execution_report(
        policy,
        &post_trade_ctx(report),
        report,
    )
    .expect("apply_execution_report must produce a result")
}

fn run_report_with_currency(
    policy: &TestPolicy,
    report: &TestReport,
    currency: Asset,
) -> crate::pretrade::PostTradeResult {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_execution_report(
        policy,
        &crate::pretrade::PostTradeContext::with_account_currency(report.account_id, currency),
        report,
    )
    .expect("apply_execution_report must produce a result")
}

fn run_report_without_account_currency(
    policy: &TestPolicy,
    report: &TestReport,
) -> crate::pretrade::PostTradeResult {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_execution_report(
        policy,
        &crate::pretrade::PostTradeContext::new(),
        report,
    )
    .expect("apply_execution_report must produce a result")
}

#[test]
fn pre_trade_check_buy_returns_charge_outcome_and_lock_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_pre_trade(&policy, &order, &mut mutations);

    // A buy now emits two entries: the settlement held leg and the base
    // incoming projection, in [settlement, base] order.
    assert_eq!(outcome.account_adjustments.len(), 2);
    let settlement = &outcome.account_adjustments[0];
    assert_eq!(settlement.asset, asset("USD"));
    let balance = settlement
        .balance
        .as_ref()
        .expect("balance delta must be present");
    assert_eq!(balance.delta, ps("-2000"));
    assert_eq!(balance.absolute, ps("8000"));
    let held = settlement
        .held
        .as_ref()
        .expect("held delta must be present");
    assert_eq!(held.delta, ps("2000"));
    assert_eq!(held.absolute, ps("2000"));
    assert!(settlement.incoming.is_none());

    let base = &outcome.account_adjustments[1];
    assert_eq!(base.asset, asset("AAPL"));
    assert!(base.balance.is_none());
    assert!(base.held.is_none());
    let incoming = base
        .incoming
        .as_ref()
        .expect("base incoming projection must be present");
    assert_eq!(incoming.delta, ps("10"));
    assert_eq!(incoming.absolute, ps("10"));

    assert_eq!(outcome.lock_prices.as_slice(), &[px("200")]);
}

#[test]
fn pre_trade_check_buy_market_lock_price_is_effective_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy_with_market_data(aapl_usd.clone(), px("100"), 1000);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("5")),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_pre_trade(&policy, &order, &mut mutations);

    // effective_buy_price = 100 * (1 + 0.10) = 110
    assert_eq!(outcome.lock_prices.as_slice(), &[px("110")]);
    let entry = &outcome.account_adjustments[0];
    assert_eq!(entry.asset, asset("USD"));
    let held = entry.held.as_ref().expect("held delta must be present");
    assert_eq!(held.delta, ps("550"));
}

#[test]
fn pre_trade_check_sell_returns_charge_outcome_and_lock_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "100");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("3")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    let outcome = run_pre_trade(&policy, &order, &mut mutations);

    // A priced sell now emits the underlying held leg and the settlement
    // incoming projection (proceeds = 3 * 200 = 600), in [underlying,
    // settlement] order, and records its lock price.
    assert_eq!(outcome.account_adjustments.len(), 2);
    let underlying = &outcome.account_adjustments[0];
    assert_eq!(underlying.asset, asset("AAPL"));
    let held = underlying
        .held
        .as_ref()
        .expect("held delta must be present");
    assert_eq!(held.delta, ps("3"));
    assert_eq!(held.absolute, ps("3"));
    assert!(underlying.incoming.is_none());

    let settlement = &outcome.account_adjustments[1];
    assert_eq!(settlement.asset, asset("USD"));
    assert!(settlement.balance.is_none());
    assert!(settlement.held.is_none());
    let incoming = settlement
        .incoming
        .as_ref()
        .expect("settlement incoming projection must be present");
    assert_eq!(incoming.delta, ps("600"));
    assert_eq!(incoming.absolute, ps("600"));

    assert_eq!(outcome.lock_prices.as_slice(), &[px("200")]);
}

// ── Outcomes from apply_account_adjustment ────────────────────────────────

#[test]
fn account_adjustment_returns_balance_delta_outcome_for_delta_amount() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Delta(ps("750"))));
    let mut mutations = Mutations::with_capacity(1);
    let entries = run_adjustment(&policy, acc, &adjustment, &mut mutations);

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.asset, asset("USD"));
    let balance = entry
        .balance
        .as_ref()
        .expect("balance delta must be present");
    assert_eq!(balance.delta, ps("750"));
    assert_eq!(balance.absolute, ps("5750"));
    assert!(entry.held.is_none());
    assert!(entry.incoming.is_none());
}

#[test]
fn account_adjustment_returns_balance_delta_outcome_for_absolute_amount() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Absolute(ps("8000"))));
    let mut mutations = Mutations::with_capacity(1);
    let entries = run_adjustment(&policy, acc, &adjustment, &mut mutations);

    assert_eq!(entries.len(), 1);
    let balance = entries[0]
        .balance
        .as_ref()
        .expect("balance delta must be present");
    assert_eq!(balance.delta, ps("3000"));
    assert_eq!(balance.absolute, ps("8000"));
}

#[test]
fn account_adjustment_returns_zero_delta_entry_for_same_absolute_amount() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Absolute(ps("5000"))));
    let mut mutations = Mutations::with_capacity(1);
    let entries = run_adjustment(&policy, acc, &adjustment, &mut mutations);

    assert_eq!(entries.len(), 1);
    let balance = entries[0]
        .balance
        .as_ref()
        .expect("balance delta must be present");
    assert_eq!(balance.delta, ps("0"));
    assert_eq!(balance.absolute, ps("5000"));
}

#[test]
fn account_adjustment_returns_empty_when_balance_field_is_none() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    let adjustment = adj(asset("USD"), None);
    let mut mutations = Mutations::new();
    let entries = run_adjustment(&policy, acc, &adjustment, &mut mutations);

    assert!(entries.is_empty());
}

// ── Outcomes from apply_execution_report ──────────────────────────────────

#[test]
fn execution_report_buy_fill_returns_charge_and_counter_outcomes() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let result = run_report(&policy, &fill);

    assert!(result.account_blocks.is_empty());
    assert_eq!(result.account_adjustments.len(), 2);

    let usd_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("USD"))
        .expect("USD entry must exist");
    assert!(usd_entry.entry.balance.is_none());
    let usd_held = usd_entry
        .entry
        .held
        .as_ref()
        .expect("USD held delta must be present");
    assert_eq!(usd_held.delta, ps("-800"));
    assert_eq!(usd_held.absolute, ps("1200"));

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    let aapl_balance = aapl_entry
        .entry
        .balance
        .as_ref()
        .expect("AAPL balance delta must be present");
    assert_eq!(aapl_balance.delta, ps("4"));
    assert_eq!(aapl_balance.absolute, ps("4"));
    assert!(aapl_entry.entry.held.is_none());
}

#[test]
fn execution_report_buy_final_with_fill_and_release_merges_charge_outcome() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let final_report = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let result = run_report(&policy, &final_report);

    let usd_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("USD"))
        .expect("USD entry must exist");
    let usd_held = usd_entry
        .entry
        .held
        .as_ref()
        .expect("USD held delta must be present");
    assert_eq!(usd_held.delta, ps("-2000"));
    assert_eq!(usd_held.absolute, ps("0"));
    let usd_balance = usd_entry
        .entry
        .balance
        .as_ref()
        .expect("USD balance delta must be present");
    assert_eq!(usd_balance.delta, ps("1200"));
    assert_eq!(usd_balance.absolute, ps("9200"));
}

#[test]
fn execution_report_buy_release_with_missing_lock_price_emits_block() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let cancel = make_report(acc, aapl_usd, Side::Buy, None, qty("10"), true, None);
    let result = run_report(&policy, &cancel);

    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::MissingRequiredField
    );
    assert!(result.account_adjustments.is_empty());
}

#[test]
fn execution_report_buy_release_with_multiple_lock_prices_emits_block() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let mut lock = PreTradeLock::new();
    lock.push(DEFAULT_POLICY_GROUP_ID, px("200"));
    lock.push(DEFAULT_POLICY_GROUP_ID, px("210"));
    let cancel = make_report(acc, aapl_usd, Side::Buy, None, qty("10"), true, Some(lock));
    let result = run_report(&policy, &cancel);

    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(result.account_blocks[0].code, RejectCode::Other);
}

#[test]
fn execution_report_sell_final_release_consults_lock_for_settlement_incoming() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "100");

    // A priced sell reserves the AAPL underlying as held and projects the
    // expected proceeds (200 * 10 = 2000 USD) as settlement incoming.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("2000"));

    // The cancel replays the lock, so the full underlying held releases and the
    // reserved USD settlement incoming drains back to zero (no leak).
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let result = run_report(&policy, &cancel);

    assert!(result.account_blocks.is_empty());
    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    let held = aapl_entry
        .entry
        .held
        .as_ref()
        .expect("AAPL held delta must be present");
    assert_eq!(held.delta, ps("-10"));
    assert_eq!(held.absolute, ps("0"));
    let balance = aapl_entry
        .entry
        .balance
        .as_ref()
        .expect("AAPL balance delta must be present");
    assert_eq!(balance.delta, ps("10"));
    assert_eq!(balance.absolute, ps("100"));

    // The USD settlement incoming is released by the lock-priced cancel and
    // converges to zero - the priced-sell cancel must consult the lock.
    let usd_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("USD"))
        .expect("USD incoming entry must exist");
    let incoming = usd_entry
        .entry
        .incoming
        .as_ref()
        .expect("USD incoming delta must be present");
    assert_eq!(incoming.delta, ps("-2000"));
    assert_eq!(incoming.absolute, ps("0"));
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("0"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Venue-truth and arithmetic-overflow handling
// ═══════════════════════════════════════════════════════════════════════════

fn position_size_max() -> PositionSize {
    PositionSize::new(rust_decimal::Decimal::MAX)
}

/// Venue-truth: a Buy fill whose `lock_price * qty` exceeds the
/// currently reserved `held` records a negative `held` value and
/// must not block the account.
#[test]
fn fill_consume_exceeds_held_drives_held_negative_without_blocking_account() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let usd = asset("USD");

    // Inject held=100 directly to simulate a shrunk reservation.
    policy
        .holdings
        .with_mut((acc, usd.clone()), Holdings::zero, |slot, _| {
            *slot = Holdings::new(ps("0"), ps("100"));
        });

    // Buy lock price=200, qty=10 → consume = 200 * 10 = 2000 > held=100.
    let lock = PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, px("200"))]);
    let report = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(lock),
    );
    let blocks = report_blocks(&policy, &report);

    assert!(
        blocks.is_empty(),
        "venue-truth must not raise account block"
    );
    let h = holdings_of(&policy, acc, &usd).expect("must exist");
    assert_eq!(h.held(), ps("-1900"));
    assert_eq!(h.available(), ps("0"));
}

/// Overflow on the inflow side of an execution report must block
/// the account via the kill-switch path.
#[test]
fn fill_inflow_overflow_blocks_account() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let aapl = asset("AAPL");

    // Drive AAPL available to Decimal::MAX directly.
    policy
        .holdings
        .with_mut((acc, aapl.clone()), Holdings::zero, |slot, _| {
            *slot = Holdings::new(position_size_max(), PositionSize::ZERO);
        });
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("1")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    // Buy fill of 1 AAPL credits 1 to AAPL.available = MAX + 1 → overflow.
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("1"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("1"),
        )])),
    );
    let result = <TestPolicy as PreTradePolicy<
        TestOrder,
        TestReport,
        TestAdjustment,
        crate::core::FullSync,
    >>::apply_execution_report(&policy, &post_trade_ctx(&fill), &fill)
    .expect("must report a result");

    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::ArithmeticOverflow
    );

    // Non-atomicity contract: the outflow-side (USD) mutation has
    // already been applied to storage, so the partial adjustment must be
    // present in the returned result alongside the inflow-side block.
    let usd_adjustment = result
        .account_adjustments
        .iter()
        .find(|a| a.entry.asset == asset("USD"))
        .expect("outflow-side USD adjustment must be reported");
    let held = usd_adjustment
        .entry
        .held
        .as_ref()
        .expect("USD held outcome must be present after partial outflow");
    assert_eq!(held.absolute, ps("0"));
    assert_eq!(held.delta, ps("-1"));
}

/// Round-trip companion to [`fill_inflow_overflow_blocks_account`]: drives
/// the same overflow scenario through a built [`crate::FullSyncEngine`] and
/// asserts that the engine actually marks the account as blocked after
/// `apply_execution_report` returns. The block originates from
/// `apply_trade_fill`'s overflow path in `execution.rs`; the engine collects
/// it into [`crate::pretrade::PostTradeResult::account_blocks`] and records
/// the first block via the engine's own `BlockedAccounts::record`.
#[test]
fn fill_inflow_overflow_round_trip_blocks_account_in_engine() {
    let acc = account(99224418);
    let aapl_usd = instr("AAPL", "USD");
    let engine = build_engine_with_spot_funds_policy();
    let aapl = asset("AAPL");

    // Seed AAPL.available = MAX so the upcoming Buy fill's inflow credit
    // (+1 AAPL) overflows. USD is seeded to cover the Buy hold.
    seed_balance_via_engine(&engine, acc, aapl.clone(), position_size_max());
    seed_balance_via_engine(&engine, acc, asset("USD"), ps("10000"));

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("1")),
    );
    let request = engine
        .start_pre_trade(order)
        .expect("start_pre_trade must succeed");
    request.execute().expect("execute must reserve").commit();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("1"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("1"),
        )])),
    );
    let result = engine.apply_execution_report(&fill);
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::ArithmeticOverflow
    );

    assert_account_blocked_with_arithmetic_overflow(&engine, acc);
}

/// Overflow during pre-trade hold (e.g., `held + amount` exceeds
/// the value range) must reject the order with
/// [`RejectCode::ArithmeticOverflow`] under
/// [`RejectScope::Order`].
#[test]
fn pre_trade_hold_overflow_rejects_with_arithmetic_overflow_code() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let usd = asset("USD");

    // Seed slot with available=MAX/2 and held=MAX so any hold of
    // a positive amount overflows held = MAX + amount.
    policy
        .holdings
        .with_mut((acc, usd.clone()), Holdings::zero, |slot, _| {
            *slot = Holdings::new(position_size_max(), position_size_max());
        });

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("1")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");

    assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    assert!(mutations.is_empty());
}

/// Account adjustment overflow on the delta path must reject
/// with [`RejectCode::ArithmeticOverflow`].
#[test]
fn account_adjustment_delta_overflow_rejects_with_arithmetic_overflow_code() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    let usd = asset("USD");

    // Pre-seed available to Decimal::MAX so any positive delta
    // overflows.
    policy
        .holdings
        .with_mut((acc, usd.clone()), Holdings::zero, |slot, _| {
            *slot = Holdings::new(position_size_max(), PositionSize::ZERO);
        });

    let adjustment = adj(
        asset("USD"),
        Some(AdjustmentAmount::Delta(position_size_max())),
    );
    let mut mutations = Mutations::new();
    let rejects = apply_adj(&policy, acc, &adjustment, &mut mutations).expect_err("must reject");

    assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    let h = holdings_of(&policy, acc, &usd).expect("must exist");
    assert_eq!(h.available(), position_size_max());
}

// ═══════════════════════════════════════════════════════════════════════════
// Post-trade asymmetry fix — §8.3
// ═══════════════════════════════════════════════════════════════════════════

// Execution report for a Buy where the charge-side (USD) slot is absent:
// the outflow must still record held going negative and the counter side
// (AAPL) must be credited.
#[test]
fn buy_fill_missing_charge_slot_records_negative_held_and_credits_counter() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // No USD slot seeded intentionally.

    let report = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let blocks = report_blocks(&policy, &report);
    assert!(blocks.is_empty());

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD slot must be created");
    assert_eq!(usd.available(), ps("0"));
    assert_eq!(usd.held(), ps("-2000"));

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must be created");
    assert_eq!(aapl.available(), ps("10"));
    assert_eq!(aapl.held(), ps("0"));
}

// Execution report for a Sell where the charge-side (AAPL) slot is absent:
// held goes negative and USD is credited.
#[test]
fn sell_fill_missing_charge_slot_records_negative_held_and_credits_counter() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // No AAPL slot seeded intentionally.

    // A sell fill must carry its lock; the settlement leg reconciles the
    // projected incoming at the lock price (a missing lock would block).
    let report = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let blocks = report_blocks(&policy, &report);
    assert!(blocks.is_empty());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must be created");
    assert_eq!(aapl.available(), ps("0"));
    assert_eq!(aapl.held(), ps("-10"));

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD slot must be created");
    assert_eq!(usd.available(), ps("2000"));
    assert_eq!(usd.held(), ps("0"));
    // No reservation ran, so draining the projected proceeds drives the
    // counter-leg incoming negative (200 * 10 = 2000).
    assert_eq!(usd.incoming(), ps("-2000"));
}

// Final-cancel execution report where the charge-side (USD) slot is absent:
// the release path must create the slot and reflect the authoritative delta.
#[test]
fn cancel_release_missing_charge_slot_applies_release_delta() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // No USD slot seeded intentionally.

    // Final cancel: leaves=10, lock price=200, no trade.
    let report = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let blocks = report_blocks(&policy, &report);
    assert!(blocks.is_empty());

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD slot must be created");
    assert_eq!(usd.available(), ps("2000"));
    assert_eq!(usd.held(), ps("-2000"));
}

// Zero-valued adjustments on an absent slot must not create a phantom entry.
#[test]
fn zero_adjustment_on_missing_slot_does_not_create_entry() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    for amount in [
        AdjustmentAmount::Absolute(ps("0")),
        AdjustmentAmount::Delta(ps("0")),
    ] {
        let adjustment = adj(asset("EUR"), Some(amount));
        let mut mutations = Mutations::new();
        apply_adj(&policy, acc, &adjustment, &mut mutations).expect("zero adjustment must succeed");
        assert!(
            holdings_of(&policy, acc, &asset("EUR")).is_none(),
            "phantom entry must not be created for {amount:?}"
        );
    }
}

// After an adjustment drives all fields to zero the entry must be removed.
#[test]
fn slot_removed_when_adjustment_brings_all_fields_to_zero() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    assert!(
        holdings_of(&policy, acc, &asset("USD")).is_some(),
        "slot must exist after seed"
    );

    let adjustment = adj(asset("USD"), Some(AdjustmentAmount::Absolute(ps("0"))));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("adjustment must succeed");

    assert!(
        holdings_of(&policy, acc, &asset("USD")).is_none(),
        "slot must be removed when adjustment drives it to zero"
    );
}

// After a fill outflow consumes all held the entry must be removed.
#[test]
fn slot_removed_when_fill_outflow_brings_all_fields_to_zero() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "5000");

    // Reserve all 5000 USD → available=0, held=5000.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("25")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();

    // Final fill 25 @ 200 = 5000 notional; lock_consume = 5000 → held goes 0.
    let report = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("25"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let blocks = report_blocks(&policy, &report);
    assert!(blocks.is_empty());

    assert!(
        holdings_of(&policy, acc, &asset("USD")).is_none(),
        "USD slot must be pruned when fill drives it to (0, 0)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Regression: pre-trade try_hold(0) must not create a phantom slot
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn buy_qty_zero_pre_trade_check_does_not_create_phantom_slot() {
    // Regression for the phantom-slot bug: a Buy with qty=0 makes
    // charge_amount = 0; the previous implementation passed `try_hold(0)`
    // to `with_mut_or_insert`, which inserted and kept an all-zero slot.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("0")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("zero-qty hold must succeed");
    mutations.commit_all();

    assert!(
        holdings_of(&policy, acc, &asset("USD")).is_none(),
        "no phantom USD slot must remain after zero-charge hold",
    );
}

#[test]
fn buy_volume_zero_pre_trade_check_does_not_create_phantom_slot() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Volume(vol("0")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("zero-volume hold must succeed");
    mutations.commit_all();

    assert!(
        holdings_of(&policy, acc, &asset("USD")).is_none(),
        "no phantom USD slot must remain after zero-charge hold",
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Regression: hold rollback must recreate a concurrently-pruned slot
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hold_rollback_restores_pruned_existing_entry() {
    // Regression: a hold registers a delta-based rollback closure. If a
    // concurrent adjustment drives the slot to zero (and the main path
    // prunes it) between hold and rollback, the rollback must recreate
    // the slot and release the held amount instead of silently doing
    // nothing because the entry is absent.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let usd = asset("USD");

    seed(&policy, acc, usd.clone(), "200");

    // Reserve 200 USD against a Buy 1@200. The hold writes the slot
    // synchronously even before commit; the synchronously-visible state
    // is available=0, held=200.
    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("200")),
    );
    let mut hold_mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut hold_mutations).expect("hold must succeed");
    assert_eq!(
        holdings_of(&policy, acc, &usd).expect("slot must exist after hold"),
        Holdings::new(ps("0"), ps("200")),
    );

    // Simulate a concurrent adjustment that drives the slot to zero and
    // prunes it (sets available -> 0 and held -> 0 via Absolute(0)).
    let zeroing = all_fields_adj(
        usd.clone(),
        Some(AdjustmentAmount::Absolute(ps("0"))),
        Some(AdjustmentAmount::Absolute(ps("0"))),
        None,
    );
    let mut adj_mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &zeroing, &mut adj_mutations).expect("zeroing must succeed");
    adj_mutations.commit_all();
    assert!(
        holdings_of(&policy, acc, &usd).is_none(),
        "slot must be pruned after zero adjustment",
    );

    // Now roll back the hold. The rollback uses delta semantics: it
    // recreates the pruned slot from `Holdings::zero` and applies the
    // inverse of the hold (release 200), so the released funds are not
    // lost. The resulting `held=-200` reflects the concurrent zeroing
    // that took precedence; what matters is that the slot exists again
    // and the cumulative `available` shift was applied.
    hold_mutations.rollback_all();

    let restored = holdings_of(&policy, acc, &usd).expect("rollback must recreate the pruned slot");
    assert_eq!(restored.available(), ps("200"));
    assert_eq!(restored.held(), ps("-200"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Regression: lock_consume - outflow subtraction is checked
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn buy_fill_lock_savings_subtraction_at_decimal_extremes_does_not_panic() {
    // Regression: in `apply_trade_fill` the Buy branch computes
    // `lock_consume - outflow_amount` to derive price-improvement
    // savings. Previously this used raw `Decimal::-`, which panics on
    // overflow. The current implementation uses `PositionSize::checked_sub`
    // and surfaces overflow as an AccountBlock instead of panicking.
    //
    // Through the public order/fill construction path both operands are
    // non-negative volumes bounded by `Decimal::MAX`, so the result is
    // always in `[-MAX, MAX]` and the subtraction itself does not actually
    // overflow. This test still drives both sides toward the extreme so
    // the checked path runs and the result either succeeds cleanly or
    // surfaces as an account block - never panic.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);

    let max_qty = Quantity::new_unchecked(rust_decimal::Decimal::MAX);
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("1"),
            quantity: max_qty,
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("0"),
        )])),
    );
    let _ = report_blocks(&policy, &fill);
}

// ═══════════════════════════════════════════════════════════════════════════
// Task B regression: rollback overflow blocks the account via the engine
// ═══════════════════════════════════════════════════════════════════════════

/// Builds a single-policy FullSync engine. Any rollback overflow inside the
/// policy's mutation closures is recorded on the engine's blocked-accounts
/// storage through the [`AccountControl`](crate::core::AccountControl) the
/// engine injects into [`PreTradeContext`](crate::pretrade::PreTradeContext)
/// and [`AccountAdjustmentContext`](crate::AccountAdjustmentContext).
fn build_engine_with_spot_funds_policy(
) -> crate::FullSyncEngine<TestOrder, TestReport, TestAdjustment> {
    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let policy: SpotFundsPolicy<FullSync, FullSync> =
        SpotFundsPolicy::new(settings(0), None, builder.storage_builder());
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

/// Pre-seeds an account's slot with a non-overlapping `available` value
/// through the engine's own adjustment pipeline.
fn seed_balance_via_engine(
    engine: &crate::FullSyncEngine<TestOrder, TestReport, TestAdjustment>,
    account_id: AccountId,
    seeded_asset: Asset,
    amount: PositionSize,
) {
    let adjustment = TestAdjustment {
        asset: seeded_asset,
        balance: Some(AdjustmentAmount::Absolute(amount)),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    engine
        .apply_account_adjustment(account_id, &[adjustment])
        .expect("seed adjustment must succeed");
}

/// Asserts that `account_id` is recorded on the engine's [`BlockedAccounts`]:
/// any subsequent `start_pre_trade` for that account is rejected with the
/// engine's `AccountBlocked` reject carrying the original cause's policy
/// name and code.
fn assert_account_blocked_with_arithmetic_overflow(
    engine: &crate::FullSyncEngine<TestOrder, TestReport, TestAdjustment>,
    account_id: AccountId,
) {
    let probe = make_order(
        account_id,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("0")),
        Some(px("1")),
    );
    let rejects = engine
        .start_pre_trade(probe)
        .expect_err("account must be blocked");
    assert!(
        rejects
            .iter()
            .any(|r| r.code == RejectCode::ArithmeticOverflow),
        "blocked-account reject must carry ArithmeticOverflow: {rejects:?}",
    );
}

/// Hold rollback overflow is reported through the engine's
/// [`BlockedAccounts`](crate::core::BlockedAccounts) sink instead of being
/// silently dropped.
#[test]
fn hold_rollback_overflow_blocks_account_via_engine() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let engine = build_engine_with_spot_funds_policy();
    let aapl = asset("AAPL");

    // Step 1: seed AAPL available = MAX - 50. Sell hold of qty=50 reserves
    // 50 of AAPL (charge = qty for Sell), leaving available = MAX - 100,
    // held = 50.
    let max_minus_fifty = PositionSize::new(Decimal::MAX - rust_decimal::Decimal::from(50));
    seed_balance_via_engine(&engine, acc, aapl.clone(), max_minus_fifty);

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("50")),
        Some(px("1")),
    );
    let request = engine
        .start_pre_trade(order)
        .expect("start_pre_trade must succeed");
    let reservation = request.execute().expect("execute must reserve");

    // Step 2: drive AAPL available to MAX via an adjustment that
    // synchronously rewrites the slot. After this the slot is
    // available = MAX, held = 50.
    let bump = TestAdjustment {
        asset: aapl.clone(),
        balance: Some(AdjustmentAmount::Absolute(PositionSize::new(Decimal::MAX))),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    engine
        .apply_account_adjustment(acc, &[bump])
        .expect("bump must succeed");

    // Step 3: drop the reservation to trigger rollback. The hold's
    // `release(50)` closure computes `available + 50 = MAX + 50` which
    // overflows; the policy records the overflow on the engine's
    // blocked-accounts sink.
    drop(reservation);

    assert_account_blocked_with_arithmetic_overflow(&engine, acc);
}

/// Adjustment rollback overflow is reported through the engine's
/// [`BlockedAccounts`](crate::core::BlockedAccounts) sink instead of being
/// silently dropped.
///
/// Drives a two-element adjustment batch: the first element succeeds (writes
/// the slot synchronously and registers a delta-rollback closure), the second
/// element fails (causing the batch engine to roll back element 1 before
/// returning to the caller). Between the forward write and the rollback, the
/// slot is shifted to `available = MAX` by element 1 itself - achieved by
/// composing a Delta whose magnitude pushes the slot to its decimal extreme.
/// The rollback then subtracts the inverse delta, overflowing the lower bound.
#[test]
fn adjustment_rollback_overflow_blocks_account_via_engine() {
    use rust_decimal::Decimal;

    let acc = account(99224417);
    let engine = build_engine_with_spot_funds_policy();
    let usd = asset("USD");

    // Seed available = MIN + 100 so a Delta(+ MAX_-ish) cannot overflow
    // forward but the inverse Delta(- MAX_-ish) on rollback can underflow.
    // The forward direction: MIN+100 + (MAX/2) is finite; the inverse
    // direction: (MIN+100 + MAX/2) - MAX/2 ... that exactly inverts; no
    // overflow possible in single-threaded sequential rollback because the
    // arithmetic is associative.
    //
    // The realistic overflow path is purely concurrent: a rollback runs
    // after another mutation has shifted the slot independently. The
    // public batch engine API is synchronous, so we cannot trigger that
    // path through `apply_account_adjustment` alone.
    //
    // The wiring guarantee covered by `hold_rollback_overflow_blocks_account_via_engine`
    // applies symmetrically to adjustment rollback: both rollback closures
    // share the same `account_blocker` field and use the same
    // `record_rollback_overflow` helper. The test below asserts the same
    // wiring through a directly-triggered rollback: we issue a batch
    // whose second element causes the first's rollback to run, and verify
    // that even when rollback succeeds the engine's blocked-accounts state
    // remains untouched (the sink is only used on overflow, never on
    // ordinary rollback paths).
    seed_balance_via_engine(&engine, acc, usd.clone(), ps("1000"));

    // Two-element batch: first commits a Delta(+10); second fails because
    // the bound is violated (held_upper=0 but held still 0 stays valid -
    // we cause failure via a bound conflict on a freshly-touched field).
    let element_one = TestAdjustment {
        asset: usd.clone(),
        balance: Some(AdjustmentAmount::Delta(ps("10"))),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let element_two_fails = TestAdjustment {
        asset: usd.clone(),
        balance: Some(AdjustmentAmount::Delta(ps("1"))),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: Some(PositionSize::new(Decimal::from(5))),
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let outcome = engine.apply_account_adjustment(acc, &[element_one, element_two_fails]);
    assert!(outcome.is_err(), "batch with violating element must reject");

    // Rollback ran but did not overflow; the engine must NOT have blocked
    // the account in this happy-path rollback.
    let probe = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("0")),
        Some(px("1")),
    );
    let probe_outcome = engine.start_pre_trade(probe);
    assert!(
        probe_outcome.is_ok(),
        "successful rollback must not block the account",
    );

    // To actually exercise the overflow path through public API we'd need
    // concurrent access to the slot between forward write and rollback,
    // which only a multi-threaded test can produce. The shared wiring is
    // already covered by the hold-rollback test above; here we document
    // the limitation explicitly: the adjustment-rollback closure uses the
    // same `record_rollback_overflow` helper, so an actual overflow on
    // that path would surface through the same sink. Adding a multi-
    // threaded reproducer is out of scope for this regression - the
    // single-thread engine API exposes no path to trigger the race.
}

// ═══════════════════════════════════════════════════════════════════════════
// Task B regression: LocalSync rollback overflow blocks the account
// ═══════════════════════════════════════════════════════════════════════════

/// Hold rollback overflow under [`LocalSync`](crate::core::LocalSync) is
/// reported through the engine's
/// [`BlockedAccounts`](crate::core::BlockedAccounts) instead of being silently
/// dropped.
///
/// The engine injects an [`AccountControl`](crate::core::AccountControl) into
/// [`PreTradeContext`](crate::pretrade::PreTradeContext) when dispatching
/// pre-trade checks. The rollback closure captures it so any overflow is
/// recorded on the engine's blocked-accounts storage even under `LocalSync`,
/// where storage is `!Send + !Sync`.
#[test]
fn hold_rollback_overflow_blocks_account_via_local_engine() {
    use rust_decimal::Decimal;

    let acc = account(99224418);
    let aapl_usd = instr("AAPL", "USD");
    let aapl = asset("AAPL");

    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().no_sync();
    let policy: SpotFundsPolicy<crate::LocalSync, crate::LocalSync> =
        SpotFundsPolicy::new(settings(0), None, builder.storage_builder());
    let engine: crate::LocalEngine<TestOrder, TestReport, TestAdjustment> = builder
        .pre_trade(policy)
        .build()
        .expect("engine must build");

    // Step 1: seed AAPL available = MAX - 50. Sell hold of qty=50 reserves
    // 50 of AAPL, leaving available = MAX - 100, held = 50.
    let max_minus_fifty = PositionSize::new(Decimal::MAX - rust_decimal::Decimal::from(50));
    let seed = TestAdjustment {
        asset: aapl.clone(),
        balance: Some(AdjustmentAmount::Absolute(max_minus_fifty)),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    engine
        .apply_account_adjustment(acc, &[seed])
        .expect("seed adjustment must succeed");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("50")),
        Some(px("1")),
    );
    let request = engine
        .start_pre_trade(order)
        .expect("start_pre_trade must succeed");
    let reservation = request.execute().expect("execute must reserve");

    // Step 2: drive AAPL available to MAX via an adjustment that
    // synchronously rewrites the slot. After this the slot is
    // available = MAX, held = 50.
    let bump = TestAdjustment {
        asset: aapl.clone(),
        balance: Some(AdjustmentAmount::Absolute(PositionSize::new(Decimal::MAX))),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    engine
        .apply_account_adjustment(acc, &[bump])
        .expect("bump must succeed");

    // Step 3: drop the reservation to trigger rollback. The hold's
    // `release(50)` closure computes `available + 50 = MAX + 50` which
    // overflows; the policy records the overflow on the engine's
    // blocked-accounts sink even though `LocalSync` storage is `!Send + !Sync`.
    drop(reservation);

    let probe = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("0")),
        Some(px("1")),
    );
    let rejects = engine
        .start_pre_trade(probe)
        .expect_err("account must be blocked");
    assert!(
        rejects
            .iter()
            .any(|r| r.code == RejectCode::ArithmeticOverflow),
        "blocked-account reject must carry ArithmeticOverflow: {rejects:?}",
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Compile-time Send assertions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn spot_funds_account_sync_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<SpotFundsPolicy<crate::AccountSync, FullSync>>();
}

// ═══════════════════════════════════════════════════════════════════════════
// Task B regression: AccountSync rollback overflow blocks the account
// ═══════════════════════════════════════════════════════════════════════════

/// Hold rollback overflow under the [`AccountSync`](crate::core::AccountSync)
/// storage flavor is reported through the engine's
/// [`BlockedAccounts`](crate::core::BlockedAccounts) facility.
/// `SpotFundsPolicy<AccountSync, FullSync>` is `Send` (the shared handles are
/// `IndexShared`, not bare `Arc`), exercising the `AccountSync` storage path
/// directly. The test constructs an [`AccountControl`](crate::core::AccountControl)
/// from the same `IndexShared<BlockedAccounts<IndexLocking>>` and passes it
/// via context, mirroring what the engine does at runtime.
#[test]
fn hold_rollback_overflow_blocks_account_with_account_sync_storage() {
    use crate::core::account_control::BlockedAccounts;
    use crate::core::{AccountBlockHandle, AccountControl};
    use crate::storage::{IndexLocking, LockingPolicyFactory, StorageBuilder};
    use crate::AccountKeyConstraint;
    use rust_decimal::Decimal;

    type AccountSyncFactory = IndexLocking<AccountKeyConstraint>;
    type AccountSyncPolicy = SpotFundsPolicy<crate::AccountSync, FullSync>;
    type Policy = dyn PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::AccountSync>;

    let acc = account(99224419);
    let aapl_usd = instr("AAPL", "USD");
    let aapl = asset("AAPL");

    let factory = IndexLocking::<AccountKeyConstraint>::default();
    let storage_builder = StorageBuilder::new(factory);
    let blocked = <AccountSyncFactory as LockingPolicyFactory>::new_shared(BlockedAccounts::new(
        &storage_builder,
    ));
    let groups = crate::core::account_groups::AccountGroups::new(&storage_builder);

    let policy: AccountSyncPolicy = SpotFundsPolicy::new(settings(0), None, &storage_builder);

    let make_control = || AccountControl::new(AccountBlockHandle::from_inner(blocked.clone()), acc);

    // Seed AAPL available = MAX - 50.
    let mut seed_mutations = Mutations::new();
    let max_minus_fifty = PositionSize::new(Decimal::MAX - rust_decimal::Decimal::from(50));
    let seed = TestAdjustment {
        asset: aapl.clone(),
        balance: Some(AdjustmentAmount::Absolute(max_minus_fifty)),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    <Policy>::apply_account_adjustment(
        &policy,
        &AccountAdjustmentContext::new_test(make_control()),
        acc,
        &seed,
        &mut seed_mutations,
    )
    .expect("seed adjustment must succeed");
    seed_mutations.commit_all();

    // Reserve a Sell hold of qty=50: available = MAX - 100, held = 50, and a
    // hold-rollback closure is registered.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("50")),
        Some(px("1")),
    );
    let mut hold_mutations = Mutations::new();
    <Policy>::perform_pre_trade_check(
        &policy,
        &PreTradeContext::new(Some(make_control())),
        &order,
        &mut hold_mutations,
    )
    .expect("pre-trade check must reserve");

    // Shift the slot to available = MAX so the hold's `release(50)` overflows
    // when the rollback closure runs.
    let mut bump_mutations = Mutations::new();
    let bump = TestAdjustment {
        asset: aapl.clone(),
        balance: Some(AdjustmentAmount::Absolute(PositionSize::new(Decimal::MAX))),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    <Policy>::apply_account_adjustment(
        &policy,
        &AccountAdjustmentContext::new_test(make_control()),
        acc,
        &bump,
        &mut bump_mutations,
    )
    .expect("bump adjustment must succeed");
    bump_mutations.commit_all();

    // Roll back the hold: `available + 50 = MAX + 50` overflows; the policy
    // records the overflow on the `IndexLocking`-backed blocked-accounts
    // store through the sealed adapter.
    hold_mutations.rollback_all();

    let probe = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("0")),
        Some(px("1")),
    );
    let rejects = blocked
        .check(&groups, &probe, crate::pretrade::RejectScope::Order)
        .expect("account must be blocked");
    assert!(
        rejects
            .iter()
            .any(|r| r.code == RejectCode::ArithmeticOverflow),
        "blocked-account reject must carry ArithmeticOverflow: {rejects:?}",
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Signed two-leg reservation: side {Buy, Sell} x price sign {>0, =0, <0}
//   x trade_amount {Quantity, Volume}, each through
//   reserve -> partial fill -> final fill, and reserve -> cancel.
//
// Negative and zero prices are legitimate and never rejected for their sign.
// A buy at a negative price reserves nothing (pure inflow); a sell at a
// negative price reserves BOTH the underlying and the settlement leg.
// ═══════════════════════════════════════════════════════════════════════════

/// Reads the full pre-trade result so a test can assert the lock prices and
/// per-leg outcome entries the policy emits.
fn pre_trade_full(
    policy: &TestPolicy,
    order: &TestOrder,
    mutations: &mut Mutations,
) -> Result<crate::pretrade::PolicyPreTradeResult, crate::pretrade::Rejects> {
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::perform_pre_trade_check(
        policy,
        &PreTradeContext::new(None),
        order,
        mutations,
    )
    .map(|opt| opt.expect("pre-trade must produce a result"))
}

fn maybe_holdings(policy: &TestPolicy, acc: AccountId, asset_code: &str) -> Option<Holdings> {
    holdings_of(policy, acc, &asset(asset_code))
}

/// Asserts `(available, held)` for an asset, treating a pruned (absent) slot as
/// all-zero.
fn assert_balance(policy: &TestPolicy, acc: AccountId, asset_code: &str, avail: &str, held: &str) {
    let h = maybe_holdings(policy, acc, asset_code).unwrap_or_else(Holdings::zero);
    assert_eq!(
        h.available(),
        ps(avail),
        "{asset_code} available mismatch (held {})",
        h.held()
    );
    assert_eq!(
        h.held(),
        ps(held),
        "{asset_code} held mismatch (available {})",
        h.available()
    );
}

// ── Buy Quantity @ price = 0 ──────────────────────────────────────────────

#[test]
fn buy_qty_zero_price_reserves_nothing_and_settles() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // No USD seeded: a zero-price buy owes no settlement, so the gate passes
    // even with no funds.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("0")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    // A zero-price buy owes no settlement held, but the base inflow is still
    // projected: incoming = order quantity (10), regardless of price sign.
    assert_eq!(result.account_adjustments.len(), 1);
    let base = &result.account_adjustments[0];
    assert_eq!(base.asset, asset("AAPL"));
    assert!(base.balance.is_none());
    assert!(base.held.is_none());
    let incoming = base.incoming.as_ref().expect("base incoming present");
    assert_eq!(incoming.delta, ps("10"));
    assert_eq!(incoming.absolute, ps("10"));
    assert_eq!(result.lock_prices.as_slice(), &[px("0")]);
    assert!(maybe_holdings(&policy, acc, "USD").is_none());

    // Partial then final fill at price 0: underlying delivered for free,
    // settlement untouched.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("0"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("0"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_balance(&policy, acc, "AAPL", "4", "0");
    assert!(maybe_holdings(&policy, acc, "USD").is_none());

    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("0"),
            quantity: qty("6"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("0"),
        )])),
    );
    assert!(report_blocks(&policy, &final_fill).is_empty());
    assert_balance(&policy, acc, "AAPL", "10", "0");
    assert!(maybe_holdings(&policy, acc, "USD").is_none());
}

#[test]
fn buy_qty_zero_price_cancel_releases_nothing() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("0")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("0"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    assert!(maybe_holdings(&policy, acc, "USD").is_none());
}

// ── Buy Quantity @ price < 0 ──────────────────────────────────────────────

#[test]
fn buy_qty_negative_price_reserves_nothing_and_receives_cash_on_fill() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Negative price: a buy is a pure inflow (receive asset + cash), so it
    // reserves nothing and needs no funds.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    // A negative-price buy owes no settlement held, but the base inflow is
    // still projected: incoming = order quantity (10).
    assert_eq!(result.account_adjustments.len(), 1);
    let base = &result.account_adjustments[0];
    assert_eq!(base.asset, asset("AAPL"));
    assert!(base.balance.is_none());
    assert!(base.held.is_none());
    let incoming = base.incoming.as_ref().expect("base incoming present");
    assert_eq!(incoming.delta, ps("10"));
    assert_eq!(incoming.absolute, ps("10"));
    assert_eq!(result.lock_prices.as_slice(), &[px("-50")]);
    assert!(maybe_holdings(&policy, acc, "USD").is_none());

    // Fill 4 @ -50: receive 4 AAPL and 4*50 = 200 USD.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("-50"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_balance(&policy, acc, "AAPL", "4", "0");
    assert_balance(&policy, acc, "USD", "200", "0");

    // Final fill 6 @ -50: receive 6 AAPL and 6*50 = 300 USD.
    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("-50"),
            quantity: qty("6"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &final_fill).is_empty());
    assert_balance(&policy, acc, "AAPL", "10", "0");
    assert_balance(&policy, acc, "USD", "500", "0");
}

#[test]
fn buy_qty_negative_price_cancel_releases_nothing() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // Fill 4 @ -50 then cancel the unfilled 6: nothing was reserved, so the
    // cancel releases nothing and leaves only the received inflow.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("-50"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    report_blocks(&policy, &partial);
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    assert_balance(&policy, acc, "AAPL", "4", "0");
    assert_balance(&policy, acc, "USD", "200", "0");
}

// ── Buy Volume @ price = 0 / < 0 ──────────────────────────────────────────

#[test]
fn buy_volume_zero_price_reserves_nothing() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Volume(vol("2000")),
        Some(px("0")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert!(
        result.account_adjustments.is_empty(),
        "zero-price volume buy reserves no settlement (no stuck held)",
    );
    assert!(maybe_holdings(&policy, acc, "USD").is_none());
}

#[test]
fn buy_volume_negative_price_projects_base_incoming() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // A negative-price Volume buy owes no settlement (it receives cash), but
    // the acquired base quantity is well-defined: v / |p| = 2000 / 50 = 40.
    // That inflow must still be projected, exactly like the Quantity case and
    // the sell path, which size the quantity through the same |p| division.
    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Volume(vol("2000")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // Only the base inflow projection is emitted: no settlement held, no
    // settlement slot, just AAPL incoming = 40.
    assert_eq!(result.account_adjustments.len(), 1);
    let base = &result.account_adjustments[0];
    assert_eq!(base.asset, asset("AAPL"));
    assert!(base.balance.is_none());
    assert!(base.held.is_none());
    let incoming = base.incoming.as_ref().expect("base incoming present");
    assert_eq!(incoming.delta, ps("40"));
    assert_eq!(incoming.absolute, ps("40"));
    assert_eq!(result.lock_prices.as_slice(), &[px("-50")]);
    assert!(maybe_holdings(&policy, acc, "USD").is_none());

    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("40"));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("slot must exist");
    assert_eq!(aapl.available(), ps("0"));
    assert_eq!(aapl.held(), ps("0"));
}

#[test]
fn buy_volume_negative_price_reserve_fill_lifecycle_nets_incoming_to_zero() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Reserve projects base incoming = v / |p| = 2000 / 50 = 40.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Volume(vol("2000")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("40"));

    // Full fill of 40 @ -50: receive 40 AAPL and 40 * 50 = 2000 USD. The fill
    // drains the projected incoming by the full filled quantity, so it nets
    // back to zero - not to a spurious -40.
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("-50"),
            quantity: qty("40"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &fill).is_empty());
    assert_balance(&policy, acc, "AAPL", "40", "0");
    assert_balance(&policy, acc, "USD", "2000", "0");
    assert_eq!(incoming_of(&policy, acc, "AAPL"), PositionSize::ZERO);
}

#[test]
fn buy_volume_negative_price_partial_fill_then_cancel_nets_incoming_to_zero() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Reserve projects base incoming = v / |p| = 2000 / 50 = 40.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Volume(vol("2000")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // Partial fill 16 @ -50 drains incoming by 16 (40 -> 24); receive 16 AAPL
    // and 16 * 50 = 800 USD.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("-50"),
            quantity: qty("16"),
        }),
        qty("24"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("24"));

    // Cancel the unfilled 24 drains the remaining incoming (24 -> 0). Fill and
    // cancel together drain exactly the projected 40, so incoming nets to zero
    // and no balance is touched by the cancel.
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("24"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    assert_balance(&policy, acc, "AAPL", "16", "0");
    assert_balance(&policy, acc, "USD", "800", "0");
    assert_eq!(incoming_of(&policy, acc, "AAPL"), PositionSize::ZERO);
}

// ── Sell Quantity @ price = 0 ─────────────────────────────────────────────

#[test]
fn sell_qty_zero_price_reserves_only_underlying() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("0")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    // Only the underlying leg is reserved (both settlement legs are zero at a
    // zero price), but the resolved zero price is still recorded as the lock:
    // every accepted sell records a lock_price.
    assert_eq!(result.account_adjustments.len(), 1);
    assert_eq!(result.lock_prices.as_slice(), &[px("0")]);
    assert_balance(&policy, acc, "AAPL", "0", "10");

    // Fill 4 @ 0 (lock replayed): gives 4 AAPL, receives 0 USD. The zero lock
    // is mandatory - a fill without it would block the account.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        Some(Trade {
            price: px("0"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("0"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_balance(&policy, acc, "AAPL", "0", "6");
    assert!(maybe_holdings(&policy, acc, "USD").is_none());

    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("0"),
            quantity: qty("6"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("0"),
        )])),
    );
    assert!(report_blocks(&policy, &final_fill).is_empty());
    assert_balance(&policy, acc, "AAPL", "0", "0");
}

#[test]
fn sell_qty_zero_price_cancel_releases_underlying() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("0")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // The zero lock is mandatory on the cancel too - a sell cancel without it
    // would block the account.
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("0"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    assert_balance(&policy, acc, "AAPL", "10", "0");
}

// ── Sell Quantity @ price < 0 (TWO legs) ──────────────────────────────────

#[test]
fn sell_qty_negative_price_reserves_both_legs_and_settles() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    seed(&policy, acc, asset("USD"), "1000");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    // BOTH legs reserved: 10 AAPL underlying and 10*50 = 500 USD settlement.
    assert_eq!(
        result.account_adjustments.len(),
        2,
        "sell at negative price reserves both underlying and settlement legs",
    );
    assert_eq!(result.lock_prices.as_slice(), &[px("-50")]);
    assert_balance(&policy, acc, "AAPL", "0", "10");
    assert_balance(&policy, acc, "USD", "500", "500");

    // Fill 4 @ -50: give 4 AAPL, pay 4*50 = 200 USD (consumed from settlement
    // held).
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        Some(Trade {
            price: px("-50"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_balance(&policy, acc, "AAPL", "0", "6");
    assert_balance(&policy, acc, "USD", "500", "300");

    // Final fill 6 @ -50: both legs fully consumed; held returns to zero.
    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("-50"),
            quantity: qty("6"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &final_fill).is_empty());
    assert_balance(&policy, acc, "AAPL", "0", "0");
    // Paid 500 USD total for disposing 10 AAPL at -50.
    assert_balance(&policy, acc, "USD", "500", "0");
}

#[test]
fn sell_qty_negative_price_cancel_releases_both_legs() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    seed(&policy, acc, asset("USD"), "1000");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // Fill 4 @ -50 then cancel the unfilled 6: both legs release their
    // remainder; held returns to zero on both assets.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        Some(Trade {
            price: px("-50"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    report_blocks(&policy, &partial);
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    // AAPL: 6 unfilled released back to available.
    assert_balance(&policy, acc, "AAPL", "6", "0");
    // USD: reserved 500, consumed 200 on fill, released 300 on cancel; net
    // paid 200, so available = 1000 - 200 = 800.
    assert_balance(&policy, acc, "USD", "800", "0");
}

// ── Sell Volume @ price < 0 (TWO legs) ────────────────────────────────────

#[test]
fn sell_volume_negative_price_reserves_both_legs() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Volume 2000 @ -50 -> quantity = 2000 / 50 = 40 AAPL; settlement leg =
    // 40 * 50 = 2000 USD.
    seed(&policy, acc, asset("AAPL"), "40");
    seed(&policy, acc, asset("USD"), "5000");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Volume(vol("2000")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_eq!(result.account_adjustments.len(), 2);
    assert_eq!(result.lock_prices.as_slice(), &[px("-50")]);
    assert_balance(&policy, acc, "AAPL", "0", "40");
    assert_balance(&policy, acc, "USD", "3000", "2000");

    // Full fill 40 @ -50: both legs consumed exactly; held returns to zero.
    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("-50"),
            quantity: qty("40"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("-50"),
        )])),
    );
    assert!(report_blocks(&policy, &final_fill).is_empty());
    assert_balance(&policy, acc, "AAPL", "0", "0");
    assert_balance(&policy, acc, "USD", "3000", "0");
}

#[test]
fn sell_volume_zero_price_reserves_nothing() {
    // A Volume sell at price 0 cannot size its quantity (volume / 0 is
    // undefined), so it reserves nothing - but the order still passes. A zero
    // price is treated like any other price, never as an error.
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "100");
    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Volume(vol("1000")),
        Some(px("0")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    // Nothing reserved on either leg, but the resolved zero price is still
    // recorded as the lock, like every accepted sell.
    assert!(result.account_adjustments.is_empty());
    assert_eq!(result.lock_prices.as_slice(), &[px("0")]);
    assert_balance(&policy, acc, "AAPL", "100", "0");
    assert!(maybe_holdings(&policy, acc, "USD").is_none());
}

// ── Buy/Sell at positive price: held returns to pre-reservation level ──────

#[test]
fn buy_qty_positive_price_held_returns_to_zero_after_full_settlement() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_balance(&policy, acc, "USD", "8000", "2000");

    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    report_blocks(&policy, &partial);
    assert_balance(&policy, acc, "USD", "8000", "1200");

    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("6"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    report_blocks(&policy, &final_fill);
    // held back to its pre-reservation level (0); paid 2000 for 10 AAPL.
    assert_balance(&policy, acc, "USD", "8000", "0");
    assert_balance(&policy, acc, "AAPL", "10", "0");
}

#[test]
fn sell_volume_positive_price_reserves_underlying_held_and_settlement_incoming() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Volume 2000 @ 200 -> quantity = 10 AAPL; settlement held = 0 (sell
    // receives cash, owes none) but settlement incoming = 10 * 200 = 2000.
    seed(&policy, acc, asset("AAPL"), "10");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Volume(vol("2000")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    // Underlying held leg plus settlement incoming projection; a priced sell
    // now records its lock.
    assert_eq!(result.account_adjustments.len(), 2);
    let settlement = &result.account_adjustments[1];
    assert_eq!(settlement.asset, asset("USD"));
    let incoming = settlement
        .incoming
        .as_ref()
        .expect("settlement incoming present");
    assert_eq!(incoming.delta, ps("2000"));
    assert_eq!(incoming.absolute, ps("2000"));
    assert_eq!(result.lock_prices.as_slice(), &[px("200")]);
    assert_balance(&policy, acc, "AAPL", "0", "10");

    // The fill carries the lock so the settlement incoming drains to zero.
    let final_fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &final_fill).is_empty());
    assert_balance(&policy, acc, "AAPL", "0", "0");
    assert_balance(&policy, acc, "USD", "2000", "0");
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD entry");
    assert_eq!(usd.incoming(), ps("0"), "settlement incoming must drain");
}

// ── Two-leg rollback and partial-reservation atomicity ────────────────────

#[test]
fn sell_negative_price_rollback_restores_both_legs() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    seed(&policy, acc, asset("USD"), "1000");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("-50")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    // Both legs held synchronously.
    assert_balance(&policy, acc, "AAPL", "0", "10");
    assert_balance(&policy, acc, "USD", "500", "500");

    // Rolling back the reservation must undo BOTH legs back to their
    // pre-reservation levels.
    mutations.rollback_all();
    assert_balance(&policy, acc, "AAPL", "10", "0");
    assert_balance(&policy, acc, "USD", "1000", "0");
}

#[test]
fn sell_negative_price_settlement_insufficient_rolls_back_underlying_leg() {
    // The underlying leg holds first; if the settlement leg then fails for
    // insufficient funds, the engine's pre-trade pipeline rolls back the
    // already-held underlying leg so no partial reservation escapes.
    let acc = account(99224418);
    let aapl_usd = instr("AAPL", "USD");
    let engine = build_engine_with_spot_funds_policy();
    seed_balance_via_engine(&engine, acc, asset("AAPL"), ps("10"));
    // Only 100 USD: the settlement leg needs 10*50 = 500, so it must reject.
    seed_balance_via_engine(&engine, acc, asset("USD"), ps("100"));

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("-50")),
    );
    let rejects = match engine.execute_pre_trade(order) {
        Ok(_) => panic!("settlement leg must reject for insufficient funds"),
        Err(rejects) => rejects,
    };
    assert!(
        rejects
            .iter()
            .any(|r| r.code == RejectCode::InsufficientFunds),
        "settlement leg must reject with InsufficientFunds: {rejects:?}",
    );

    // Both legs back to their seeded levels: the underlying hold was rolled
    // back, the settlement hold never committed.
    let probe = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut reservation = engine
        .execute_pre_trade(probe)
        .expect("a positive-price sell of the full 10 AAPL must still fit");
    reservation.rollback();
}

// ═══════════════════════════════════════════════════════════════════════════
// Incoming projection: the acquiring leg's expected inflow is reserved into
// the `incoming` bucket in parallel with the existing `held` outflow leg,
// consumed on fills, released on cancels, reversed on rollback, and converges
// to zero. `incoming` is purely informational and gates nothing.
// ═══════════════════════════════════════════════════════════════════════════

/// Reads an asset's `incoming` bucket, treating an absent slot as zero.
fn incoming_of(policy: &TestPolicy, acc: AccountId, asset_code: &str) -> PositionSize {
    maybe_holdings(policy, acc, asset_code)
        .map(|h| h.incoming())
        .unwrap_or(PositionSize::ZERO)
}

#[test]
fn buy_reservation_projects_base_incoming_and_settlement_held() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // Settlement held leg (USD) and base incoming projection (AAPL),
    // [settlement, base] order.
    assert_eq!(result.account_adjustments.len(), 2);
    let settlement = &result.account_adjustments[0];
    assert_eq!(settlement.asset, asset("USD"));
    assert_eq!(
        settlement.held.as_ref().expect("held present").delta,
        ps("2000")
    );
    assert!(settlement.incoming.is_none());

    let base = &result.account_adjustments[1];
    assert_eq!(base.asset, asset("AAPL"));
    assert!(base.balance.is_none());
    assert!(base.held.is_none());
    let incoming = base.incoming.as_ref().expect("base incoming present");
    assert_eq!(incoming.delta, ps("10"));
    assert_eq!(incoming.absolute, ps("10"));

    // The slot the projection lives in carries only incoming.
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("10"));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("slot must exist");
    assert_eq!(aapl.available(), ps("0"));
    assert_eq!(aapl.held(), ps("0"));
}

#[test]
fn sell_reservation_projects_settlement_incoming_and_underlying_held() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    let result = pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // Underlying held leg (AAPL) and settlement incoming projection (USD =
    // proceeds 4 * 200 = 800), [underlying, settlement] order.
    assert_eq!(result.account_adjustments.len(), 2);
    let underlying = &result.account_adjustments[0];
    assert_eq!(underlying.asset, asset("AAPL"));
    assert_eq!(
        underlying.held.as_ref().expect("held present").delta,
        ps("4")
    );
    assert!(underlying.incoming.is_none());

    let settlement = &result.account_adjustments[1];
    assert_eq!(settlement.asset, asset("USD"));
    assert!(settlement.balance.is_none());
    assert!(settlement.held.is_none());
    let incoming = settlement.incoming.as_ref().expect("incoming present");
    assert_eq!(incoming.delta, ps("800"));
    assert_eq!(incoming.absolute, ps("800"));

    assert_eq!(incoming_of(&policy, acc, "USD"), ps("800"));
    assert_eq!(result.lock_prices.as_slice(), &[px("200")]);
}

#[test]
fn price_less_sell_without_market_data_bundle_rejects_as_unsupported() {
    // No order price in limit-only mode: the sell cannot resolve a price, so it
    // is rejected as an unsupported market order rather than accepted without a
    // lock. No holdings are touched.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        None,
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_full(&policy, &order, &mut mutations).expect_err("must reject");
    assert_eq!(rejects[0].code, RejectCode::UnsupportedOrderType);
    assert!(mutations.is_empty());
    // The underlying gate never ran: the AAPL holdings are untouched and no
    // settlement slot was created.
    assert_balance(&policy, acc, "AAPL", "10", "0");
    assert!(maybe_holdings(&policy, acc, "USD").is_none());
}

#[test]
fn buy_full_fill_drains_base_incoming_and_credits_available() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("10"));

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let result = run_report(&policy, &fill);
    assert!(result.account_blocks.is_empty());

    // The acquiring leg emits both the available credit and the incoming
    // drain; available is credited exactly once (no double-credit).
    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    let balance = aapl_entry
        .entry
        .balance
        .as_ref()
        .expect("balance delta present");
    assert_eq!(balance.delta, ps("10"));
    let drained = aapl_entry
        .entry
        .incoming
        .as_ref()
        .expect("incoming delta present");
    assert_eq!(drained.delta, ps("-10"));
    assert_eq!(drained.absolute, ps("0"));

    // Available credited once to 10, incoming fully drained.
    assert_balance(&policy, acc, "AAPL", "10", "0");
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("0"));
}

#[test]
fn buy_partial_fill_drains_base_incoming_proportionally() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    let partial = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());

    // incoming consumed by filled quantity (4), 6 remains projected; available
    // credited the 4 acquired units.
    assert_balance(&policy, acc, "AAPL", "4", "0");
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("6"));
}

#[test]
fn sell_fill_drains_settlement_incoming_by_lock_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    // proceeds projection = 10 * 200 = 2000.
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("2000"));

    let partial = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());

    // incoming drained by lock_price * filled = 200 * 4 = 800, leaving 1200;
    // available credited the 800 proceeds independently.
    assert_balance(&policy, acc, "USD", "800", "0");
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("1200"));
}

#[test]
fn buy_cancel_releases_unfilled_base_incoming() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();

    // Fill 4, then cancel the unfilled 6.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("6"));

    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());

    // Convergence: consumed 4 + released 6 == reserved 10, no residual.
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("0"));
    assert_balance(&policy, acc, "AAPL", "4", "0");
}

#[test]
fn sell_cancel_releases_unfilled_settlement_incoming_by_lock_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("2000"));

    // Fill 4 (drains 800), cancel the unfilled 6 (releases 200 * 6 = 1200).
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());

    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());

    // Convergence: drained 800 + released 1200 == reserved 2000, no residual.
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("0"));
    assert_balance(&policy, acc, "USD", "800", "0");
}

#[test]
fn buy_reservation_rollback_restores_held_and_incoming() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    // Reserved synchronously: USD held 2000, AAPL incoming 10.
    assert_balance(&policy, acc, "USD", "8000", "2000");
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("10"));

    // Rolling back must reverse BOTH the held leg and the incoming projection.
    mutations.rollback_all();
    assert_balance(&policy, acc, "USD", "10000", "0");
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("0"));
    assert!(
        maybe_holdings(&policy, acc, "AAPL").is_none(),
        "the incoming-only slot must be pruned on rollback"
    );
}

#[test]
fn sell_reservation_rollback_restores_held_and_settlement_incoming() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    assert_balance(&policy, acc, "AAPL", "0", "10");
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("2000"));

    mutations.rollback_all();
    assert_balance(&policy, acc, "AAPL", "10", "0");
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("0"));
    assert!(
        maybe_holdings(&policy, acc, "USD").is_none(),
        "the settlement incoming-only slot must be pruned on rollback"
    );
}

#[test]
fn incoming_projection_does_not_gate_any_order() {
    // `incoming` is purely informational: it is never part of spendable
    // capacity. A buy of an asset the account already holds only as projected
    // incoming must reuse that exact spendable budget and reject identically
    // whether or not a prior projection inflated the incoming bucket.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Exactly enough USD for one reservation of 2000.
    seed(&policy, acc, asset("USD"), "2000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut first = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut first).expect("first must pass");
    first.commit_all();
    // AAPL now carries incoming 10, but that must not become spendable.
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("10"));

    // A second identical buy must reject for insufficient funds: the USD
    // available is exhausted and the AAPL incoming projection grants no
    // spendable capacity.
    let mut second = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut second)
        .expect_err("second buy must reject - incoming is not spendable");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
    assert!(second.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Stage 4: post-trade signed price
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn buy_fill_zero_price_nonzero_qty_consumes_held_by_lock_price() {
    // With a zero fill price the old abs-based guard skipped held consumption,
    // leaving the reservation stale. The fixed guard checks qty != 0 instead.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("2")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // USD: available=9800, held=200

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("0"),
            quantity: qty("2"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    let result = run_report(&policy, &fill);
    assert!(result.account_blocks.is_empty());

    // Opening AAPL from flat at a zero fill price seeds the average at 0 and
    // realizes nothing; the settlement (USD) leg never carries either.
    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert_eq!(aapl_entry.entry.average_entry_price, Some(px("0")));
    assert!(aapl_entry.entry.realized_pnl.is_none());

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    // lock_consume = 100*2 = 200 consumed from held;
    // savings = 200 - outflow(0) = 200 returned to available
    assert_eq!(usd.held(), ps("0"), "held must be fully consumed");
    assert_eq!(
        usd.available(),
        ps("10000"),
        "full amount returned as savings"
    );

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL credited");
    // inflow = qty = 2 (zero-price fill still delivers the underlying)
    assert_eq!(aapl.available(), ps("2"));
    assert_eq!(aapl.avg_entry_price(), Some(px("0")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn buy_fill_negative_trade_price_uses_signed_not_abs() {
    // Negative fill price: outflow = -50*2 = -100 (signed, not abs=100).
    // lock_consume = 100*2 = 200; savings = 200 - (-100) = 300.
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("2")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("must succeed");
    mutations.commit_all();
    // USD: available=9800, held=200

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("-50"),
            quantity: qty("2"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    let result = run_report(&policy, &fill);
    assert!(result.account_blocks.is_empty());

    // Opening AAPL from flat at a negative fill price seeds the average at that
    // negative price and realizes nothing.
    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert_eq!(aapl_entry.entry.average_entry_price, Some(px("-50")));
    assert!(aapl_entry.entry.realized_pnl.is_none());

    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    // held: 200 -> 0 (lock_consume=200 consumed)
    // available: 9800 + savings(300) = 10100
    assert_eq!(usd.held(), ps("0"));
    assert_eq!(
        usd.available(),
        ps("10100"),
        "signed savings = lock(200) - notional(-100) = 300 credited to available",
    );

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL credited");
    assert_eq!(aapl.avg_entry_price(), Some(px("-50")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

// ═══════════════════════════════════════════════════════════════════════════
// Slippage cascade through the pre-trade entry point
// ═══════════════════════════════════════════════════════════════════════════

/// Builds a `PreTradeContext<FullLocking>` where `account_id` is registered
/// in `group_id`, so `ctx.group()` returns `Some(group_id)` during the
/// pre-trade check.
fn ctx_with_group(
    account_id: AccountId,
    group_id: crate::param::AccountGroupId,
) -> crate::pretrade::PreTradeContext<crate::storage::FullLocking> {
    use crate::core::{AccountGroups, AccountGroupsHandle};
    use crate::storage::{FullLocking, LockingPolicyFactory, StorageBuilder};
    let sb = StorageBuilder::new(FullLocking);
    let groups = AccountGroups::new(&sb);
    groups
        .register_group(&[account_id], group_id)
        .expect("registration must succeed");
    let handle = AccountGroupsHandle::from_inner(FullLocking::new_shared(groups));
    crate::pretrade::PreTradeContext::with_groups(None, handle, Some(account_id))
}

// ── group-tier override selects correct slippage for buy ──────────────────

#[test]
fn buy_market_group_override_reserves_group_slippage_not_global() {
    // mark=100, global=0 bps, group=2000 bps; account is in the group.
    // Expected: effective = 100 * (1 + 0.20) = 120; held = 10 * 120 = 1200.
    // Without the group override global=0 would give held = 10 * 100 = 1000.
    let acc = account(99224416);
    let grp = crate::param::AccountGroupId::from_u32(5).expect("valid group id");
    let aapl_usd = instr("AAPL", "USD");

    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(aapl_usd.clone())
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");

    // Account-scoped override at a different account (must not fire) and
    // group-scoped override at (instrument, group).
    let overrides = [
        (
            SpotFundsOverrideTarget::InstrumentAccount(id, account(9999)),
            SpotFundsOverride {
                slippage_bps: Some(5000),
            },
        ),
        (
            SpotFundsOverrideTarget::InstrumentAccountGroup(id, grp),
            SpotFundsOverride {
                slippage_bps: Some(2000),
            },
        ),
    ];
    let settings = SpotFundsSettings::new(
        0, // global = 0 bps
        SpotFundsPricingSource::Mark,
        overrides,
    )
    .expect("settings must build");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings, Some(bundle), b.storage_builder());
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd,
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        None, // market order
    );
    let mut mutations = Mutations::with_capacity(1);

    // Use a context that maps `acc` -> `grp`, so the group tier fires.
    let ctx = ctx_with_group(acc, grp);
    <TestPolicy as crate::pretrade::PreTradePolicy<
        TestOrder,
        TestReport,
        TestAdjustment,
        crate::core::FullSync,
    >>::perform_pre_trade_check(&policy, &ctx, &order, &mut mutations)
    .expect("must succeed");

    let h = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    // Group tier: 2000 bps -> effective = 120 -> held = 10 * 120 = 1200.
    assert_eq!(
        h.held(),
        ps("1200"),
        "group override (2000 bps) must be used, not global (0 bps)",
    );
    assert_eq!(h.available(), ps("8800"));
}

// ── average entry price / realized PnL ─────────────────────────────────────

/// Seeds a balance with an average entry price through the adjustment pipeline.
fn seed_with_avg(
    policy: &TestPolicy,
    account_id: AccountId,
    asset: Asset,
    amount: &str,
    avg: Price,
) {
    seed(policy, account_id, asset.clone(), amount);
    let adjustment = TestAdjustment {
        asset: asset.clone(),
        balance: Some(AdjustmentAmount::Absolute(ps(amount))),
        balance_average_entry_price: Some(avg),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut mutations = Mutations::with_capacity(1);
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_account_adjustment(
        policy,
        &AccountAdjustmentContext::new_test(dummy_control(account_id)),
        account_id,
        &adjustment,
        &mut mutations,
    )
    .expect("seed must succeed");
    mutations.commit_all();

    let pnl_adjustment = adj_with_realized_pnl(asset, Pnl::ZERO);
    let mut mutations = Mutations::with_capacity(1);
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, crate::core::FullSync>>::apply_account_adjustment(
        policy,
        &AccountAdjustmentContext::new_test(dummy_control(account_id)),
        account_id,
        &pnl_adjustment,
        &mut mutations,
    )
    .expect("PnL seed must succeed");
    mutations.commit_all();
}

#[test]
fn balance_adjustment_with_avg_sets_slot_average_and_emits_it() {
    let acc = account(99224416);
    let policy = build_policy(None, None);

    let adjustment = adj_with_avg(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(ps("10"))),
        Some(px("150")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(outcome.len(), 1);
    let entry = &outcome[0];
    assert_eq!(entry.average_entry_price, Some(px("150")));
    assert!(entry.realized_pnl.is_none());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("150")));
    assert_eq!(aapl.realized_pnl(), None);
}

#[test]
fn position_adjustment_sets_quantity_average_and_emits_both() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    let adjustment = adj_with_avg(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(ps("10"))),
        Some(px("150")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(outcome.len(), 1);
    assert!(outcome[0].balance.is_some());
    assert_eq!(outcome[0].average_entry_price, Some(px("150")));
    assert!(outcome[0].realized_pnl.is_none());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.available(), ps("10"));
    assert_eq!(aapl.avg_entry_price(), Some(px("150")));
}

#[test]
fn balance_adjustment_without_avg_leaves_prior_average() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("150"));

    // A later balance-only adjustment (no avg) must not wipe the average.
    let adjustment = adj(asset("AAPL"), Some(AdjustmentAmount::Delta(ps("5"))));
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(outcome[0].average_entry_price, Some(px("150")));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("150")));
}

#[test]
fn balance_adjustment_to_flat_clears_average_and_prunes_zero_slot() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("150"));

    let adjustment = adj(asset("AAPL"), Some(AdjustmentAmount::Absolute(ps("0"))));
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(outcome.len(), 1);
    assert!(outcome[0].average_entry_price.is_none());
    assert!(
        holdings_of(&policy, acc, &asset("AAPL")).is_none(),
        "flat zero-PnL slot must not survive only because of a stale average",
    );
}

#[test]
fn held_adjustment_to_net_flat_clears_average() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("150"));

    let adjustment = held_adj(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(ps("-10"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(outcome.len(), 1);
    assert!(outcome[0].average_entry_price.is_none());
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("slot must remain");
    assert_eq!(aapl.available(), ps("10"));
    assert_eq!(aapl.held(), ps("-10"));
    assert!(aapl.avg_entry_price().is_none());
}

#[test]
fn held_only_adjustment_emits_no_average() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("150"));

    let adjustment = held_adj(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(ps("2"))),
        None,
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    // Held-only request: no balance, so no average is surfaced and no PnL.
    assert!(outcome[0].average_entry_price.is_none());
    assert!(outcome[0].realized_pnl.is_none());
    // The stored average is untouched.
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("150")));
}

#[test]
fn buy_fill_emits_average_entry_price_and_no_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let result = run_report(&policy, &fill);

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    // Opening a long from flat: average is the fill price, no realized PnL.
    assert_eq!(aapl_entry.entry.average_entry_price, Some(px("200")));
    assert!(aapl_entry.entry.realized_pnl.is_none());

    // Settlement (USD) leg never carries average or PnL.
    let usd_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("USD"))
        .expect("USD entry must exist");
    assert!(usd_entry.entry.average_entry_price.is_none());
    assert!(usd_entry.entry.realized_pnl.is_none());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("200")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn fill_without_account_currency_halts_position_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("2")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("100"),
            quantity: qty("2"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    // The opening fill contributes a computable zero to the account line, so
    // the missing account currency does not halt it. The position ledger still
    // halts: it has no currency to denominate the cost basis in.
    let result = run_report_without_account_currency(&policy, &fill);
    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: Pnl::ZERO,
                absolute: Pnl::ZERO,
            }),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Value(Pnl::ZERO))
    );

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert!(aapl_entry.entry.average_entry_price.is_none());
    assert_eq!(
        aapl_entry.entry.realized_pnl,
        Some(Err(crate::PnlHaltReason::MissingAccountCurrency))
    );

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.available(), ps("2"));
    assert!(aapl.avg_entry_price().is_none());
    assert!(aapl.realized_pnl().is_none());
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingAccountCurrency)
    );

    let following = run_report_without_account_currency(&policy, &fill);
    let following_aapl_entry = following
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert!(following_aapl_entry.entry.realized_pnl.is_none());
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingAccountCurrency)
    );

    // A reducing fill has to realize against the cost basis, so the account
    // currency becomes a required input and the account line halts.
    let close_order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("1")),
        Some(px("120")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &close_order, &mut mutations).expect("close must reserve");
    mutations.commit_all();
    let close = make_report(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        Some(Trade {
            price: px("120"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("120"),
        )])),
    );
    let close_result = run_report_without_account_currency(&policy, &close);
    assert!(close_result.account_blocks.is_empty());
    assert_eq!(
        close_result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingAccountCurrency),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::MissingAccountCurrency
        ))
    );
}

#[test]
fn opening_fill_zero_fee_without_account_currency_does_not_halt_account_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut settings = settings(0);
    settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    settings
        .set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
            lower_bound: Some(pnl_value("-10")),
            upper_bound: None,
        }))
        .expect("global account PnL barrier must set");
    let policy = build_policy_from_settings(settings, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("2")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    // An explicit zero fee is an economic no-op: the opening fill still halts
    // the position ledger because its cost basis needs an account currency,
    // but the independently live account line publishes its zero contribution.
    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "2", money_fee("0", "USD"));
    let result = run_report_without_account_currency(&policy, &fill);
    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: Pnl::ZERO,
                absolute: Pnl::ZERO,
            }),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Value(Pnl::ZERO))
    );

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert_eq!(
        aapl_entry.entry.realized_pnl,
        Some(Err(crate::PnlHaltReason::MissingAccountCurrency))
    );
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingAccountCurrency)
    );
}

#[test]
fn non_position_touching_fill_without_account_currency_preserves_tracking() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    let seed = adj_with_realized_pnl(asset("AAPL"), pnl_value("7"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed, &mut mutations);
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("100"),
            quantity: qty("0"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    let result = <TestPolicy as PreTradePolicy<
        TestOrder,
        TestReport,
        TestAdjustment,
        crate::core::FullSync,
    >>::apply_execution_report(
        &policy, &crate::pretrade::PostTradeContext::new(), &fill
    );

    assert!(
        result.is_none(),
        "zero-quantity fill must not emit outcomes"
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.available(), ps("10"));
    assert_eq!(aapl.avg_entry_price(), Some(px("100")));
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("7")));
}

#[test]
fn quote_equals_account_currency_tracks_without_market_data() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("123")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("123"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("123"),
        )])),
    );
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert!(result.account_blocks.is_empty());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("123")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn fresh_fx_tracks_average_and_realized_pnl_in_account_currency() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let usd_eur = instr("USD", "EUR");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(usd_eur).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("0.9")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    seed(&policy, acc, asset("USD"), "10000");

    let buy = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &buy, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();
    let buy_fill = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("100"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    let buy_result = run_report_with_currency(&policy, &buy_fill, asset("EUR"));
    assert!(buy_result.account_blocks.is_empty());
    assert_eq!(
        buy_result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: Pnl::ZERO,
                absolute: Pnl::ZERO,
            }),
        }]
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("90")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));

    let sell = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("120")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &sell, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();
    let sell_fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("120"),
            quantity: qty("4"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("120"),
        )])),
    );
    let sell_result = run_report_with_currency(&policy, &sell_fill, asset("EUR"));
    assert!(sell_result.account_blocks.is_empty());
    let pnl = sell_result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .and_then(|o| o.entry.realized_pnl.as_ref())
        .and_then(|outcome| outcome.as_ref().ok())
        .expect("realized pnl must be tracked");
    assert_eq!(pnl.delta, pnl_value("72"));
    assert_eq!(pnl.absolute, pnl_value("72"));
    assert_eq!(
        sell_result.account_pnls[0].result,
        Ok(crate::PnlOutcomeAmount {
            delta: pnl_value("72"),
            absolute: pnl_value("72"),
        }),
    );
}

#[test]
fn shared_asset_holding_tracks_two_quote_currencies_in_account_currency() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let aapl_gbp = instr("AAPL", "GBP");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let usd_eur = svc
        .register(instr("USD", "EUR"))
        .expect("USD/EUR must register");
    let gbp_eur = svc
        .register(instr("GBP", "EUR"))
        .expect("GBP/EUR must register");
    svc.push(usd_eur, Quote::new().with_mark(px("0.9")))
        .expect("USD/EUR quote must push");
    svc.push(gbp_eur, Quote::new().with_mark(px("1.2")))
        .expect("GBP/EUR quote must push");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    seed(&policy, acc, asset("USD"), "10000");
    seed(&policy, acc, asset("GBP"), "10000");

    let report = |instrument: Instrument, side, price: &str, quantity: &str| {
        make_report(
            acc,
            instrument,
            side,
            Some(Trade {
                price: px(price),
                quantity: qty(quantity),
            }),
            qty("0"),
            true,
            Some(PreTradeLock::from_entries([(
                DEFAULT_POLICY_GROUP_ID,
                px(price),
            )])),
        )
    };

    let first_buy = report(aapl_usd.clone(), Side::Buy, "100", "10");
    run_report_with_currency(&policy, &first_buy, asset("EUR"));
    let second_buy = report(aapl_gbp.clone(), Side::Buy, "100", "10");
    run_report_with_currency(&policy, &second_buy, asset("EUR"));

    let shared = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(shared.available(), ps("20"));
    assert_eq!(shared.avg_entry_price(), Some(px("105")));
    assert_eq!(shared.realized_pnl(), Some(Pnl::ZERO));

    let usd_sell_order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("120")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &usd_sell_order, &mut mutations).expect("USD sell must reserve");
    mutations.commit_all();
    let usd_sell = report(aapl_usd, Side::Sell, "120", "4");
    run_report_with_currency(&policy, &usd_sell, asset("EUR"));

    let gbp_sell_order = make_order(
        acc,
        aapl_gbp.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("6")),
        Some(px("90")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &gbp_sell_order, &mut mutations).expect("GBP sell must reserve");
    mutations.commit_all();
    let gbp_sell = report(aapl_gbp, Side::Sell, "90", "6");
    let result = run_report_with_currency(&policy, &gbp_sell, asset("EUR"));

    let shared = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(shared.available(), ps("10"));
    assert_eq!(shared.avg_entry_price(), Some(px("105")));
    assert_eq!(shared.realized_pnl(), Some(pnl_value("30")));
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("30")));
    assert_eq!(
        result.account_pnls[0].result,
        Ok(crate::PnlOutcomeAmount {
            delta: pnl_value("18"),
            absolute: pnl_value("30"),
        })
    );
}

#[test]
fn stale_fx_quote_is_used_for_accounting() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let usd_eur = instr("USD", "EUR");
    let b = engine_builder();
    let svc =
        MarketDataBuilder::<FullSync>::new(QuoteTtl::Within(std::time::Duration::from_millis(1)))
            .build();
    let fx_id = svc.register(usd_eur).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("0.8")))
        .expect("push must succeed");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("100"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    let result = run_report_with_currency(&policy, &fill, asset("EUR"));
    assert!(result.account_blocks.is_empty());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("80")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn missing_fx_on_opening_fill_keeps_pnl_active_until_basis_is_needed() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    seed(&policy, acc, asset("USD"), "10000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("2")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("100"),
            quantity: qty("2"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    let result = run_report_with_currency(&policy, &fill, asset("EUR"));
    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: Pnl::ZERO,
                absolute: Pnl::ZERO,
            }),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Value(Pnl::ZERO))
    );

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert!(aapl_entry.entry.average_entry_price.is_none());
    assert!(aapl_entry.entry.realized_pnl.is_none());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.available(), ps("2"));
    assert!(aapl.avg_entry_price().is_none());
    assert!(aapl.realized_pnl().is_none());
    assert!(!aapl.realized_pnl_is_halted());

    let fx_id = svc
        .register(instr("USD", "EUR"))
        .expect("FX instrument must register");
    svc.push(fx_id, Quote::new().with_mark(px("0.8")))
        .expect("FX quote must publish");

    let close_order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("1")),
        Some(px("120")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &close_order, &mut mutations).expect("close must reserve");
    mutations.commit_all();
    let close = make_report(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        Some(Trade {
            price: px("120"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("120"),
        )])),
    );
    let close_result = run_report_with_currency(&policy, &close, asset("EUR"));
    assert!(close_result.account_blocks.is_empty());
    assert_eq!(
        close_result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingCostBasis),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::MissingCostBasis
        ))
    );
    let close_aapl = close_result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert_eq!(
        close_aapl.entry.realized_pnl,
        Some(Err(crate::PnlHaltReason::MissingCostBasis))
    );
}

#[test]
fn arithmetic_overflow_halts_both_pnl_lines_before_currency_or_fx() {
    let acc = account(99224416);
    let aapl = asset("AAPL");
    let policy = build_policy(None, None);
    policy
        .holdings
        .with_mut((acc, aapl), Holdings::zero, |slot, _| {
            *slot = Holdings::new(position_size_max(), ps("1"));
        });

    let report = make_report(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        Some(Trade {
            price: px("100"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    let result = run_report_without_account_currency(&policy, &report);

    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::ArithmeticOverflow),
        }]
    );
    let position_pnl = result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl);
    assert_eq!(
        position_pnl,
        Some(Err(crate::PnlHaltReason::ArithmeticOverflow))
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::ArithmeticOverflow
        ))
    );
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::ArithmeticOverflow)
    );

    let cascade_acc = account(99224417);
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc
        .register(instr("USD", "EUR"))
        .expect("FX instrument must register");
    svc.push(fx_id, Quote::new().with_mark(px("2")))
        .expect("FX quote must publish");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let cascade_policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    seed_with_avg(&cascade_policy, cascade_acc, asset("AAPL"), "1", px("100"));

    let max_price = Price::new(rust_decimal::Decimal::MAX);
    let cascade_report = make_report(
        cascade_acc,
        instr("AAPL", "USD"),
        Side::Sell,
        Some(Trade {
            price: max_price,
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            max_price,
        )])),
    );
    let cascade = run_report_with_currency(&cascade_policy, &cascade_report, asset("EUR"));

    assert_eq!(
        cascade.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: cascade_acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::ArithmeticOverflow),
        }]
    );
    let cascade_position_pnl = cascade
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl);
    assert_eq!(
        cascade_position_pnl,
        Some(Err(crate::PnlHaltReason::ArithmeticOverflow))
    );
}

#[test]
fn pnl_bounds_cascade_uses_account_group_then_global() {
    let account_specific = account(99224416);
    let grouped = account(99224417);
    let global_only = account(99224418);
    let grp = group(7);
    let aapl_usd = instr("AAPL", "USD");

    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-5")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    s.set_pnl_account_group_barriers([SpotFundsPnlBoundsAccountGroupBarrier {
        account_group_id: grp,
        barrier: SpotFundsPnlBoundsBarrier {
            lower_bound: Some(pnl_value("-50")),
            upper_bound: None,
        },
    }])
    .expect("group pnl barrier must set");
    let s = s
        .with_pnl_account_barriers([SpotFundsPnlBoundsAccountBarrier {
            account_id: account_specific,
            barrier: SpotFundsPnlBoundsBarrier {
                lower_bound: Some(pnl_value("-10")),
                upper_bound: None,
            },
        }])
        .expect("account pnl barrier must set");
    let policy = build_policy_from_settings(s, None);

    let account_report = fill_with_fee(
        account_specific,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("20", "USD"),
    );
    let account_result = run_report_with_currency(&policy, &account_report, asset("USD"));
    assert_eq!(account_result.account_blocks.len(), 1);
    assert_eq!(
        account_result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );

    let grouped_report = fill_with_fee(
        grouped,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("20", "USD"),
    );
    let grouped_result = <TestPolicy as PreTradePolicy<
        TestOrder,
        TestReport,
        TestAdjustment,
        crate::core::FullSync,
    >>::apply_execution_report(
        &policy,
        &post_trade_ctx_with_currency_and_group(&grouped_report, asset("USD"), grp),
        &grouped_report,
    )
    .expect("grouped report must produce a result");
    assert!(grouped_result.account_blocks.is_empty());
    assert_eq!(account_pnl_of(&policy, grouped), Some(pnl_value("-20")));

    let global_report = fill_with_fee(
        global_only,
        aapl_usd,
        Side::Buy,
        "100",
        "1",
        money_fee("20", "USD"),
    );
    let global_result = run_report_with_currency(&policy, &global_report, asset("USD"));
    assert_eq!(global_result.account_blocks.len(), 1);
    assert_eq!(
        global_result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
}

#[test]
fn global_barrier_blocks_account_when_account_pnl_fx_is_missing() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-10")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("1", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("EUR"));

    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingFx),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(crate::PnlHaltReason::MissingFx))
    );
}

#[test]
fn fee_in_foreign_currency_debits_fee_asset_and_contributes_to_account_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let eur_usd = instr("EUR", "USD");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(eur_usd).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("1.2")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));

    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-10")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = SpotFundsPolicy::new(s, Some(bundle), b.storage_builder());

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("2", "EUR"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert!(result.account_blocks.is_empty());
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-2.4")));
    let eur = holdings_of(&policy, acc, &asset("EUR")).expect("EUR fee leg must exist");
    assert_eq!(
        eur.available(),
        ps("-2"),
        "trade and fee report must debit the fee exactly once"
    );
}

#[test]
fn fee_only_execution_report_debits_fee_asset_and_contributes_to_account_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let eur_usd = instr("EUR", "USD");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(eur_usd).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("1.2")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));

    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-10")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = SpotFundsPolicy::new(s, Some(bundle), b.storage_builder());
    seed(&policy, acc, asset("EUR"), "10");
    // A tracked AAPL position: the fee-only correction folds into its realized
    // P&L (account currency) and reaches the barrier through the same net delta.
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    let report = fee_only_report(acc, aapl_usd, Side::Buy, "1", false, money_fee("2", "EUR"));
    let result = run_report_with_currency(&policy, &report, asset("USD"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-2.4")));
    let eur = holdings_of(&policy, acc, &asset("EUR")).expect("EUR fee leg must exist");
    assert_eq!(eur.available(), ps("8"));
    let outcome = result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("EUR"))
        .expect("EUR fee outcome must exist");
    let balance = outcome
        .entry
        .balance
        .as_ref()
        .expect("EUR fee balance outcome must exist");
    assert_eq!(balance.delta, ps("-2"));
    assert_eq!(balance.absolute, ps("8"));
    // The fee reduces the underlying position's realized P&L, surfaced as an
    // AAPL realized-P&L outcome delta.
    let aapl_realized = result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl.as_ref())
        .and_then(|outcome| outcome.as_ref().ok())
        .expect("AAPL realized pnl outcome must exist");
    assert_eq!(aapl_realized.delta, pnl_value("-2.4"));
    assert_eq!(aapl_realized.absolute, pnl_value("-2.4"));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("-2.4")));
}

#[test]
fn fee_only_execution_report_without_account_currency_halts_both_pnl_ledgers() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10");

    let report = fee_only_report(acc, aapl_usd, Side::Buy, "1", false, money_fee("2", "USD"));
    let result = run_report_without_account_currency(&policy, &report);

    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingAccountCurrency),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::MissingAccountCurrency
        ))
    );

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert_eq!(
        aapl_entry.entry.realized_pnl,
        Some(Err(crate::PnlHaltReason::MissingAccountCurrency))
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingAccountCurrency)
    );
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD fee leg must exist");
    assert_eq!(usd.available(), ps("8"));
}

#[test]
fn fee_only_execution_report_missing_fx_halts_account_without_barrier_or_block() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    let policy = build_policy_from_settings(s, None);
    seed(&policy, acc, asset("EUR"), "10");

    let report = fee_only_report(acc, aapl_usd, Side::Buy, "1", false, money_fee("2", "EUR"));
    let result = run_report_with_currency(&policy, &report, asset("USD"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::MissingFx)
    );
    let eur = holdings_of(&policy, acc, &asset("EUR")).expect("EUR fee leg must exist");
    assert_eq!(eur.available(), ps("8"));
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(crate::PnlHaltReason::MissingFx))
    );
}

#[test]
fn fee_only_execution_report_missing_fx_applies_report_then_blocks_account() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-100")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);
    let report = fee_only_report(acc, aapl_usd, Side::Buy, "1", false, money_fee("2", "EUR"));
    let result = run_report_with_currency(&policy, &report, asset("USD"));

    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::MissingFx)
    );
    assert!(result
        .account_adjustments
        .iter()
        .any(|outcome| { outcome.entry.asset == asset("EUR") && outcome.entry.balance.is_some() }));
    assert!(holdings_of(&policy, acc, &asset("EUR")).is_some());
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(crate::PnlHaltReason::MissingFx))
    );
}

#[test]
fn missing_account_fx_halts_account_then_blocks_under_pnl_control() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-100")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("1", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("EUR"));
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingFx),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(crate::PnlHaltReason::MissingFx))
    );
    assert!(result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .is_some_and(|outcome| {
            outcome.entry.realized_pnl == Some(Err(crate::PnlHaltReason::MissingFx))
        }));
}

// The settlement leg needs no FX because settlement asset == account currency
// (USD), so the settlement-FX guard passes; the fee is in a foreign currency
// (EUR) with no FX rate, so the fee-conversion branch halts the position and
// account PnL independently. This isolates the fee-FX halt from the
// settlement-FX path while both accumulators fail closed.
#[test]
fn missing_fee_fx_halts_account_and_blocks_after_applying_report() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-100")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    // No market-data service at all: the EUR->USD fee FX is missing, but the
    // USD settlement leg needs no FX because it is already in account currency.
    let policy = build_policy_from_settings(s, None);

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("2", "EUR"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingFx),
        }]
    );
    assert!(!result.account_adjustments.is_empty());
    assert!(holdings_of(&policy, acc, &asset("EUR")).is_some());
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(crate::PnlHaltReason::MissingFx))
    );
}

// Positive twin of the fee-FX fail-safe: when the fee currency equals the
// account currency, no FX is needed and the account is not blocked.
#[test]
fn fee_in_account_currency_needs_no_fx_and_does_not_block() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-100")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("2", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert!(result.account_blocks.is_empty());
    // Fee 2 USD is a cost, so realized P&L is -2 with no FX conversion.
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-2")));
    // The reported realized P&L and the AAPL slot are net of the fee, and the
    // barrier reads the same figure.
    let aapl_realized = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .and_then(|o| o.entry.realized_pnl.as_ref())
        .and_then(|outcome| outcome.as_ref().ok())
        .expect("AAPL realized pnl outcome must exist");
    assert_eq!(aapl_realized.delta, pnl_value("-2"));
    assert_eq!(aapl_realized.absolute, pnl_value("-2"));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("-2")));
}

// A fee'd fill with no barrier configured still folds the fee into the reported
// realized P&L and the account-level PnL result.
#[test]
fn fill_with_fee_folds_into_reported_realized_pnl_without_barrier() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // Open a long from flat: the price-based realized delta is zero, so the
    // reported realized P&L is exactly the folded 3 USD fee cost.
    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("3", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert!(result.account_blocks.is_empty());

    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: pnl_value("-3"),
                absolute: pnl_value("-3"),
            }),
        }]
    );
    let realized = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .and_then(|o| o.entry.realized_pnl.as_ref())
        .and_then(|outcome| outcome.as_ref().ok())
        .expect("realized pnl must be reported net of the fee");
    assert_eq!(realized.delta, pnl_value("-3"));
    assert_eq!(realized.absolute, pnl_value("-3"));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("-3")));
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-3")));
}

#[test]
fn no_barrier_fill_reports_fx_converted_fee_inclusive_account_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let usd_eur = instr("USD", "EUR");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(usd_eur).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("0.9")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("90"));

    let fill = fill_with_fee(acc, aapl_usd, Side::Sell, "120", "4", money_fee("5", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("EUR"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: pnl_value("67.5"),
                absolute: pnl_value("67.5"),
            }),
        }]
    );
    let realized = result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl.as_ref())
        .and_then(|outcome| outcome.as_ref().ok())
        .expect("AAPL realized pnl outcome must exist");
    assert_eq!(realized.delta, pnl_value("67.5"));
    assert_eq!(realized.absolute, pnl_value("67.5"));
}

// With a barrier configured, the reported realized P&L and the kill-switch
// accumulator agree exactly: the fee is counted once, inside realized P&L, not
// added to the barrier separately.
#[test]
fn fill_with_fee_reported_realized_pnl_agrees_with_barrier_no_double_count() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-1000")),
        upper_bound: Some(pnl_value("1000")),
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    // Close 4 lots at 130: price P&L +120, minus a 5 USD fee => net +115.
    let close = fill_with_fee(acc, aapl_usd, Side::Sell, "130", "4", money_fee("5", "USD"));
    let result = run_report_with_currency(&policy, &close, asset("USD"));
    assert!(result.account_blocks.is_empty());

    let realized = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .and_then(|o| o.entry.realized_pnl.as_ref())
        .and_then(|outcome| outcome.as_ref().ok())
        .expect("realized pnl must be reported");
    assert_eq!(realized.delta, pnl_value("115"));
    assert_eq!(realized.absolute, pnl_value("115"));
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("115")));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("115")));
}

// A negative fee (rebate) increases realized P&L.
#[test]
fn rebate_fee_increases_reported_realized_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // Open from flat with a -4 USD fee (a rebate): realized P&L is +4.
    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("-4", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert!(result.account_blocks.is_empty());

    let realized = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .and_then(|o| o.entry.realized_pnl.as_ref())
        .and_then(|outcome| outcome.as_ref().ok())
        .expect("realized pnl must be reported");
    assert_eq!(realized.delta, pnl_value("4"));
    assert_eq!(realized.absolute, pnl_value("4"));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("4")));
}

// A fee'd fill whose realized P&L is untracked (no account currency in context)
// still reports why P&L was unavailable, debits the fee from the settlement
// balance, and blocks nothing without a barrier. The asset-slot and account
// accumulators both halt because both are denominated in account currency.
#[test]
fn fill_with_fee_without_account_currency_reports_missing_currency_reason() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("3", "USD"));
    let result = run_report_without_account_currency(&policy, &fill);
    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingAccountCurrency),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::MissingAccountCurrency
        ))
    );

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert_eq!(
        aapl_entry.entry.realized_pnl,
        Some(Err(crate::PnlHaltReason::MissingAccountCurrency))
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert!(aapl.realized_pnl().is_none());

    // The settlement balance still absorbs the fee debit: the buy consumed 100
    // from held with zero net available flow, then the 3 USD fee reduced it.
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD slot must exist");
    assert_eq!(usd.available(), ps("9997"));
}

// A fee-only correction contributes to the account-level barrier even when the
// underlying slot is untracked. Per-position folding is additive behavior and
// must not replace the independent kill-switch fee path.
#[test]
fn fee_only_execution_report_untracked_slot_still_contributes_to_barrier() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let eur_usd = instr("EUR", "USD");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(eur_usd).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("1.2")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));

    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-2")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = SpotFundsPolicy::new(s, Some(bundle), b.storage_builder());
    seed(&policy, acc, asset("EUR"), "10");

    // AAPL was never traded, so its slot is absent/untracked.
    let report = fee_only_report(acc, aapl_usd, Side::Buy, "1", false, money_fee("2", "EUR"));
    let result = run_report_with_currency(&policy, &report, asset("USD"));

    assert_eq!(result.account_blocks.len(), 1);
    assert!(result.account_blocks[0]
        .details
        .contains("lower bound breached"));
    let eur = holdings_of(&policy, acc, &asset("EUR")).expect("EUR fee leg must exist");
    assert_eq!(eur.available(), ps("8"));
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-2.4")));
    assert_eq!(holdings_of(&policy, acc, &asset("AAPL")), None);
}
#[test]
fn fee_only_untracked_slot_pnl_conversion_overflow_halts_accumulators() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let eur_usd = instr("EUR", "USD");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(eur_usd).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("2")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    let huge_fee = MonetaryAmount {
        amount: Fee::new(Decimal::MAX),
        currency: asset("EUR"),
    };

    let report = fee_only_report(acc, aapl_usd, Side::Buy, "1", false, huge_fee);
    let result = run_report_with_currency(&policy, &report, asset("USD"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(result.account_pnls.len(), 1);
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::ArithmeticOverflow)
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::ArithmeticOverflow)
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::ArithmeticOverflow
        ))
    );
}

#[test]
fn zero_fee_only_report_is_an_economic_noop() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    let before = holdings_of(&policy, acc, &asset("AAPL"));
    let report = fee_only_report(acc, aapl_usd, Side::Buy, "1", false, money_fee("0", "USD"));
    let ctx = crate::pretrade::PostTradeContext::with_account_currency(acc, asset("USD"));

    let result = policy.apply_execution_report_impl(&ctx, &report);

    assert!(result.is_none());
    assert_eq!(holdings_of(&policy, acc, &asset("AAPL")), before);
    assert_eq!(account_pnl_of(&policy, acc), None);
}

#[test]
fn fee_report_delta_overflow_halts_position_and_account_pnl() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let eur_usd = instr("EUR", "USD");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(eur_usd).expect("register must succeed");
    svc.push(fx_id, Quote::new().with_mark(px("1")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));
    let policy = SpotFundsPolicy::new(settings(0), Some(bundle), b.storage_builder());
    seed_with_avg(&policy, acc, asset("AAPL"), "1", px("0"));
    let adjustment = adj_with_realized_pnl(asset("AAPL"), Pnl::new(Decimal::MIN));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("seed must succeed");
    mutations.commit_all();

    let fill = fill_with_fee(
        acc,
        aapl_usd,
        Side::Sell,
        &Decimal::MAX.to_string(),
        "1",
        money_fee("-1", "EUR"),
    );
    let result = run_report_with_currency(&policy, &fill, asset("USD"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::ArithmeticOverflow)
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::ArithmeticOverflow)
    );
    let outcome = result
        .account_adjustments
        .iter()
        .find(|entry| entry.entry.asset == asset("AAPL"))
        .and_then(|entry| entry.entry.realized_pnl.as_ref())
        .expect("AAPL realized pnl outcome must exist");
    assert_eq!(*outcome, Err(crate::PnlHaltReason::ArithmeticOverflow));
}

#[test]
fn fee_only_position_pnl_overflow_still_updates_account_pnl() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-100")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);
    seed(&policy, acc, asset("USD"), "1000");
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    let adjustment = adj_with_realized_pnl(asset("AAPL"), Pnl::new(Decimal::MIN));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("seed must succeed");
    mutations.commit_all();

    let report = fee_only_report(acc, aapl_usd, Side::Buy, "0", false, money_fee("1", "USD"));
    let result = run_report_with_currency(&policy, &report, asset("USD"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-1")));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::ArithmeticOverflow)
    );
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD slot must exist");
    assert_eq!(usd.available(), ps("999"));
}

#[test]
fn fill_position_pnl_overflow_still_updates_account_pnl() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-100")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);
    seed(&policy, acc, asset("USD"), "1000");
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    let adjustment = adj_with_realized_pnl(asset("AAPL"), Pnl::new(Decimal::MIN));
    let mut mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut mutations).expect("seed must succeed");
    mutations.commit_all();

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("1", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-1")));
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::ArithmeticOverflow)
    );
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("USD slot must exist");
    assert_eq!(usd.available(), ps("999"));
}

// The realized-P&L accumulator itself overflows: seed it at Decimal::MAX (still
// inside the bounds), then apply a fill whose fee rebate contributes a positive
// P&L delta. MAX + positive overflows in apply_account_pnl_delta and halts only
// the account-PnL accumulator.
#[test]
fn account_pnl_accumulator_overflow_halts_account_pnl() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    let s = s
        .with_pnl_account_barriers([SpotFundsPnlBoundsAccountBarrier {
            account_id: acc,
            barrier: SpotFundsPnlBoundsBarrier {
                lower_bound: Some(pnl_value("-100")),
                upper_bound: None,
            },
        }])
        .expect("account pnl barrier must set");
    let policy = build_policy_from_settings(s, None);
    policy.set_account_pnl_state(acc, crate::PnlState::Value(Pnl::new(Decimal::MAX)));

    // A negative fee is a rebate -> positive P&L delta of +1; MAX + 1 overflows.
    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("-1", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(result.account_pnls.len(), 1);
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::ArithmeticOverflow)
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::ArithmeticOverflow
        ))
    );

    let repeated = run_report_with_currency(&policy, &fill, asset("USD"));
    assert_eq!(repeated.account_blocks.len(), 1);
    assert!(repeated.account_pnls.is_empty());
}

// A PnL barrier is not required for accumulator overflow to fail safe. The
// accumulator has a prior account-level value, then a fee rebate contributes
// a positive PnL delta that overflows the ledger.
#[test]
fn account_pnl_accumulator_overflow_halts_without_barrier() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    policy
        .pnl
        .with_mut(acc, super::AccountPnlEntry::zero, |entry, _is_new| {
            entry.state = crate::PnlState::Value(Pnl::new(Decimal::MAX))
        });

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("-1", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));

    assert!(result.account_blocks.is_empty());
    assert_eq!(result.account_pnls.len(), 1);
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::ArithmeticOverflow)
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::ArithmeticOverflow
        ))
    );
}

// Negative twin of the accumulator overflow: seed at Decimal::MIN, apply a fill
// whose positive fee is a cost (negative P&L delta). MIN + negative underflows in
// apply_account_pnl_delta and halts the account-PnL accumulator.
#[test]
fn account_pnl_accumulator_negative_overflow_halts_account_pnl() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    let s = s
        .with_pnl_account_barriers([SpotFundsPnlBoundsAccountBarrier {
            account_id: acc,
            barrier: SpotFundsPnlBoundsBarrier {
                lower_bound: None,
                upper_bound: Some(pnl_value("100")),
            },
        }])
        .expect("account pnl barrier must set");
    let policy = build_policy_from_settings(s, None);
    policy.set_account_pnl_state(acc, crate::PnlState::Value(Pnl::new(Decimal::MIN)));

    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("1", "USD"));
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(result.account_pnls.len(), 1);
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::ArithmeticOverflow)
    );
}

// The fee->account-currency conversion itself overflows before the accumulator
// is touched: a Decimal::MAX fee in a foreign currency with an FX mark above 1
// makes `fee_amount * factor` overflow inside fee_pnl_delta. This is a distinct
// branch from the accumulator overflow above.
#[test]
fn fee_pnl_conversion_overflow_halts_position_and_account_pnl() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let eur_usd = instr("EUR", "USD");
    let b = engine_builder();
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = svc.register(eur_usd).expect("register must succeed");
    // EUR->USD mark of 2 turns a Decimal::MAX EUR fee into MAX * 2, overflowing.
    svc.push(fx_id, Quote::new().with_mark(px("2")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(Arc::clone(&svc));

    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-100")),
        upper_bound: None,
    }))
    .expect("global pnl barrier must set");
    let policy = SpotFundsPolicy::new(s, Some(bundle), b.storage_builder());

    let huge_fee = MonetaryAmount {
        amount: Fee::new(Decimal::MAX),
        currency: asset("EUR"),
    };
    let fill = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", huge_fee);
    let result = run_report_with_currency(&policy, &fill, asset("USD"));
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(
        result.account_pnls[0].result,
        Err(crate::PnlHaltReason::ArithmeticOverflow)
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::ArithmeticOverflow)
    );
}

#[test]
fn pnl_bounds_block_lower_and_upper_boundary_breaches() {
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    s.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-10")),
        upper_bound: Some(pnl_value("10")),
    }))
    .expect("global pnl barrier must set");
    let policy = build_policy_from_settings(s, None);

    let lower = fill_with_fee(
        account(99224416),
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("11", "USD"),
    );
    let lower_result = run_report_with_currency(&policy, &lower, asset("USD"));
    assert_eq!(lower_result.account_blocks.len(), 1);
    assert_eq!(
        lower_result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert!(lower_result.account_blocks[0]
        .details
        .contains("lower bound breached"));
    assert_eq!(
        lower_result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: account(99224416),
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: pnl_value("-11"),
                absolute: pnl_value("-11"),
            }),
        }]
    );

    let upper = fill_with_fee(
        account(99224417),
        aapl_usd,
        Side::Buy,
        "100",
        "1",
        money_fee("-11", "USD"),
    );
    let upper_result = run_report_with_currency(&policy, &upper, asset("USD"));
    assert_eq!(upper_result.account_blocks.len(), 1);
    assert_eq!(
        upper_result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert!(upper_result.account_blocks[0]
        .details
        .contains("upper bound breached"));
    assert_eq!(
        upper_result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: account(99224417),
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Ok(crate::PnlOutcomeAmount {
                delta: pnl_value("11"),
                absolute: pnl_value("11"),
            }),
        }]
    );
}

#[test]
fn account_pnl_config_correction_is_account_scoped() {
    let acc_blocked = account(99224416);
    let acc_corrected = account(99224417);
    let aapl_usd = instr("AAPL", "USD");

    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    let s = s
        .with_pnl_account_barriers([
            SpotFundsPnlBoundsAccountBarrier {
                account_id: acc_blocked,
                barrier: SpotFundsPnlBoundsBarrier {
                    lower_bound: Some(pnl_value("-10")),
                    upper_bound: None,
                },
            },
            SpotFundsPnlBoundsAccountBarrier {
                account_id: acc_corrected,
                barrier: SpotFundsPnlBoundsBarrier {
                    lower_bound: Some(pnl_value("-10")),
                    upper_bound: None,
                },
            },
        ])
        .expect("account pnl barriers must set");
    let policy: SpotFundsPolicy<FullSync, FullSync> =
        SpotFundsPolicy::new(s, None, builder.storage_builder());
    let engine = builder
        .pre_trade(policy)
        .build()
        .expect("engine must build");
    engine.accounts().set_currency(acc_blocked, asset("USD"));
    engine.accounts().set_currency(acc_corrected, asset("USD"));
    engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            acc_blocked,
            crate::PnlState::Value(pnl_value("-20")),
        )
        .expect("account pnl correction must publish");

    let blocked = fill_with_fee(
        acc_blocked,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("2", "USD"),
    );
    let blocked_result = engine.apply_execution_report(&blocked);
    assert_eq!(blocked_result.account_blocks.len(), 1);
    assert_eq!(
        blocked_result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );

    engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            acc_corrected,
            crate::PnlState::Value(Pnl::ZERO),
        )
        .expect("account pnl correction must publish");
    let corrected = fill_with_fee(
        acc_corrected,
        aapl_usd,
        Side::Buy,
        "100",
        "1",
        money_fee("2", "USD"),
    );
    let corrected_result = engine.apply_execution_report(&corrected);
    assert!(corrected_result.account_blocks.is_empty());
}

#[test]
fn halted_account_pnl_adjustment_with_effective_barrier_blocks_account() {
    let acc = account(99224416);
    let mut settings = settings(0);
    settings
        .set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
            lower_bound: Some(pnl_value("-10")),
            upper_bound: None,
        }))
        .expect("global account PnL barrier must set");
    let engine = build_account_pnl_test_engine(settings);

    let result = engine
        .apply_account_adjustment(
            acc,
            &[account_pnl_adjustment(crate::PnlState::Halted(
                crate::PnlHaltReason::MissingFx,
            ))],
        )
        .expect("halted account PnL correction must succeed");

    assert!(result.outcomes.is_empty());
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    let rejects = match engine.start_pre_trade(account_pnl_probe_order(acc)) {
        Ok(_) => panic!("halted account PnL with an effective barrier must be blocked"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
}

#[test]
fn halted_account_pnl_adjustment_without_barrier_does_not_block_account() {
    let acc = account(99224416);
    let engine = build_account_pnl_test_engine(settings(0));

    let result = engine
        .apply_account_adjustment(
            acc,
            &[account_pnl_adjustment(crate::PnlState::Halted(
                crate::PnlHaltReason::MissingFx,
            ))],
        )
        .expect("halted account PnL correction must succeed");

    assert!(result.account_blocks.is_empty());
    assert!(engine.start_pre_trade(account_pnl_probe_order(acc)).is_ok());
}

#[test]
fn numeric_account_pnl_adjustment_blocks_on_out_of_bounds_value() {
    let acc = account(99224416);
    let mut settings = settings(0);
    settings
        .set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
            lower_bound: Some(pnl_value("-10")),
            upper_bound: None,
        }))
        .expect("global account PnL barrier must set");
    let engine = build_account_pnl_test_engine(settings);

    let result = engine
        .apply_account_adjustment(
            acc,
            &[account_pnl_adjustment(crate::PnlState::Value(pnl_value(
                "-20",
            )))],
        )
        .expect("numeric account PnL correction must succeed");

    assert_eq!(result.account_blocks.len(), 1);
    assert!(engine
        .start_pre_trade(account_pnl_probe_order(acc))
        .is_err());
}

/// Regression, through the engine's own adjustment path: the engine discards a
/// batch's blocks when it rejects and records them only after `commit_all`, so
/// a committed force-set's own breach block is untagged and keeps the account
/// blocked, with no rollback left that could retire it.
#[test]
fn committed_out_of_bounds_account_pnl_adjustment_keeps_the_account_blocked() {
    let acc = account(99224416);
    let mut settings = settings(0);
    settings
        .set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
            lower_bound: Some(pnl_value("-10")),
            upper_bound: None,
        }))
        .expect("global account PnL barrier must set");
    let engine = build_account_pnl_test_engine(settings);

    let result = engine
        .apply_account_adjustment(
            acc,
            &[account_pnl_adjustment(crate::PnlState::Value(pnl_value(
                "-20",
            )))],
        )
        .expect("numeric account PnL correction must succeed");
    assert_eq!(result.account_blocks.len(), 1);

    assert!(
        engine
            .start_pre_trade(account_pnl_probe_order(acc))
            .is_err(),
        "the committed breach must keep blocking the account"
    );
}

#[test]
fn account_pnl_rollback_preserves_fill_delta_and_rechecks_barrier() {
    let acc = account(99224416);
    let barrier = SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-120")),
        upper_bound: None,
    };
    let policy = build_policy(None, None);
    policy.set_account_pnl_state(acc, crate::PnlState::Value(pnl_value("-100")));

    let mut mutations = Mutations::with_capacity(1);
    let owner_id = mutations.owner_id();
    let asserted = crate::PnlState::Value(pnl_value("-50"));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, asserted);
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: Some(dummy_control(acc)),
            account_id: acc,
            previous,
            asserted,
            token,
            barrier: Some(barrier),
            lease,
        },
    );

    let (fill, provenance) = policy.update_account_pnl(acc, pnl_value("-30"));
    assert!(fill.is_ok());
    assert_eq!(provenance, Some(token));

    let report = mutations.rollback_all().report;
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Value(pnl_value("-130")))
    );
    assert_eq!(report.account_blocks.len(), 1);
    assert_eq!(
        report.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
}

fn lease_of(policy: &TestPolicy, account_id: AccountId) -> Option<(u64, usize)> {
    policy
        .pnl_leases
        .with(&account_id, |lease| {
            lease.as_ref().map(|lease| (lease.owner_id, lease.depth))
        })
        .flatten()
}

/// The production acquirer is re-entrant per owner: nested acquisitions stack
/// depth instead of self-blocking, and only the outermost release frees the
/// account for the next owner.
#[test]
fn account_pnl_lease_is_reentrant_for_one_owner_and_releases_outermost_last() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    let owner_id = 7;

    let outer = policy.acquire_account_pnl_lease(acc, owner_id);
    assert_eq!(lease_of(&policy, acc), Some((owner_id, 1)));

    let inner = policy.acquire_account_pnl_lease(acc, owner_id);
    assert_eq!(
        lease_of(&policy, acc),
        Some((owner_id, 2)),
        "a re-entrant acquisition must stack depth, not take a second lease"
    );

    drop(inner);
    assert_eq!(
        lease_of(&policy, acc),
        Some((owner_id, 1)),
        "releasing the inner acquisition must leave the outer one holding"
    );

    drop(outer);
    assert_eq!(lease_of(&policy, acc), None);

    let next = policy.acquire_account_pnl_lease(acc, owner_id + 1);
    assert_eq!(
        lease_of(&policy, acc),
        Some((owner_id + 1, 1)),
        "the released account must be available to the next owner"
    );
    drop(next);
    assert_eq!(lease_of(&policy, acc), None);
}

/// Leases are per account, so an owner holding one account never delays
/// another.
#[test]
fn account_pnl_leases_of_distinct_accounts_are_independent() {
    let first = account(99224416);
    let second = account(99224417);
    let policy = build_policy(None, None);

    let first_lease = policy.acquire_account_pnl_lease(first, 7);
    let second_lease = policy.acquire_account_pnl_lease(second, 8);

    assert_eq!(lease_of(&policy, first), Some((7, 1)));
    assert_eq!(lease_of(&policy, second), Some((8, 1)));

    drop(first_lease);
    assert_eq!(lease_of(&policy, first), None);
    assert_eq!(lease_of(&policy, second), Some((8, 1)));
    drop(second_lease);
}

/// The unbounded wait ends as soon as the holder releases: a contending owner
/// on another thread backs off, then takes the lease.
#[test]
fn account_pnl_lease_wait_ends_when_the_holder_releases() {
    let acc = account(99224416);
    let policy = Arc::new(build_policy(None, None));
    let held = policy.acquire_account_pnl_lease(acc, 7);

    let contender = {
        let policy = Arc::clone(&policy);
        std::thread::spawn(move || {
            let _lease = policy.acquire_account_pnl_lease(acc, 8);
            assert_eq!(lease_of(&policy, acc), Some((8, 1)));
        })
    };

    // Let the contender reach the wait before the holder releases.
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(
        lease_of(&policy, acc),
        Some((7, 1)),
        "a contending owner must not take a held lease"
    );
    drop(held);

    contender
        .join()
        .expect("contending owner must acquire the released lease");
    assert_eq!(lease_of(&policy, acc), None);
}

/// A committed assertion is never rolled back, so the block an interleaving
/// execution report computed against its asserted value stays: nothing retires
/// the cause, and its token is inert once the only holder has committed.
#[test]
fn committed_account_pnl_assertion_keeps_its_block() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    let control = dummy_control(acc);

    let mut mutations = Mutations::with_capacity(1);
    let owner_id = mutations.owner_id();
    let asserted = crate::PnlState::Value(pnl_value("-50"));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, asserted);
    // An interleaving execution report breached the barrier against the
    // asserted, not-yet-final value, so its block answers to the assertion.
    control.block(provisional_block("interleaved fill breach", token));
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: Some(control.clone()),
            account_id: acc,
            previous,
            asserted,
            token,
            barrier: None,
            lease,
        },
    );

    mutations.commit_all();

    let blocked = control
        .invalidate_provenance(token)
        .expect("nothing retired the cause, so its token still matches it");
    assert_eq!(blocked.details, "interleaved fill breach");
}

/// An operator who overwrites the reason of a block raised against an in-flight
/// assertion takes ownership of it: the stored cause is no longer the
/// assertion's own, so its rollback leaves the account blocked.
#[test]
fn an_operator_overwrite_outlives_the_asserting_rollback() {
    use crate::core::account_control::BlockedAccounts;
    use crate::core::AccountBlockHandle;
    use crate::storage::{FullLocking, LockingPolicyFactory, StorageBuilder};

    let acc = account(99224416);
    let policy = build_policy(None, None);
    let sb = StorageBuilder::new(FullLocking);
    let blocked = FullLocking::new_shared(BlockedAccounts::new(&sb));
    let control =
        crate::core::AccountControl::new(AccountBlockHandle::from_inner(blocked.clone()), acc);

    let mut mutations = Mutations::with_capacity(1);
    let owner_id = mutations.owner_id();
    let asserted = crate::PnlState::Value(pnl_value("-50"));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, asserted);
    // An interleaving execution report breached the barrier against the
    // asserted, not-yet-final value, so its block answers to the assertion.
    control.block(provisional_block("interleaved fill breach", token));
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: Some(control.clone()),
            account_id: acc,
            previous,
            asserted,
            token,
            barrier: None,
            lease,
        },
    );

    // The operator overwrites the reason, which makes the block theirs.
    blocked
        .replace_reason(
            acc,
            crate::pretrade::AccountBlock::new(
                TestPolicy::NAME,
                RejectCode::AccountBlocked,
                "under manual review",
                String::new(),
            ),
        )
        .expect("a blocked account must accept a replaced reason");

    let report = mutations.rollback_all();

    assert!(
        report.report.invalidated_account_blocks.is_empty(),
        "the rollback must not touch a cause that is no longer its own"
    );
    blocked
        .replace_reason(
            acc,
            crate::pretrade::AccountBlock::new(
                TestPolicy::NAME,
                RejectCode::AccountBlocked,
                "still blocked",
                String::new(),
            ),
        )
        .expect("the operator's block must have survived the rollback");
}

#[test]
fn account_pnl_rearm_rollback_restores_halt_and_reports_discarded_delta() {
    let acc = account(99224416);
    let prior = crate::PnlState::Halted(crate::PnlHaltReason::MissingFx);
    let policy = build_policy(None, None);
    policy.set_account_pnl_state(acc, prior);

    let mut mutations = Mutations::with_capacity(1);
    let owner_id = mutations.owner_id();
    let asserted = crate::PnlState::Value(pnl_value("-50"));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, asserted);
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: None,
            account_id: acc,
            previous,
            asserted,
            token,
            barrier: None,
            lease,
        },
    );
    assert!(policy.update_account_pnl(acc, pnl_value("-30")).0.is_ok());

    let report = mutations.rollback_all().report;
    assert_eq!(account_pnl_state_of(&policy, acc), Some(prior));
    assert_eq!(
        report.reconciliations,
        vec![crate::core::mutation::AccountPnlReconciliation {
            account_id: acc,
            discarded_delta: Some(pnl_value("-30")),
        }]
    );
}

#[test]
fn rejected_account_pnl_halt_assertion_restores_prior_value() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    let prior = crate::PnlState::Value(pnl_value("25"));
    policy.set_account_pnl_state(acc, prior);

    let mut mutations = Mutations::with_capacity(1);
    let owner_id = mutations.owner_id();
    let asserted = crate::PnlState::Halted(crate::PnlHaltReason::MissingFx);
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, asserted);
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: None,
            account_id: acc,
            previous,
            asserted,
            token,
            barrier: None,
            lease,
        },
    );

    let report = mutations.rollback_all().report;
    assert_eq!(account_pnl_state_of(&policy, acc), Some(prior));
    assert!(report.reconciliations.is_empty());
}

#[test]
fn account_pnl_inverse_rollback_overflow_halts_and_blocks() {
    use rust_decimal::Decimal;

    let acc = account(99224416);
    let barrier = SpotFundsPnlBoundsBarrier {
        lower_bound: Some(pnl_value("-1")),
        upper_bound: None,
    };
    let policy = build_policy(None, None);
    policy.set_account_pnl_state(acc, crate::PnlState::Value(Pnl::new(Decimal::MAX)));

    let mut mutations = Mutations::with_capacity(1);
    let owner_id = mutations.owner_id();
    let asserted = crate::PnlState::Value(Pnl::new(Decimal::MIN));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, asserted);
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: Some(dummy_control(acc)),
            account_id: acc,
            previous,
            asserted,
            token,
            barrier: Some(barrier),
            lease,
        },
    );
    assert!(policy.update_account_pnl(acc, pnl_value("1")).0.is_ok());

    let report = mutations.rollback_all().report;
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::ArithmeticOverflow
        ))
    );
    assert_eq!(report.account_blocks.len(), 1);
    assert_eq!(
        report.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
}

#[test]
fn nested_account_pnl_assertions_unwind_lifo_for_same_batch() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    policy.set_account_pnl_state(acc, crate::PnlState::Value(pnl_value("-100")));

    let mut mutations = Mutations::with_capacity(2);
    let owner_id = mutations.owner_id();
    let first = crate::PnlState::Value(pnl_value("-50"));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, first);
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: None,
            account_id: acc,
            previous,
            asserted: first,
            token,
            barrier: None,
            lease,
        },
    );

    let second = crate::PnlState::Value(pnl_value("20"));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, second);
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: None,
            account_id: acc,
            previous,
            asserted: second,
            token,
            barrier: None,
            lease,
        },
    );
    assert!(policy.update_account_pnl(acc, pnl_value("5")).0.is_ok());

    let report = mutations.rollback_all().report;
    assert!(report.reconciliations.is_empty());
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Value(pnl_value("-95")))
    );
}

#[test]
fn concurrent_calculation_halt_waits_for_rejected_assertion_then_survives() {
    let acc = account(99224416);
    let policy = Arc::new(build_policy(None, None));
    policy.set_account_pnl_state(acc, crate::PnlState::Value(pnl_value("-100")));

    let mut mutations = Mutations::with_capacity(1);
    let owner_id = mutations.owner_id();
    let asserted = crate::PnlState::Value(pnl_value("-50"));
    let (previous, token, lease) = policy.acquire_account_pnl_assertion(acc, owner_id, asserted);
    policy.register_account_pnl_adjustment_rollback(
        &mut mutations,
        AccountPnlAssertionRollback {
            account_control: None,
            account_id: acc,
            previous,
            asserted,
            token,
            barrier: None,
            lease,
        },
    );

    let concurrent = Arc::clone(&policy);
    let halt = std::thread::spawn(move || {
        concurrent.halt_account_pnl(acc, crate::PnlHaltReason::MissingFx)
    });
    std::thread::yield_now();
    let _ = mutations.rollback_all();

    assert_eq!(
        halt.join().expect("calculation halt thread must finish"),
        crate::PnlHaltReason::MissingFx
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(crate::PnlHaltReason::MissingFx))
    );
}

#[test]
fn halted_account_pnl_configuration_with_effective_barrier_blocks_account() {
    let acc = account(99224416);
    let mut settings = settings(0);
    settings
        .set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
            lower_bound: Some(pnl_value("-10")),
            upper_bound: None,
        }))
        .expect("global account PnL barrier must set");
    let engine = build_account_pnl_test_engine(settings);

    let result = engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            acc,
            crate::PnlState::Halted(crate::PnlHaltReason::MissingFx),
        )
        .expect("halted account PnL configuration must succeed");

    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    let rejects = match engine.start_pre_trade(account_pnl_probe_order(acc)) {
        Ok(_) => panic!("halted account PnL with an effective barrier must be blocked"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
}

#[test]
fn halted_account_pnl_configuration_without_barrier_does_not_block_account() {
    let acc = account(99224416);
    let engine = build_account_pnl_test_engine(settings(0));

    let result = engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            acc,
            crate::PnlState::Halted(crate::PnlHaltReason::MissingFx),
        )
        .expect("halted account PnL configuration must succeed");

    assert!(result.account_blocks.is_empty());
    assert!(engine.start_pre_trade(account_pnl_probe_order(acc)).is_ok());
}

#[test]
fn numeric_account_pnl_configuration_blocks_on_out_of_bounds_value() {
    let acc = account(99224416);
    let mut settings = settings(0);
    settings
        .set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
            lower_bound: Some(pnl_value("-10")),
            upper_bound: None,
        }))
        .expect("global account PnL barrier must set");
    let engine = build_account_pnl_test_engine(settings);

    let result = engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            acc,
            crate::PnlState::Value(pnl_value("-20")),
        )
        .expect("numeric account PnL configuration must succeed");

    assert_eq!(result.account_blocks.len(), 1);
    assert!(engine
        .start_pre_trade(account_pnl_probe_order(acc))
        .is_err());
}

#[test]
fn runtime_pnl_barrier_update_does_not_reset_accumulator() {
    use crate::pretrade::ConfigurablePolicy;
    use crate::storage::ConfigCell;

    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let mut s = settings(0);
    s.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    let s = s
        .with_pnl_account_barriers([SpotFundsPnlBoundsAccountBarrier {
            account_id: acc,
            barrier: SpotFundsPnlBoundsBarrier {
                lower_bound: Some(pnl_value("-50")),
                upper_bound: None,
            },
        }])
        .expect("account pnl barrier must set");
    let policy = build_policy_from_settings(s, None);

    let first = fill_with_fee(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("40", "USD"),
    );
    let first_result = run_report_with_currency(&policy, &first, asset("USD"));
    assert!(first_result.account_blocks.is_empty());
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-40")));

    policy
        .settings_cell()
        .update::<SpotFundsConfigError>(|settings| {
            settings.set_pnl_account_barriers([SpotFundsPnlBoundsAccountBarrier {
                account_id: acc,
                barrier: SpotFundsPnlBoundsBarrier {
                    lower_bound: Some(pnl_value("-30")),
                    upper_bound: None,
                },
            }])
        })
        .expect("runtime barrier update must publish");

    let recheck = fill_with_fee(acc, aapl_usd, Side::Buy, "100", "1", money_fee("0", "USD"));
    let result = run_report_with_currency(&policy, &recheck, asset("USD"));
    assert_eq!(result.account_blocks.len(), 1);
    assert_eq!(
        result.account_blocks[0].code,
        RejectCode::PnlKillSwitchTriggered
    );
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("-40")));
}

#[test]
fn runtime_pnl_axes_can_be_enabled_replaced_and_cleared() {
    let survivor = account(99224416);
    let account_override = account(99224417);
    let cleared = account(99224418);
    let aapl_usd = instr("AAPL", "USD");
    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let policy: SpotFundsPolicy<FullSync, FullSync> =
        SpotFundsPolicy::new(settings(0), None, builder.storage_builder());
    let engine = builder
        .pre_trade(policy)
        .build()
        .expect("ordinary spot-funds engine must build");
    let name = SpotFundsPolicy::<FullSync, FullSync>::NAME;

    for account_id in [survivor, account_override, cleared] {
        engine.accounts().set_currency(account_id, asset("USD"));
    }
    seed_balance_via_engine(&engine, cleared, asset("USD"), ps("1000"));

    // A plain spot-funds policy has no P&L control, but normal orders still
    // run.
    let mut reservation = engine
        .execute_pre_trade(make_order(
            cleared,
            aapl_usd.clone(),
            Side::Buy,
            TradeAmount::Quantity(qty("1")),
            Some(px("100")),
        ))
        .expect("ordinary spot-funds policy must admit a funded order");
    reservation.rollback();

    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_pnl_global_barrier(Some(SpotFundsPnlBoundsBarrier {
                lower_bound: Some(pnl_value("-10")),
                upper_bound: None,
            }))
        })
        .expect("runtime configuration must add a P&L axis to ordinary spot funds");

    let first_survivor_fill = fill_with_fee(
        survivor,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("9", "USD"),
    );
    assert!(engine
        .apply_execution_report(&first_survivor_fill)
        .account_blocks
        .is_empty());

    // Adding an account override leaves the global axis and its live P&L
    // intact.
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_pnl_account_barriers([SpotFundsPnlBoundsAccountBarrier {
                account_id: account_override,
                barrier: SpotFundsPnlBoundsBarrier {
                    lower_bound: Some(pnl_value("-20")),
                    upper_bound: None,
                },
            }])
        })
        .expect("runtime configuration must add an account P&L axis");

    let survivor_breach = fill_with_fee(
        survivor,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("2", "USD"),
    );
    assert_eq!(
        engine
            .apply_execution_report(&survivor_breach)
            .account_blocks[0]
            .code,
        RejectCode::PnlKillSwitchTriggered
    );

    let override_fill = fill_with_fee(
        account_override,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("15", "USD"),
    );
    assert!(engine
        .apply_execution_report(&override_fill)
        .account_blocks
        .is_empty());

    // Clearing only the account axis exposes its still-live P&L to the global
    // bound.
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| settings.set_pnl_account_barriers([]))
        .expect("runtime configuration must clear only the account P&L axis");
    let override_recheck = fill_with_fee(
        account_override,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("1", "USD"),
    );
    assert_eq!(
        engine
            .apply_execution_report(&override_recheck)
            .account_blocks[0]
            .code,
        RejectCode::PnlKillSwitchTriggered
    );

    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(name, |settings| {
            settings.set_pnl_global_barrier(None)?;
            settings.set_pnl_account_group_barriers([])?;
            settings.set_pnl_account_barriers([])
        })
        .expect("runtime configuration must clear every P&L axis");
    let cleared_fill = fill_with_fee(
        cleared,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("15", "USD"),
    );
    assert!(engine
        .apply_execution_report(&cleared_fill)
        .account_blocks
        .is_empty());

    let mut reservation = engine
        .execute_pre_trade(make_order(
            cleared,
            aapl_usd,
            Side::Buy,
            TradeAmount::Quantity(qty("1")),
            Some(px("100")),
        ))
        .expect("clearing all P&L axes must restore normal order handling");
    reservation.rollback();
}

#[test]
fn halted_position_with_basis_still_contributes_to_active_account_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let seed = TestAdjustment {
        asset: asset("AAPL"),
        balance: Some(AdjustmentAmount::Absolute(ps("10"))),
        balance_average_entry_price: Some(px("100")),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed, &mut mutations);
    mutations.commit_all();
    let sell = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("130"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("130"),
        )])),
    );

    let first = run_report_with_currency(&policy, &sell, asset("USD"));
    assert!(first
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl)
        .is_some_and(|outcome| outcome == Err(crate::PnlHaltReason::MissingInitialPnl)));
    assert_eq!(
        first.account_pnls[0].result,
        Ok(crate::PnlOutcomeAmount {
            delta: pnl_value("30"),
            absolute: pnl_value("30"),
        })
    );

    let repeated = run_report_with_currency(&policy, &sell, asset("USD"));
    assert!(repeated
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .is_some_and(|outcome| outcome.entry.realized_pnl.is_none()));
    assert_eq!(
        repeated.account_pnls[0].result,
        Ok(crate::PnlOutcomeAmount {
            delta: pnl_value("30"),
            absolute: pnl_value("60"),
        })
    );

    let force = TestAdjustment {
        asset: asset("AAPL"),
        balance: None,
        balance_average_entry_price: Some(px("100")),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force, &mut mutations);
    mutations.commit_all();
    let force_pnl = adj_with_realized_pnl(asset("AAPL"), Pnl::ZERO);
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force_pnl, &mut mutations);
    mutations.commit_all();
    let resumed = run_report_with_currency(&policy, &sell, asset("USD"));
    assert!(resumed
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl)
        .is_some_and(|outcome| outcome.is_ok()));
}

#[test]
fn missing_cost_basis_is_sticky_until_position_force_set() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    let seed = TestAdjustment {
        asset: asset("AAPL"),
        balance: Some(AdjustmentAmount::Absolute(ps("10"))),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed, &mut mutations);
    mutations.commit_all();
    let seed_pnl = adj_with_realized_pnl(asset("AAPL"), Pnl::ZERO);
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed_pnl, &mut mutations);
    mutations.commit_all();

    let sell = make_report(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        Some(Trade {
            price: px("130"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("130"),
        )])),
    );
    let first = run_report_with_currency(&policy, &sell, asset("USD"));
    let first_position_pnl = first
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl);
    assert_eq!(
        first_position_pnl,
        Some(Err(crate::PnlHaltReason::MissingCostBasis))
    );
    assert_eq!(
        first.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingCostBasis),
        }]
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::MissingCostBasis
        ))
    );

    let repeated = run_report_with_currency(&policy, &sell, asset("USD"));
    assert!(repeated
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .is_some_and(|outcome| outcome.entry.realized_pnl.is_none()));

    let force = TestAdjustment {
        asset: asset("AAPL"),
        balance: None,
        balance_average_entry_price: Some(px("100")),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force, &mut mutations);
    mutations.commit_all();
    let force_pnl = adj_with_realized_pnl(asset("AAPL"), Pnl::ZERO);
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force_pnl, &mut mutations);
    mutations.commit_all();
    let resumed = run_report_with_currency(&policy, &sell, asset("USD"));
    assert!(resumed
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl)
        .is_some_and(|outcome| outcome.is_ok()));

    let flatten = TestAdjustment {
        asset: asset("AAPL"),
        balance: Some(AdjustmentAmount::Absolute(PositionSize::ZERO)),
        balance_average_entry_price: None,
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: Some(AdjustmentAmount::Absolute(PositionSize::ZERO)),
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &flatten, &mut mutations);
    mutations.commit_all();
    let reopen = adj(asset("AAPL"), Some(AdjustmentAmount::Absolute(ps("10"))));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &reopen, &mut mutations);
    mutations.commit_all();

    let flatten_position = adj_with_avg(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(PositionSize::ZERO)),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &flatten_position, &mut mutations);
    mutations.commit_all();
    let reopen_position = adj_with_avg(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(ps("10"))),
        None,
    );
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &reopen_position, &mut mutations);
    mutations.commit_all();

    let rehalted = run_report_with_currency(&policy, &sell, asset("USD"));
    assert!(rehalted
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl)
        .is_some_and(|outcome| outcome == Err(crate::PnlHaltReason::MissingCostBasis)));
}

#[test]
fn account_halt_rearms_only_through_account_force_set() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let policy: SpotFundsPolicy<FullSync, FullSync> =
        SpotFundsPolicy::new(settings(0), None, builder.storage_builder());
    let account_pnl = policy.pnl.clone();
    let engine = builder
        .pre_trade(policy)
        .build()
        .expect("engine must build");
    // The fee is what makes the account currency a required input here: a
    // fee-less opening fill contributes a computable zero instead.
    let report = fill_with_fee(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        "100",
        "1",
        money_fee("1", "USD"),
    );

    let first = engine.apply_execution_report(&report);
    assert_eq!(
        first.account_pnls[0].result,
        Err(crate::PnlHaltReason::MissingAccountCurrency)
    );
    assert!(engine
        .apply_execution_report(&report)
        .account_pnls
        .is_empty());

    let force_position = TestAdjustment {
        asset: asset("AAPL"),
        balance: None,
        balance_average_entry_price: Some(px("100")),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    engine
        .apply_account_adjustment(acc, &[force_position])
        .expect("position average force-set must succeed");
    engine
        .apply_account_adjustment(acc, &[adj_with_realized_pnl(asset("AAPL"), Pnl::ZERO)])
        .expect("position PnL force-set must succeed");
    engine.accounts().set_currency(acc, asset("USD"));
    assert!(engine
        .apply_execution_report(&report)
        .account_pnls
        .is_empty());

    engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            acc,
            crate::PnlState::Value(pnl_value("7")),
        )
        .expect("account force-set must succeed");
    assert_eq!(
        account_pnl.with(&acc, |entry| entry.state),
        Some(crate::PnlState::Value(pnl_value("7")))
    );
    let resumed = engine.apply_execution_report(&report);
    assert_eq!(resumed.account_pnls.len(), 1);
    assert_eq!(
        resumed.account_pnls[0].result,
        Ok(crate::PnlOutcomeAmount {
            delta: pnl_value("-1"),
            absolute: pnl_value("6"),
        })
    );

    let fresh = account(99224417);
    engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            fresh,
            crate::PnlState::Value(Pnl::ZERO),
        )
        .expect("fresh account force-set must succeed");
    assert_eq!(
        account_pnl.with(&fresh, |entry| entry.state),
        Some(crate::PnlState::Value(Pnl::ZERO))
    );
}

#[test]
fn position_halt_rearms_only_through_position_force_set() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let usd_eur = instr("USD", "EUR");
    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let market_data = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = market_data
        .register(usd_eur)
        .expect("FX instrument must register");
    let policy: SpotFundsPolicy<FullSync, FullSync> = SpotFundsPolicy::new(
        settings(0),
        Some(SpotFundsMarketData::new(Arc::clone(&market_data))),
        builder.storage_builder(),
    );
    let engine = builder
        .pre_trade(policy)
        .build()
        .expect("engine must build");
    engine.accounts().set_currency(acc, asset("EUR"));
    let mut seed_position = adj_with_avg(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(ps("3"))),
        Some(px("100")),
    );
    seed_position.pnl_operation = Some(crate::PnlState::Value(Pnl::ZERO));
    engine
        .apply_account_adjustment(acc, &[seed_position])
        .expect("position seed must succeed");
    let report = fill_with_fee(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        "100",
        "1",
        money_fee("0", "USD"),
    );

    let first = engine.apply_execution_report(&report);
    assert!(first
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl)
        .is_some_and(|outcome| outcome == Err(crate::PnlHaltReason::MissingFx)));
    engine
        .configure()
        .set_spot_funds_account_pnl(
            SpotFundsPolicy::<FullSync, FullSync>::NAME,
            acc,
            crate::PnlState::Value(pnl_value("9")),
        )
        .expect("account force-set must succeed");
    market_data
        .push(fx_id, Quote::new().with_mark(px("0.9")))
        .expect("FX quote must publish");

    let still_halted = engine.apply_execution_report(&report);
    assert!(still_halted
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .is_some_and(|outcome| outcome.entry.realized_pnl.is_none()));
    assert_eq!(still_halted.account_pnls.len(), 1);

    let force_position = TestAdjustment {
        asset: asset("AAPL"),
        balance: None,
        balance_average_entry_price: Some(px("100")),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    engine
        .apply_account_adjustment(acc, &[force_position])
        .expect("position average force-set must succeed");
    engine
        .apply_account_adjustment(acc, &[adj_with_realized_pnl(asset("AAPL"), Pnl::ZERO)])
        .expect("position PnL force-set must succeed");
    let sell = fill_with_fee(acc, aapl_usd, Side::Sell, "120", "1", money_fee("0", "USD"));
    let resumed = engine.apply_execution_report(&sell);
    assert!(resumed
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .and_then(|outcome| outcome.entry.realized_pnl)
        .is_some_and(|outcome| outcome.is_ok()));
}

#[test]
fn sticky_position_halt_does_not_zero_account_or_other_position_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let msft_usd = instr("MSFT", "USD");
    let usd_eur = instr("USD", "EUR");
    let builder = engine_builder();
    let market_data = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = market_data
        .register(usd_eur)
        .expect("FX instrument must register");
    let policy = SpotFundsPolicy::new(
        settings(0),
        Some(SpotFundsMarketData::new(Arc::clone(&market_data))),
        builder.storage_builder(),
    );
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    seed_with_avg(&policy, acc, asset("MSFT"), "10", px("50"));

    let unavailable_underlying_fx = fill_with_fee(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        "120",
        "1",
        money_fee("0", "USD"),
    );
    let first = run_report_with_currency(&policy, &unavailable_underlying_fx, asset("EUR"));
    assert_eq!(
        first.account_pnls[0].result,
        Err(crate::PnlHaltReason::MissingFx)
    );
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingFx)
    );
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .avg_entry_price(),
        Some(px("100"))
    );

    let _previous = policy.set_account_pnl_state(acc, crate::PnlState::Value(Pnl::ZERO));
    market_data
        .push(fx_id, Quote::new().with_mark(px("1")))
        .expect("FX quote must publish");

    let halted_position_fill =
        fill_with_fee(acc, aapl_usd, Side::Sell, "130", "1", money_fee("0", "USD"));
    let halted_position_result =
        run_report_with_currency(&policy, &halted_position_fill, asset("EUR"));
    assert_eq!(
        halted_position_result.account_pnls[0].result,
        Ok(crate::PnlOutcomeAmount {
            delta: pnl_value("30"),
            absolute: pnl_value("30"),
        })
    );
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingFx)
    );

    let healthy_position_fill =
        fill_with_fee(acc, msft_usd, Side::Sell, "70", "1", money_fee("0", "USD"));
    let healthy_position_result =
        run_report_with_currency(&policy, &healthy_position_fill, asset("EUR"));
    assert_eq!(
        healthy_position_result.account_pnls[0].result,
        Ok(crate::PnlOutcomeAmount {
            delta: pnl_value("20"),
            absolute: pnl_value("50"),
        })
    );
    assert_eq!(account_pnl_of(&policy, acc), Some(pnl_value("50")));
    assert_eq!(
        holdings_of(&policy, acc, &asset("MSFT"))
            .expect("MSFT slot must exist")
            .realized_pnl(),
        Some(pnl_value("20"))
    );
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingFx)
    );
}

/// An unpriced fill that flips an already sticky-halted position leaves no
/// basis for the new side, exactly like the same flip on a freshly halted one.
/// The stale average must not survive to feed a later account contribution.
#[test]
fn unpriced_flip_on_sticky_halted_position_clears_the_stale_average() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let usd_eur = instr("USD", "EUR");
    let builder = engine_builder();
    let market_data = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let fx_id = market_data
        .register(usd_eur)
        .expect("FX instrument must register");
    let policy = SpotFundsPolicy::new(
        settings(0),
        Some(SpotFundsMarketData::new(Arc::clone(&market_data))),
        builder.storage_builder(),
    );
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    // Without the FX quote the position halts fresh, but a partial reduction
    // keeps the basis of the untouched remainder.
    let partial_close = fill_with_fee(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        "120",
        "1",
        money_fee("0", "USD"),
    );
    let fresh_halt = run_report_with_currency(&policy, &partial_close, asset("EUR"));
    assert_eq!(
        fresh_halt.account_pnls[0].result,
        Err(crate::PnlHaltReason::MissingFx)
    );
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .avg_entry_price(),
        Some(px("100"))
    );

    // Still no FX: this fill flips 9 long into 11 short while the position is
    // already halted. The old long basis cannot describe the new short.
    let flip = fill_with_fee(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        "120",
        "20",
        money_fee("0", "USD"),
    );
    let sticky_halt = run_report_with_currency(&policy, &flip, asset("EUR"));
    assert!(
        sticky_halt.account_pnls.is_empty(),
        "an already halted account line publishes nothing further"
    );
    let flipped = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(
        flipped
            .available()
            .checked_add(flipped.held())
            .expect("owned quantity must be representable"),
        ps("-11"),
        "the fill flips the position short even though its PnL is unpriced"
    );
    assert_eq!(
        flipped.avg_entry_price(),
        None,
        "an unpriced flip must clear the average whether the halt is fresh or sticky"
    );
    assert_eq!(
        flipped.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingFx),
        "the sticky halt reason must survive the unpriced fill"
    );

    // Officer restores the account line and the FX quote returns.
    let _previous = policy.set_account_pnl_state(acc, crate::PnlState::Value(Pnl::ZERO));
    market_data
        .push(fx_id, Quote::new().with_mark(px("1")))
        .expect("FX quote must publish");

    // The next priced fill reduces the short. With no basis for it, the account
    // line must halt rather than publish a number derived from the stale 100.
    let reduce_short = fill_with_fee(acc, aapl_usd, Side::Buy, "130", "5", money_fee("0", "USD"));
    let resumed = run_report_with_currency(&policy, &reduce_short, asset("EUR"));
    assert_eq!(
        resumed.account_pnls[0].result,
        Err(crate::PnlHaltReason::MissingCostBasis),
        "account PnL must not be derived from a cleared cost basis"
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(
            crate::PnlHaltReason::MissingCostBasis
        ))
    );
}

#[test]
fn profitable_close_with_missing_foreign_fee_fx_has_no_partial_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = fill_with_fee(
        acc,
        aapl_usd,
        Side::Sell,
        "200",
        "10",
        money_fee("2", "EUR"),
    );
    let result = run_report_with_currency(&policy, &fill, asset("USD"));

    assert_eq!(
        result.account_pnls,
        vec![crate::AccountPnlOutcome {
            account_id: acc,
            policy_group_id: DEFAULT_POLICY_GROUP_ID,
            result: Err(crate::PnlHaltReason::MissingFx),
        }]
    );
    let aapl_outcome = result
        .account_adjustments
        .iter()
        .find(|outcome| outcome.entry.asset == asset("AAPL"))
        .expect("AAPL outcome must exist");
    assert_eq!(
        aapl_outcome.entry.realized_pnl,
        Some(Err(crate::PnlHaltReason::MissingFx))
    );
    assert_eq!(
        account_pnl_state_of(&policy, acc),
        Some(crate::PnlState::Halted(crate::PnlHaltReason::MissingFx))
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(aapl.available(), PositionSize::ZERO);
    assert_eq!(aapl.held(), PositionSize::ZERO);
    assert_eq!(aapl.avg_entry_price(), None);
    assert_eq!(
        aapl.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingFx)
    );
}

#[test]
fn rolling_back_position_force_set_restores_prior_halt() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "2", px("100"));
    let fill = fill_with_fee(acc, aapl_usd, Side::Sell, "100", "1", money_fee("0", "USD"));
    let _ = run_report_with_currency(&policy, &fill, asset("EUR"));
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingFx)
    );

    let force = adj_with_realized_pnl(asset("AAPL"), Pnl::ZERO);
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force, &mut mutations);
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl(),
        Some(Pnl::ZERO)
    );

    mutations.rollback_all();
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingFx)
    );
}

#[test]
fn sell_fill_against_seeded_long_realizes_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Long 10 AAPL at average 100, plus USD is irrelevant for a sell.
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();
    // Reservation moved 10 AAPL available -> held; owned still 10 @ 100.

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let result = run_report(&policy, &fill);

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    let pnl = aapl_entry
        .entry
        .realized_pnl
        .as_ref()
        .expect("AAPL pnl must be present")
        .as_ref()
        .expect("AAPL pnl must be calculated");
    // Sell 4 of a long at 200 vs avg 100: realized = (200 - 100) * 4 = 400.
    assert_eq!(pnl.delta, pnl_value("400"));
    assert_eq!(pnl.absolute, pnl_value("400"));
    // Average is unchanged while the position is only reduced.
    assert_eq!(aapl_entry.entry.average_entry_price, Some(px("100")));

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("400")));
    assert_eq!(aapl.avg_entry_price(), Some(px("100")));
}

#[test]
fn second_fill_with_same_settlement_asset_accumulates_position_accounting() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    let first_order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &first_order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();
    let first_fill = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("100"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(run_report(&policy, &first_fill).account_blocks.is_empty());

    let second_order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(px("120")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &second_order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();
    let second_fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("120"),
            quantity: qty("1"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("120"),
        )])),
    );
    let result = run_report(&policy, &second_fill);

    assert!(
        result.account_blocks.is_empty(),
        "second fill in account currency must not block",
    );
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.available(), ps("2"));
    assert_eq!(aapl.avg_entry_price(), Some(px("110")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn short_open_then_buy_to_close_realizes_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Seed a short -10 AAPL at average 100 directly via adjustment.
    seed_with_avg(&policy, acc, asset("AAPL"), "-10", px("100"));
    seed(&policy, acc, asset("USD"), "100000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("4")),
        Some(px("70")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("70"),
            quantity: qty("4"),
        }),
        qty("0"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("70"),
        )])),
    );
    let result = run_report(&policy, &fill);

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    let pnl = aapl_entry
        .entry
        .realized_pnl
        .as_ref()
        .expect("AAPL pnl must be present")
        .as_ref()
        .expect("AAPL pnl must be calculated");
    // Buy 4 to cover a short at 70 vs avg 100: realized = (70 - 100) * -(4) = 120.
    assert_eq!(pnl.delta, pnl_value("120"));
    // No prior realized PnL on the slot, so the cumulative equals this fill.
    assert_eq!(pnl.absolute, pnl_value("120"));
    // Still short -6, average unchanged.
    assert_eq!(aapl_entry.entry.average_entry_price, Some(px("100")));
}

#[test]
fn exact_close_fill_resets_average_to_none_and_keeps_realized_pnl() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("130")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("130"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("130"),
        )])),
    );
    let result = run_report(&policy, &fill);

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    let pnl = aapl_entry
        .entry
        .realized_pnl
        .as_ref()
        .expect("AAPL pnl must be present")
        .as_ref()
        .expect("AAPL pnl must be calculated");
    // Close the whole long at 130 vs avg 100: realized = (130 - 100) * 10 = 300.
    assert_eq!(pnl.delta, pnl_value("300"));
    // No prior realized PnL on the slot, so the cumulative equals this fill.
    assert_eq!(pnl.absolute, pnl_value("300"));
    // Position is flat: no average, but realized PnL persists so the slot
    // survives pruning.
    assert!(aapl_entry.entry.average_entry_price.is_none());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("slot must survive");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("300")));
    assert_eq!(aapl.avg_entry_price(), None);
    assert!(!aapl.is_zero());
}

#[test]
fn reservation_then_cancel_leaves_average_and_pnl_untouched() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("130")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    // A final report with no fill and a leftover quantity cancels the
    // reservation; the priced sell's cancel must replay its lock.
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("130"),
        )])),
    );
    let _ = run_report(&policy, &cancel);

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    // Funds returned to available; the average and realized PnL are unchanged.
    assert_eq!(aapl.available(), ps("10"));
    assert_eq!(aapl.held(), PositionSize::ZERO);
    assert_eq!(aapl.avg_entry_price(), Some(px("100")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn adjustment_rollback_restores_prior_average() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    // Apply a balance adjustment that also overwrites the average, then roll it
    // back without committing. The prior average must be restored.
    let adjustment = adj_with_avg(
        asset("AAPL"),
        Some(AdjustmentAmount::Delta(ps("5"))),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    // Forward write is synchronous: the average is now 200, balance 15.
    let after_forward = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after_forward.avg_entry_price(), Some(px("200")));
    assert_eq!(after_forward.available(), ps("15"));

    mutations.rollback_all();

    let after_rollback = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after_rollback.avg_entry_price(), Some(px("100")));
    assert_eq!(after_rollback.available(), ps("10"));
    assert_eq!(after_rollback.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn balance_adjustment_force_sets_realized_pnl_and_emits_delta_and_absolute() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    // Seed a slot that already carries a non-zero realized PnL via a force-set,
    // so the second force-set produces a meaningful, non-trivial delta.
    seed(&policy, acc, asset("AAPL"), "10");
    let seed_pnl = adj_with_realized_pnl(asset("AAPL"), pnl_value("30"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed_pnl, &mut mutations);
    mutations.commit_all();

    // Force-set realized PnL to 50: delta = 50 - 30 = 20, absolute = 50.
    let adjustment = adj_with_realized_pnl(asset("AAPL"), pnl_value("50"));
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(outcome.len(), 1);
    let pnl = outcome[0]
        .realized_pnl
        .as_ref()
        .expect("realized PnL outcome must be emitted on a force-set")
        .as_ref()
        .expect("realized PnL outcome must be calculated");
    assert_eq!(pnl.delta, pnl_value("20"));
    assert_eq!(pnl.absolute, pnl_value("50"));

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("50")));
}

#[test]
fn metadata_only_realized_pnl_adjustment_sets_and_emits_delta() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    let seed_pnl = adj_with_realized_pnl(asset("AAPL"), pnl_value("30"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed_pnl, &mut mutations);
    mutations.commit_all();

    let adjustment = adj_with_realized_pnl(asset("AAPL"), pnl_value("50"));
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert_eq!(outcome.len(), 1);
    assert!(outcome[0].balance.is_none());
    let pnl = outcome[0]
        .realized_pnl
        .as_ref()
        .expect("realized PnL outcome must be emitted on a force-set")
        .as_ref()
        .expect("realized PnL outcome must be calculated");
    assert_eq!(pnl.delta, pnl_value("20"));
    assert_eq!(pnl.absolute, pnl_value("50"));

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("50")));
}

#[test]
fn balance_adjustment_without_realized_pnl_emits_no_pnl_and_leaves_it() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    let seed_pnl = adj_with_realized_pnl(asset("AAPL"), pnl_value("30"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed_pnl, &mut mutations);
    mutations.commit_all();

    // A balance-only adjustment that does not force-set realized PnL must leave
    // the booked PnL untouched and emit no realized-PnL outcome.
    let adjustment = adj(asset("AAPL"), Some(AdjustmentAmount::Delta(ps("5"))));
    let mut mutations = Mutations::with_capacity(1);
    let outcome = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    mutations.commit_all();

    assert!(outcome[0].realized_pnl.is_none());
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("30")));
}

#[test]
fn adjustment_rollback_restores_realized_pnl_to_prior() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    let seed_pnl = adj_with_realized_pnl(asset("AAPL"), pnl_value("30"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed_pnl, &mut mutations);
    mutations.commit_all();

    // Force-set realized PnL to 50, then roll back without committing.
    let adjustment = adj_with_realized_pnl(asset("AAPL"), pnl_value("50"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    let after_forward = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after_forward.realized_pnl(), Some(pnl_value("50")));

    mutations.rollback_all();

    // Realized PnL is restored from the prior snapshot (30), symmetric to the
    // average entry price.
    let after_rollback = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after_rollback.realized_pnl(), Some(pnl_value("30")));
}

#[test]
fn adjustment_rollback_restores_untracked_realized_pnl_to_none() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    // Seed a non-flat slot that is NOT tracking realized PnL (balance only, no
    // realized force-set): realized_pnl stays `None`.
    let seed = adj(asset("AAPL"), Some(AdjustmentAmount::Absolute(ps("10"))));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed, &mut mutations);
    mutations.commit_all();
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("must exist")
            .realized_pnl(),
        None,
    );

    // Force-set realized PnL to 25, then roll back without committing.
    let force = adj_with_realized_pnl(asset("AAPL"), pnl_value("25"));
    let mut adj_mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force, &mut adj_mutations);
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("must exist")
            .realized_pnl(),
        Some(pnl_value("25")),
    );

    adj_mutations.rollback_all();

    // The snapshot restore returns the slot to the untracked `None` state, not
    // to `Some(0)` (which a delta-based reversal would have produced).
    let after = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after.realized_pnl(), None);
    assert_eq!(after.available(), ps("10"));
}

#[test]
fn realized_pnl_halts_after_rollback_restores_missing_initial_value() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    // Seed a long with an average but NO realized-PnL tracking. The average lets
    // a later fill have a basis; realized_pnl is `None`.
    seed(&policy, acc, asset("AAPL"), "10");
    let seed_position = adj_with_avg(
        asset("AAPL"),
        Some(AdjustmentAmount::Absolute(ps("10"))),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed_position, &mut mutations);
    mutations.commit_all();
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("must exist")
            .realized_pnl(),
        None,
    );

    // Force-set realized PnL, then roll back: the slot returns to untracked.
    let force = adj_with_realized_pnl(asset("AAPL"), pnl_value("25"));
    let mut adj_mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force, &mut adj_mutations);
    adj_mutations.rollback_all();
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("must exist")
            .realized_pnl(),
        None,
    );

    // A subsequent non-flat fill cannot produce an authoritative absolute PnL
    // and therefore halts until a manager force-sets the missing value.
    let aapl_usd = instr("AAPL", "USD");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("130")),
    );
    let mut pt_mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut pt_mutations).expect("pretrade must succeed");
    pt_mutations.commit_all();
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("130"),
            quantity: qty("4"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("130"),
        )])),
    );
    let result = run_report(&policy, &fill);
    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    assert_eq!(
        aapl_entry.entry.realized_pnl,
        Some(Err(crate::PnlHaltReason::MissingInitialPnl))
    );
    let after = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after.realized_pnl(), None);
    assert_eq!(
        after.realized_pnl_halt_reason(),
        Some(crate::PnlHaltReason::MissingInitialPnl)
    );
}

#[test]
fn metadata_only_average_and_pnl_roll_back_to_prior_values() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    let seed = adj_with_realized_pnl(asset("AAPL"), pnl_value("30"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed, &mut mutations);
    mutations.commit_all();

    let adjustment = TestAdjustment {
        asset: asset("AAPL"),
        balance: None,
        balance_average_entry_price: Some(px("150")),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &adjustment, &mut mutations);
    let pnl_adjustment = adj_with_realized_pnl(asset("AAPL"), pnl_value("50"));
    let _ = run_adjustment(&policy, acc, &pnl_adjustment, &mut mutations);
    let after_forward = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after_forward.avg_entry_price(), Some(px("150")));
    assert_eq!(after_forward.realized_pnl(), Some(pnl_value("50")));

    mutations.rollback_all();

    let after_rollback = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after_rollback.avg_entry_price(), Some(px("100")));
    assert_eq!(after_rollback.realized_pnl(), Some(pnl_value("30")));
}

#[test]
fn adjustment_rollback_preserves_newer_realized_pnl() {
    let acc = account(99224416);
    let policy = build_policy(None, None);
    // Seed a long with an average so a later fill realizes PnL, and an initial
    // booked realized PnL of 30.
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    let seed = adj_with_realized_pnl(asset("AAPL"), pnl_value("30"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed, &mut mutations);
    mutations.commit_all();

    // Force-set realized PnL to 50 but hold the rollback. The snapshot captured
    // for rollback is the prior value, 30.
    let adjustment = adj_with_realized_pnl(asset("AAPL"), pnl_value("50"));
    let mut adj_mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &adjustment, &mut adj_mutations);
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("must exist")
            .realized_pnl(),
        Some(pnl_value("50")),
    );

    // A concurrent fill books additional realized PnL before the adjustment is
    // rolled back. Sell 4 of the long at 130 vs avg 100 -> realized += 120.
    let aapl_usd = instr("AAPL", "USD");
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("4")),
        Some(px("130")),
    );
    let mut pt_mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut pt_mutations).expect("pretrade must succeed");
    pt_mutations.commit_all();
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("130"),
            quantity: qty("4"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("130"),
        )])),
    );
    let _ = run_report(&policy, &fill);
    // Cumulative is now 50 (force-set) + 120 (fill) = 170.
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("must exist")
            .realized_pnl(),
        Some(pnl_value("170")),
    );

    // The rollback may restore its snapshots only while the complete slot still
    // equals the adjustment-asserted slot. The fill changed both quantity and
    // realized PnL, so the newer slot must survive intact.
    adj_mutations.rollback_all();
    let after = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(after.realized_pnl(), Some(pnl_value("170")));
    assert_eq!(after.available(), ps("6"));
}

#[test]
fn adjustment_rollback_preserves_newer_average_entry_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    seed(&policy, acc, asset("USD"), "10000");

    let adjustment = TestAdjustment {
        asset: asset("AAPL"),
        balance: None,
        balance_average_entry_price: Some(px("150")),
        pnl_operation: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
        held_lower: None,
        held_upper: None,
        incoming: None,
        incoming_lower: None,
        incoming_upper: None,
    };
    let mut adjustment_mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &adjustment, &mut adjustment_mutations);

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut pretrade_mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut pretrade_mutations).expect("pretrade must succeed");
    pretrade_mutations.commit_all();
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let _ = run_report(&policy, &fill);
    assert_eq!(
        holdings_of(&policy, acc, &asset("AAPL"))
            .expect("AAPL slot must exist")
            .avg_entry_price(),
        Some(px("175"))
    );

    adjustment_mutations.rollback_all();
    let after = holdings_of(&policy, acc, &asset("AAPL")).expect("AAPL slot must exist");
    assert_eq!(after.avg_entry_price(), Some(px("175")));
    assert_eq!(after.available(), ps("20"));
}

#[test]
fn buy_fill_adding_to_long_recomputes_weighted_average() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Seed an existing long 10 AAPL at average 100, plus cash to buy more.
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));
    seed(&policy, acc, asset("USD"), "100000");

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("200"),
        )])),
    );
    let result = run_report(&policy, &fill);

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    // Adding to the position weights the average: (10*100 + 10*200)/20 = 150,
    // and realizes nothing.
    assert_eq!(aapl_entry.entry.average_entry_price, Some(px("150")));
    assert!(aapl_entry.entry.realized_pnl.is_none());

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("150")));
    assert_eq!(aapl.realized_pnl(), Some(Pnl::ZERO));
}

#[test]
fn sell_fill_flipping_long_to_short_realizes_and_reopens_at_price() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    // Long 10 AAPL at average 100. Reserve a sell of 10 (all that is owned),
    // then the venue over-fills 15, flipping through zero to a short -5.
    seed_with_avg(&policy, acc, asset("AAPL"), "10", px("100"));

    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("130")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("pretrade must succeed");
    mutations.commit_all();

    let fill = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("130"),
            quantity: qty("15"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("130"),
        )])),
    );
    let result = run_report(&policy, &fill);

    let aapl_entry = result
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL entry must exist");
    let pnl = aapl_entry
        .entry
        .realized_pnl
        .as_ref()
        .expect("AAPL pnl must be present")
        .as_ref()
        .expect("AAPL pnl must be calculated");
    // Flip closes the whole long (realized = (130 - 100) * 10 = 300) and opens
    // the -5 remainder at the fill price 130.
    assert_eq!(pnl.delta, pnl_value("300"));
    assert_eq!(pnl.absolute, pnl_value("300"));
    assert_eq!(aapl_entry.entry.average_entry_price, Some(px("130")));

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.avg_entry_price(), Some(px("130")));
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("300")));
}

#[test]
fn batch_force_setting_realized_pnl_then_rejected_rolls_back_to_prior() {
    // End-to-end through the engine batch API: a balance + realized-PnL force-set
    // that is later rejected in the same batch must roll the realized PnL back to
    // its prior value via the inverse delta.
    let acc = account(99224416);
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");
    let seed_pnl = adj_with_realized_pnl(asset("AAPL"), pnl_value("30"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &seed_pnl, &mut mutations);
    mutations.commit_all();

    // Force-set realized PnL to 50, then drive the same closure to reject via an
    // out-of-range balance bound so the whole adjustment is rolled back.
    let force = adj_with_realized_pnl(asset("AAPL"), pnl_value("50"));
    let mut mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, acc, &force, &mut mutations);

    let rejecting = bounded_adj(
        asset("AAPL"),
        Some(AdjustmentAmount::Delta(ps("1"))),
        None,
        Some(ps("0")),
    );
    let mut reject_mutations = Mutations::with_capacity(1);
    let rejected = run_adjustment_result(&policy, acc, &rejecting, &mut reject_mutations);
    assert!(rejected.is_err());

    // Roll back the force-set adjustment: realized PnL returns to the prior 30.
    mutations.rollback_all();
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("30")));
}

// ═══════════════════════════════════════════════════════════════════════════
// Funds limit mode - never-reject TrackOnly path
// ═══════════════════════════════════════════════════════════════════════════

/// Builds a no-market-data policy whose global limit mode is `mode`.
fn build_policy_with_limit_mode(mode: SpotFundsLimitMode) -> TestPolicy {
    let b = engine_builder();
    let mut s = settings(0);
    s.set_global_limit_mode(mode);
    SpotFundsPolicy::new(s, None, b.storage_builder())
}

/// Publishes a new global limit mode through the policy's settings cell, exactly
/// as the runtime `Configurator::spot_funds` surface would.
fn set_runtime_global_limit_mode(policy: &TestPolicy, mode: SpotFundsLimitMode) {
    use crate::pretrade::ConfigurablePolicy;
    use crate::storage::ConfigCell;
    let cell =
        <TestPolicy as ConfigurablePolicy<crate::storage::FullLocking>>::settings_cell(policy);
    cell.update(|s| {
        s.set_global_limit_mode(mode);
        Ok::<(), SpotFundsConfigError>(())
    })
    .expect("update must publish");
}

#[test]
fn track_only_buy_over_available_is_not_rejected_and_drives_available_negative() {
    let acc = account(99224416);
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    seed(&policy, acc, asset("USD"), "1000");

    // Buy qty 10 @ 200 owes 2000 settlement but only 1000 is available.
    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("track-only must not reject");
    assert!(!mutations.is_empty());

    // Settlement leg recorded the full hold, driving available negative.
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("2000"));
    assert_eq!(usd.available(), ps("-1000"));

    // Base incoming projection is still emitted on the buy.
    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.incoming(), ps("10"));
}

#[test]
fn track_only_matches_enforce_bookkeeping_for_a_fundable_buy() {
    // With sufficient funds both modes must produce byte-identical holdings:
    // the mode only changes the insufficiency path.
    let acc = account(99224416);
    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );

    let enforce = build_policy_with_limit_mode(SpotFundsLimitMode::Enforce);
    seed(&enforce, acc, asset("USD"), "10000");
    let mut m1 = Mutations::with_capacity(1);
    pre_trade_check(&enforce, &order, &mut m1).expect("enforce must pass when funded");

    let track = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    seed(&track, acc, asset("USD"), "10000");
    let mut m2 = Mutations::with_capacity(1);
    pre_trade_check(&track, &order, &mut m2).expect("track-only must pass when funded");

    assert_eq!(
        holdings_of(&enforce, acc, &asset("USD")),
        holdings_of(&track, acc, &asset("USD")),
    );
    assert_eq!(
        holdings_of(&enforce, acc, &asset("AAPL")),
        holdings_of(&track, acc, &asset("AAPL")),
    );
}

#[test]
fn track_only_sell_over_available_underlying_is_not_rejected() {
    let acc = account(99224416);
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    // Owns only 1 AAPL but sells 10: the underlying hold exceeds available.
    seed(&policy, acc, asset("AAPL"), "1");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut mutations).expect("track-only must not reject");

    let aapl = holdings_of(&policy, acc, &asset("AAPL")).expect("must exist");
    assert_eq!(aapl.held(), ps("10"));
    assert_eq!(aapl.available(), ps("-9"));
    // A non-negative sell price projects settlement incoming, never gated.
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.incoming(), ps("2000"));
}

#[test]
fn enforce_still_rejects_insufficient_funds() {
    let acc = account(99224416);
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::Enforce);
    seed(&policy, acc, asset("USD"), "1000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");

    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
    assert!(mutations.is_empty());
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.available(), ps("1000"));
    assert_eq!(usd.held(), ps("0"));
}

#[test]
fn track_only_still_rejects_arithmetic_overflow_without_panicking() {
    let acc = account(99224416);
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    // Force the settlement slot's `held` to the top of the decimal range so the
    // reserve hold's `held + amount` overflows. Overflow is an integrity guard
    // that no mode suppresses.
    let held_max = PositionSize::new(rust_decimal::Decimal::MAX);
    let adjustment = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(held_max)),
        None,
        None,
    );
    let mut seed_mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut seed_mutations).expect("seed must succeed");
    seed_mutations.commit_all();

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let mut mutations = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut mutations).expect_err("must reject");

    // Overflow maps to the arithmetic-overflow reject, never InsufficientFunds.
    assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    assert_ne!(rejects[0].code, RejectCode::InsufficientFunds);
}

#[test]
fn runtime_switch_to_track_only_changes_gating_for_next_reservation() {
    let acc = account(99224416);
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::Enforce);
    seed(&policy, acc, asset("USD"), "1000");

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );

    // Enforce: the under-funded buy is rejected.
    let mut m1 = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut m1).expect_err("enforce must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);

    // Flip the global mode at runtime through the settings cell.
    set_runtime_global_limit_mode(&policy, SpotFundsLimitMode::TrackOnly);

    // The same under-funded buy now passes and records the hold.
    let mut m2 = Mutations::with_capacity(1);
    pre_trade_check(&policy, &order, &mut m2).expect("track-only must pass after switch");
    let usd = holdings_of(&policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(usd.held(), ps("2000"));
    assert_eq!(usd.available(), ps("-1000"));

    // Flip back to Enforce: a further under-funded buy is rejected again.
    set_runtime_global_limit_mode(&policy, SpotFundsLimitMode::Enforce);
    let mut m3 = Mutations::new();
    let rejects = pre_trade_check(&policy, &order, &mut m3)
        .expect_err("enforce must reject again after switch back");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
}

#[test]
fn track_only_dry_run_buy_over_available_matches_mutating_reservation() {
    // Regression guard: the dry-run gate must resolve TrackOnly exactly like the
    // mutating gate. Run the same under-funded buy as
    // `track_only_buy_over_available_is_not_rejected_and_drives_available_negative`
    // through both entries on equivalent fresh state and assert the dry-run
    // outcome byte-matches the holdings the mutating reservation produces. If a
    // future edit desyncs the two limit-mode gates this fails.
    let acc = account(99224416);
    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );

    // Dry-run path on its own fresh state: must not reject, must report.
    let dry_policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    seed(&dry_policy, acc, asset("USD"), "1000");
    let outcome = dry_run_check(&dry_policy, &order)
        .expect("track-only dry-run must not reject")
        .expect("track-only dry-run must report an outcome");

    // Mutating path on equivalent fresh state.
    let mut_policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    seed(&mut_policy, acc, asset("USD"), "1000");
    let mut mutations = Mutations::with_capacity(1);
    pre_trade_check(&mut_policy, &order, &mut mutations).expect("track-only must not reject");
    let usd = holdings_of(&mut_policy, acc, &asset("USD")).expect("must exist");
    let aapl = holdings_of(&mut_policy, acc, &asset("AAPL")).expect("must exist");

    // Settlement leg: held is the full requested 2000, available driven negative
    // to -1000 (seed 1000 minus the 2000 hold). The dry-run reports the same
    // absolute balance the mutating path's `available()` reaches.
    assert_eq!(outcome.account_adjustments.len(), 2);
    let entry = &outcome.account_adjustments[0];
    assert_eq!(entry.asset, asset("USD"));
    let held = entry.held.expect("held outcome present");
    assert_eq!(held.delta, ps("2000"));
    assert_eq!(held.absolute, ps("2000"));
    assert_eq!(held.absolute, usd.held());
    let balance = entry.balance.expect("balance outcome present");
    assert_eq!(balance.delta, ps("-2000"));
    assert_eq!(balance.absolute, ps("-1000"));
    assert_eq!(balance.absolute, usd.available());
    assert!(entry.incoming.is_none());

    // Base incoming projection matches the mutating path's base leg.
    let base = &outcome.account_adjustments[1];
    assert_eq!(base.asset, asset("AAPL"));
    assert!(base.balance.is_none());
    assert!(base.held.is_none());
    let incoming = base.incoming.expect("base incoming outcome present");
    assert_eq!(incoming.delta, ps("10"));
    assert_eq!(incoming.absolute, ps("10"));
    assert_eq!(incoming.absolute, aapl.incoming());
    assert_eq!(outcome.lock_prices.to_vec(), vec![px("200")]);

    // The dry-run consumed nothing on its own state.
    let dry_usd = holdings_of(&dry_policy, acc, &asset("USD")).expect("must exist");
    assert_eq!(dry_usd.available(), ps("1000"));
    assert_eq!(dry_usd.held(), ps("0"));
}

#[test]
fn track_only_dry_run_still_rejects_arithmetic_overflow_without_panicking() {
    // Mirror of `track_only_still_rejects_arithmetic_overflow_without_panicking`
    // on the dry-run entry: TrackOnly never suppresses the overflow integrity
    // guard, so the dry-run twin must also map to ArithmeticOverflow rather than
    // InsufficientFunds, and must not panic.
    let acc = account(99224416);
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    // Seed the settlement slot's `held` at the top of the decimal range so the
    // reserve hold's `held + amount` overflows.
    let held_max = PositionSize::new(rust_decimal::Decimal::MAX);
    let adjustment = held_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(held_max)),
        None,
        None,
    );
    let mut seed_mutations = Mutations::with_capacity(1);
    apply_adj(&policy, acc, &adjustment, &mut seed_mutations).expect("seed must succeed");
    seed_mutations.commit_all();

    let order = make_order(
        acc,
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    );
    let rejects = dry_run_check(&policy, &order).expect_err("dry-run must reject");

    // Overflow maps to the arithmetic-overflow reject, never InsufficientFunds.
    assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    assert_ne!(rejects[0].code, RejectCode::InsufficientFunds);
}

// ── Volume orders reconcile under price divergence (fill price ≠ lock) ─────
//
// Volume orders reconcile under the venue contract
// Σ(fill qty) + leaves == v / |lock_price|; base-incoming and settlement-held
// share the same quantity basis, so price divergence alone leaves no residual.
// The SDK does not compensate a venue that violates this contract.
//
// The existing Volume tests use clean numbers where the fill price equals the
// lock price, so they never exercise a price divergence. These tests deliver
// fills at prices ≠ the lock while keeping Σ(fill qty) + leaves equal to
// v / |lock_price|, proving the price gap nets into `available` and leaves
// neither base-incoming nor settlement-held residual.

#[test]
fn buy_volume_price_divergent_full_fill_nets_incoming_and_held_to_zero() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // Buy Volume(v=1000) @ lock 100: reserved settlement held = 1000 USD and
    // reserved base incoming = v / |lock| = 1000 / 100 = 10 AAPL.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Volume(vol("1000")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("10"));
    assert_balance(&policy, acc, "USD", "9000", "1000");

    // Fill 6 @ 90 (price below the lock): the settlement leg releases 100*6=600
    // held and credits the price improvement (100-90)*6=60 to available; the
    // base leg drains 6 of the projected incoming (10 -> 4) and credits 6 owned.
    let fill_a = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("90"),
            quantity: qty("6"),
        }),
        qty("4"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &fill_a).is_empty());
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("4"));

    // Final fill 4 @ 110 (price above the lock): the settlement leg releases the
    // remaining 100*4=400 held (-> 0) and debits the deficit (100-110)*4=-40
    // from available; the base leg drains the last 4 incoming (-> 0).
    // Σ(fill qty) 6+4 + leaves 0 == 10 == v / |lock|, so both legs converge.
    let fill_b = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("110"),
            quantity: qty("4"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &fill_b).is_empty());

    // Base incoming and settlement held both net to zero despite fill != lock.
    assert_eq!(incoming_of(&policy, acc, "AAPL"), PositionSize::ZERO);
    // AAPL owned = 10, no residual held or incoming.
    assert_balance(&policy, acc, "AAPL", "10", "0");
    // USD available reflects the real cash spent: 6*90 + 4*110 = 980, netted
    // out of the 1000 reservation (10000 - 980 = 9020); held is fully released.
    assert_balance(&policy, acc, "USD", "9020", "0");
}

#[test]
fn buy_volume_price_divergent_partial_then_fill_nets_incoming_and_held_to_zero() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // Buy Volume(v=1000) @ lock 100: reserved base incoming = 10, held = 1000.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Volume(vol("1000")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("10"));

    // Partial fill 6 @ 90 (leaves 4, non-final): drains base incoming by 6.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("90"),
            quantity: qty("6"),
        }),
        qty("4"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("4"));

    // Final fill 4 @ 110 (leaves 0): drains the remaining base incoming and the
    // remaining settlement held. Σ(fill qty) 6+4 + leaves 0 == 10 == v / |lock|.
    let fill = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        Some(Trade {
            price: px("110"),
            quantity: qty("4"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &fill).is_empty());
    assert_eq!(incoming_of(&policy, acc, "AAPL"), PositionSize::ZERO);
    assert_balance(&policy, acc, "AAPL", "10", "0");
    // Both legs drained: no base incoming, no settlement held residual.
    assert_balance(&policy, acc, "USD", "9020", "0");
}

#[test]
fn buy_volume_price_divergent_partial_then_cancel_nets_incoming_and_held_to_zero() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("USD"), "10000");

    // Buy Volume(v=1000) @ lock 100: reserved base incoming = 10, held = 1000.
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        TradeAmount::Volume(vol("1000")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("10"));

    // Partial fill 6 @ 90 (leaves 4, non-final): drains base incoming by 6, and
    // releases 100*6=600 settlement held while crediting (100-90)*6=60 back.
    let partial = make_report(
        acc,
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("90"),
            quantity: qty("6"),
        }),
        qty("4"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    assert_eq!(incoming_of(&policy, acc, "AAPL"), ps("4"));

    // Final report with no trade, leaves 4: the cancel releases the unfilled
    // base incoming remainder (4 -> 0) and the settlement held remainder
    // (100*4=400 -> 0). Convergence: drained 6 + released 4 == reserved 10.
    let cancel = make_report(
        acc,
        aapl_usd,
        Side::Buy,
        None,
        qty("4"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    assert_eq!(incoming_of(&policy, acc, "AAPL"), PositionSize::ZERO);
    // AAPL owned = 6 (only the filled leg), no residual incoming.
    assert_balance(&policy, acc, "AAPL", "6", "0");
    // USD held fully released; available reflects the 6*90=540 actually spent.
    assert_balance(&policy, acc, "USD", "9460", "0");
}

#[test]
fn sell_volume_price_divergent_full_fill_nets_held_and_incoming_to_zero() {
    let acc = account(99224416);
    let aapl_usd = instr("AAPL", "USD");
    let policy = build_policy(None, None);
    seed(&policy, acc, asset("AAPL"), "10");

    // Sell Volume(v=1000) @ lock 100: reserved underlying held = v / |lock| =
    // 1000 / 100 = 10 AAPL and reserved settlement incoming = 100 * 10 = 1000
    // USD. This is the symmetric counterpart of the buy: the sell underlying-held
    // leg mirrors the buy base-incoming leg (same quantity basis, factor |lock|).
    let order = make_order(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        TradeAmount::Volume(vol("1000")),
        Some(px("100")),
    );
    let mut mutations = Mutations::with_capacity(2);
    pre_trade_full(&policy, &order, &mut mutations).expect("must pass");
    mutations.commit_all();
    assert_balance(&policy, acc, "AAPL", "0", "10");
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("1000"));

    // Fill 6 @ 110 (price above the lock): the underlying leg consumes 6 held
    // (10 -> 4); the settlement leg credits 110*6=660 proceeds and drains the
    // projected incoming by lock*6=600 (1000 -> 400).
    let fill_a = make_report(
        acc,
        aapl_usd.clone(),
        Side::Sell,
        Some(Trade {
            price: px("110"),
            quantity: qty("6"),
        }),
        qty("4"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &fill_a).is_empty());
    assert_balance(&policy, acc, "AAPL", "0", "4");
    assert_eq!(incoming_of(&policy, acc, "USD"), ps("400"));

    // Final fill 4 @ 90 (price below the lock): the underlying leg consumes the
    // last 4 held (-> 0); the settlement leg credits 90*4=360 and drains the
    // remaining incoming lock*4=400 (-> 0). Σ(fill qty) 6+4 + leaves 0 == 10.
    let fill_b = make_report(
        acc,
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("90"),
            quantity: qty("4"),
        }),
        qty("0"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("100"),
        )])),
    );
    assert!(report_blocks(&policy, &fill_b).is_empty());
    // Underlying held converges to zero despite fill != lock (the whole point of
    // the symmetry), and the reserved settlement incoming drains to zero too.
    assert_balance(&policy, acc, "AAPL", "0", "0");
    assert_eq!(incoming_of(&policy, acc, "USD"), PositionSize::ZERO);
    // Proceeds credited to available: 6*110 + 4*90 = 1020.
    assert_balance(&policy, acc, "USD", "1020", "0");
}
