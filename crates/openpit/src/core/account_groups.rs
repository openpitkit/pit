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

//! Account-group membership registry for [`Engine`](crate::Engine).
//!
//! Maps each [`AccountId`] to at most one [`AccountGroupId`]. The registry is
//! populated by the application through the [`Accounts`](crate::Accounts)
//! handle returned by [`Engine::accounts`](crate::Engine::accounts) and read by
//! policies and contexts to route per-account behavior by group.

use std::cell::OnceCell;
use std::fmt::{Display, Formatter};

use crate::core::{Accounts, RequestFieldAccessError};
use crate::param::{AccountGroupId, AccountId, Asset, DEFAULT_ACCOUNT_GROUP};
use crate::storage::{self, LockingPolicy, OnceValue, Storage, StorageBuilder};

// ─── AccountGroupError ───────────────────────────────────────────────────────

/// Error returned by
/// [`Accounts::register_group`](crate::Accounts::register_group)
/// and [`Accounts::unregister_group`](crate::Accounts::unregister_group).
///
/// Both operations are atomic: when they fail, the registry is left unchanged
/// and the error names the offending account.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccountGroupError {
    /// The target group passed to
    /// [`Accounts::register_group`](crate::Accounts::register_group) or
    /// [`Accounts::unregister_group`](crate::Accounts::unregister_group) is
    /// the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
    ///
    /// Accounts belong to the default group implicitly, so it cannot be a
    /// target of an explicit registration or unregistration.
    ReservedGroup,
    /// An account passed to
    /// [`Accounts::register_group`](crate::Accounts::register_group) is already
    /// a member of a group.
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
    /// An account passed to
    /// [`Accounts::unregister_group`](crate::Accounts::unregister_group) is not
    /// currently a member of the requested group.
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
            Self::AlreadyRegistered { .. } => {
                formatter.write_str("account is already registered in a different group")
            }
            Self::NotInGroup {
                current_group: Some(_),
                ..
            } => formatter.write_str(
                "account is not in the requested group; it belongs to a different group",
            ),
            Self::NotInGroup {
                current_group: None,
                ..
            } => {
                formatter.write_str("account is not in the requested group; it belongs to no group")
            }
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

    /// Removes one account's explicit membership, if present.
    pub(crate) fn retire_account(&self, account: AccountId) {
        let _guard = self.guard.write_index();
        self.memberships.remove(&account);
    }

    /// Returns the group of `account`, or `None` when it is not registered.
    pub(crate) fn group_of(&self, account: AccountId) -> Option<AccountGroupId> {
        // Shared whole-map section: reads observe a consistent snapshot with
        // respect to multi-account mutations.
        let _guard = self.guard.read_index();
        self.memberships.with(&account, |group| *group)
    }

    /// Snapshots every explicit account-group membership.
    pub(crate) fn memberships(&self) -> Vec<(AccountId, AccountGroupId)> {
        let _guard = self.guard.read_index();
        self.memberships
            .keys()
            .into_iter()
            .filter_map(|account| self.memberships.with(&account, |group| (account, *group)))
            .collect()
    }
}

// ─── AccountGroupsHandle ─────────────────────────────────────────────────────

/// Shared handle to the engine's [`AccountGroups`] facility.
///
/// Cloneable; every clone refers to the same membership map. The handle's
/// auto-traits derive from `StorageFactory::Shared<...>`, the sync-mode-aware
/// wrapper chosen by [`LockingPolicyFactory::Shared`]:
///
/// - Under [`FullSync`](crate::core::FullSync) this is `Arc<...>`:
///   `Send + Sync`.
/// - Under [`LocalSync`](crate::core::LocalSync) this is `Rc<...>`:
///   `!Send + !Sync`.
/// - Under [`AccountSync`](crate::core::AccountSync) this is
///   `IndexShared<...>`: `Send + !Sync`.
///
/// [`LockingPolicyFactory::Shared`]:
///     crate::storage::LockingPolicyFactory::Shared
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

    /// Atomically registers every account in `accounts` into `group`.
    pub(crate) fn register_group(
        &self,
        accounts: &[AccountId],
        group: AccountGroupId,
    ) -> Result<(), AccountGroupError> {
        self.inner.register_group(accounts, group)
    }

    /// Atomically removes every account in `accounts` from `group`.
    pub(crate) fn unregister_group(
        &self,
        accounts: &[AccountId],
        group: AccountGroupId,
    ) -> Result<(), AccountGroupError> {
        self.inner.unregister_group(accounts, group)
    }

    /// Returns the group of `account`, or `None` when it is not registered.
    pub(crate) fn group_of(&self, account: AccountId) -> Option<AccountGroupId> {
        self.inner.group_of(account)
    }

    /// Snapshots every explicit account-group membership.
    pub(crate) fn memberships(&self) -> Vec<(AccountId, AccountGroupId)> {
        self.inner.memberships()
    }
}

