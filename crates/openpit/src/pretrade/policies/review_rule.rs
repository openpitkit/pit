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

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

use super::account_scoped::{AccountScope, AccountScoped};
use crate::core::{AccountRetirementRefusal, HasAccountId};
use crate::param::{AccountGroupId, AccountId};
use crate::pretrade::policy::{missing_required_field_reject, PolicyGroupId, PolicyName};
use crate::pretrade::{
    ConfigurablePolicy, PolicyPreTradeResult, PreTradeContext, PreTradePolicy, Reject, RejectCode,
    RejectScope, Rejects, DEFAULT_POLICY_GROUP_ID,
};
use crate::storage::ConfigCell;
use crate::{HasInstrument, Instrument, Mutations};

/// Instruments covered by one review rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewRuleInstruments {
    /// Covers every instrument without requesting the order's instrument.
    All,
    /// Covers a non-empty list of distinct exact instrument pairs.
    ///
    /// Settings reject empty lists and duplicates, and store the list as a set.
    Exact(Vec<Instrument>),
}

/// A review rule covering the members of one account group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRuleAccountGroupRule {
    /// Instruments covered by this rule.
    pub instruments: ReviewRuleInstruments,
    /// Group whose members the rule covers.
    pub account_group_id: AccountGroupId,
}

/// A review rule covering one account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRuleAccountRule {
    /// Instruments covered by this rule.
    pub instruments: ReviewRuleInstruments,
    /// Account covered by the rule.
    pub account_id: AccountId,
}

/// Errors returned by review-rule construction and runtime settings setters.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewRulePolicyError {
    /// A group key was repeated in the group rules.
    DuplicateAccountGroupRule {
        /// Group whose key was repeated.
        account_group_id: AccountGroupId,
    },
    /// An account key was repeated in the account rules.
    DuplicateAccountRule {
        /// Account whose key was repeated.
        account_id: AccountId,
    },
    /// An instrument was repeated within one exact list.
    DuplicateInstrument {
        /// Instrument whose value was repeated.
        instrument: Instrument,
    },
    /// An exact instrument list was empty.
    EmptyInstruments,
}

impl Display for ReviewRulePolicyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateAccountGroupRule { .. } => {
                write!(formatter, "duplicate account group review rule")
            }
            Self::DuplicateAccountRule { .. } => {
                write!(formatter, "duplicate account review rule")
            }
            Self::DuplicateInstrument { instrument } => {
                write!(
                    formatter,
                    "duplicate review rule instrument {}/{}",
                    instrument.underlying_asset(),
                    instrument.settlement_asset()
                )
            }
            Self::EmptyInstruments => {
                write!(formatter, "an exact instrument list must not be empty")
            }
        }
    }
}

impl std::error::Error for ReviewRulePolicyError {}

#[derive(Clone, Debug)]
enum InstrumentFilter {
    All,
    Exact(HashSet<Instrument>),
}

impl TryFrom<ReviewRuleInstruments> for InstrumentFilter {
    type Error = ReviewRulePolicyError;

    fn try_from(instruments: ReviewRuleInstruments) -> Result<Self, Self::Error> {
        match instruments {
            ReviewRuleInstruments::All => Ok(Self::All),
            ReviewRuleInstruments::Exact(instruments) => {
                if instruments.is_empty() {
                    return Err(ReviewRulePolicyError::EmptyInstruments);
                }
                let mut unique = HashSet::new();
                for instrument in instruments {
                    if !unique.insert(instrument.clone()) {
                        return Err(ReviewRulePolicyError::DuplicateInstrument { instrument });
                    }
                }
                Ok(Self::Exact(unique))
            }
        }
    }
}

/// Runtime-updatable rules for [`ReviewRulePolicy`].
///
/// Rules combine across levels. An account rule never hides a group or global
/// rule. All levels may be empty, in which case every order passes. Setters
/// validate their entire input before replacing a level; an error leaves the
/// settings unchanged. Rules can be added through
/// [`Engine::configure`](crate::Engine::configure).
#[derive(Clone, Debug)]
pub struct ReviewRuleSettings {
    rules: AccountScoped<Option<InstrumentFilter>>,
}

impl ReviewRuleSettings {
    /// Creates settings from an optional global rule and per-level iterables.
    ///
    /// # Errors
    ///
    /// Rejects repeated group or account keys, duplicate instruments within an
    /// exact list, and empty exact lists with [`ReviewRulePolicyError`].
    pub fn new(
        global: Option<ReviewRuleInstruments>,
        account_group_rules: impl IntoIterator<Item = ReviewRuleAccountGroupRule>,
        account_rules: impl IntoIterator<Item = ReviewRuleAccountRule>,
    ) -> Result<Self, ReviewRulePolicyError> {
        let mut settings = Self {
            rules: AccountScoped::new(None),
        };
        settings.set_global(global)?;
        settings.set_account_group_rules(account_group_rules)?;
        settings.set_account_rules(account_rules)?;
        Ok(settings)
    }

