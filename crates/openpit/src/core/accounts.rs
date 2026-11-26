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

//! Public account administration handle for [`Engine`](crate::Engine).
//!
//! All membership, retirement and currency setters/clearers are caller-guaranteed
//! exclusive operations in every mode, including FullSync. Before changing an
//! active account's effective currency, the caller explicitly clears and zeros
//! the account. This includes group movement, group currency, the global/default
//! currency and retirement clearing an explicit account currency. The SDK does
//! not validate activity, reset, convert or recalculate stored state. In-flight
//! operations retain resolved values; unrequested optional values remain lazy.

use crate::core::account_control::{AccountBlockError, AccountBlockHandle};
use crate::core::account_groups::{AccountGroupError, AccountGroupsHandle};
use crate::core::ConfigRegistry;
use crate::param::{AccountGroupId, AccountId, Asset, DEFAULT_ACCOUNT_GROUP};
use crate::pretrade::{AccountBlock, RejectCode};
use crate::storage::{self, EngineWideStorage, Storage, StorageBuilder};

/// Debug-only check of caller-guaranteed exclusive transitions against entire
/// engine calls, including custom callbacks before any built-in state write.
#[cfg(debug_assertions)]
#[derive(Default)]
pub(crate) struct ContractCheck {
    state: std::sync::atomic::AtomicUsize,
}

#[cfg(debug_assertions)]
const CONTRACT_EXCLUSIVE: usize = !(usize::MAX >> 1);

#[cfg(debug_assertions)]
impl ContractCheck {
    pub(crate) fn enter_shared(&self) -> ContractSharedMark<'_> {
        let previous = self.state.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        // The mark exists before the assert so that unwinding takes the count
        // back and leaves the word clean for later calls.
        let mark = ContractSharedMark { check: self };
        assert!(
            previous & CONTRACT_EXCLUSIVE == 0,
            "openpit: an account-state write ran during an exclusive engine \
             operation; the caller \
             must run group membership, all currency setters/clearers, Spot Funds retune and \
             account retirement with no other engine call in flight"
        );
        mark
    }

    pub(crate) fn enter_exclusive(&self) -> ContractExclusiveMark<'_> {
        let entered = self.state.compare_exchange(
            0,
            CONTRACT_EXCLUSIVE,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        );
        assert!(
            entered.is_ok(),
            "openpit: an exclusive engine operation overlapped another engine \
             call; the caller must \
             run group membership, all currency setters/clearers, Spot Funds retune and \
             account retirement with no other engine call in flight"
        );
        ContractExclusiveMark { check: self }
    }
}

#[cfg(debug_assertions)]
pub(crate) struct ContractSharedMark<'a> {
    check: &'a ContractCheck,
}

#[cfg(debug_assertions)]
impl Drop for ContractSharedMark<'_> {
    fn drop(&mut self) {
        self.check
            .state
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

#[cfg(debug_assertions)]
pub(crate) struct ContractExclusiveMark<'a> {
    check: &'a ContractCheck,
}

#[cfg(debug_assertions)]
impl Drop for ContractExclusiveMark<'_> {
    fn drop(&mut self) {
        self.check
            .state
            .fetch_and(!CONTRACT_EXCLUSIVE, std::sync::atomic::Ordering::AcqRel);
    }
}

// ─── AccountCurrencies ──────────────────────────────────────────────────────

pub(crate) struct AccountCurrencies<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    accounts: Storage<AccountId, Asset, StorageFactory::Policy>,
    groups: EngineWideStorage<AccountGroupId, Asset, StorageFactory::Policy>,
}

impl<StorageFactory> AccountCurrencies<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Creates a new, empty account-currency storage using `builder`'s locking
    /// policy.
    pub(crate) fn new(builder: &StorageBuilder<StorageFactory>) -> Self {
        Self {
            accounts: builder.create_for_bound_key(),
            groups: builder.create_engine_wide(),
        }
    }

    fn set_account_currency(&self, account: AccountId, currency: Asset) {
        let initial_currency = currency.clone();
        self.accounts
            .with_mut(account, || initial_currency, |slot, _| *slot = currency);
    }

    pub(crate) fn clear_account_currency(&self, account: AccountId) {
        self.accounts.remove(&account);
    }

    fn account_currency(&self, account: AccountId) -> Option<Asset> {
        self.accounts.with(&account, |currency| currency.clone())
    }

    /// Group currency is part of every member's effective currency, so it
    /// changes as an exclusive transition: no member's state write sees two
    /// currencies within one lease.
    fn set_group_currency(&self, group: AccountGroupId, currency: Asset) {
        let initial_currency = currency.clone();
        self.groups
            .upsert(group, || initial_currency, |slot, _| *slot = currency);
    }

    fn clear_group_currency(&self, group: AccountGroupId) {
        self.groups.remove(&group);
    }

    fn group_currency(&self, group: AccountGroupId) -> Option<Asset> {
        self.groups.with(&group, |currency| currency.clone())
    }

    pub(crate) fn currency_of(
        &self,
        account: AccountId,
        account_group: Option<AccountGroupId>,
    ) -> Option<Asset> {
        self.account_currency(account)
            .or_else(|| account_group.and_then(|group| self.group_currency(group)))
            .or_else(|| self.group_currency(DEFAULT_ACCOUNT_GROUP))
    }

    pub(crate) fn known_account_keys(&self) -> Vec<AccountId> {
        self.accounts.keys()
    }
}

