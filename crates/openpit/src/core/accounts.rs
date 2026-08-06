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

use crate::core::account_control::{AccountBlockError, AccountBlockHandle};
use crate::core::account_groups::{AccountGroupError, AccountGroupsHandle};
use crate::core::ConfigRegistry;
use crate::param::{AccountGroupId, AccountId, Asset, DEFAULT_ACCOUNT_GROUP};
use crate::pretrade::{AccountBlock, RejectCode};
use crate::storage::{self, LockingPolicyFactory, Storage, StorageBuilder};

#[derive(Default)]
struct StateTransitionAccess {
    readers: usize,
    writer: bool,
    waiting_writers: usize,
}

#[derive(Clone, Copy)]
enum StateTransitionAccessKind {
    Shared,
    Exclusive,
}

struct StateTransitionAccessGuard<'a, StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    accesses: &'a Storage<
        AccountId,
        StateTransitionAccess,
        <StorageFactory as LockingPolicyFactory>::Policy,
    >,
    account: AccountId,
    kind: StateTransitionAccessKind,
}

impl<StorageFactory> Drop for StateTransitionAccessGuard<'_, StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn drop(&mut self) {
        self.accesses
            .with_mut_if_present(&self.account, |access| match self.kind {
                StateTransitionAccessKind::Shared => {
                    access.readers = access.readers.saturating_sub(1);
                }
                StateTransitionAccessKind::Exclusive => {
                    access.writer = false;
                }
            });
    }
}

/// Serializes account-state writes against barrier and membership transitions.
///
/// All ordinary state writers share one global lease, so they remain parallel
/// with each other under `FullSync`. Barrier retunes and membership changes
/// take the single global exclusive lease. While one runs, account-state writes
/// for every account pause until the transition completes. The pause includes
/// the spin-and-sleep wait while an exclusive writer is queued behind readers.
struct StateTransitionCoordinator<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    access:
        Storage<AccountId, StateTransitionAccess, <StorageFactory as LockingPolicyFactory>::Policy>,
    #[cfg(test)]
    exclusive_wait_hooks: Storage<
        AccountId,
        Option<std::sync::Arc<std::sync::Barrier>>,
        <StorageFactory as LockingPolicyFactory>::Policy,
    >,
}

impl<StorageFactory> StateTransitionCoordinator<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn new(builder: &StorageBuilder<StorageFactory>) -> Self {
        Self {
            access: builder.create_for_bound_key(),
            #[cfg(test)]
            exclusive_wait_hooks: builder.create_for_bound_key(),
        }
    }

    fn with_shared<R>(&self, operation: impl FnOnce() -> R) -> R {
        let _access = Self::acquire_shared(&self.access, Self::access_key(), false);
        operation()
    }

    fn with_rollback_shared<R>(&self, operation: impl FnOnce() -> R) -> R {
        // Rollback can already own the account-PnL assertion lease. It must be
        // allowed to finish even when a transition is queued behind another
        // state reader that is waiting for that lease.
        let _access = Self::acquire_shared(&self.access, Self::access_key(), true);
        operation()
    }

    fn with_exclusive<R>(&self, operation: impl FnOnce() -> R) -> R {
        let _access = self.acquire_exclusive(&self.access, Self::access_key());
        operation()
    }

    #[cfg(test)]
    fn set_exclusive_wait_hook(&self, hook: std::sync::Arc<std::sync::Barrier>) {
        self.exclusive_wait_hooks.with_mut(
            Self::access_key(),
            || None,
            |slot, _| {
                *slot = Some(hook);
            },
        );
    }

    fn access_key() -> AccountId {
        AccountId::from_u64(0)
    }

    fn acquire_shared(
        accesses: &Storage<
            AccountId,
            StateTransitionAccess,
            <StorageFactory as LockingPolicyFactory>::Policy,
        >,
        account: AccountId,
        bypass_waiting_writer: bool,
    ) -> StateTransitionAccessGuard<'_, StorageFactory> {
        let mut attempts = 0_u32;
        loop {
            let acquired =
                accesses.with_mut(account, StateTransitionAccess::default, |access, _| {
                    if access.writer || (!bypass_waiting_writer && access.waiting_writers != 0) {
                        return false;
                    }
                    let Some(readers) = access.readers.checked_add(1) else {
                        return false;
                    };
                    access.readers = readers;
                    true
                });
            if acquired {
                return StateTransitionAccessGuard {
                    accesses,
                    account,
                    kind: StateTransitionAccessKind::Shared,
                };
            }
            Self::back_off(&mut attempts);
        }
    }

    fn acquire_exclusive<'a>(
        &self,
        accesses: &'a Storage<
            AccountId,
            StateTransitionAccess,
            <StorageFactory as LockingPolicyFactory>::Policy,
        >,
        account: AccountId,
    ) -> StateTransitionAccessGuard<'a, StorageFactory> {
        accesses.with_mut(account, StateTransitionAccess::default, |access, _| {
            access.waiting_writers = access.waiting_writers.saturating_add(1);
        });
        #[cfg(test)]
        {
            let hook = self
                .exclusive_wait_hooks
                .with_mut(account, || None, |slot, _| slot.take());
            if let Some(hook) = hook {
                hook.wait();
            }
        }
        let mut attempts = 0_u32;
        loop {
            let acquired =
                accesses.with_mut(account, StateTransitionAccess::default, |access, _| {
                    if access.writer || access.readers != 0 {
                        return false;
                    }
                    access.waiting_writers = access.waiting_writers.saturating_sub(1);
                    access.writer = true;
                    true
                });
            if acquired {
                return StateTransitionAccessGuard {
                    accesses,
                    account,
                    kind: StateTransitionAccessKind::Exclusive,
                };
            }
            Self::back_off(&mut attempts);
        }
    }

    fn back_off(attempts: &mut u32) {
        if *attempts < 8 {
            std::thread::yield_now();
        } else {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        *attempts = attempts.saturating_add(1);
    }
}

