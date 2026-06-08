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

//! Account-group membership registry for [`Engine`](crate::Engine).
//!
//! Maps each [`AccountId`] to at most one [`AccountGroupId`]. The registry is
//! populated by the application through the [`Accounts`] handle returned by
//! [`Engine::accounts`](crate::Engine::accounts) and read by policies and
//! contexts to route per-account behavior by group.

use std::cell::OnceCell;
use std::fmt::{Display, Formatter};

use crate::core::account_control::{AccountBlockError, AccountBlockHandle};
use crate::param::{AccountGroupId, AccountId, DEFAULT_ACCOUNT_GROUP};
use crate::pretrade::{AccountBlock, RejectCode};
use crate::storage::{self, LockingPolicy, Storage, StorageBuilder};

// ─── AccountGroupError ───────────────────────────────────────────────────────

/// Error returned by [`Accounts::register_group`] and
/// [`Accounts::unregister_group`].
///
/// Both operations are atomic: when they fail, the registry is left unchanged
/// and the error names the offending account.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccountGroupError {
    /// The target group passed to [`Accounts::register_group`] or
    /// [`Accounts::unregister_group`] is the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
    ///
    /// Accounts belong to the default group implicitly, so it cannot be a
    /// target of an explicit registration or unregistration.
    ReservedGroup,
    /// An account passed to [`Accounts::register_group`] is already a member of
    /// a group.
    ///
    /// The conflict applies whether the existing group equals the requested
    /// group or differs from it: an account may belong to at most one group,
    /// so it must be unregistered before it can be registered again.
    AlreadyRegistered {
        /// Account that is already a member of a group.
        account: AccountId,
        /// Group the account currently belongs to.
        current_group: AccountGroupId,
    },
    /// An account passed to [`Accounts::unregister_group`] is not currently a
    /// member of the requested group.
    ///
    /// `current_group` is `Some` when the account belongs to a different group
    /// and `None` when the account belongs to no group at all.
    NotInGroup {
        /// Account that is not a member of the requested group.
        account: AccountId,
        /// Group requested by the caller.
        requested_group: AccountGroupId,
        /// Group the account currently belongs to, or `None` when ungrouped.
        current_group: Option<AccountGroupId>,
    },
}

impl Display for AccountGroupError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReservedGroup => {
                formatter.write_str("the reserved default account group is not a valid target")
            }
            Self::AlreadyRegistered {
                account,
                current_group,
            } => write!(
                formatter,
                "account {account} is already registered in group {current_group}"
            ),
            Self::NotInGroup {
                account,
                requested_group,
                current_group: Some(current),
            } => write!(
                formatter,
                "account {account} is not in group {requested_group}; \
                 it belongs to group {current}"
            ),
            Self::NotInGroup {
                account,
                requested_group,
                current_group: None,
            } => write!(
                formatter,
                "account {account} is not in group {requested_group}; \
                 it belongs to no group"
            ),
        }
    }
}

impl std::error::Error for AccountGroupError {}

// ─── AccountGroups ───────────────────────────────────────────────────────────

/// Per-engine storage for account-group membership.
///
/// Membership is stored in `memberships`, mapping each registered
/// [`AccountId`] to its single [`AccountGroupId`]. Per-key synchronization is
/// delegated to the `Storage` infrastructure matching the engine's
/// synchronization mode.
///
/// # Multi-account atomicity
///
/// [`register_group`](Self::register_group) and
/// [`unregister_group`](Self::unregister_group) mutate several keys and must be
/// all-or-nothing even under [`FullSync`](crate::core::FullSync), where real
/// threads can interleave. A naive check-all-then-mutate-all has a TOCTOU race.
/// To close it, the registry owns a dedicated standalone locking policy
/// (`guard`) and brackets every multi-account section in that policy's index
/// domain: an exclusive guard for mutations, a shared guard for reads. The
/// guard is a zero-cost no-op under [`LocalSync`](crate::core::LocalSync)
/// (single-observer, no real threads) and a real reader-writer lock under
/// both [`AccountSync`](crate::core::AccountSync) and
/// [`FullSync`](crate::core::FullSync). The mechanism reuses the same
/// `LockingPolicyFactory`/`SyncMode` machinery the rest of the crate relies on,
/// so no raw `std::sync::Mutex` is introduced and `LocalSync`'s `!Send`
/// guarantee is preserved.
pub(crate) struct AccountGroups<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    guard: <StorageFactory as storage::LockingPolicyFactory>::Policy,
    memberships: Storage<AccountId, AccountGroupId, StorageFactory::Policy>,
}