// ─── Accounts ────────────────────────────────────────────────────────────────

/// Public handle to the engine's account registry.
///
/// Obtained from [`Engine::accounts`](crate::Engine::accounts). Cloneable;
/// every clone refers to the same account-group registry, blocked-accounts set,
/// and account-currency storage. An account belongs to at most one group at a
/// time.
///
/// The handle exposes three facilities:
///
/// - **Group membership**: [`register_group`](Self::register_group),
///   [`unregister_group`](Self::unregister_group), [`group_of`](Self::group_of).
/// - **Admin blocking**: [`block`](Self::block),
///   [`block_with_cause`](Self::block_with_cause), [`unblock`](Self::unblock),
///   [`replace_block_reason`](Self::replace_block_reason), and their group
///   counterparts [`block_group`](Self::block_group),
///   [`unblock_group`](Self::unblock_group),
///   [`replace_group_block_reason`](Self::replace_group_block_reason), plus
///   [`unblock_all`](Self::unblock_all) for the engine-wide block. A blocked
///   account or a member of a blocked group has every pre-trade request
///   rejected before any policy runs; group blocking is a live predicate, so it
///   tracks membership changes without re-blocking.
/// - **Currency routing**: [`set_currency`](Self::set_currency),
///   [`clear_currency`](Self::clear_currency),
///   [`set_group_currency`](Self::set_group_currency), and
///   [`clear_group_currency`](Self::clear_group_currency).
///
/// For an example of good practice in building a control plane on this SDK,
/// see [Pit Officer](http://officer.openpit.dev/).
///
/// # Thread-safety
///
/// `Accounts` inherits the engine's synchronization mode through its inner
/// handles; see [`Engine`](crate::Engine)'s threading section for the contract.
/// Group membership (`register_group`, `unregister_group`) and all account/group
/// currency setters and clearers (`set_currency`, `clear_currency`,
/// `set_group_currency`, `clear_group_currency`), including default/global currency,
/// are caller-exclusive in every mode, including [`FullSync`](crate::FullSync).
/// The caller keeps every other engine call out of flight for the entire operation.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::Engine;
/// use openpit::OrderOperation;
/// use openpit::param::{AccountGroupId, AccountId};
/// use openpit::pretrade::policies::OrderValidationPolicy;
///
/// let engine: openpit::LocalEngine<OrderOperation> = Engine::builder()
///     .no_sync()
///     .pre_trade(OrderValidationPolicy::new())
///     .build()?;
///
/// let accounts = engine.accounts();
/// let group = AccountGroupId::from_u32(1)?;
/// accounts.register_group(&[AccountId::from_u64(10), AccountId::from_u64(11)], group)?;
///
/// assert_eq!(accounts.group_of(AccountId::from_u64(10)), Some(group));
/// assert_eq!(accounts.group_of(AccountId::from_u64(99)), None);
///
/// accounts.block(AccountId::from_u64(10), "manual review".to_owned());
/// accounts.unblock(AccountId::from_u64(10));
/// # Ok(())
/// # }
/// ```
pub struct Accounts<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    handle: AccountGroupsHandle<StorageFactory>,
    block_handle: AccountBlockHandle<StorageFactory>,
    currencies: StorageFactory::Shared<AccountCurrencies<StorageFactory>>,
    config_registry: StorageFactory::Shared<ConfigRegistry<StorageFactory>>,
}

impl<StorageFactory> Clone for Accounts<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            block_handle: self.block_handle.clone(),
            currencies: self.currencies.clone(),
            config_registry: self.config_registry.clone(),
        }
    }
}