// Operation snapshot

/// Account data shared by every policy of an operation. Optional values are
/// pinned on first use; pre-trade keeps them from start through execution.
/// Repeated reads in this reader use a local cache without synchronization.
pub struct OperationSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    source: SharedOperationSnapshot<StorageFactory>,
    cached_group: OnceCell<Option<AccountGroupId>>,
    cached_currency: OnceCell<Option<Asset>>,
}

struct SnapshotSource<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    handle: AccountGroupsHandle<StorageFactory>,
    accounts: Option<Accounts<StorageFactory>>,
    account: Result<AccountId, RequestFieldAccessError>,
    group: <StorageFactory::Policy as LockingPolicy>::OnceValue<Option<AccountGroupId>>,
    currency: <StorageFactory::Policy as LockingPolicy>::OnceValue<Option<Asset>>,
}

/// Read-only access to an operation's canonical first-use account data.
///
/// Readers follow the engine's selected synchronization contract. Full mode
/// checks synchronized readiness on each optional getter; local and account
/// modes rely on their existing serialization contract without readiness
/// synchronization. Use [`OperationSnapshot`] for a per-reader local fast
/// cache.
pub struct SharedOperationSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    source: StorageFactory::Shared<SnapshotSource<StorageFactory>>,
}

impl<StorageFactory> Clone for SharedOperationSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
        }
    }
}

impl<StorageFactory> SharedOperationSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Returns the account fixed at operation entry, including its original
    /// error.
    ///
    /// # Errors
    /// Returns the original accessor error when the account was unreadable.
    pub fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.source.account.clone()
    }

    /// Resolves the operation's group once, shared by all of its readers.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        *self.source.group.get_or_init(|| {
            self.source
                .account
                .as_ref()
                .ok()
                .copied()
                .and_then(|account| self.source.handle.group_of(account))
        })
    }

    fn account_currency(&self) -> Option<Asset> {
        self.source
            .currency
            .get_or_init(|| {
                let account = self.source.account.as_ref().ok().copied()?;
                self.source
                    .accounts
                    .as_ref()?
                    .currency_of_in_group(account, self.account_group())
            })
            .clone()
    }
}

impl<StorageFactory> Clone for OperationSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            cached_group: self.cached_group.clone(),
            cached_currency: self.cached_currency.clone(),
        }
    }
}

impl<StorageFactory> OperationSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn new(
        handle: AccountGroupsHandle<StorageFactory>,
        account: Result<AccountId, RequestFieldAccessError>,
        accounts: Option<Accounts<StorageFactory>>,
    ) -> Self {
        let group = handle.inner.guard.new_once_value();
        let currency = handle.inner.guard.new_once_value();
        Self {
            source: SharedOperationSnapshot {
                source: StorageFactory::new_shared(SnapshotSource {
                    handle,
                    account,
                    accounts,
                    group,
                    currency,
                }),
            },
            cached_group: OnceCell::new(),
            cached_currency: OnceCell::new(),
        }
    }

    /// Lazily looks up the bound account's group and caches the result.
    ///
    /// Computed once on first call; subsequent calls return the cached value
    /// even if the registry changes afterwards. Returns `None` when no account
    /// is bound.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        *self
            .cached_group
            .get_or_init(|| self.source.account_group())
    }

    /// Returns the routing account read once when the operation started.
    ///
    /// # Errors
    /// Returns the original error when the payload account was unreadable.
    pub fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.source.account_id()
    }

    /// Resolves effective currency once from the snapshotted group. Every
    /// reader shares that first value; subsequent reads in this reader use
    /// its local cache without synchronization.
    pub(crate) fn account_currency(&self) -> Option<Asset> {
        self.cached_currency
            .get_or_init(|| self.source.account_currency())
            .clone()
    }

    /// Returns a reader sharing canonical first resolution without resolving
    /// data. Its optional getters follow the selected core synchronization
    /// policy.
    pub fn shared_reader(&self) -> SharedOperationSnapshot<StorageFactory> {
        self.source.clone()
    }
}