impl<StorageFactory> AccountGroups<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Creates a new, empty account-group registry using `builder`'s locking
    /// policy.
    pub(crate) fn new(builder: &StorageBuilder<StorageFactory>) -> Self {
        Self {
            guard: builder.create_policy(),
            memberships: builder.create_for_bound_key(),
        }
    }

    /// Atomically registers every account in `accounts` into `group`.
    ///
    /// Fails with [`AccountGroupError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`], or with
    /// [`AccountGroupError::AlreadyRegistered`] when any listed account already
    /// belongs to a group (including `group` itself); in either case no account
    /// is registered.
    pub(crate) fn register_group(
        &self,
        accounts: &[AccountId],
        group: AccountGroupId,
    ) -> Result<(), AccountGroupError> {
        if group == DEFAULT_ACCOUNT_GROUP {
            return Err(AccountGroupError::ReservedGroup);
        }

        // Exclusive whole-map section: the check and the inserts are serialized
        // against any other registry mutation, so the multi-account update is
        // all-or-nothing under every sync mode.
        let _guard = self.guard.write_index();

        for account in accounts {
            if let Some(current_group) = self.memberships.with(account, |group| *group) {
                return Err(AccountGroupError::AlreadyRegistered {
                    account: *account,
                    current_group,
                });
            }
        }

        for account in accounts {
            self.memberships.with_mut(
                *account,
                || group,
                |slot, _| {
                    *slot = group;
                },
            );
        }

        Ok(())
    }

    /// Atomically removes every account in `accounts` from `group`.
    ///
    /// Fails with [`AccountGroupError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`], or with
    /// [`AccountGroupError::NotInGroup`] when any listed account is not
    /// currently a member of `group` (ungrouped or in another group); in either
    /// case no account is removed.
    pub(crate) fn unregister_group(
        &self,
        accounts: &[AccountId],
        group: AccountGroupId,
    ) -> Result<(), AccountGroupError> {
        if group == DEFAULT_ACCOUNT_GROUP {
            return Err(AccountGroupError::ReservedGroup);
        }

        // Exclusive whole-map section: the check and the removals are
        // serialized against any other registry mutation, so the
        // multi-account update is all-or-nothing under every sync mode.
        let _guard = self.guard.write_index();

        for account in accounts {
            let current_group = self.memberships.with(account, |group| *group);
            if current_group != Some(group) {
                return Err(AccountGroupError::NotInGroup {
                    account: *account,
                    requested_group: group,
                    current_group,
                });
            }
        }

        for account in accounts {
            self.memberships.remove(account);
        }

        Ok(())
    }

    /// Returns the group of `account`, or `None` when it is not registered.
    pub(crate) fn group_of(&self, account: AccountId) -> Option<AccountGroupId> {
        // Shared whole-map section: reads observe a consistent snapshot with
        // respect to multi-account mutations.
        let _guard = self.guard.read_index();
        self.memberships.with(&account, |group| *group)
    }
}

// ─── AccountGroupsHandle ─────────────────────────────────────────────────────