impl<StorageFactory> Accounts<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn new(
        handle: AccountGroupsHandle<StorageFactory>,
        block_handle: AccountBlockHandle<StorageFactory>,
        currencies: StorageFactory::Shared<AccountCurrencies<StorageFactory>>,
        config_registry: StorageFactory::Shared<ConfigRegistry<StorageFactory>>,
    ) -> Self {
        Self {
            handle,
            block_handle,
            currencies,
            config_registry,
        }
    }

    /// Atomically registers every account in `accounts` into `group`.
    ///
    /// The operation is all-or-nothing: if any listed account is already a
    /// member of any group (including `group`), no account is registered and
    /// the returned [`AccountGroupError::AlreadyRegistered`] names the
    /// offending account and its current group.
    ///
    /// `group` must be an explicit group: the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP) is not a
    /// valid target, since accounts belong to it implicitly.
    ///
    /// When membership changes the effective account PnL barrier, the stored
    /// state (including implicit zero or a halt) is evaluated in the same call.
    /// An unchanged effective barrier is not re-evaluated.
    ///
    /// # Warning
    ///
    /// If joining `group` changes effective currency, stored PnL and cost
    /// basis accumulated under the previous one lose their meaning; the SDK
    /// guarantees nothing about them.
    ///
    /// # Concurrency
    ///
    /// An exclusive operation; see [`Engine`](crate::Engine)'s threading
    /// section. Moving an account between groups is
    /// [`unregister_group`](Self::unregister_group) followed by this call; run
    /// both in one exclusive stretch so no other call observes the account in
    /// the default group in between.
    ///
    /// # Errors
    ///
    /// Returns [`AccountGroupError::ReservedGroup`] when `group` is the reserved
    /// default group, or [`AccountGroupError::AlreadyRegistered`] when any
    /// listed account already belongs to a group.
    pub fn register_group(
        &self,
        accounts: &[AccountId],
        group: AccountGroupId,
    ) -> Result<(), AccountGroupError> {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_exclusive();
        {
            let previous_groups = accounts
                .iter()
                .map(|account| (*account, self.group_of(*account)))
                .collect::<Vec<_>>();
            self.handle.register_group(accounts, group)?;
            self.block_effective_barrier_changes(previous_groups);
            Ok(())
        }
    }

    /// Atomically removes every account in `accounts` from `group`.
    ///
    /// The operation is all-or-nothing: every listed account must currently be
    /// a member of `group`. If any is not (ungrouped or in another group), no
    /// account is removed and the returned [`AccountGroupError::NotInGroup`]
    /// names the offending account.
    ///
    /// `group` must be an explicit group: the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP) is not a
    /// valid target, since accounts belong to it implicitly.
    ///
    /// When membership changes the effective account PnL barrier, the stored
    /// state (including implicit zero or a halt) is evaluated in the same call.
    /// An unchanged effective barrier is not re-evaluated.
    ///
    /// # Warning
    ///
    /// If leaving `group` changes effective currency, stored PnL and cost
    /// basis accumulated under the previous one lose their meaning; the SDK
    /// guarantees nothing about them.
    ///
    /// # Concurrency
    ///
    /// An exclusive operation; see [`Engine`](crate::Engine)'s threading
    /// section.
    ///
    /// # Errors
    ///
    /// Returns [`AccountGroupError::ReservedGroup`] when `group` is the reserved
    /// default group, or [`AccountGroupError::NotInGroup`] when any listed
    /// account is not currently a member of `group`.
    pub fn unregister_group(
        &self,
        accounts: &[AccountId],
        group: AccountGroupId,
    ) -> Result<(), AccountGroupError> {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_exclusive();
        {
            let previous_groups = accounts
                .iter()
                .map(|account| (*account, self.group_of(*account)))
                .collect::<Vec<_>>();
            self.handle.unregister_group(accounts, group)?;
            self.block_effective_barrier_changes(previous_groups);
            Ok(())
        }
    }

    /// Returns the group of `account`, or `None` when it is not registered.
    /// Policy callbacks use their context's cheaper operation snapshot instead.
    pub fn group_of(&self, account: AccountId) -> Option<AccountGroupId> {
        self.handle.group_of(account)
    }

    /// Sets the currency for `account`.
    ///
    /// The account-level value overrides group and default-group currency
    /// settings for this account.
    ///
    /// Effective currency resolves through the account, then its group, then
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
    ///
    /// # Warning
    ///
    /// This write is unchecked and re-evaluates nothing. Stored PnL and cost
    /// basis accumulated under a different effective currency lose their
    /// meaning; the SDK guarantees nothing about them and does not convert,
    /// detect, or report the change. Barrier selection itself stays
    /// deterministic: new operations resolve the new currency, while an
    /// in-flight operation retains its already resolved value.
    ///
    /// # Concurrency
    /// Caller-guaranteed exclusive in every mode, including full synchronization:
    /// no other engine call may be in flight for the entire write.
    pub fn set_currency(&self, account: AccountId, currency: Asset) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_exclusive();
        self.currencies.set_account_currency(account, currency);
    }

    /// Clears the currency set directly on `account`.
    ///
    /// Effective currency resolves through the account, then its group, then
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
    ///
    /// # Warning
    ///
    /// This write is unchecked and re-evaluates nothing. Stored PnL and cost
    /// basis accumulated under a different effective currency lose their
    /// meaning; the SDK guarantees nothing about them and does not convert,
    /// detect, or report the change. Barrier selection itself stays
    /// deterministic: new operations resolve the new currency, while an
    /// in-flight operation retains its already resolved value.
    ///
    /// # Concurrency
    /// Caller-guaranteed exclusive in every mode, including full synchronization:
    /// no other engine call may be in flight for the entire write.
    pub fn clear_currency(&self, account: AccountId) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_exclusive();
        self.currencies.clear_account_currency(account);
    }

    /// Sets the currency for `group`.
    ///
    /// Passing [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP)
    /// sets the global default currency tier.
    ///
    /// Effective currency resolves through the account, then its group, then
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
    ///
    /// # Warning
    ///
    /// This write is unchecked and re-evaluates nothing. Stored PnL and cost
    /// basis accumulated under a different effective currency lose their
    /// meaning; the SDK guarantees nothing about them and does not convert,
    /// detect, or report the change. Barrier selection itself stays
    /// deterministic: new operations resolve the new currency, while an
    /// in-flight operation retains its already resolved value.
    ///
    /// # Concurrency
    ///
    /// An exclusive operation; see [`Engine`](crate::Engine)'s threading
    /// section. No other engine call may run beside it in any mode.
    pub fn set_group_currency(&self, group: AccountGroupId, currency: Asset) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_exclusive();
        self.currencies.set_group_currency(group, currency);
    }

    /// Clears the currency set for `group`.
    ///
    /// Passing [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP)
    /// clears the global default currency tier.
    ///
    /// Effective currency resolves through the account, then its group, then
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
    ///
    /// # Warning
    ///
    /// This write is unchecked and re-evaluates nothing. Stored PnL and cost
    /// basis accumulated under a different effective currency lose their
    /// meaning; the SDK guarantees nothing about them and does not convert,
    /// detect, or report the change. Barrier selection itself stays
    /// deterministic: new operations resolve the new currency, while an
    /// in-flight operation retains its already resolved value.
    ///
    /// # Concurrency
    ///
    /// An exclusive operation; see [`Engine`](crate::Engine)'s threading
    /// section.
    pub fn clear_group_currency(&self, group: AccountGroupId) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_exclusive();
        self.currencies.clear_group_currency(group);
    }

    #[cfg(test)]
    /// Test-only; production resolves the group, then calls `currency_of_in_group`.
    pub(crate) fn currency_of(&self, account: AccountId) -> Option<Asset> {
        self.currency_of_in_group(account, self.group_of(account))
    }

    pub(crate) fn currency_of_in_group(
        &self,
        account: AccountId,
        account_group: Option<AccountGroupId>,
    ) -> Option<Asset> {
        self.currencies.currency_of(account, account_group)
    }

    fn block_effective_barrier_changes(&self, previous: Vec<(AccountId, Option<AccountGroupId>)>) {
        for (account, previous_group) in previous {
            let current_group = self.group_of(account);
            let previous_currency = self.currencies.currency_of(account, previous_group);
            let current_currency = self.currencies.currency_of(account, current_group);
            for block in self.config_registry.account_pnl_membership_change_blocks(
                account,
                previous_group,
                current_group,
                previous_currency,
                current_currency,
            ) {
                self.block_handle.block_account(account, block);
            }
        }
    }

    /// Blocks `account` out of band with the operator-supplied `reason`.
    ///
    /// Every subsequent pre-trade request for `account` is rejected before any
    /// policy runs, with [`RejectCode::AccountBlocked`] and `reason`. The block
    /// shares the engine's single blocked-accounts set with kill-switch blocks.
    ///
    /// Idempotent: the first cause for an account wins, so re-blocking an
    /// already-blocked account (whether by an admin call or a prior kill switch)
    /// is a no-op and does **not** overwrite the stored reason. Use
    /// [`replace_block_reason`](Self::replace_block_reason) to change it.
    pub fn block(&self, account: AccountId, reason: String) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle
            .block_account(account, engine_block(reason));
    }

    /// Restores a persisted block for `account` with its original cause.
    ///
    /// Later pre-trade requests are rejected with that cause before any policy
    /// runs. Transaction provenance is not restored, so rollback cannot remove
    /// the cause; only [`unblock`](Self::unblock) can. User data remains an
    /// opaque bit pattern and refers to no engine-known object after restart.
    ///
    /// The first cause in the account's own slot wins. An occupied slot makes
    /// this call a successful no-op. Group and engine-wide blocks do not occupy
    /// that slot: this call still inserts the account cause, which pre-trade
    /// checks prefer in account, group, engine-wide order.
    ///
    /// Call this on a newly built engine before pre-trade, account adjustment,
    /// execution-report, drop-copy, policy-reconfiguration, or account-group
    /// work can record a cause for `account`, and wait for it to return. If an
    /// in-flight operation already holds a provisional cause, this succeeds
    /// without replacing it; rollback may then remove that cause and leave the
    /// account unblocked.
    pub fn block_with_cause(&self, account: AccountId, cause: AccountBlock) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle
            .block_account(account, cause.with_provenance(None));
    }

    /// Unblocks `account`, clearing any block on it.
    ///
    /// Idempotent: a no-op when `account` is not blocked. This clears the block
    /// regardless of its origin - an admin block or a kill-switch block are both
    /// removed.
    pub fn unblock(&self, account: AccountId) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle.unblock_account(account);
    }

    /// Clears the engine-wide block, letting every account through again.
    ///
    /// A global block is raised by the engine itself, never by an admin call:
    /// a kill switch reported by an execution report with no readable account,
    /// or a failed mutation finalizer registered by a custom policy, whose
    /// state reach the engine cannot bound (see
    /// [`Mutation`](crate::Mutation)). This is the operator's counterpart, so
    /// the engine can be returned to service once the inconsistency has been
    /// investigated.
    ///
    /// Idempotent: a no-op when no global block is active. Accounts and groups
    /// blocked individually stay blocked; clear those with
    /// [`unblock`](Self::unblock) and [`unblock_group`](Self::unblock_group).
    pub fn unblock_all(&self) {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle.unblock_all();
    }

    /// Replaces the stored reason of an already-blocked account.
    ///
    /// Unlike [`block`](Self::block), which preserves the first cause, this
    /// overwrites the stored cause with `reason`, leaving the account blocked.
    /// The block becomes yours: whatever raised the overwritten cause - an
    /// engine kill-switch included - no longer owns it, so the account stays
    /// blocked by `reason` until it is explicitly unblocked.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::AccountNotBlocked`] when `account` is not
    /// currently blocked; the blocked set is left unchanged.
    pub fn replace_block_reason(
        &self,
        account: AccountId,
        reason: String,
    ) -> Result<(), AccountBlockError> {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle
            .replace_reason(account, engine_block(reason))
    }

    /// Blocks the account group `group` out of band with `reason`.
    ///
    /// Group blocking is a live predicate: every pre-trade request whose account
    /// currently belongs to `group` is rejected with
    /// [`RejectCode::AccountBlocked`] and `reason`, before any policy runs. The
    /// group is **not** expanded into its members, so an account registered into
    /// `group` after the block takes effect immediately, and an account that
    /// leaves `group` is no longer group-blocked unless blocked individually.
    ///
    /// Idempotent: the first cause for a group wins, so re-blocking an
    /// already-blocked group is a no-op. Use
    /// [`replace_group_block_reason`](Self::replace_group_block_reason) to change
    /// the stored reason.
    ///
    /// `group` must be an explicit group: the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP) is not a
    /// valid target, since accounts belong to it implicitly.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the reserved
    /// default group.
    pub fn block_group(
        &self,
        group: AccountGroupId,
        reason: String,
    ) -> Result<(), AccountBlockError> {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle.block_group(group, engine_block(reason))
    }

    /// Unblocks the account group `group`, clearing the group block.
    ///
    /// Idempotent: a no-op when `group` is not blocked. Accounts blocked
    /// individually remain blocked.
    ///
    /// `group` must be an explicit group: the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP) is not a
    /// valid target.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the reserved
    /// default group.
    pub fn unblock_group(&self, group: AccountGroupId) -> Result<(), AccountBlockError> {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle.unblock_group(group)
    }

    /// Replaces the stored reason of an already-blocked account group.
    ///
    /// Unlike [`block_group`](Self::block_group), which preserves the first
    /// cause, this overwrites the stored cause with `reason`, leaving the group
    /// blocked.
    ///
    /// `group` must be an explicit group: the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP) is not a
    /// valid target.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the reserved
    /// default group, or [`AccountBlockError::GroupNotBlocked`] when `group` is
    /// not currently blocked; the blocked set is left unchanged.
    pub fn replace_group_block_reason(
        &self,
        group: AccountGroupId,
        reason: String,
    ) -> Result<(), AccountBlockError> {
        #[cfg(debug_assertions)]
        let _call = self.config_registry.engine_calls.enter_free();
        self.block_handle
            .replace_group_reason(group, engine_block(reason))
    }
}