impl<StorageFactory> crate::marketdata::AccountInfo for OperationSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Uses the operation's first-use group snapshot.
    fn group(&self) -> Option<AccountGroupId> {
        self.account_group()
    }
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
            "account is already registered in a different group"
        );

        let mismatch = AccountGroupError::NotInGroup {
            account: account(1),
            requested_group: group(2),
            current_group: Some(group(3)),
        };
        assert_eq!(
            mismatch.to_string(),
            "account is not in the requested group; it belongs to a different group"
        );

        let ungrouped = AccountGroupError::NotInGroup {
            account: account(1),
            requested_group: group(2),
            current_group: None,
        };
        assert_eq!(
            ungrouped.to_string(),
            "account is not in the requested group; it belongs to no group"
        );
    }

    // The Display text must not leak the account, requested-group, or
    // current-group id, yet the structured variant fields must still carry them
    // for programmatic access.
    #[test]
    fn account_group_error_display_hides_ids_but_keeps_structured_fields() {
        let sentinel_account = account(424242);
        let sentinel_requested = group(515151);
        let sentinel_current = group(626262);

        let already = AccountGroupError::AlreadyRegistered {
            account: sentinel_account,
            current_group: sentinel_requested,
        };
        let text = already.to_string();
        assert!(
            !text.contains("424242"),
            "display leaked account id: {text}"
        );
        assert!(!text.contains("515151"), "display leaked group id: {text}");
        let AccountGroupError::AlreadyRegistered {
            account,
            current_group,
        } = already
        else {
            panic!("variant must be preserved");
        };
        assert_eq!(account, sentinel_account);
        assert_eq!(current_group, sentinel_requested);

        let mismatch = AccountGroupError::NotInGroup {
            account: sentinel_account,
            requested_group: sentinel_requested,
            current_group: Some(sentinel_current),
        };
        let text = mismatch.to_string();
        assert!(
            !text.contains("424242"),
            "display leaked account id: {text}"
        );
        assert!(!text.contains("515151"), "display leaked group id: {text}");
        assert!(!text.contains("626262"), "display leaked group id: {text}");
        let AccountGroupError::NotInGroup {
            account,
            requested_group,
            current_group,
        } = mismatch
        else {
            panic!("variant must be preserved");
        };
        assert_eq!(account, sentinel_account);
        assert_eq!(requested_group, sentinel_requested);
        assert_eq!(current_group, Some(sentinel_current));
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

        let ctx = PreTradeContext::with_groups(None, handle, Ok(account(1)), Ok(false), Ok(false));
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

        let ctx = PreTradeContext::with_groups(
            None,
            handle,
            Err(crate::RequestFieldAccessError::new("account_id")),
            Ok(false),
            Ok(false),
        );
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

        let ctx = PreTradeContext::with_groups(
            None,
            handle.clone(),
            Ok(account(1)),
            Ok(false),
            Ok(false),
        );
        // First call populates the cache.
        assert_eq!(ctx.account_group(), Some(group(7)));

        // Mutate the registry after the first lookup; the cached value must
        // win.
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
    fn engine_context_and_account_info_groups_are_cached_for_one_evaluation() {
        use crate::core::account_control::BlockedAccounts;
        use crate::core::{AccountBlockHandle, AccountControl, AccountCurrencies, ConfigRegistry};
        use crate::pretrade::{PostTradeContext, PreTradeContext};
        use crate::AccountAdjustmentContext;

        let builder = StorageBuilder::new(NoLocking);
        let handle = new_handle();
        handle
            .inner
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        let block_handle =
            AccountBlockHandle::from_inner(NoLocking::new_shared(BlockedAccounts::new(&builder)));
        let accounts = crate::Accounts::new(
            handle.clone(),
            block_handle.clone(),
            NoLocking::new_shared(AccountCurrencies::new(&builder)),
            NoLocking::new_shared(ConfigRegistry::empty()),
        );
        let pre_trade = PreTradeContext::with_accounts(
            None,
            accounts.clone(),
            handle.clone(),
            Ok(account(1)),
            Ok(false),
            Ok(false),
        );
        let post_trade =
            PostTradeContext::with_accounts(accounts.clone(), handle.clone(), Ok(account(1)));
        let adjustment = AccountAdjustmentContext::with_accounts(
            AccountControl::new(block_handle, account(1)),
            accounts.clone(),
            handle,
            account(1),
        );

        assert_eq!(pre_trade.account_group(), Some(group(7)));
        assert_eq!(post_trade.account_group(), Some(group(7)));
        assert_eq!(adjustment.account_group(), Some(group(7)));
        assert_eq!(
            crate::marketdata::AccountInfo::group(&pre_trade),
            Some(group(7))
        );
        assert_eq!(
            crate::marketdata::AccountInfo::group(&post_trade),
            Some(group(7))
        );

        accounts
            .unregister_group(&[account(1)], group(7))
            .expect("unregistration must succeed");
        accounts
            .register_group(&[account(1)], group(9))
            .expect("re-registration must succeed");

        assert_eq!(pre_trade.account_group(), Some(group(7)));
        assert_eq!(post_trade.account_group(), Some(group(7)));
        assert_eq!(adjustment.account_group(), Some(group(7)));
        assert_eq!(
            crate::marketdata::AccountInfo::group(&pre_trade),
            Some(group(7))
        );
        assert_eq!(
            crate::marketdata::AccountInfo::group(&post_trade),
            Some(group(7))
        );
        assert_eq!(pre_trade.account_group(), Some(group(7)));
        assert_eq!(post_trade.account_group(), Some(group(7)));
        assert_eq!(adjustment.account_group(), Some(group(7)));
    }

    #[test]
    fn shared_readers_stay_lazy_and_pin_the_same_group_and_currency() {
        use crate::core::{AccountBlockHandle, AccountCurrencies, BlockedAccounts, ConfigRegistry};
        use crate::pretrade::PreTradeContext;
        let usd = crate::param::Asset::new("USD").unwrap();
        let eur = crate::param::Asset::new("EUR").unwrap();
        let cad = crate::param::Asset::new("CAD").unwrap();
        let builder = StorageBuilder::new(NoLocking);
        let handle = new_handle();
        let blocks =
            AccountBlockHandle::from_inner(NoLocking::new_shared(BlockedAccounts::new(&builder)));
        let accounts = crate::Accounts::new(
            handle.clone(),
            blocks,
            NoLocking::new_shared(AccountCurrencies::new(&builder)),
            NoLocking::new_shared(ConfigRegistry::empty()),
        );
        accounts.register_group(&[account(1)], group(3)).unwrap();
        accounts.set_group_currency(group(3), usd);
        accounts.set_group_currency(group(4), eur.clone());
        let ctx = PreTradeContext::with_accounts(
            None,
            accounts.clone(),
            handle,
            Ok(account(1)),
            Ok(false),
            Ok(false),
        );
        let native = ctx.operation_snapshot();
        let first = native.shared_reader();
        let native_copy = native.clone();
        accounts.unregister_group(&[account(1)], group(3)).unwrap();
        accounts.register_group(&[account(1)], group(4)).unwrap();
        assert_eq!(
            first.account_currency(),
            Some(eur.clone()),
            "constructing and cloning readers must leave group and currency lazy"
        );
        accounts.set_group_currency(group(4), cad);
        accounts.unregister_group(&[account(1)], group(4)).unwrap();
        accounts.register_group(&[account(1)], group(5)).unwrap();
        let later = first.clone();
        assert_eq!(
            later.account_group(),
            Some(group(4)),
            "a later reader must use the canonical first group"
        );
        assert_eq!(native_copy.account_group(), Some(group(4)));
        assert_eq!(
            native_copy.account_currency(),
            Some(eur.clone()),
            "native and shared readers must use the canonical first currency"
        );
        assert_eq!(later.account_currency(), Some(eur));
    }

    #[test]
    fn post_trade_context_group_returns_report_account_group() {
        use crate::pretrade::PostTradeContext;

        let handle = new_handle();
        handle
            .inner
            .register_group(&[account(5)], group(3))
            .expect("registration must succeed");

        let ctx = PostTradeContext::with_groups(handle, Ok(account(5)));
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

    // Concurrency: under FullLocking (FullSync engines) the whole-map guard
    // must serialize register/unregister so a failed multi-account call leaves
    // the membership map completely unchanged even with real thread contention.
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
                        // Each batch ends with the conflicting account(0), so
                        // the whole batch must roll back: account(tid) must
                        // never be registered.
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
}