// ─── AccountCurrencies ──────────────────────────────────────────────────────

pub(crate) struct AccountCurrencies<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    accounts: Storage<AccountId, Asset, StorageFactory::Policy>,
    groups: Storage<AccountGroupId, Asset, StorageFactory::Policy>,
    state_transition: StateTransitionCoordinator<StorageFactory>,
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
            groups: builder.create_for_any_key(),
            state_transition: StateTransitionCoordinator::new(builder),
        }
    }

    pub(crate) fn with_state_writer<R>(&self, operation: impl FnOnce() -> R) -> R {
        self.state_transition.with_shared(operation)
    }

    pub(crate) fn with_state_rollback<R>(&self, operation: impl FnOnce() -> R) -> R {
        self.state_transition.with_rollback_shared(operation)
    }

    #[cfg(test)]
    fn set_state_transition_wait_hook(&self, hook: std::sync::Arc<std::sync::Barrier>) {
        self.state_transition.set_exclusive_wait_hook(hook);
    }

    fn with_membership_transition<R>(&self, operation: impl FnOnce() -> R) -> R {
        self.state_transition.with_exclusive(operation)
    }

    pub(crate) fn with_state_transition<R>(&self, operation: impl FnOnce() -> R) -> R {
        self.state_transition.with_exclusive(operation)
    }

    fn set_account_currency(&self, account: AccountId, currency: Asset) {
        let initial_currency = currency.clone();
        self.accounts
            .with_mut(account, || initial_currency, |slot, _| *slot = currency);
    }

    fn clear_account_currency(&self, account: AccountId) {
        self.accounts.remove(&account);
    }

    fn account_currency(&self, account: AccountId) -> Option<Asset> {
        self.accounts.with(&account, |currency| currency.clone())
    }

    fn set_group_currency(&self, group: AccountGroupId, currency: Asset) {
        let initial_currency = currency.clone();
        self.groups
            .with_mut(group, || initial_currency, |slot, _| *slot = currency);
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
/// - **Admin blocking**: [`block`](Self::block), [`unblock`](Self::unblock),
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
        self.currencies.with_membership_transition(|| {
            let previous_groups = accounts
                .iter()
                .map(|account| (*account, self.group_of(*account)))
                .collect::<Vec<_>>();
            self.handle.register_group(accounts, group)?;
            self.block_effective_barrier_changes(previous_groups);
            Ok(())
        })
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
        self.currencies.with_membership_transition(|| {
            let previous_groups = accounts
                .iter()
                .map(|account| (*account, self.group_of(*account)))
                .collect::<Vec<_>>();
            self.handle.unregister_group(accounts, group)?;
            self.block_effective_barrier_changes(previous_groups);
            Ok(())
        })
    }

    /// Returns the group of `account`, or `None` when it is not registered.
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
    /// deterministic: the next policy access re-resolves the cascade with the
    /// new effective currency.
    pub fn set_currency(&self, account: AccountId, currency: Asset) {
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
    /// deterministic: the next policy access re-resolves the cascade with the
    /// new effective currency.
    pub fn clear_currency(&self, account: AccountId) {
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
    /// deterministic: the next policy access re-resolves the cascade with the
    /// new effective currency.
    pub fn set_group_currency(&self, group: AccountGroupId, currency: Asset) {
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
    /// deterministic: the next policy access re-resolves the cascade with the
    /// new effective currency.
    pub fn clear_group_currency(&self, group: AccountGroupId) {
        self.currencies.clear_group_currency(group);
    }

    pub(crate) fn with_state_writer<R>(&self, operation: impl FnOnce() -> R) -> R {
        self.currencies.with_state_writer(operation)
    }

    pub(crate) fn with_state_rollback<R>(&self, operation: impl FnOnce() -> R) -> R {
        self.currencies.with_state_rollback(operation)
    }

    #[cfg(test)]
    pub(crate) fn set_state_transition_wait_hook(&self, hook: std::sync::Arc<std::sync::Barrier>) {
        self.currencies.set_state_transition_wait_hook(hook);
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
        self.block_handle
            .block_account(account, engine_block(reason));
    }

    /// Unblocks `account`, clearing any block on it.
    ///
    /// Idempotent: a no-op when `account` is not blocked. This clears the block
    /// regardless of its origin - an admin block or a kill-switch block are both
    /// removed.
    pub fn unblock(&self, account: AccountId) {
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
    use crate::core::HasAccountId;
    use crate::pretrade::RejectScope;
    use crate::storage::{FullLocking, LockingPolicyFactory, NoLocking, StorageBuilder};
    use crate::RequestFieldAccessError;

    fn account(id: u64) -> AccountId {
        AccountId::from_u64(id)
    }

    fn group(id: u32) -> AccountGroupId {
        AccountGroupId::from_u32(id).expect("account group id must be valid")
    }

    fn asset(value: &str) -> Asset {
        Asset::new(value).expect("asset must be valid")
    }

    struct AccountOrder(AccountId);

    impl HasAccountId for AccountOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Ok(self.0)
        }
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
    fn state_transition_is_preferred_but_rollback_reader_bypasses_its_queue() {
        use std::sync::{mpsc, Arc, Barrier};
        use std::thread;
        use std::time::Duration;

        let coordinator = Arc::new(StateTransitionCoordinator::new(&StorageBuilder::new(
            FullLocking,
        )));
        let timeout = Duration::from_secs(1);
        let blocked_timeout = Duration::from_millis(50);

        let (release_first_tx, release_first_rx) = mpsc::sync_channel(1);
        let (first_entered_tx, first_entered_rx) = mpsc::sync_channel(1);
        let first_reader = {
            let coordinator = Arc::clone(&coordinator);
            thread::spawn(move || {
                coordinator.with_shared(|| {
                    first_entered_tx
                        .send(())
                        .expect("first reader entry must be observed");
                    release_first_rx
                        .recv()
                        .expect("first reader must be released");
                });
            })
        };
        first_entered_rx
            .recv_timeout(timeout)
            .expect("first reader must enter");

        let writer_queued = Arc::new(Barrier::new(2));
        coordinator.set_exclusive_wait_hook(Arc::clone(&writer_queued));
        let (release_writer_tx, release_writer_rx) = mpsc::sync_channel(1);
        let (writer_entered_tx, writer_entered_rx) = mpsc::sync_channel(1);
        let writer = {
            let coordinator = Arc::clone(&coordinator);
            thread::spawn(move || {
                coordinator.with_exclusive(|| {
                    writer_entered_tx
                        .send(())
                        .expect("global writer entry must be observed");
                    release_writer_rx
                        .recv()
                        .expect("global writer must be released");
                });
            })
        };
        writer_queued.wait();

        let (ordinary_entered_tx, ordinary_entered_rx) = mpsc::sync_channel(1);
        let ordinary_reader = {
            let coordinator = Arc::clone(&coordinator);
            thread::spawn(move || {
                coordinator.with_shared(|| {
                    ordinary_entered_tx
                        .send(())
                        .expect("ordinary reader entry must be observed");
                });
            })
        };
        assert!(ordinary_entered_rx.recv_timeout(blocked_timeout).is_err());

        let (rollback_entered_tx, rollback_entered_rx) = mpsc::sync_channel(1);
        let rollback_reader = {
            let coordinator = Arc::clone(&coordinator);
            thread::spawn(move || {
                coordinator.with_rollback_shared(|| {
                    rollback_entered_tx
                        .send(())
                        .expect("rollback reader entry must be observed");
                });
            })
        };
        rollback_entered_rx
            .recv_timeout(timeout)
            .expect("rollback reader must bypass the queued transition");

        release_first_tx
            .send(())
            .expect("first reader must still be waiting");
        writer_entered_rx
            .recv_timeout(timeout)
            .expect("queued transition must enter after readers drain");
        assert!(ordinary_entered_rx.recv_timeout(blocked_timeout).is_err());
        release_writer_tx
            .send(())
            .expect("transition must still be waiting");
        ordinary_entered_rx
            .recv_timeout(timeout)
            .expect("ordinary reader must enter after the global writer");

        first_reader.join().expect("first reader must finish");
        rollback_reader.join().expect("rollback reader must finish");
        writer.join().expect("transition must finish");
        ordinary_reader.join().expect("ordinary reader must finish");
    }

    #[test]
    fn accounts_block_then_check_rejects_and_unblock_clears() {
        let (accounts, blocked, registry) = new_accounts();
        accounts.block(account(1), "manual review".to_owned());
        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());

        accounts.unblock(account(1));
        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
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
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());

        accounts.unblock_all();

        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_unblock_all_without_a_global_block_is_noop() {
        let (accounts, blocked, registry) = new_accounts();
        accounts.unblock_all();
        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn accounts_unblock_of_non_blocked_account_is_noop() {
        let (accounts, blocked, registry) = new_accounts();
        accounts.unblock(account(1));
        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
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
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
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
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());

        registry
            .unregister_group(&[account(1)], group(7))
            .expect("unregistration must succeed");
        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
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
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .expect("member of blocked group must be rejected");
        assert_eq!(rejects[0].reason, "second");

        accounts
            .unblock_group(group(7))
            .expect("group unblock must succeed");
        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
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
}