/// Builds the [`AccountBlock`] stored for an admin (engine-sourced) block.
///
/// The source/policy is fixed to `"Engine"` and the code to
/// [`RejectCode::AccountBlocked`]; `reason` is the operator-supplied cause and
/// is carried as both the human-readable reason and the case-specific details.
fn engine_block(reason: String) -> AccountBlock {
    AccountBlock::new("Engine", RejectCode::AccountBlocked, reason.clone(), reason)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::account_control::BlockedAccounts;
    use crate::core::account_groups::AccountGroups;
    use crate::pretrade::RejectScope;
    use crate::storage::{LockingPolicyFactory, NoLocking, StorageBuilder};

    fn account(id: u64) -> AccountId {
        AccountId::from_u64(id)
    }

    #[cfg(debug_assertions)]
    #[test]
    fn exclusive_transition_contract_and_recovery_hold_in_every_mode() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        fn check<StorageLockingPolicyFactory>()
        where
            StorageLockingPolicyFactory:
                LockingPolicyFactory + storage::CreateStorageFor<AccountId> + Default + 'static,
        {
            let builder = StorageBuilder::new(StorageLockingPolicyFactory::default());
            let groups = AccountGroupsHandle::<StorageLockingPolicyFactory>::from_inner(
                StorageLockingPolicyFactory::new_shared(AccountGroups::new(&builder)),
            );
            let blocks = AccountBlockHandle::<StorageLockingPolicyFactory>::from_inner(
                StorageLockingPolicyFactory::new_shared(BlockedAccounts::new(&builder)),
            );
            let registry = StorageLockingPolicyFactory::new_shared(ConfigRegistry::empty());
            let accounts = Accounts::new(
                groups,
                blocks,
                StorageLockingPolicyFactory::new_shared(AccountCurrencies::new(&builder)),
                registry.clone(),
            );
            let entered = std::sync::Arc::new(std::sync::Barrier::new(2));
            let release = std::sync::Arc::new(std::sync::Barrier::new(2));
            let worker = {
                let tracker = registry.engine_calls.clone();
                let entered = entered.clone();
                let release = release.clone();
                std::thread::spawn(move || {
                    let _call = tracker.enter();
                    entered.wait();
                    release.wait();
                })
            };
            entered.wait();
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                accounts.set_currency(account(1), asset("USD"))
            }));
            release.wait();
            worker.join().expect("active call must finish");
            assert!(
                rejected.is_err(),
                "every mode must reject a transition during an engine call"
            );
            accounts.set_currency(account(1), asset("USD"));
            let worker = {
                let tracker = registry.engine_calls.clone();
                let entered = entered.clone();
                let release = release.clone();
                std::thread::spawn(move || {
                    let _transition = tracker.enter_exclusive();
                    entered.wait();
                    release.wait();
                })
            };
            entered.wait();
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let _call = registry.engine_calls.enter();
            }));
            release.wait();
            worker.join().expect("active transition must finish");
            assert!(
                rejected.is_err(),
                "every mode must reject an engine call during a transition"
            );
            let _recovered = registry.engine_calls.enter();
        }
        check::<NoLocking>();
        check::<crate::storage::IndexLocking<crate::AccountKeyConstraint>>();
        check::<crate::storage::FullLocking>();
    }

    #[cfg(debug_assertions)]
    #[test]
    fn free_writes_obey_exclusion_without_reentry_and_recover() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let (accounts, blocked, groups) = new_accounts();
        {
            let _ordinary = accounts.config_registry.engine_calls.enter();
            accounts.block(account(1), "ordinary callback".to_owned());
            accounts.unblock(account(1));
        }
        let entered = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        for exclusive in [true, false] {
            let worker = {
                let tracker = accounts.config_registry.engine_calls.clone();
                let entered = entered.clone();
                let release = release.clone();
                std::thread::spawn(move || {
                    if exclusive {
                        let _mark = tracker.enter_exclusive();
                        entered.wait();
                        release.wait();
                    } else {
                        let _mark = tracker.enter_free();
                        entered.wait();
                        release.wait();
                    }
                })
            };
            entered.wait();
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                if exclusive {
                    accounts.block(account(1), "forbidden".to_owned());
                } else {
                    accounts.set_currency(account(1), asset("USD"));
                }
            }));
            let no_write = blocked
                .check(&groups, Some(account(1)), RejectScope::Order)
                .is_none();
            release.wait();
            worker.join().unwrap();
            assert!(
                rejected.is_err(),
                "free writes and exclusive calls must reject either overlap direction"
            );
            assert!(no_write, "a rejected free write must not insert its cause");
        }
        accounts.block(account(1), "recovered".to_owned());
        accounts.unblock(account(1));
        accounts.set_currency(account(1), asset("USD"));
    }

    fn group(id: u32) -> AccountGroupId {
        AccountGroupId::from_u32(id).expect("account group id must be valid")
    }

    fn asset(value: &str) -> Asset {
        Asset::new(value).expect("asset must be valid")
    }

    type TestAccountsHandles = (
        Accounts<NoLocking>,
        <NoLocking as LockingPolicyFactory>::Shared<BlockedAccounts<NoLocking>>,
        <NoLocking as LockingPolicyFactory>::Shared<AccountGroups<NoLocking>>,
    );

    /// Builds an [`Accounts`] handle plus the shared blocked-set and registry it
    /// wraps, so tests can drive the public API and then observe the effect
    /// through [`BlockedAccounts::check`].
    fn new_accounts() -> TestAccountsHandles {
        let builder = StorageBuilder::new(NoLocking);
        let blocked = NoLocking::new_shared(BlockedAccounts::new(&builder));
        let registry = NoLocking::new_shared(AccountGroups::new(&builder));
        let currencies = NoLocking::new_shared(AccountCurrencies::new(&builder));
        let config_registry = NoLocking::new_shared(ConfigRegistry::empty());
        let accounts = Accounts::new(
            AccountGroupsHandle::from_inner(registry.clone()),
            AccountBlockHandle::from_inner(blocked.clone()),
            currencies,
            config_registry,
        );
        (accounts, blocked, registry)
    }

    #[test]
    fn accounts_block_then_check_rejects_and_unblock_clears() {
        let (accounts, blocked, registry) = new_accounts();
        accounts.block(account(1), "manual review".to_owned());
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_some());

        accounts.unblock(account(1));
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_block_with_cause_discards_internal_provenance() {
        let (accounts, blocked, registry) = new_accounts();
        let cause = AccountBlock::new(
            "PersistedPnlPolicy",
            RejectCode::PnlKillSwitchTriggered,
            "persisted pnl floor breach",
            "account pnl -501 is below floor -500",
        )
        .with_provenance(Some(42));

        accounts.block_with_cause(account(1), cause.clone());

        assert!(
            blocked.invalidate_provenance(account(1), 42).is_none(),
            "restored durable block must not be invalidated by its former provenance"
        );
        let rejects = blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .expect("restored account must stay blocked");
        assert_eq!(rejects[0].policy, cause.policy);
        assert_eq!(rejects[0].code, cause.code);
        assert_eq!(rejects[0].reason, cause.reason);
        assert_eq!(rejects[0].details, cause.details);
    }

    #[test]
    fn accounts_block_with_cause_keeps_existing_account_cause() {
        let (accounts, blocked, registry) = new_accounts();
        accounts.block(account(1), "manual review".to_owned());

        accounts.block_with_cause(
            account(1),
            AccountBlock::new(
                "PersistedPnlPolicy",
                RejectCode::PnlKillSwitchTriggered,
                "persisted pnl floor breach",
                "pnl is below the persisted floor",
            ),
        );

        let rejects = blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .expect("the first account cause must remain active");
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
        assert_eq!(rejects[0].reason, "manual review");
        assert_eq!(rejects[0].details, "manual review");
    }

    #[test]
    fn accounts_block_with_cause_keeps_provisional_cause_until_rollback() {
        let (accounts, blocked, registry) = new_accounts();
        let provisional = AccountBlock::new(
            "LivePnlPolicy",
            RejectCode::PnlKillSwitchTriggered,
            "live pnl floor breach",
            "pnl is below the live floor",
        )
        .with_provenance(Some(42));
        blocked.block_account(account(1), provisional.clone());

        accounts.block_with_cause(
            account(1),
            AccountBlock::new(
                "PersistedPnlPolicy",
                RejectCode::RiskLimitExceeded,
                "persisted risk breach",
                "risk is above the persisted limit",
            ),
        );

        let rejects = blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .expect("the provisional cause must win the occupied slot");
        assert_eq!(rejects[0].policy, provisional.policy);
        assert_eq!(rejects[0].reason, provisional.reason);
        assert!(blocked.invalidate_provenance(account(1), 42).is_some());
        assert!(
            blocked
                .check(&registry, Some(account(1)), RejectScope::Order)
                .is_none(),
            "rollback must remove the winning provisional cause"
        );
    }

    #[test]
    fn accounts_block_with_cause_wins_over_group_cause() {
        let (accounts, blocked, registry) = new_accounts();
        accounts
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        accounts
            .block_group(group(7), "group halt".to_owned())
            .expect("group block must succeed");
        let restored = AccountBlock::new(
            "PersistedPnlPolicy",
            RejectCode::PnlKillSwitchTriggered,
            "persisted pnl floor breach",
            "pnl is below the persisted floor",
        );

        accounts.block_with_cause(account(1), restored.clone());

        let rejects = blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .expect("the restored account cause must block the account");
        assert_eq!(rejects[0].policy, restored.policy);
        assert_eq!(rejects[0].code, restored.code);
        assert_eq!(rejects[0].reason, restored.reason);
        assert_eq!(rejects[0].details, restored.details);
    }

    #[test]
    fn accounts_unblock_all_clears_the_global_block() {
        let (accounts, blocked, registry) = new_accounts();
        blocked.block_all_with_cause(AccountBlock::new(
            "Engine",
            RejectCode::SystemUnavailable,
            "mutation finalizer failed",
            "engine state may be inconsistent",
        ));
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_some());

        accounts.unblock_all();

        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_unblock_all_without_a_global_block_is_noop() {
        let (accounts, blocked, registry) = new_accounts();
        accounts.unblock_all();
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_unblock_of_non_blocked_account_is_noop() {
        let (accounts, blocked, registry) = new_accounts();
        accounts.unblock(account(1));
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_replace_block_reason_updates_cause_and_errors_when_absent() {
        let (accounts, blocked, registry) = new_accounts();
        assert_eq!(
            accounts.replace_block_reason(account(1), "x".to_owned()),
            Err(AccountBlockError::AccountNotBlocked {
                account: account(1)
            })
        );

        accounts.block(account(1), "first".to_owned());
        accounts
            .replace_block_reason(account(1), "second".to_owned())
            .expect("replacing reason on a blocked account must succeed");
        let rejects = blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
        assert_eq!(rejects[0].reason, "second");
    }

    #[test]
    fn accounts_block_group_tracks_membership_live() {
        let (accounts, blocked, registry) = new_accounts();
        accounts
            .block_group(group(7), "group halt".to_owned())
            .expect("group block must succeed");
        registry
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_some());

        registry
            .unregister_group(&[account(1)], group(7))
            .expect("unregistration must succeed");
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_unblock_group_and_replace_group_reason() {
        let (accounts, blocked, registry) = new_accounts();
        registry
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        accounts
            .block_group(group(7), "first".to_owned())
            .expect("group block must succeed");
        accounts
            .replace_group_block_reason(group(7), "second".to_owned())
            .expect("replacing group reason must succeed");
        let rejects = blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .expect("member of blocked group must be rejected");
        assert_eq!(rejects[0].reason, "second");

        accounts
            .unblock_group(group(7))
            .expect("group unblock must succeed");
        assert!(blocked
            .check(&registry, Some(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_group_operations_reject_reserved_default_group() {
        let (accounts, _blocked, _registry) = new_accounts();
        assert_eq!(
            accounts.block_group(DEFAULT_ACCOUNT_GROUP, "x".to_owned()),
            Err(AccountBlockError::ReservedGroup)
        );
        assert_eq!(
            accounts.unblock_group(DEFAULT_ACCOUNT_GROUP),
            Err(AccountBlockError::ReservedGroup)
        );
        assert_eq!(
            accounts.replace_group_block_reason(DEFAULT_ACCOUNT_GROUP, "x".to_owned()),
            Err(AccountBlockError::ReservedGroup)
        );
    }

    #[test]
    fn accounts_replace_group_block_reason_errors_when_group_not_blocked() {
        let (accounts, _blocked, _registry) = new_accounts();
        assert_eq!(
            accounts.replace_group_block_reason(group(7), "x".to_owned()),
            Err(AccountBlockError::GroupNotBlocked { group: group(7) })
        );
    }

    #[test]
    fn currency_of_prefers_account_override() {
        let (accounts, _blocked, _registry) = new_accounts();
        accounts
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        accounts.set_group_currency(group(7), asset("USD"));
        accounts.set_currency(account(1), asset("EUR"));

        assert_eq!(accounts.currency_of(account(1)), Some(asset("EUR")));
    }

    #[test]
    fn currency_of_uses_group_fallback() {
        let (accounts, _blocked, _registry) = new_accounts();
        accounts
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        accounts.set_group_currency(group(7), asset("USD"));

        assert_eq!(accounts.currency_of(account(1)), Some(asset("USD")));
    }

    #[test]
    fn currency_of_uses_default_fallback() {
        let (accounts, _blocked, _registry) = new_accounts();
        accounts
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        accounts.set_group_currency(DEFAULT_ACCOUNT_GROUP, asset("USD"));

        assert_eq!(accounts.currency_of(account(1)), Some(asset("USD")));
        assert_eq!(accounts.currency_of(account(2)), Some(asset("USD")));
    }

    #[test]
    fn currency_clear_reveals_lower_priority_tiers() {
        let (accounts, _blocked, _registry) = new_accounts();
        accounts
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        accounts.set_group_currency(DEFAULT_ACCOUNT_GROUP, asset("USD"));
        accounts.set_group_currency(group(7), asset("EUR"));
        accounts.set_currency(account(1), asset("GBP"));

        accounts.clear_currency(account(1));
        assert_eq!(accounts.currency_of(account(1)), Some(asset("EUR")));

        accounts.clear_group_currency(group(7));
        assert_eq!(accounts.currency_of(account(1)), Some(asset("USD")));

        accounts.clear_group_currency(DEFAULT_ACCOUNT_GROUP);
        assert_eq!(accounts.currency_of(account(1)), None);
    }

    #[test]
    fn currency_of_unset_returns_none() {
        let (accounts, _blocked, _registry) = new_accounts();

        assert_eq!(accounts.currency_of(account(1)), None);
    }

    #[test]
    fn default_account_group_currency_is_allowed() {
        let (accounts, _blocked, _registry) = new_accounts();
        accounts.set_group_currency(DEFAULT_ACCOUNT_GROUP, asset("USD"));

        assert_eq!(accounts.currency_of(account(1)), Some(asset("USD")));

        accounts.clear_group_currency(DEFAULT_ACCOUNT_GROUP);
        assert_eq!(accounts.currency_of(account(1)), None);
    }

    #[test]
    fn owned_rollback_reader_keeps_optional_scope_lazy_until_first_use() {
        let (accounts, _, _) = new_accounts();
        let account = account(1);
        let before = group(3);
        let after = group(4);
        accounts.register_group(&[account], before).unwrap();
        accounts.set_group_currency(before, asset("USD"));
        accounts.set_group_currency(after, asset("EUR"));
        let ctx = crate::AccountAdjustmentContext::with_accounts(
            crate::core::AccountControl::new(accounts.block_handle.clone(), account),
            accounts.clone(),
            accounts.handle.clone(),
            account,
        );
        let rollback = ctx.operation_snapshot();
        accounts.unregister_group(&[account], before).unwrap();
        accounts.register_group(&[account], after).unwrap();
        {
            let group = rollback.account_group();
            let currency = rollback.account_currency();
            assert_eq!(
                group,
                Some(after),
                "capturing rollback metadata must not resolve group eagerly"
            );
            assert_eq!(
                currency,
                Some(asset("EUR")),
                "capturing rollback metadata must not resolve currency eagerly"
            );
        }
        accounts.set_group_currency(after, asset("CAD"));
        {
            let group = rollback.account_group();
            let currency = rollback.account_currency();
            assert_eq!(group, Some(after));
            assert_eq!(
                currency,
                Some(asset("EUR")),
                "rollback readers must retain resolved currency"
            );
        }
    }
}