/// Shared handle to the engine's [`AccountGroups`] facility.
///
/// Cloneable; every clone refers to the same membership map. The handle's
/// auto-traits derive from `StorageFactory::Shared<...>`, the sync-mode-aware
/// wrapper chosen by [`LockingPolicyFactory::Shared`](crate::storage::LockingPolicyFactory::Shared):
///
/// - Under [`FullSync`](crate::core::FullSync) this is `Arc<...>`:
///   `Send + Sync`.
/// - Under [`LocalSync`](crate::core::LocalSync) this is `Rc<...>`:
///   `!Send + !Sync`.
/// - Under [`AccountSync`](crate::core::AccountSync) this is `IndexShared<...>`:
///   `Send + !Sync`.
pub(crate) struct AccountGroupsHandle<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    inner: StorageFactory::Shared<AccountGroups<StorageFactory>>,
}

impl<StorageFactory> Clone for AccountGroupsHandle<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<StorageFactory> AccountGroupsHandle<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Wraps a shared [`AccountGroups`] in a handle.
    ///
    /// Used by the engine builder so that the engine and every context share
    /// one [`AccountGroups`] instance.
    pub(crate) fn from_inner(inner: StorageFactory::Shared<AccountGroups<StorageFactory>>) -> Self {
        Self { inner }
    }

    /// Returns the group of `account`, or `None` when it is not registered.
    pub(crate) fn group_of(&self, account: AccountId) -> Option<AccountGroupId> {
        self.inner.group_of(account)
    }
}

// ─── GroupLookup ─────────────────────────────────────────────────────────────

/// Lazy, per-context account-group lookup shared by the engine contexts.
///
/// Holds a cloned [`AccountGroupsHandle`], the bound account (or `None` when the
/// request carried no recognizable account identifier), and a single-thread
/// cache for the bound account's group. A context is created and consumed
/// within one engine call and is never shared across threads, so a non-`Sync`
/// [`OnceCell`] cache is sound; it keeps the embedding context `Send` (the cell
/// is `Send` when its value is) while making it `!Sync`.
pub(crate) struct GroupLookup<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    handle: AccountGroupsHandle<StorageFactory>,
    account: Option<AccountId>,
    cached_group: OnceCell<Option<AccountGroupId>>,
}

impl<StorageFactory> GroupLookup<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn new(
        handle: AccountGroupsHandle<StorageFactory>,
        account: Option<AccountId>,
    ) -> Self {
        Self {
            handle,
            account,
            cached_group: OnceCell::new(),
        }
    }

    /// Lazily looks up the bound account's group and caches the result.
    ///
    /// Computed once on first call; subsequent calls return the cached value
    /// even if the registry changes afterwards. Returns `None` when no account
    /// is bound.
    pub(crate) fn group(&self) -> Option<AccountGroupId> {
        *self.cached_group.get_or_init(|| {
            self.account
                .and_then(|account| self.handle.group_of(account))
        })
    }
}

impl<StorageFactory> crate::marketdata::AccountInfo for GroupLookup<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Delegates to [`GroupLookup::group`], preserving its OnceCell laziness so
    /// the membership lookup runs at most once per context.
    fn group(&self) -> Option<AccountGroupId> {
        self.group()
    }
}

// ─── Accounts ────────────────────────────────────────────────────────────────

/// Public handle to the engine's account registry.
///
/// Obtained from [`Engine::accounts`](crate::Engine::accounts). Cloneable;
/// every clone refers to the same account-group registry and the same
/// blocked-accounts set. An account belongs to at most one group at a time.
///
/// The handle exposes two facilities:
///
/// - **Group membership**: [`register_group`](Self::register_group),
///   [`unregister_group`](Self::unregister_group), [`group_of`](Self::group_of).
/// - **Admin blocking**: [`block`](Self::block), [`unblock`](Self::unblock),
///   [`replace_block_reason`](Self::replace_block_reason), and their group
///   counterparts [`block_group`](Self::block_group),
///   [`unblock_group`](Self::unblock_group),
///   [`replace_group_block_reason`](Self::replace_group_block_reason). A blocked
///   account or a member of a blocked group has every pre-trade request
///   rejected before any policy runs; group blocking is a live predicate, so it
///   tracks membership changes without re-blocking.
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
}