    /// Replaces the global rule; `None` clears it.
    ///
    /// # Errors
    ///
    /// Rejects an empty exact list or duplicate instruments without changing
    /// settings.
    pub fn set_global(
        &mut self,
        global: Option<ReviewRuleInstruments>,
    ) -> Result<(), ReviewRulePolicyError> {
        let global = global.map(InstrumentFilter::try_from).transpose()?;
        self.rules.set_global(global);
        Ok(())
    }

    /// Replaces all group rules; an empty iterable clears the level.
    ///
    /// # Errors
    ///
    /// Rejects duplicate group keys, empty exact lists, and duplicate
    /// instruments without changing settings.
    pub fn set_account_group_rules(
        &mut self,
        rules: impl IntoIterator<Item = ReviewRuleAccountGroupRule>,
    ) -> Result<(), ReviewRulePolicyError> {
        let mut groups = HashMap::new();
        for rule in rules {
            let filter = InstrumentFilter::try_from(rule.instruments)?;
            match groups.entry(rule.account_group_id) {
                Entry::Vacant(entry) => {
                    entry.insert(Some(filter));
                }
                Entry::Occupied(_) => {
                    return Err(ReviewRulePolicyError::DuplicateAccountGroupRule {
                        account_group_id: rule.account_group_id,
                    });
                }
            }
        }
        self.rules.set_groups(groups);
        Ok(())
    }

    /// Replaces all account rules; an empty iterable clears the level.
    ///
    /// # Errors
    ///
    /// Rejects duplicate account keys, empty exact lists, and duplicate
    /// instruments without changing settings.
    pub fn set_account_rules(
        &mut self,
        rules: impl IntoIterator<Item = ReviewRuleAccountRule>,
    ) -> Result<(), ReviewRulePolicyError> {
        let mut accounts = HashMap::new();
        for rule in rules {
            let filter = InstrumentFilter::try_from(rule.instruments)?;
            match accounts.entry(rule.account_id) {
                Entry::Vacant(entry) => {
                    entry.insert(Some(filter));
                }
                Entry::Occupied(_) => {
                    return Err(ReviewRulePolicyError::DuplicateAccountRule {
                        account_id: rule.account_id,
                    });
                }
            }
        }
        self.rules.set_accounts(accounts);
        Ok(())
    }
}

/// Main-stage policy applying rules to accounts, account groups, or all
/// accounts.
///
/// The registration instance supplies the mode: a match is an order-scoped
/// `ComplianceRestriction` record, classified by the engine as a reject or a
/// review mark. Rules combine by union across levels and instances. Each
/// instance returns one record naming its most specific matching rule:
/// account, then account group, then global. A nonmatching account filter does
/// not hide a matching group or global rule.
///
/// Checks use one settings snapshot. When rules exist, the account is read
/// first through the context. The instrument is read at most once, only when
/// an exact filter is reached; an all-instruments match needs no instrument.
/// Missing fields produce `MissingRequiredField` records. An empty policy or
/// drop-copy reads no order fields. The engine may itself read fields before
/// calling the policy. Review flags are left to the engine. Default dry-run
/// hooks perform the same checks.
///
/// This policy has no runtime account state and emits no mutations.
///
/// # Account retirement
///
/// Account retirement via
/// [`Engine::retire_account`](crate::Engine::retire_account) is refused with
/// `ConfigurationReferencesAccount` while an account rule references it;
/// group and global rules do not hold accounts.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{AccountId, Asset, Quantity, Side, TradeAmount};
/// use openpit::pretrade::policies::{
///     ReviewRuleAccountRule, ReviewRuleInstruments, ReviewRulePolicy,
///     ReviewRuleSettings,
/// };
/// use openpit::pretrade::RuleMode;
/// use openpit::storage::NoLocking;
/// use openpit::{
///     Engine, Instrument, OrderOperation, OrderReview, PolicyInstance,
///     WithOrderOperation, WithOrderReview,
/// };
///
/// let settings = ReviewRuleSettings::new(None, [], [ReviewRuleAccountRule {
///     account_id: AccountId::from_u64(1001),
///     instruments: ReviewRuleInstruments::All,
/// }])?;
/// type Order = WithOrderOperation<WithOrderReview<()>>;
/// let engine = Engine::builder::<Order, (), ()>()
///     .with_order_review()
///     .no_sync()
///     .pre_trade_instance(
///         ReviewRulePolicy::<NoLocking>::new(settings),
///         PolicyInstance {
///             name: Some("account-review".to_owned()),
///             mode: RuleMode::ReviewWithHold,
///         },
///     )
///     .build()?;
/// let order = WithOrderOperation {
///     operation: OrderOperation {
///         instrument: Instrument::new(
///             Asset::new("AAPL")?, Asset::new("USD")?,
///         ),
///         account_id: AccountId::from_u64(1001),
///         side: Side::Buy,
///         trade_amount: TradeAmount::Quantity(Quantity::from_str("10")?),
///         price: None,
///     },
///     inner: WithOrderReview { inner: (), review: OrderReview::default() },
/// };
/// let reservation = engine.start_pre_trade(order)?.execute()?;
/// assert_eq!(reservation.reviews().len(), 1);
/// assert_eq!(reservation.reviews()[0].mode, RuleMode::ReviewWithHold);
/// # Ok(())
/// # }
/// ```
pub struct ReviewRulePolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    group_id: PolicyGroupId,
    settings:
        <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Config<ReviewRuleSettings>,
}

impl<LockingPolicyFactory> ReviewRulePolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    /// Stable policy name.
    pub const NAME: &'static str = "ReviewRulePolicy";

    /// Creates a policy from validated settings.
    pub fn new(settings: ReviewRuleSettings) -> Self {
        Self {
            group_id: DEFAULT_POLICY_GROUP_ID,
            settings: LockingPolicyFactory::new_config(settings),
        }
    }

    /// Assigns a group tag to this instance.
    ///
    /// See [`PolicyGroupId`] and [`DEFAULT_POLICY_GROUP_ID`] for details.
    pub fn with_policy_group_id(mut self, id: PolicyGroupId) -> Self {
        self.group_id = id;
        self
    }
}

impl<LockingPolicyFactory> PolicyName for ReviewRulePolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<LockingPolicyFactory> ConfigurablePolicy<LockingPolicyFactory>
    for ReviewRulePolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    type Settings = ReviewRuleSettings;

    fn settings_cell(&self) -> LockingPolicyFactory::Config<ReviewRuleSettings> {
        self.settings.clone()
    }
}

impl<LockingPolicyFactory, Order, ExecutionReport, AccountAdjustment, Sync>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
    for ReviewRulePolicy<LockingPolicyFactory>
where
    LockingPolicyFactory:
        crate::storage::LockingPolicyFactory + crate::storage::CreateStorageFor<AccountId>,
    Order: HasInstrument + HasAccountId,
    Sync: crate::core::SyncMode<StorageLockingPolicyFactory = LockingPolicyFactory>,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id
    }

    fn retire_account(
        &self,
        account_id: AccountId,
        _mutations: &mut Mutations,
    ) -> Result<(), AccountRetirementRefusal> {
        if self
            .settings
            .with(|settings| settings.rules.references_account(account_id))
        {
            return Err(AccountRetirementRefusal::ConfigurationReferencesAccount);
        }
        Ok(())
    }

    #[allow(private_interfaces)]
    fn built_in_config_entry(&self) -> Option<crate::core::ConfigEntry<LockingPolicyFactory>> {
        Some(crate::core::ConfigEntry::ReviewRule(self.settings_cell()))
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<LockingPolicyFactory>,
        order: &Order,
        _mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        if ctx.is_drop_copy() {
            return Ok(None);
        }
        // Accessors may publish settings, but this check keeps its original
        // version.
        self.settings.with_snapshot(|settings| {
            if settings.rules.global().is_none() && !settings.rules.has_overrides() {
                return Ok(None);
            }
            let account_id = ctx.account_id().map_err(|error| {
                Rejects::from(missing_required_field_reject(self, "account ID", &error))
            })?;
            let mut instrument = None;
            for (scope, rule) in settings.rules.all_levels(account_id, ctx) {
                let Some(filter) = rule else {
                    continue;
                };
                let instruments = match filter {
                    InstrumentFilter::All => "all instruments".to_owned(),
                    InstrumentFilter::Exact(instruments) => {
                        let value = match instrument {
                            Some(value) => value,
                            None => {
                                let value = order.instrument().map_err(|error| {
                                    Rejects::from(missing_required_field_reject(
                                        self,
                                        "instrument",
                                        &error,
                                    ))
                                })?;
                                instrument = Some(value);
                                value
                            }
                        };
                        if !instruments.contains(value) {
                            continue;
                        }
                        format!("{}/{}", value.underlying_asset(), value.settlement_asset())
                    }
                };
                let (level, target) = match scope {
                    AccountScope::Account(id) => ("account", format!("account {id}")),
                    AccountScope::Group(id) => ("account group", format!("account group {id}")),
                    AccountScope::Global => ("global", "global".to_owned()),
                };
                return Err(Reject::new(
                    Self::NAME,
                    RejectScope::Order,
                    RejectCode::ComplianceRestriction,
                    format!("{level} review rule matched"),
                    format!("{target}: {instruments}"),
                )
                .into());
            }
            Ok(None)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use proptest::prelude::*;

    use super::*;
    use crate::core::{AccountGroups, AccountGroupsHandle, LocalSync};
    use crate::param::Asset;
    use crate::pretrade::RuleMode;
    use crate::storage::{LocalConfigCell, LockingPolicyFactory, NoLocking, StorageBuilder};
    use crate::{Engine, RequestFieldAccessError};

    type TestPolicy = ReviewRulePolicy<NoLocking>;

    #[derive(Clone)]
    struct TestOrder {
        instrument: Instrument,
        account_id: AccountId,
        account_calls: Rc<Cell<usize>>,
        instrument_calls: Rc<Cell<usize>>,
        on_instrument: Option<Rc<dyn Fn()>>,
        fail_account: bool,
        fail_instrument: bool,
    }

    impl HasAccountId for TestOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            self.account_calls.set(self.account_calls.get() + 1);
            if self.fail_account {
                Err(RequestFieldAccessError::new("account unavailable"))
            } else {
                Ok(self.account_id)
            }
        }
    }

    impl HasInstrument for TestOrder {
        fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
            self.instrument_calls.set(self.instrument_calls.get() + 1);
            if let Some(callback) = &self.on_instrument {
                callback();
            }
            if self.fail_instrument {
                Err(RequestFieldAccessError::new("instrument unavailable"))
            } else {
                Ok(&self.instrument)
            }
        }
    }

    impl crate::HasTradeAmount for TestOrder {
        fn trade_amount(&self) -> Result<crate::param::TradeAmount, RequestFieldAccessError> {
            Ok(crate::param::TradeAmount::Quantity(
                crate::param::Quantity::from_str("10").unwrap(),
            ))
        }
    }

    impl crate::HasOrderPrice for TestOrder {
        fn price(&self) -> Result<Option<crate::param::Price>, RequestFieldAccessError> {
            Ok(Some(crate::param::Price::from_str("200").unwrap()))
        }
    }

    fn instrument(underlying: &str, settlement: &str) -> Instrument {
        Instrument::new(
            Asset::new(underlying).unwrap(),
            Asset::new(settlement).unwrap(),
        )
    }

    fn order(account: u64, underlying: &str, settlement: &str) -> TestOrder {
        TestOrder {
            instrument: instrument(underlying, settlement),
            account_id: AccountId::from_u64(account),
            account_calls: Rc::new(Cell::new(0)),
            instrument_calls: Rc::new(Cell::new(0)),
            on_instrument: None,
            fail_account: false,
            fail_instrument: false,
        }
    }

    fn account_rule(account: u64, instruments: ReviewRuleInstruments) -> ReviewRuleAccountRule {
        ReviewRuleAccountRule {
            account_id: AccountId::from_u64(account),
            instruments,
        }
    }

    fn group_rule(group: u32, instruments: ReviewRuleInstruments) -> ReviewRuleAccountGroupRule {
        ReviewRuleAccountGroupRule {
            account_group_id: AccountGroupId::from_u32(group).unwrap(),
            instruments,
        }
    }

    fn exact(underlying: &str, settlement: &str) -> ReviewRuleInstruments {
        ReviewRuleInstruments::Exact(vec![instrument(underlying, settlement)])
    }

    fn policy(
        global: Option<ReviewRuleInstruments>,
        groups: impl IntoIterator<Item = ReviewRuleAccountGroupRule>,
        accounts: impl IntoIterator<Item = ReviewRuleAccountRule>,
    ) -> TestPolicy {
        TestPolicy::new(ReviewRuleSettings::new(global, groups, accounts).unwrap())
    }

    fn context(order: &TestOrder) -> PreTradeContext<NoLocking> {
        let builder = StorageBuilder::new(NoLocking);
        let groups =
            AccountGroupsHandle::from_inner(NoLocking::new_shared(AccountGroups::new(&builder)));
        groups
            .register_group(
                &[AccountId::from_u64(1001), AccountId::from_u64(1002)],
                AccountGroupId::from_u32(7).unwrap(),
            )
            .unwrap();
        groups
            .register_group(
                &[AccountId::from_u64(1003)],
                AccountGroupId::from_u32(8).unwrap(),
            )
            .unwrap();
        PreTradeContext::with_groups(None, groups, order.account_id(), Ok(false), Ok(false))
    }

    fn check(
        policy: &TestPolicy,
        order: &TestOrder,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        <TestPolicy as PreTradePolicy<TestOrder, (), (), LocalSync>>::perform_pre_trade_check(
            policy,
            &context(order),
            order,
            &mut Mutations::new(),
        )
    }

    fn assert_match(
        policy: &TestPolicy,
        order: &TestOrder,
        level: &str,
        target: &str,
        instruments: &str,
    ) {
        let records = check(policy, order).expect_err("a matching rule must produce a record");
        assert_eq!(records.len(), 1, "one record per instance");
        assert_eq!(records[0].code, RejectCode::ComplianceRestriction);
        assert_eq!(records[0].scope, RejectScope::Order);
        assert_eq!(records[0].policy, TestPolicy::NAME);
        assert_eq!(
            records[0].mode,
            RuleMode::Reject,
            "the instance supplies the mode"
        );
        assert_eq!(records[0].reason, format!("{level} review rule matched"));
        assert_eq!(records[0].details, format!("{target}: {instruments}"));
    }

    #[test]
    fn account_rule_covers_only_its_account() {
        let policy = policy(None, [], [account_rule(1001, ReviewRuleInstruments::All)]);
        assert_match(
            &policy,
            &order(1001, "AAPL", "USD"),
            "account",
            "account 1001",
            "all instruments",
        );
        assert!(check(&policy, &order(1002, "AAPL", "USD"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn group_rule_covers_members_and_excludes_other_or_ungrouped_accounts() {
        let policy = policy(None, [group_rule(7, ReviewRuleInstruments::All)], []);
        for account in [1001, 1002] {
            assert_match(
                &policy,
                &order(account, "AAPL", "USD"),
                "account group",
                "account group 7",
                "all instruments",
            );
        }
        for account in [1003, 1004] {
            assert!(check(&policy, &order(account, "AAPL", "USD"))
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn global_exact_rule_matches_the_whole_instrument_pair() {
        let policy = policy(Some(exact("AAPL", "USD")), [], []);
        assert_match(
            &policy,
            &order(1004, "AAPL", "USD"),
            "global",
            "global",
            "AAPL/USD",
        );
        for (underlying, settlement) in [("MSFT", "USD"), ("AAPL", "EUR")] {
            assert!(check(&policy, &order(1001, underlying, settlement))
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn union_uses_the_most_specific_matching_rule_not_the_first_existing_rule() {
        let policy = policy(
            None,
            [group_rule(7, ReviewRuleInstruments::All)],
            [account_rule(1001, exact("AAPL", "USD"))],
        );
        assert_match(
            &policy,
            &order(1001, "AAPL", "EUR"),
            "account group",
            "account group 7",
            "all instruments",
        );
        assert_match(
            &policy,
            &order(1001, "AAPL", "USD"),
            "account",
            "account 1001",
            "AAPL/USD",
        );
    }

    #[test]
    fn exact_filters_read_the_instrument_once_across_all_levels() {
        let policy = policy(
            Some(exact("MSFT", "USD")),
            [group_rule(7, exact("AAPL", "EUR"))],
            [account_rule(1001, exact("AAPL", "USD"))],
        );
        let order = order(1001, "MSFT", "USD");
        assert_match(&policy, &order, "global", "global", "MSFT/USD");
        assert_eq!(
            order.instrument_calls.get(),
            1,
            "one instrument read across three exact rules"
        );
        assert_eq!(
            order.account_calls.get(),
            1,
            "the policy uses the context's account snapshot"
        );
    }

    #[test]
    fn account_failure_precedes_instrument_failure() {
        let policy = policy(Some(exact("AAPL", "USD")), [], []);
        let mut order = order(1001, "AAPL", "USD");
        order.fail_account = true;
        order.fail_instrument = true;
        let rejects = check(&policy, &order).unwrap_err();
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(
            rejects[0].reason,
            "failed to access required field 'account ID'"
        );
        assert_eq!(
            order.instrument_calls.get(),
            0,
            "account failure must precede instrument access"
        );
    }

    #[test]
    fn all_rules_do_not_access_the_instrument() {
        for policy in [
            policy(Some(ReviewRuleInstruments::All), [], []),
            policy(None, [group_rule(7, ReviewRuleInstruments::All)], []),
            policy(None, [], [account_rule(1001, ReviewRuleInstruments::All)]),
        ] {
            let mut order = order(1001, "AAPL", "USD");
            order.fail_instrument = true;
            assert_eq!(
                check(&policy, &order).unwrap_err()[0].code,
                RejectCode::ComplianceRestriction
            );
            assert_eq!(
                order.instrument_calls.get(),
                0,
                "All must not request an instrument"
            );
        }
    }

    #[test]
    fn an_exact_rule_reports_an_instrument_access_failure() {
        let policy = policy(Some(exact("AAPL", "USD")), [], []);
        let mut order = order(1001, "AAPL", "USD");
        order.fail_instrument = true;
        let rejects = check(&policy, &order).unwrap_err();
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(
            rejects[0].reason,
            "failed to access required field 'instrument'"
        );
    }

    #[test]
    fn empty_policy_does_not_access_order_fields() {
        let policy = policy(None, [], []);
        let mut order = order(1001, "AAPL", "USD");
        order.fail_account = true;
        order.fail_instrument = true;
        let ctx = context(&order);
        order.account_calls.set(0);
        assert!(
            <TestPolicy as PreTradePolicy<TestOrder, (), (), LocalSync>>::perform_pre_trade_check(
                &policy,
                &ctx,
                &order,
                &mut Mutations::new(),
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            order.account_calls.get(),
            0,
            "an empty policy must not request the account"
        );
        assert_eq!(order.instrument_calls.get(), 0);
    }

    #[test]
    fn drop_copy_does_not_access_order_fields_beyond_the_engine_read() {
        let policy = policy(Some(exact("AAPL", "USD")), [], []);
        let mut order = order(1001, "AAPL", "USD");
        order.fail_instrument = true;
        let engine = Engine::builder::<TestOrder, (), ()>()
            .no_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        engine.apply_drop_copy(order.clone()).unwrap().commit();
        assert_eq!(
            order.account_calls.get(),
            1,
            "only the engine reads the account"
        );
        assert_eq!(
            order.instrument_calls.get(),
            0,
            "drop-copy must not request the instrument"
        );
    }

    #[test]
    fn duplicate_scope_keys_are_rejected_and_failed_setters_keep_the_old_rules() {
        let group = group_rule(7, ReviewRuleInstruments::All);
        let account = account_rule(1001, ReviewRuleInstruments::All);
        let group_error = ReviewRulePolicyError::DuplicateAccountGroupRule {
            account_group_id: group.account_group_id,
        };
        let account_error = ReviewRulePolicyError::DuplicateAccountRule {
            account_id: account.account_id,
        };
        assert_eq!(
            ReviewRuleSettings::new(None, [group.clone(), group.clone()], []).unwrap_err(),
            group_error
        );
        assert_eq!(
            ReviewRuleSettings::new(None, [], [account.clone(), account.clone()]).unwrap_err(),
            account_error
        );

        let mut settings = ReviewRuleSettings::new(None, [group.clone()], []).unwrap();
        assert_eq!(
            settings
                .set_account_group_rules([
                    group_rule(8, ReviewRuleInstruments::All),
                    group.clone(),
                    group
                ])
                .unwrap_err(),
            group_error
        );
        let policy = TestPolicy::new(settings);
        assert_match(
            &policy,
            &order(1001, "AAPL", "USD"),
            "account group",
            "account group 7",
            "all instruments",
        );
        assert!(
            check(&policy, &order(1003, "AAPL", "USD"))
                .unwrap()
                .is_none(),
            "a failed setter must not install its valid prefix"
        );

        let mut settings = ReviewRuleSettings::new(None, [], [account.clone()]).unwrap();
        assert_eq!(
            settings
                .set_account_rules([
                    account_rule(1002, ReviewRuleInstruments::All),
                    account.clone(),
                    account
                ])
                .unwrap_err(),
            account_error
        );
        let policy = TestPolicy::new(settings);
        assert_match(
            &policy,
            &order(1001, "AAPL", "USD"),
            "account",
            "account 1001",
            "all instruments",
        );
        assert!(
            check(&policy, &order(1002, "AAPL", "USD"))
                .unwrap()
                .is_none(),
            "a failed setter must not install its valid prefix"
        );
    }

    #[test]
    fn invalid_instrument_lists_are_rejected_on_every_level_without_partial_updates() {
        let aapl = instrument("AAPL", "USD");
        for (invalid, error) in [
            (
                ReviewRuleInstruments::Exact(vec![]),
                ReviewRulePolicyError::EmptyInstruments,
            ),
            (
                ReviewRuleInstruments::Exact(vec![aapl.clone(), aapl.clone()]),
                ReviewRulePolicyError::DuplicateInstrument { instrument: aapl },
            ),
        ] {
            assert_eq!(
                ReviewRuleSettings::new(Some(invalid.clone()), [], []).unwrap_err(),
                error
            );
            assert_eq!(
                ReviewRuleSettings::new(None, [group_rule(7, invalid.clone())], []).unwrap_err(),
                error
            );
            assert_eq!(
                ReviewRuleSettings::new(None, [], [account_rule(1001, invalid.clone())])
                    .unwrap_err(),
                error
            );

            let mut settings = ReviewRuleSettings::new(Some(exact("AAPL", "USD")), [], []).unwrap();
            assert_eq!(
                settings.set_global(Some(invalid.clone())).unwrap_err(),
                error
            );
            assert_match(
                &TestPolicy::new(settings),
                &order(1001, "AAPL", "USD"),
                "global",
                "global",
                "AAPL/USD",
            );

            let mut settings =
                ReviewRuleSettings::new(None, [group_rule(7, exact("AAPL", "USD"))], []).unwrap();
            assert_eq!(
                settings
                    .set_account_group_rules([
                        group_rule(8, ReviewRuleInstruments::All),
                        group_rule(7, invalid.clone())
                    ])
                    .unwrap_err(),
                error
            );
            let policy = TestPolicy::new(settings);
            assert_match(
                &policy,
                &order(1001, "AAPL", "USD"),
                "account group",
                "account group 7",
                "AAPL/USD",
            );
            assert!(check(&policy, &order(1003, "MSFT", "USD"))
                .unwrap()
                .is_none());

            let mut settings =
                ReviewRuleSettings::new(None, [], [account_rule(1001, exact("AAPL", "USD"))])
                    .unwrap();
            assert_eq!(
                settings
                    .set_account_rules([
                        account_rule(1002, ReviewRuleInstruments::All),
                        account_rule(1001, invalid)
                    ])
                    .unwrap_err(),
                error
            );
            let policy = TestPolicy::new(settings);
            assert_match(
                &policy,
                &order(1001, "AAPL", "USD"),
                "account",
                "account 1001",
                "AAPL/USD",
            );
            assert!(check(&policy, &order(1002, "MSFT", "USD"))
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn replacing_and_clearing_levels_does_not_change_other_levels() {
        let mut settings = ReviewRuleSettings::new(
            Some(exact("MSFT", "USD")),
            [group_rule(7, ReviewRuleInstruments::All)],
            [account_rule(1001, ReviewRuleInstruments::All)],
        )
        .unwrap();
        settings
            .set_account_rules([account_rule(1002, exact("AAPL", "USD"))])
            .unwrap();
        let policy = TestPolicy::new(settings.clone());
        assert_match(
            &policy,
            &order(1001, "AAPL", "USD"),
            "account group",
            "account group 7",
            "all instruments",
        );
        assert_match(
            &policy,
            &order(1002, "AAPL", "USD"),
            "account",
            "account 1002",
            "AAPL/USD",
        );
        settings.set_account_rules([]).unwrap();
        settings
            .set_account_group_rules([group_rule(8, exact("AAPL", "USD"))])
            .unwrap();
        let policy = TestPolicy::new(settings.clone());
        assert!(check(&policy, &order(1001, "AAPL", "USD"))
            .unwrap()
            .is_none());
        assert_match(
            &policy,
            &order(1003, "AAPL", "USD"),
            "account group",
            "account group 8",
            "AAPL/USD",
        );
        settings.set_account_group_rules([]).unwrap();
        assert_match(
            &TestPolicy::new(settings.clone()),
            &order(1003, "MSFT", "USD"),
            "global",
            "global",
            "MSFT/USD",
        );
        settings.set_global(None).unwrap();
        let policy = TestPolicy::new(settings);
        for account in [1001, 1002, 1003, 1004] {
            assert!(check(&policy, &order(account, "MSFT", "USD"))
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn only_account_rules_refuse_retirement_and_clearing_releases_the_account() {
        fn retire(policy: &TestPolicy, account: u64) -> Result<(), AccountRetirementRefusal> {
            <TestPolicy as PreTradePolicy<TestOrder, (), (), LocalSync>>::retire_account(
                policy,
                AccountId::from_u64(account),
                &mut Mutations::new(),
            )
        }
        let configured = policy(
            Some(ReviewRuleInstruments::All),
            [group_rule(7, ReviewRuleInstruments::All)],
            [account_rule(1001, ReviewRuleInstruments::All)],
        );
        assert_eq!(
            retire(&configured, 1001),
            Err(AccountRetirementRefusal::ConfigurationReferencesAccount)
        );
        assert_eq!(
            retire(&configured, 1002),
            Ok(()),
            "group rules do not hold their members"
        );
        configured
            .settings_cell()
            .update(|settings| settings.set_account_rules([]))
            .unwrap();
        assert_eq!(
            retire(&configured, 1001),
            Ok(()),
            "clearing account rules releases retirement"
        );
        for policy in [
            policy(Some(ReviewRuleInstruments::All), [], []),
            policy(None, [group_rule(7, ReviewRuleInstruments::All)], []),
        ] {
            assert_eq!(retire(&policy, 1001), Ok(()));
        }
    }

    #[test]
    fn instrument_accessor_reconfiguration_keeps_one_coherent_settings_snapshot() {
        let policy = policy(
            None,
            [group_rule(7, ReviewRuleInstruments::All)],
            [account_rule(1001, exact("AAPL", "USD"))],
        );
        let cell: LocalConfigCell<ReviewRuleSettings> = policy.settings_cell();
        let mut order = order(1001, "AAPL", "EUR");
        order.on_instrument = Some(Rc::new(move || {
            cell.update(|settings| {
                settings.set_account_rules([])?;
                settings.set_account_group_rules([])
            })
            .expect("an accessor can reconfigure without borrowing the active snapshot");
        }));
        assert_match(
            &policy,
            &order,
            "account group",
            "account group 7",
            "all instruments",
        );
        assert!(
            check(&policy, &order).unwrap().is_none(),
            "the next check observes published settings"
        );
    }

    fn generated_filter(code: u8, instruments: &[Instrument]) -> Option<ReviewRuleInstruments> {
        match code {
            0 => None,
            1 => Some(ReviewRuleInstruments::All),
            mask => Some(ReviewRuleInstruments::Exact(
                instruments
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| (mask - 1) & (1 << index) != 0)
                    .map(|(_, instrument)| instrument.clone())
                    .collect(),
            )),
        }
    }

    proptest! {
        #![proptest_config(super::super::proptest_config::config(256))]

        #[test]
        fn generated_rules_match_the_scope_and_instrument_union(
            codes in prop::collection::vec(0u8..9, 7),
            account_index in 0usize..4,
            instrument_index in 0usize..3,
        ) {
            let instruments = [
                instrument("AAPL", "USD"), instrument("AAPL", "EUR"), instrument("MSFT", "USD"),
            ];
            let groups = [Some(7), Some(7), Some(8), None];
            let policy = policy(
                generated_filter(codes[0], &instruments),
                (0..2).filter_map(|index| generated_filter(codes[index + 1], &instruments)
                    .map(|filter| group_rule(7 + index as u32, filter))),
                (0..4).filter_map(|index| generated_filter(codes[index + 3], &instruments)
                    .map(|filter| account_rule(1001 + index as u64, filter))),
            );
            let mut order = order(1001 + account_index as u64, "AAPL", "USD");
            order.instrument = instruments[instrument_index].clone();

            // Direct definition: scope membership and instrument coverage,
            // without the store or matcher.
            let covers = |code: u8|
                code == 1 || (code >= 2 && (code - 1) & (1 << instrument_index) != 0);
            let mut expected = None;
            if covers(codes[0]) {
                expected = Some(("global", "global".to_owned(), codes[0]));
            }
            for (index, group) in [7, 8].into_iter().enumerate() {
                if groups[account_index] == Some(group) && covers(codes[index + 1]) {
                    expected = Some((
                        "account group", format!("account group {group}"), codes[index + 1],
                    ));
                }
            }
            if covers(codes[account_index + 3]) {
                expected = Some((
                    "account",
                    format!("account {}", 1001 + account_index),
                    codes[account_index + 3],
                ));
            }
            match expected {
                None => prop_assert!(
                    check(&policy, &order).unwrap().is_none(), "no scope and filter match"
                ),
                Some((level, target, code)) => {
                    let records = check(&policy, &order)
                        .expect_err("the oracle found an applicable rule");
                    prop_assert_eq!(records.len(), 1);
                    prop_assert_eq!(records[0].code, RejectCode::ComplianceRestriction);
                    prop_assert_eq!(&records[0].scope, &RejectScope::Order);
                    prop_assert_eq!(&records[0].reason, &format!("{level} review rule matched"));
                    let filter = if code == 1 { "all instruments".to_owned() } else {
                        format!(
                            "{}/{}",
                            instruments[instrument_index].underlying_asset(),
                            instruments[instrument_index].settlement_asset()
                        )
                    };
                    prop_assert_eq!(&records[0].details, &format!("{target}: {filter}"));
                }
            }
            prop_assert!(order.instrument_calls.get() <= 1);
        }
    }
}