impl<StorageFactory> Clone for Accounts<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            block_handle: self.block_handle.clone(),
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
    ) -> Self {
        Self {
            handle,
            block_handle,
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
        self.handle.inner.register_group(accounts, group)
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
        self.handle.inner.unregister_group(accounts, group)
    }

    /// Returns the group of `account`, or `None` when it is not registered.
    pub fn group_of(&self, account: AccountId) -> Option<AccountGroupId> {
        self.handle.group_of(account)
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
        self.block_handle.record(account, engine_block(reason));
    }

    /// Unblocks `account`, clearing any block on it.
    ///
    /// Idempotent: a no-op when `account` is not blocked. This clears the block
    /// regardless of its origin - an admin block or a kill-switch block are both
    /// removed.
    pub fn unblock(&self, account: AccountId) {
        self.block_handle.unblock_account(account);
    }

    /// Replaces the stored reason of an already-blocked account.
    ///
    /// Unlike [`block`](Self::block), which preserves the first cause, this
    /// overwrites the stored cause with `reason`, leaving the account blocked.
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

    use crate::param::{AccountGroupId, AccountId};
    use crate::storage::{LockingPolicyFactory, NoLocking, StorageBuilder};

    fn new_registry() -> AccountGroups<NoLocking> {
        AccountGroups::new(&StorageBuilder::new(NoLocking))
    }

    fn account(id: u64) -> AccountId {
        AccountId::from_u64(id)
    }

    fn group(id: u32) -> AccountGroupId {
        AccountGroupId::from_u32(id).expect("account group id must be valid")
    }

    #[test]
    fn register_group_happy_path() {
        let registry = new_registry();
        registry
            .register_group(&[account(1), account(2)], group(7))
            .expect("registration must succeed");

        assert_eq!(registry.group_of(account(1)), Some(group(7)));
        assert_eq!(registry.group_of(account(2)), Some(group(7)));
    }

    #[test]
    fn register_group_rejects_account_in_other_group_and_changes_nothing() {
        let registry = new_registry();
        registry
            .register_group(&[account(1)], group(1))
            .expect("first registration must succeed");

        let error = registry
            .register_group(&[account(2), account(1)], group(2))
            .expect_err("registration must fail on conflict");

        assert_eq!(
            error,
            AccountGroupError::AlreadyRegistered {
                account: account(1),
                current_group: group(1),
            }
        );
        // Atomic rollback: account 2 must NOT have been registered.
        assert_eq!(registry.group_of(account(2)), None);
        assert_eq!(registry.group_of(account(1)), Some(group(1)));
    }

    #[test]
    fn register_group_rejects_account_already_in_same_group_and_changes_nothing() {
        let registry = new_registry();
        registry
            .register_group(&[account(1)], group(5))
            .expect("first registration must succeed");

        let error = registry
            .register_group(&[account(2), account(1)], group(5))
            .expect_err("re-registering into the same group must fail");

        assert_eq!(
            error,
            AccountGroupError::AlreadyRegistered {
                account: account(1),
                current_group: group(5),
            }
        );
        assert_eq!(registry.group_of(account(2)), None);
    }

    #[test]
    fn unregister_group_happy_path() {
        let registry = new_registry();
        registry
            .register_group(&[account(1), account(2)], group(3))
            .expect("registration must succeed");

        registry
            .unregister_group(&[account(1), account(2)], group(3))
            .expect("unregistration must succeed");

        assert_eq!(registry.group_of(account(1)), None);
        assert_eq!(registry.group_of(account(2)), None);
    }

    #[test]
    fn unregister_group_rejects_ungrouped_account_and_removes_nothing() {
        let registry = new_registry();
        registry
            .register_group(&[account(1)], group(3))
            .expect("registration must succeed");

        let error = registry
            .unregister_group(&[account(1), account(2)], group(3))
            .expect_err("unregistration must fail when an account is ungrouped");

        assert_eq!(
            error,
            AccountGroupError::NotInGroup {
                account: account(2),
                requested_group: group(3),
                current_group: None,
            }
        );
        // Atomic rollback: account 1 must still be registered.
        assert_eq!(registry.group_of(account(1)), Some(group(3)));
    }

    #[test]
    fn unregister_group_rejects_account_in_other_group_and_removes_nothing() {
        let registry = new_registry();
        registry
            .register_group(&[account(1)], group(3))
            .expect("registration must succeed");
        registry
            .register_group(&[account(2)], group(4))
            .expect("registration must succeed");

        let error = registry
            .unregister_group(&[account(1), account(2)], group(3))
            .expect_err("unregistration must fail on group mismatch");

        assert_eq!(
            error,
            AccountGroupError::NotInGroup {
                account: account(2),
                requested_group: group(3),
                current_group: Some(group(4)),
            }
        );
        assert_eq!(registry.group_of(account(1)), Some(group(3)));
        assert_eq!(registry.group_of(account(2)), Some(group(4)));
    }

    #[test]
    fn group_of_present_and_absent() {
        let registry = new_registry();
        registry
            .register_group(&[account(1)], group(9))
            .expect("registration must succeed");

        assert_eq!(registry.group_of(account(1)), Some(group(9)));
        assert_eq!(registry.group_of(account(2)), None);
    }

    #[test]
    fn register_group_empty_slice_is_noop() {
        let registry = new_registry();
        registry
            .register_group(&[], group(1))
            .expect("empty registration must succeed");
        assert_eq!(registry.group_of(account(1)), None);
    }

    #[test]
    fn register_group_rejects_reserved_default_group() {
        let registry = new_registry();
        let error = registry
            .register_group(&[account(1)], DEFAULT_ACCOUNT_GROUP)
            .expect_err("registering into the default group must fail");

        assert_eq!(error, AccountGroupError::ReservedGroup);
        assert_eq!(registry.group_of(account(1)), None);
    }

    #[test]
    fn unregister_group_rejects_reserved_default_group() {
        let registry = new_registry();
        let error = registry
            .unregister_group(&[account(1)], DEFAULT_ACCOUNT_GROUP)
            .expect_err("unregistering from the default group must fail");

        assert_eq!(error, AccountGroupError::ReservedGroup);
    }

    #[test]
    fn account_group_error_display_is_stable() {
        assert_eq!(
            AccountGroupError::ReservedGroup.to_string(),
            "the reserved default account group is not a valid target"
        );

        let already = AccountGroupError::AlreadyRegistered {
            account: account(1),
            current_group: group(2),
        };
        assert_eq!(
            already.to_string(),
            "account 1 is already registered in group 2"
        );

        let mismatch = AccountGroupError::NotInGroup {
            account: account(1),
            requested_group: group(2),
            current_group: Some(group(3)),
        };
        assert_eq!(
            mismatch.to_string(),
            "account 1 is not in group 2; it belongs to group 3"
        );

        let ungrouped = AccountGroupError::NotInGroup {
            account: account(1),
            requested_group: group(2),
            current_group: None,
        };
        assert_eq!(
            ungrouped.to_string(),
            "account 1 is not in group 2; it belongs to no group"
        );
    }

    fn new_handle() -> AccountGroupsHandle<NoLocking> {
        AccountGroupsHandle::from_inner(NoLocking::new_shared(new_registry()))
    }

    #[test]
    fn pre_trade_context_group_returns_bound_account_group() {
        use crate::pretrade::PreTradeContext;

        let handle = new_handle();
        handle
            .inner
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");

        let ctx = PreTradeContext::with_groups(None, handle, Some(account(1)));
        assert_eq!(ctx.account_group(), Some(group(7)));
    }

    #[test]
    fn pre_trade_context_group_is_none_when_account_absent() {
        use crate::pretrade::PreTradeContext;

        let handle = new_handle();
        handle
            .inner
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");

        let ctx = PreTradeContext::with_groups(None, handle, None);
        assert_eq!(ctx.account_group(), None);
    }

    #[test]
    fn pre_trade_context_group_is_cached_after_first_call() {
        use crate::pretrade::PreTradeContext;

        let handle = new_handle();
        handle
            .inner
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");

        let ctx = PreTradeContext::with_groups(None, handle.clone(), Some(account(1)));
        // First call populates the cache.
        assert_eq!(ctx.account_group(), Some(group(7)));

        // Mutate the registry after the first lookup; the cached value must win.
        handle
            .inner
            .unregister_group(&[account(1)], group(7))
            .expect("unregistration must succeed");
        handle
            .inner
            .register_group(&[account(1)], group(9))
            .expect("re-registration must succeed");

        assert_eq!(ctx.account_group(), Some(group(7)));
    }

    #[test]
    fn post_trade_context_group_returns_report_account_group() {
        use crate::pretrade::PostTradeContext;

        let handle = new_handle();
        handle
            .inner
            .register_group(&[account(5)], group(3))
            .expect("registration must succeed");

        let ctx = PostTradeContext::with_groups(handle, Some(account(5)));
        assert_eq!(ctx.account_group(), Some(group(3)));
    }

    #[test]
    fn account_adjustment_context_group_returns_adjusted_account_group() {
        use crate::core::account_control::BlockedAccounts;
        use crate::core::{AccountBlockHandle, AccountControl};
        use crate::AccountAdjustmentContext;

        let handle = new_handle();
        handle
            .inner
            .register_group(&[account(8)], group(4))
            .expect("registration must succeed");

        let block_handle = AccountBlockHandle::from_inner(NoLocking::new_shared(
            BlockedAccounts::new(&StorageBuilder::new(NoLocking)),
        ));
        let control = AccountControl::new(block_handle, account(8));
        let ctx = AccountAdjustmentContext::with_groups(control, handle, account(8));
        assert_eq!(ctx.account_group(), Some(group(4)));
    }

    // Concurrency: under FullLocking (FullSync engines) the whole-map guard must
    // serialize register/unregister so a failed multi-account call leaves the
    // membership map completely unchanged even with real thread contention.
    #[test]
    fn full_locking_failed_register_leaves_membership_unchanged_under_contention() {
        use crate::storage::FullLocking;
        use std::sync::Arc;
        use std::thread;

        let registry: Arc<AccountGroups<FullLocking>> =
            Arc::new(AccountGroups::new(&StorageBuilder::new(FullLocking)));
        // Pre-register a conflict account that makes every batch including it
        // fail atomically.
        registry
            .register_group(&[account(0)], group(100))
            .expect("seed registration must succeed");

        thread::scope(|scope| {
            for tid in 1..=8u64 {
                let registry = Arc::clone(&registry);
                scope.spawn(move || {
                    for _ in 0..200 {
                        // Each batch ends with the conflicting account(0), so the
                        // whole batch must roll back: account(tid) must never be
                        // registered.
                        let result =
                            registry.register_group(&[account(tid), account(0)], group(tid as u32));
                        assert!(result.is_err(), "batch with conflict must fail");
                    }
                });
            }
        });

        // No worker account leaked into the map despite heavy contention.
        for tid in 1..=8u64 {
            assert_eq!(registry.group_of(account(tid)), None);
        }
        assert_eq!(registry.group_of(account(0)), Some(group(100)));
    }

    // ─── Accounts admin-blocking surface ──────────────────────────────────

    use crate::core::account_control::BlockedAccounts;
    use crate::core::HasAccountId;
    use crate::pretrade::RejectScope;
    use crate::RequestFieldAccessError;

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
        let blocked = NoLocking::new_shared(BlockedAccounts::new(&StorageBuilder::new(NoLocking)));
        let registry = NoLocking::new_shared(new_registry());
        let accounts = Accounts::new(
            AccountGroupsHandle::from_inner(registry.clone()),
            AccountBlockHandle::from_inner(blocked.clone()),
        );
        (accounts, blocked, registry)
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
        // Member added after the block is blocked automatically.
        registry
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        assert!(blocked
            .check(&registry, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());

        // Leaving the group lifts the block.
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
}
