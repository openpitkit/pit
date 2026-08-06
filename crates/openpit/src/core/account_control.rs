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

//! Account blocking for [`Engine`](crate::Engine).
//!
//! Two paths populate the same per-engine blocked set:
//!
//! - **Kill switch.** When a policy's `apply_execution_report` returns a block
//!   (kill switch), the engine records the affected account here. All
//!   subsequent pre-trade requests for that account are rejected immediately,
//!   before any policy is invoked.
//! - **Admin.** Through the public [`Accounts`](crate::Accounts) handle an
//!   operator may block or unblock an individual account or a whole account
//!   group out of band. Group blocking is a live predicate evaluated at check
//!   time against the engine's [`AccountGroups`](crate::core::AccountGroups)
//!   registry, so membership changes take effect without re-blocking.

use std::fmt::{Display, Formatter};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::core::account_groups::AccountGroups;
use crate::core::mutation::MutationFailureScope;
use crate::core::HasAccountId;
use crate::param::{AccountGroupId, AccountId, DEFAULT_ACCOUNT_GROUP};
use crate::pretrade::{AccountBlock, Reject, RejectCode, RejectScope, Rejects};
use crate::storage::{self, IndexFlag, LockingPolicy, Storage, StorageBuilder};

// ─── AccountBlockError ───────────────────────────────────────────────────────

/// Error returned by the admin block operations on
/// [`Accounts`](crate::Accounts).
///
/// Every operation that returns this error leaves the blocked set unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccountBlockError {
    /// The target group is the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
    ///
    /// Accounts belong to the default group implicitly, so it cannot be a
    /// target of an explicit group block, unblock, or reason replacement.
    ReservedGroup,
    /// [`Accounts::replace_block_reason`](crate::Accounts::replace_block_reason)
    /// was called for an account that is not currently blocked.
    AccountNotBlocked {
        /// Account that is not currently blocked.
        account: AccountId,
    },
    /// [`Accounts::replace_group_block_reason`](crate::Accounts::replace_group_block_reason)
    /// was called for a group that is not currently blocked.
    GroupNotBlocked {
        /// Group that is not currently blocked.
        group: AccountGroupId,
    },
}

impl Display for AccountBlockError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReservedGroup => {
                formatter.write_str("the reserved default account group is not a valid target")
            }
            Self::AccountNotBlocked { .. } => formatter.write_str("account is not blocked"),
            Self::GroupNotBlocked { .. } => formatter.write_str("account group is not blocked"),
        }
    }
}

impl std::error::Error for AccountBlockError {}

// ─── AccountControl ──────────────────────────────────────────────────────────

/// Per-account handle to the engine's `BlockedAccounts` facility.
///
/// Carries a specific [`AccountId`] so callers invoke [`AccountControl::block`]
/// without repeating the account argument. Obtained from
/// [`PreTradeContext::account_control`](crate::pretrade::PreTradeContext::account_control)
/// or [`AccountAdjustmentContext::account_control`](crate::AccountAdjustmentContext::account_control).
pub struct AccountControl<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    handle: AccountBlockHandle<StorageFactory>,
    deferred_operations: Option<DeferredAccountOperations>,
    account_id: AccountId,
}

impl<StorageFactory> Clone for AccountControl<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            deferred_operations: self.deferred_operations.clone(),
            account_id: self.account_id,
        }
    }
}

impl<StorageFactory> AccountControl<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn new(handle: AccountBlockHandle<StorageFactory>, account_id: AccountId) -> Self {
        Self {
            handle,
            deferred_operations: None,
            account_id,
        }
    }

    pub(crate) fn with_deferred_operations(
        mut self,
        deferred_operations: DeferredAccountOperations,
    ) -> Self {
        self.deferred_operations = Some(deferred_operations);
        self
    }

    /// Records `block` against the bound account on the engine's shared
    /// `BlockedAccounts`. The first cause for the account wins.
    pub fn block(&self, block: AccountBlock) {
        let write_through = match &self.deferred_operations {
            Some(deferred_operations) => deferred_operations
                .record_block(self.account_id, block)
                .err(),
            None => Some(block),
        };
        if let Some(block) = write_through {
            self.handle.block_account(self.account_id, block);
        }
    }

    /// Unblocks the bound account when its cause is the one `provenance`
    /// raised, and does nothing otherwise. See
    /// [`BlockedAccounts::invalidate_provenance`].
    pub(crate) fn invalidate_provenance(&self, provenance: u64) -> Option<AccountBlock> {
        if let Some(deferred_operations) = &self.deferred_operations {
            if deferred_operations.record_invalidate_provenance(self.account_id, provenance) {
                return None;
            }
        }
        self.handle
            .invalidate_provenance(self.account_id, provenance)
    }
}

#[derive(Clone, Default)]
pub(crate) struct DeferredAccountOperations {
    state: Arc<Mutex<DeferredAccountOperationsState>>,
}

struct DeferredAccountOperationsState {
    first_block: Option<AccountBlock>,
    operations: Vec<DeferredAccountOperation>,
    is_active: bool,
}

impl Default for DeferredAccountOperationsState {
    fn default() -> Self {
        Self {
            first_block: None,
            operations: Vec::new(),
            is_active: true,
        }
    }
}

enum DeferredAccountOperation {
    Block {
        account: AccountId,
        block: AccountBlock,
    },
    InvalidateProvenance {
        account: AccountId,
        provenance: u64,
    },
}

impl DeferredAccountOperations {
    pub(crate) fn record_block(
        &self,
        account: AccountId,
        block: AccountBlock,
    ) -> Result<(), AccountBlock> {
        let mut state = self.state.lock();
        if !state.is_active {
            return Err(block);
        }
        if state.first_block.is_none() {
            state.first_block = Some(block.clone());
        }
        state
            .operations
            .push(DeferredAccountOperation::Block { account, block });
        Ok(())
    }

    pub(crate) fn record_invalidate_provenance(&self, account: AccountId, provenance: u64) -> bool {
        let mut state = self.state.lock();
        if !state.is_active {
            return false;
        }
        state
            .operations
            .push(DeferredAccountOperation::InvalidateProvenance {
                account,
                provenance,
            });
        true
    }

    pub(crate) fn first_block(&self) -> Option<AccountBlock> {
        self.state.lock().first_block.clone()
    }

    pub(crate) fn apply<StorageFactory>(&self, blocked_accounts: &BlockedAccounts<StorageFactory>)
    where
        StorageFactory:
            storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
    {
        let operations = {
            let mut state = self.state.lock();
            state.is_active = false;
            state.first_block = None;
            std::mem::take(&mut state.operations)
        };
        for operation in operations {
            match operation {
                DeferredAccountOperation::Block { account, block } => {
                    blocked_accounts.block_account(account, block);
                }
                DeferredAccountOperation::InvalidateProvenance {
                    account,
                    provenance,
                } => {
                    let _ = blocked_accounts.invalidate_provenance(account, provenance);
                }
            }
        }
    }

    pub(crate) fn abandon(&self) {
        let mut state = self.state.lock();
        state.is_active = false;
        state.first_block = None;
        state.operations = Vec::new();
    }
}

// ─── AccountBlockHandle ──────────────────────────────────────────────────────

/// Public, opaque handle to the engine's `BlockedAccounts` facility.
///
/// A policy that detects a fixation-time failure (for example, an arithmetic
/// overflow inside a rollback or commit closure) has no return value through
/// which to surface an [`AccountBlock`]. Such failures must still translate to
/// a blocked account, so the engine builder hands the policy a clone of this
/// handle at construction time. Recording a block through the handle lands it
/// on the very same `BlockedAccounts` storage the engine uses for normal
/// kill-switch events.
///
/// # Thread-safety
///
/// The handle's auto-traits derive from `StorageFactory::Shared<...>` — the
/// sync-mode-aware wrapper chosen by [`LockingPolicyFactory::Shared`](crate::storage::LockingPolicyFactory::Shared):
///
/// - Under [`FullSync`](crate::core::FullSync) this is `Arc<...>`:
///   `Send + Sync`.
/// - Under [`LocalSync`](crate::core::LocalSync) this is `Rc<...>`:
///   `!Send + !Sync`.
/// - Under [`AccountSync`](crate::core::AccountSync) this is `IndexShared<...>`:
///   `Send + !Sync`, matching the account-sharded engine handle.
///
/// The factory type parameter mirrors the engine's
/// [`StorageLockingPolicyFactory`](crate::core::SyncMode::StorageLockingPolicyFactory),
/// exactly as a policy's `holdings` store does.
pub struct AccountBlockHandle<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    inner: StorageFactory::Shared<BlockedAccounts<StorageFactory>>,
}

impl<StorageFactory> Clone for AccountBlockHandle<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<StorageFactory> AccountBlockHandle<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Wraps a shared [`BlockedAccounts`] in a handle.
    ///
    /// Used by the engine builder so that the engine and every policy share
    /// one [`BlockedAccounts`] instance.
    pub(crate) fn from_inner(
        inner: StorageFactory::Shared<BlockedAccounts<StorageFactory>>,
    ) -> Self {
        Self { inner }
    }

    /// Records `block` against `account_id` on the shared
    /// [`BlockedAccounts`]. The first cause for an account wins; later calls
    /// for the same account are no-ops.
    pub(crate) fn block_account(&self, account_id: AccountId, block: AccountBlock) {
        self.inner.block_account(account_id, block);
    }

    /// Unblocks `account_id` when its cause is the one `provenance` raised, and
    /// does nothing otherwise. See
    /// [`BlockedAccounts::invalidate_provenance`].
    pub(crate) fn invalidate_provenance(
        &self,
        account_id: AccountId,
        provenance: u64,
    ) -> Option<AccountBlock> {
        self.inner.invalidate_provenance(account_id, provenance)
    }

    /// Unblocks `account_id` on the shared [`BlockedAccounts`], clearing any
    /// block (kill-switch or admin). A no-op when the account is not blocked.
    pub(crate) fn unblock_account(&self, account_id: AccountId) {
        self.inner.unblock_account(account_id);
    }

    /// Clears the engine-wide block. A no-op when no global block is active.
    pub(crate) fn unblock_all(&self) {
        self.inner.unblock_all();
    }

    /// Arms the engine kill switch for a mutation finalizer that failed.
    ///
    /// `scope` comes from the provenance of the failing mutation: an
    /// engine-owned mutation blocks only `account`, a custom-policy one blocks
    /// every account because its state reach is unknown. An engine-owned
    /// failure with no readable account also fails closed and blocks globally:
    /// state was already applied, so the engine cannot scope the damage.
    pub(crate) fn record_mutation_failure(
        &self,
        account: Option<AccountId>,
        scope: MutationFailureScope,
    ) {
        match (scope, account) {
            (MutationFailureScope::Account, Some(account)) => {
                self.inner
                    .block_account(account, new_mutation_finalizer_block());
            }
            (MutationFailureScope::Account, None) | (MutationFailureScope::Global, _) => {
                self.inner
                    .block_all_with_cause(new_mutation_finalizer_block());
            }
        }
    }

    /// Overwrites the stored cause of an already-blocked account, which makes
    /// the block the caller's own. See
    /// [`BlockedAccounts::replace_reason`].
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::AccountNotBlocked`] when `account_id` is
    /// not currently blocked.
    pub(crate) fn replace_reason(
        &self,
        account_id: AccountId,
        block: AccountBlock,
    ) -> Result<(), AccountBlockError> {
        self.inner.replace_reason(account_id, block)
    }

    /// Blocks `group` on the shared [`BlockedAccounts`]. The first cause for a
    /// group wins; later calls for the same group are no-ops.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`].
    pub(crate) fn block_group(
        &self,
        group: AccountGroupId,
        block: AccountBlock,
    ) -> Result<(), AccountBlockError> {
        self.inner.block_group(group, block)
    }

    /// Unblocks `group` on the shared [`BlockedAccounts`]. A no-op when the
    /// group is not blocked.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`].
    pub(crate) fn unblock_group(&self, group: AccountGroupId) -> Result<(), AccountBlockError> {
        self.inner.unblock_group(group)
    }

    /// Overwrites the stored cause of an already-blocked group.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`], or
    /// [`AccountBlockError::GroupNotBlocked`] when `group` is not currently
    /// blocked.
    pub(crate) fn replace_group_reason(
        &self,
        group: AccountGroupId,
        block: AccountBlock,
    ) -> Result<(), AccountBlockError> {
        self.inner.replace_group_reason(group, block)
    }
}

// ─── Reject helpers ──────────────────────────────────────────────────────────

fn new_account_blocked_rejects() -> Rejects {
    Rejects::new(vec![Reject::new(
        "Engine",
        RejectScope::Account,
        RejectCode::AccountBlocked,
        "account is blocked due to kill-switch",
        "kill-switch was previously triggered for this account".to_owned(),
    )])
}

/// Cause stored for a kill switch raised because a mutation finalizer failed.
///
/// Engine-owned and deliberately free of any account or account-group
/// identifier: the block is the engine reporting a failure to itself, and the
/// reject it produces is read by whoever calls next.
fn new_mutation_finalizer_block() -> AccountBlock {
    AccountBlock::new(
        "Engine",
        RejectCode::SystemUnavailable,
        "mutation finalizer failed",
        "a mutation commit or rollback callback failed; engine state may be inconsistent",
    )
}

/// Rejects returned when blocking is active but the request's account cannot be
/// identified, so the blocked set cannot be consulted for it.
fn new_unverifiable_block_check_rejects(scope: RejectScope) -> Rejects {
    Rejects::new(vec![Reject::new(
        "Engine",
        scope,
        RejectCode::MissingRequiredField,
        "account could not be verified as account ID is missing",
        "unable to check account for blocking".to_owned(),
    )])
}

// ─── BlockedAccounts ─────────────────────────────────────────────────────────

/// Per-engine storage for blocked accounts and account groups.
///
/// Uses:
/// - `any_flag`: active-blocking indicator for the level-1 fast path of the
///   per-account set. Set when an account is blocked or a global block fires;
///   cleared by `unblock_account` once the per-account set is empty and no
///   global block is active, so the all-clear fast path is restored.
/// - `all_flag`: set when `block_all*()` is called and cleared by
///   `unblock_all`, the operator's counterpart exposed as
///   [`Accounts::unblock_all`](crate::Accounts::unblock_all).
/// - `blocked_groups_any_flag`: active-blocking indicator gating the group
///   branch of [`check`](Self::check). Set when a group is blocked; cleared by
///   `unblock_group` once the blocked-group set is empty.
/// - `accounts`: per-account storage mapping each blocked `AccountId` to the
///   `AccountBlock` recorded for it (kill-switch or admin). The first cause for
///   an account wins until it is unblocked.
/// - `blocked_groups`: per-group storage mapping each blocked `AccountGroupId`
///   to its admin `AccountBlock`. Group membership is resolved live at check
///   time, so this set holds groups, never expanded members.
/// - `global_block`: single-slot storage holding the cause of an active global
///   block, when the caller supplied one. A global block armed without a cause
///   (an execution report with no readable account) leaves the slot empty and
///   falls back to the generic kill-switch reject.
///
/// # Multi-observer synchronization
///
/// Under [`AccountSync`](crate::core::AccountSync) the per-key *value* domain
/// (`read_values` / `write_values`) is a no-op; only the *index* domain is a
/// real reader-writer lock. The insert/remove mutators take the index lock
/// exclusively, so they already serialize against `check`'s shared index read.
/// The reason-replacing mutators use
/// `Storage::with_mut_if_present_exclusive_index`, so they also take the
/// storage index lock exclusively before overwriting a `String`-bearing
/// [`AccountBlock`]. `check` reads through `Storage::with`, which takes the
/// same storage index lock in shared mode. That keeps admin replacement rare
/// and serialized without adding a whole-structure lock to every blocked-set
/// lookup.
///
/// The index flags are active-blocking indicators, not write-once latches: an
/// `unblock_*` that empties its set flips the corresponding flag back to
/// `false` (see `unblock_account` / `unblock_group`). Multi-step map+flag
/// transitions are serialized by `mutation_guard` on the write side only.
/// `check` does not take that guard: it observes completed writes through the
/// Release/Acquire pairing on the flags, while a `check` already in flight may
/// still act on the old state. That in-flight skew is intended and is exactly
/// what lets the all-clear fast path remain lock-free.
pub(crate) struct BlockedAccounts<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Write-side guard serializing map mutations and active-flag transitions.
    ///
    /// This guard is deliberately not taken by [`check`](Self::check), so it
    /// does not add a whole-structure lock to order validation. Its job is only
    /// to keep mutators' check-then-act flag updates atomic with respect to
    /// each other: insert/remove in the maps, `is_empty()` probes, and
    /// `*_flag.store(...)` must be one critical section. `replace_*` also takes
    /// it to preserve the old external operation ordering, while the storage
    /// exclusive-index helper still serializes replacement against concurrent
    /// `check` readers of the same map.
    mutation_guard: <StorageFactory as storage::LockingPolicyFactory>::Policy,
    any_flag: <StorageFactory as storage::LockingPolicyFactory>::IndexFlag,
    all_flag: <StorageFactory as storage::LockingPolicyFactory>::IndexFlag,
    blocked_groups_any_flag: <StorageFactory as storage::LockingPolicyFactory>::IndexFlag,
    accounts: Storage<AccountId, AccountBlock, StorageFactory::Policy>,
    blocked_groups: Storage<AccountGroupId, AccountBlock, StorageFactory::Policy>,
    global_block: Storage<(), AccountBlock, StorageFactory::Policy>,
}

impl<StorageLockingPolicyFactory> BlockedAccounts<StorageLockingPolicyFactory>
where
    StorageLockingPolicyFactory:
        storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Creates a new, empty blocked-accounts storage using `builder`'s locking
    /// policy.
    pub(crate) fn new(builder: &StorageBuilder<StorageLockingPolicyFactory>) -> Self {
        Self {
            mutation_guard: builder.create_policy(),
            any_flag:
                <StorageLockingPolicyFactory as storage::LockingPolicyFactory>::IndexFlag::new(
                    false,
                ),
            all_flag:
                <StorageLockingPolicyFactory as storage::LockingPolicyFactory>::IndexFlag::new(
                    false,
                ),
            blocked_groups_any_flag:
                <StorageLockingPolicyFactory as storage::LockingPolicyFactory>::IndexFlag::new(
                    false,
                ),
            accounts: builder.create_for_bound_key(),
            // The group set is keyed by `AccountGroupId`, which is deliberately
            // not an account key, so it bypasses the account-sync key gate via
            // `create_for_any_key`. This table is engine-owned, never a
            // policy storage, so the sharding guarantee does not apply.
            blocked_groups: builder.create_for_any_key(),
            // Single-slot table holding the global cause; keyed by the unit
            // type because an engine has exactly one global block.
            global_block: builder.create_for_any_key(),
        }
    }

    /// Checks whether a pre-trade request should be rejected.
    ///
    /// `groups` is the engine's account-group registry, consulted only when at
    /// least one group is blocked; it resolves the order account's group live,
    /// so members of a blocked group are rejected without being expanded into
    /// the per-account set and an account that has left the group is no longer
    /// group-blocked.
    ///
    /// Returns `None` when the order may proceed. Returns `Some(Rejects)` when:
    /// - the order's account is individually blocked;
    /// - the order account's group is blocked;
    /// - a global block is active;
    /// - something is blocked but the account cannot be identified.
    ///
    /// Callers should return the `Rejects` immediately without running any
    /// policies.
    pub(crate) fn check<Order: HasAccountId>(
        &self,
        groups: &AccountGroups<StorageLockingPolicyFactory>,
        order: &Order,
        operation_scope: RejectScope,
    ) -> Option<Rejects> {
        self.with_blocking_cause(
            groups,
            || order.account_id().ok(),
            |block| Rejects::new(vec![Reject::from(block.clone())]),
            new_account_blocked_rejects,
            || new_unverifiable_block_check_rejects(operation_scope),
        )
    }

    /// Returns whether `id` is blocked without constructing a reject payload.
    pub(crate) fn is_blocked(
        &self,
        groups: &AccountGroups<StorageLockingPolicyFactory>,
        id: AccountId,
    ) -> bool {
        self.with_blocking_cause(groups, || Some(id), |_| (), || (), || ())
            .is_some()
    }

    /// Records a kill-switch event from an execution report.
    ///
    /// Extracts the account from `report` and blocks it, storing `cause` as
    /// the reason returned for all future pre-trade requests on that account.
    /// If the report carries no account identifier, activates a global block
    /// instead. The first cause recorded for an account wins; subsequent calls
    /// for the same account are no-ops.
    pub(crate) fn record_execution_report<Report: HasAccountId>(
        &self,
        report: &Report,
        cause: AccountBlock,
    ) {
        match report.account_id() {
            Ok(id) => {
                self.block_account(id, cause);
            }
            Err(_) => self.block_all(),
        }
    }

    /// Records a pre-trade kill-switch event for a readable account only.
    ///
    /// Unlike an execution report, a rejected pre-trade request has not created
    /// exposure. An unreadable account must therefore not activate the global
    /// block, which halts every account until an operator lifts it with
    /// [`Accounts::unblock_all`](crate::Accounts::unblock_all).
    pub(crate) fn record_pre_trade<Order: HasAccountId>(&self, order: &Order, cause: AccountBlock) {
        if let Ok(account) = order.account_id() {
            self.block_account(account, cause);
        }
    }

    /// Blocks `id` with `cause`, returning whether this call inserted it.
    ///
    /// An account holds one cause: the first one wins, so blocking an
    /// already-blocked account returns `false` without changing its cause.
    pub(crate) fn block_account(&self, id: AccountId, cause: AccountBlock) -> bool {
        let _guard = self.mutation_guard.write_index();
        let inserted = self.accounts.with_mut(id, || cause, |_, inserted| inserted);
        self.any_flag.store(true);
        inserted
    }

    #[cfg(test)]
    pub(crate) fn is_all_blocked(&self) -> bool {
        self.all_flag.load()
    }

    #[cfg(test)]
    pub(crate) fn account_block(&self, id: AccountId) -> Option<AccountBlock> {
        self.accounts.with(&id, Clone::clone)
    }

    /// Retires the block raised by the assertion holding `provenance`: when the
    /// stored cause carries that token it is removed and the account unblocks,
    /// since it is the account's only cause. When the cause is no longer the
    /// assertion's own - another cause won the account first, or an operator
    /// overwrote the reason via [`replace_reason`](Self::replace_reason) and so
    /// took ownership of the block - nothing is removed and the account stays
    /// blocked.
    ///
    /// A committed assertion needs no counterpart: its token is never reissued
    /// and its only holder is gone, so no later call can match its cause, which
    /// stays as the legitimate block it is.
    pub(crate) fn invalidate_provenance(
        &self,
        id: AccountId,
        provenance: u64,
    ) -> Option<AccountBlock> {
        let _guard = self.mutation_guard.write_index();
        let removed = self
            .accounts
            .with(&id, |cause| {
                (cause.provenance() == Some(provenance)).then(|| cause.clone())
            })
            .flatten()?;
        self.accounts.remove(&id);
        if self.accounts.is_empty() && !self.all_flag.load() {
            self.any_flag.store(false);
        }
        Some(removed)
    }

    /// Arms the global block without a stored cause, so [`check`](Self::check)
    /// answers with the generic kill-switch reject.
    fn block_all(&self) {
        let _guard = self.mutation_guard.write_index();
        self.any_flag.store(true);
        self.all_flag.store(true);
    }

    /// Arms the global block and stores `cause` as the reason returned for
    /// every account. The first cause wins, like a per-account block.
    pub(crate) fn block_all_with_cause(&self, cause: AccountBlock) {
        let _guard = self.mutation_guard.write_index();
        self.global_block.with_mut((), || cause, |_, _| ());
        self.any_flag.store(true);
        self.all_flag.store(true);
    }

    /// Clears the global block and its cause. A no-op when no global block is
    /// active.
    ///
    /// Clears `any_flag` too once the per-account set is empty, restoring the
    /// all-clear fast path.
    pub(crate) fn unblock_all(&self) {
        let _guard = self.mutation_guard.write_index();
        self.global_block.remove(&());
        self.all_flag.store(false);
        if self.accounts.is_empty() {
            self.any_flag.store(false);
        }
    }

    /// Removes any block (kill-switch or admin) for `id`. A no-op when the
    /// account is not blocked.
    ///
    /// Clears `any_flag` once the per-account set is empty, but only while no
    /// global block is active: `all_flag` keeps the per-account fast path armed
    /// on its own, and the `all_flag` -> `any_flag` invariant must hold. The
    /// flag store is ordered (Release) ahead of any `check` that starts after
    /// this call returns; an in-flight `check` may still observe the old flag
    /// (see the type-level concurrency note).
    pub(crate) fn unblock_account(&self, id: AccountId) {
        let _guard = self.mutation_guard.write_index();
        self.accounts.remove(&id);
        if self.accounts.is_empty() && !self.all_flag.load() {
            self.any_flag.store(false);
        }
    }

    /// Overwrites the stored cause of an already-blocked account.
    ///
    /// The new cause is the caller's own, so it takes ownership of the block:
    /// an operator reason carries no provenance, hence a pending assertion that
    /// raised the overwritten cause will no longer unblock the account.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::AccountNotBlocked`] when `id` is not
    /// currently blocked; `block` does not insert.
    pub(crate) fn replace_reason(
        &self,
        id: AccountId,
        block: AccountBlock,
    ) -> Result<(), AccountBlockError> {
        let _guard = self.mutation_guard.write_index();
        self.accounts
            .with_mut_if_present_exclusive_index(&id, |slot| *slot = block)
            .ok_or(AccountBlockError::AccountNotBlocked { account: id })
    }

    /// Blocks `group`, storing `cause` as the reason returned for every
    /// pre-trade request whose account currently belongs to `group`. The first
    /// cause for a group wins; subsequent calls for the same group are no-ops.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`].
    pub(crate) fn block_group(
        &self,
        group: AccountGroupId,
        cause: AccountBlock,
    ) -> Result<(), AccountBlockError> {
        if group == DEFAULT_ACCOUNT_GROUP {
            return Err(AccountBlockError::ReservedGroup);
        }
        let _guard = self.mutation_guard.write_index();
        self.blocked_groups.with_mut(group, || cause, |_, _| ());
        self.blocked_groups_any_flag.store(true);
        Ok(())
    }

    /// Unblocks `group`. A no-op when the group is not blocked.
    ///
    /// Clears `blocked_groups_any_flag` once the blocked-group set is empty, so
    /// the group branch of [`check`](Self::check) goes back to the lock-free
    /// fast path. The flag store is ordered (Release) ahead of any `check` that
    /// starts after this call returns; an in-flight `check` may still observe
    /// the old flag (see the type-level concurrency note).
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`].
    pub(crate) fn unblock_group(&self, group: AccountGroupId) -> Result<(), AccountBlockError> {
        if group == DEFAULT_ACCOUNT_GROUP {
            return Err(AccountBlockError::ReservedGroup);
        }
        let _guard = self.mutation_guard.write_index();
        self.blocked_groups.remove(&group);
        if self.blocked_groups.is_empty() {
            self.blocked_groups_any_flag.store(false);
        }
        Ok(())
    }

    /// Overwrites the stored cause of an already-blocked group.
    ///
    /// # Errors
    ///
    /// Returns [`AccountBlockError::ReservedGroup`] when `group` is the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`], or
    /// [`AccountBlockError::GroupNotBlocked`] when `group` is not currently
    /// blocked; `block` does not insert.
    pub(crate) fn replace_group_reason(
        &self,
        group: AccountGroupId,
        block: AccountBlock,
    ) -> Result<(), AccountBlockError> {
        if group == DEFAULT_ACCOUNT_GROUP {
            return Err(AccountBlockError::ReservedGroup);
        }
        let _guard = self.mutation_guard.write_index();
        self.blocked_groups
            .with_mut_if_present_exclusive_index(&group, |slot| *slot = block)
            .ok_or(AccountBlockError::GroupNotBlocked { group })
    }

    fn with_blocking_cause<Output>(
        &self,
        groups: &AccountGroups<StorageLockingPolicyFactory>,
        account_id: impl FnOnce() -> Option<AccountId>,
        on_specific: impl Fn(&AccountBlock) -> Output,
        on_global: impl FnOnce() -> Output,
        on_unverifiable: impl FnOnce() -> Output,
    ) -> Option<Output> {
        let all_blocking = self.all_flag.load();
        let account_blocking = all_blocking || self.any_flag.load();
        let group_blocking = self.blocked_groups_any_flag.load();
        if !account_blocking && !group_blocking {
            debug_assert!(!all_blocking);
            return None;
        }

        let Some(id) = account_id() else {
            return Some(on_unverifiable());
        };
        if account_blocking {
            if let Some(result) = self.accounts.with(&id, |block| on_specific(block)) {
                return Some(result);
            }
        }
        if group_blocking {
            if let Some(group) = groups.group_of(id) {
                if let Some(result) = self.blocked_groups.with(&group, |block| on_specific(block)) {
                    return Some(result);
                }
            }
        }
        if !all_blocking {
            return None;
        }
        self.global_block
            .with(&(), |block| on_specific(block))
            .or_else(|| Some(on_global()))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::HasAccountId;
    use crate::param::{AccountGroupId, AccountId};
    use crate::pretrade::RejectCode;
    use crate::storage::{LockingPolicyFactory, NoLocking, StorageBuilder};
    use crate::RequestFieldAccessError;

    fn new_set() -> BlockedAccounts<NoLocking> {
        BlockedAccounts::new(&StorageBuilder::new(NoLocking))
    }

    fn empty_groups() -> AccountGroups<NoLocking> {
        AccountGroups::new(&StorageBuilder::new(NoLocking))
    }

    fn cause(policy: &str, code: RejectCode) -> AccountBlock {
        AccountBlock::new(policy, code, "test block", "details")
    }

    fn admin(reason: &str) -> AccountBlock {
        AccountBlock::new("Engine", RejectCode::AccountBlocked, reason, reason)
    }

    struct AccountOrder(AccountId);

    impl HasAccountId for AccountOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Ok(self.0)
        }
    }

    struct NoAccountOrder;

    impl HasAccountId for NoAccountOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("account_id"))
        }
    }

    fn account(id: u64) -> AccountId {
        AccountId::from_u64(id)
    }

    fn group(id: u32) -> AccountGroupId {
        AccountGroupId::from_u32(id).expect("account group id must be valid")
    }

    #[test]
    fn initially_nothing_blocked() {
        let set = new_set();
        let groups = empty_groups();
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_none());
    }

    #[test]
    fn abandoned_deferred_operations_write_through() {
        let blocked = <NoLocking as crate::storage::LockingPolicyFactory>::new_shared(new_set());
        let account_id = account(1);
        let deferred = DeferredAccountOperations::default();
        let control: AccountControl<NoLocking> =
            AccountControl::new(AccountBlockHandle::from_inner(blocked.clone()), account_id)
                .with_deferred_operations(deferred.clone());

        control.block(cause("deferred", RejectCode::AccountBlocked));
        assert!(blocked
            .check(
                &empty_groups(),
                &AccountOrder(account_id),
                RejectScope::Order
            )
            .is_none());

        deferred.abandon();
        control.block(cause("rollback", RejectCode::ArithmeticOverflow));
        let rejects = blocked
            .check(
                &empty_groups(),
                &AccountOrder(account_id),
                RejectScope::Order,
            )
            .expect("rollback safety block must be visible immediately");
        assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    }

    #[test]
    fn applied_deferred_operations_write_through() {
        let blocked = <NoLocking as crate::storage::LockingPolicyFactory>::new_shared(new_set());
        let account_id = account(1);
        let deferred = DeferredAccountOperations::default();
        let control: AccountControl<NoLocking> =
            AccountControl::new(AccountBlockHandle::from_inner(blocked.clone()), account_id)
                .with_deferred_operations(deferred.clone());

        deferred.apply(&blocked);
        control.block(cause("retained", RejectCode::AccountBlocked));

        let rejects = blocked
            .check(
                &empty_groups(),
                &AccountOrder(account_id),
                RejectScope::Order,
            )
            .expect("a retained account-control handle must write through");
        assert_eq!(rejects[0].policy, "retained");
    }

    #[test]
    fn record_account_blocks_that_account() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &AccountOrder(account(1)),
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());
    }

    #[test]
    fn record_account_does_not_block_other_accounts() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &AccountOrder(account(1)),
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&groups, &AccountOrder(account(2)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn record_no_account_blocks_every_account() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &NoAccountOrder,
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());
        assert!(set
            .check(&groups, &AccountOrder(account(99)), RejectScope::Order)
            .is_some());
    }

    #[test]
    fn pre_trade_record_without_account_does_not_activate_global_block() {
        let set = new_set();
        let groups = empty_groups();

        set.record_pre_trade(
            &NoAccountOrder,
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );

        assert!(!set.is_all_blocked());
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn record_no_account_blocks_unidentifiable_orders() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &NoAccountOrder,
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_some());
    }

    #[test]
    fn record_account_blocks_unidentifiable_orders() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &AccountOrder(account(1)),
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_some());
    }

    #[test]
    fn initially_unidentifiable_order_is_allowed() {
        let set = new_set();
        let groups = empty_groups();
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_none());
    }

    #[test]
    fn check_returns_cause_for_blocked_account() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &AccountOrder(account(1)),
            cause("KillSwitch", RejectCode::PnlKillSwitchTriggered),
        );
        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "KillSwitch");
        assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(rejects[0].scope, RejectScope::Account);
    }

    #[test]
    fn first_cause_wins_on_repeated_block() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &AccountOrder(account(1)),
            cause("First", RejectCode::PnlKillSwitchTriggered),
        );
        set.record_execution_report(
            &AccountOrder(account(1)),
            cause("Second", RejectCode::Other),
        );
        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects[0].policy, "First");
    }

    #[test]
    fn blocking_an_already_blocked_account_keeps_the_first_cause() {
        let set = new_set();
        let groups = empty_groups();
        let id = account(1);
        assert!(set.block_account(id, admin("first")));
        for provenance in 1..=32 {
            assert!(!set.block_account(
                id,
                cause("Later", RejectCode::PnlKillSwitchTriggered)
                    .with_provenance(Some(provenance)),
            ));
            assert!(!set.block_account(id, admin("later")));
        }

        let rejects = set
            .check(&groups, &AccountOrder(id), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects[0].reason, "first");
    }

    #[test]
    fn block_handle_keeps_the_first_cause() {
        let handle: AccountBlockHandle<NoLocking> =
            AccountBlockHandle::from_inner(NoLocking::new_shared(new_set()));
        let id = account(1);

        handle.block_account(id, admin("first"));
        handle.block_account(id, admin("later"));

        assert_eq!(
            handle
                .inner
                .account_block(id)
                .expect("the first block must be stored")
                .reason,
            "first"
        );
    }

    #[test]
    fn invalidating_an_own_provenance_unblocks_the_account() {
        let set = new_set();
        let groups = empty_groups();
        let id = account(1);
        set.block_account(
            id,
            cause("Provisional", RejectCode::PnlKillSwitchTriggered).with_provenance(Some(42)),
        );

        let removed = set
            .invalidate_provenance(id, 42)
            .expect("an own cause must be removed");
        assert_eq!(removed.policy, "Provisional");
        assert!(
            set.check(&groups, &AccountOrder(id), RejectScope::Order)
                .is_none(),
            "removing the only cause must unblock the account"
        );
    }

    #[test]
    fn invalidating_a_provenance_an_operator_overwrote_keeps_the_block() {
        let set = new_set();
        let groups = empty_groups();
        let id = account(1);
        set.block_account(
            id,
            cause("Provisional", RejectCode::PnlKillSwitchTriggered).with_provenance(Some(42)),
        );
        set.replace_reason(id, admin("manual review"))
            .expect("replacing the reason of a blocked account must succeed");

        assert!(
            set.invalidate_provenance(id, 42).is_none(),
            "an overwritten cause is the operator's, not the assertion's"
        );
        let rejects = set
            .check(&groups, &AccountOrder(id), RejectScope::Order)
            .expect("the operator's block must stay active");
        assert_eq!(rejects[0].reason, "manual review");
    }

    #[test]
    fn invalidating_a_foreign_provenance_keeps_the_block() {
        let set = new_set();
        let groups = empty_groups();
        let id = account(1);
        set.block_account(id, admin("manual review"));

        assert!(
            set.invalidate_provenance(id, 42).is_none(),
            "a cause that is not the assertion's own must not be removed"
        );
        let rejects = set
            .check(&groups, &AccountOrder(id), RejectScope::Order)
            .expect("the independent block must stay active");
        assert_eq!(rejects[0].reason, "manual review");
    }

    // ─── Global block ─────────────────────────────────────────────────────

    #[test]
    fn global_block_with_cause_reports_that_cause_for_every_account() {
        let set = new_set();
        let groups = empty_groups();
        set.block_all_with_cause(cause("Engine", RejectCode::SystemUnavailable));

        for id in [1, 2] {
            let rejects = set
                .check(&groups, &AccountOrder(account(id)), RejectScope::Order)
                .expect("a global block must reject every account");
            assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
            assert_eq!(rejects[0].reason, "test block");
        }
    }

    #[test]
    fn global_block_without_cause_keeps_the_generic_reject() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &NoAccountOrder,
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );

        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("a global block must reject every account");
        assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
    }

    #[test]
    fn per_account_cause_wins_over_the_global_one() {
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("individual"));
        set.block_all_with_cause(cause("Engine", RejectCode::SystemUnavailable));

        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("the account is blocked");
        assert_eq!(rejects[0].reason, "individual");
    }

    #[test]
    fn unblock_all_clears_the_global_block_and_its_cause() {
        let set = new_set();
        let groups = empty_groups();
        set.block_all_with_cause(cause("Engine", RejectCode::SystemUnavailable));
        set.unblock_all();

        assert!(!set.is_all_blocked());
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
        // The all-clear fast path is restored, so an order with no readable
        // account is admitted again.
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_none());
    }

    #[test]
    fn unblock_all_keeps_individually_blocked_accounts() {
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("individual"));
        set.block_all_with_cause(cause("Engine", RejectCode::SystemUnavailable));
        set.unblock_all();

        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());
        assert!(set
            .check(&groups, &AccountOrder(account(2)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn unblock_all_without_a_global_block_is_noop() {
        let set = new_set();
        let groups = empty_groups();
        set.unblock_all();
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    // ─── Mutation finalizer kill switch ───────────────────────────────────

    fn new_handle(
        blocked: <NoLocking as crate::storage::LockingPolicyFactory>::Shared<
            BlockedAccounts<NoLocking>,
        >,
    ) -> AccountBlockHandle<NoLocking> {
        AccountBlockHandle::from_inner(blocked)
    }

    #[test]
    fn engine_owned_finalizer_failure_blocks_only_its_account() {
        let blocked = <NoLocking as crate::storage::LockingPolicyFactory>::new_shared(new_set());
        let groups = empty_groups();
        new_handle(blocked.clone())
            .record_mutation_failure(Some(account(1)), MutationFailureScope::Account);

        let rejects = blocked
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("the pipeline account must be blocked");
        assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
        assert_eq!(rejects[0].reason, "mutation finalizer failed");
        assert!(blocked
            .check(&groups, &AccountOrder(account(2)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn custom_policy_finalizer_failure_blocks_everything() {
        let blocked = <NoLocking as crate::storage::LockingPolicyFactory>::new_shared(new_set());
        let groups = empty_groups();
        new_handle(blocked.clone())
            .record_mutation_failure(Some(account(1)), MutationFailureScope::Global);

        for id in [1, 2] {
            let rejects = blocked
                .check(&groups, &AccountOrder(account(id)), RejectScope::Order)
                .expect("a custom-policy finalizer failure blocks every account");
            assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
        }
    }

    #[test]
    fn engine_owned_finalizer_failure_without_an_account_fails_closed() {
        let blocked = <NoLocking as crate::storage::LockingPolicyFactory>::new_shared(new_set());
        let groups = empty_groups();
        new_handle(blocked.clone()).record_mutation_failure(None, MutationFailureScope::Account);

        assert!(blocked.is_all_blocked());
        assert!(blocked
            .check(&groups, &AccountOrder(account(7)), RejectScope::Order)
            .is_some());
    }

    // ─── Admin per-account blocking ───────────────────────────────────────

    #[test]
    fn admin_block_then_check_rejects() {
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("manual review"));
        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("admin-blocked account must return rejects");
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
        assert_eq!(rejects[0].reason, "manual review");
    }

    #[test]
    fn unblock_then_check_passes() {
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("manual review"));
        set.unblock_account(account(1));
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn unblock_of_non_blocked_account_is_noop() {
        let set = new_set();
        let groups = empty_groups();
        set.unblock_account(account(1));
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn unblock_clears_kill_switch_block() {
        let set = new_set();
        let groups = empty_groups();
        set.record_execution_report(
            &AccountOrder(account(1)),
            cause("KillSwitch", RejectCode::PnlKillSwitchTriggered),
        );
        set.unblock_account(account(1));
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn replace_reason_updates_stored_cause() {
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("first"));
        set.replace_reason(account(1), admin("second"))
            .expect("replacing reason on a blocked account must succeed");
        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects[0].reason, "second");
    }

    #[test]
    fn replace_reason_errors_when_not_blocked() {
        let set = new_set();
        let error = set
            .replace_reason(account(1), admin("reason"))
            .expect_err("replacing reason on an unblocked account must fail");
        assert_eq!(
            error,
            AccountBlockError::AccountNotBlocked {
                account: account(1)
            }
        );
    }

    #[test]
    fn block_does_not_overwrite_first_reason() {
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("first"));
        set.block_account(account(1), admin("second"));
        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects[0].reason, "first");
    }

    // ─── Admin group blocking (live predicate) ────────────────────────────

    #[test]
    fn block_group_blocks_current_members() {
        let set = new_set();
        let groups = empty_groups();
        groups
            .register_group(&[account(1), account(2)], group(7))
            .expect("registration must succeed");
        set.block_group(group(7), admin("group halt"))
            .expect("group block must succeed");

        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("member of blocked group must be rejected");
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
        assert_eq!(rejects[0].reason, "group halt");
        assert!(set
            .check(&groups, &AccountOrder(account(2)), RejectScope::Order)
            .is_some());
    }

    #[test]
    fn member_registered_into_blocked_group_is_blocked_automatically() {
        let set = new_set();
        let groups = empty_groups();
        set.block_group(group(7), admin("group halt"))
            .expect("group block must succeed");
        // Account joins the already-blocked group afterwards.
        groups
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");

        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_some());
    }

    #[test]
    fn account_removed_from_blocked_group_is_no_longer_group_blocked() {
        let set = new_set();
        let groups = empty_groups();
        groups
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        set.block_group(group(7), admin("group halt"))
            .expect("group block must succeed");
        groups
            .unregister_group(&[account(1)], group(7))
            .expect("unregistration must succeed");

        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn account_removed_from_blocked_group_stays_blocked_if_individually_blocked() {
        let set = new_set();
        let groups = empty_groups();
        groups
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        set.block_group(group(7), admin("group halt"))
            .expect("group block must succeed");
        set.block_account(account(1), admin("individual"));
        groups
            .unregister_group(&[account(1)], group(7))
            .expect("unregistration must succeed");

        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("individually blocked account must remain blocked");
        assert_eq!(rejects[0].reason, "individual");
    }

    #[test]
    fn unblock_group_lets_members_pass() {
        let set = new_set();
        let groups = empty_groups();
        groups
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        set.block_group(group(7), admin("group halt"))
            .expect("group block must succeed");
        set.unblock_group(group(7))
            .expect("group unblock must succeed");

        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn unblock_group_of_non_blocked_group_is_noop() {
        let set = new_set();
        set.unblock_group(group(7))
            .expect("unblocking a non-blocked group must be a no-op");
    }

    #[test]
    fn block_group_first_cause_wins() {
        let set = new_set();
        let groups = empty_groups();
        groups
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        set.block_group(group(7), admin("first"))
            .expect("group block must succeed");
        set.block_group(group(7), admin("second"))
            .expect("re-blocking a group must be a no-op");

        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("member of blocked group must be rejected");
        assert_eq!(rejects[0].reason, "first");
    }

    #[test]
    fn replace_group_reason_updates_stored_cause() {
        let set = new_set();
        let groups = empty_groups();
        groups
            .register_group(&[account(1)], group(7))
            .expect("registration must succeed");
        set.block_group(group(7), admin("first"))
            .expect("group block must succeed");
        set.replace_group_reason(group(7), admin("second"))
            .expect("replacing reason on a blocked group must succeed");

        let rejects = set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .expect("member of blocked group must be rejected");
        assert_eq!(rejects[0].reason, "second");
    }

    #[test]
    fn replace_group_reason_errors_when_not_blocked() {
        let set = new_set();
        let error = set
            .replace_group_reason(group(7), admin("reason"))
            .expect_err("replacing reason on an unblocked group must fail");
        assert_eq!(
            error,
            AccountBlockError::GroupNotBlocked { group: group(7) }
        );
    }

    #[test]
    fn group_operations_reject_reserved_default_group() {
        let set = new_set();
        assert_eq!(
            set.block_group(DEFAULT_ACCOUNT_GROUP, admin("reason")),
            Err(AccountBlockError::ReservedGroup)
        );
        assert_eq!(
            set.unblock_group(DEFAULT_ACCOUNT_GROUP),
            Err(AccountBlockError::ReservedGroup)
        );
        assert_eq!(
            set.replace_group_reason(DEFAULT_ACCOUNT_GROUP, admin("reason")),
            Err(AccountBlockError::ReservedGroup)
        );
    }

    #[test]
    fn account_block_error_display_is_stable() {
        assert_eq!(
            AccountBlockError::ReservedGroup.to_string(),
            "the reserved default account group is not a valid target"
        );
        assert_eq!(
            AccountBlockError::AccountNotBlocked {
                account: account(1)
            }
            .to_string(),
            "account is not blocked"
        );
        assert_eq!(
            AccountBlockError::GroupNotBlocked { group: group(2) }.to_string(),
            "account group is not blocked"
        );
    }

    // The Display text must not leak the account or account-group id, yet the
    // structured variant fields must still carry them for programmatic access.
    #[test]
    fn account_block_error_display_hides_ids_but_keeps_structured_fields() {
        let sentinel_account = account(424242);
        let err = AccountBlockError::AccountNotBlocked {
            account: sentinel_account,
        };
        assert!(
            !err.to_string().contains("424242"),
            "display leaked account id: {err}"
        );
        let AccountBlockError::AccountNotBlocked { account } = err else {
            panic!("variant must be preserved");
        };
        assert_eq!(account, sentinel_account);

        let sentinel_group = group(424242);
        let err = AccountBlockError::GroupNotBlocked {
            group: sentinel_group,
        };
        assert!(
            !err.to_string().contains("424242"),
            "display leaked group id: {err}"
        );
        let AccountBlockError::GroupNotBlocked { group } = err else {
            panic!("variant must be preserved");
        };
        assert_eq!(group, sentinel_group);
    }

    // ─── Flag reset on unblock (active-blocking indicators) ───────────────

    #[test]
    fn unblock_last_account_clears_unidentifiable_order_rejection() {
        // Regression: a block→unblock cycle must restore the all-clear fast
        // path so an order without an account id is allowed again.
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("manual review"));
        set.unblock_account(account(1));
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_none());
    }

    #[test]
    fn unblock_last_group_clears_unidentifiable_order_rejection() {
        let set = new_set();
        let groups = empty_groups();
        set.block_group(group(7), admin("group halt"))
            .expect("group block must succeed");
        set.unblock_group(group(7))
            .expect("group unblock must succeed");
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_none());
    }

    #[test]
    fn unblock_one_of_two_accounts_keeps_flag_active() {
        let set = new_set();
        let groups = empty_groups();
        set.block_account(account(1), admin("first"));
        set.block_account(account(2), admin("second"));
        set.unblock_account(account(1));
        // The per-account set is still non-empty, so the indicator stays set
        // and an unidentifiable order is still rejected.
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_some());
    }

    #[test]
    fn unblock_one_of_two_groups_keeps_flag_active() {
        let set = new_set();
        let groups = empty_groups();
        set.block_group(group(7), admin("first"))
            .expect("group block must succeed");
        set.block_group(group(8), admin("second"))
            .expect("group block must succeed");
        set.unblock_group(group(7))
            .expect("group unblock must succeed");
        // The blocked-group set is still non-empty, so the group indicator
        // stays set and an unidentifiable order is still rejected.
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_some());
    }

    #[test]
    fn unblock_account_does_not_clear_global_block() {
        let set = new_set();
        let groups = empty_groups();
        // A report without an account id activates a global block.
        set.record_execution_report(
            &NoAccountOrder,
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        // Unblocking an (unrelated, never-blocked) account empties the
        // per-account set, but the global block must survive.
        set.unblock_account(account(1));
        assert!(set
            .check(&groups, &AccountOrder(account(2)), RejectScope::Order)
            .is_some());
        assert!(set
            .check(&groups, &NoAccountOrder, RejectScope::Order)
            .is_some());
    }

    #[test]
    fn full_locking_concurrent_block_and_unblock_preserve_account_flag() {
        use crate::storage::FullLocking;
        use std::sync::{Arc, Barrier};
        use std::thread;

        for _ in 0..128 {
            let set: Arc<BlockedAccounts<FullLocking>> =
                Arc::new(BlockedAccounts::new(&StorageBuilder::new(FullLocking)));
            let groups = AccountGroups::new(&StorageBuilder::new(FullLocking));
            set.block_account(account(2), admin("old"));

            let barrier = Arc::new(Barrier::new(3));
            thread::scope(|scope| {
                let block_set = Arc::clone(&set);
                let block_barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    block_barrier.wait();
                    block_set.block_account(account(1), admin("new"));
                });

                let unblock_set = Arc::clone(&set);
                let unblock_barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    unblock_barrier.wait();
                    unblock_set.unblock_account(account(2));
                });

                barrier.wait();
            });

            assert!(set
                .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
                .is_some());
        }
    }

    #[test]
    fn full_locking_concurrent_block_all_and_unblock_preserve_global_flag() {
        use crate::storage::FullLocking;
        use std::sync::{Arc, Barrier};
        use std::thread;

        for _ in 0..128 {
            let set: Arc<BlockedAccounts<FullLocking>> =
                Arc::new(BlockedAccounts::new(&StorageBuilder::new(FullLocking)));
            let groups = AccountGroups::new(&StorageBuilder::new(FullLocking));
            set.block_account(account(2), admin("old"));

            let barrier = Arc::new(Barrier::new(3));
            thread::scope(|scope| {
                let block_set = Arc::clone(&set);
                let block_barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    block_barrier.wait();
                    block_set.block_all();
                });

                let unblock_set = Arc::clone(&set);
                let unblock_barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    unblock_barrier.wait();
                    unblock_set.unblock_account(account(2));
                });

                barrier.wait();
            });

            assert!(set
                .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
                .is_some());
            assert!(set
                .check(&groups, &NoAccountOrder, RejectScope::Order)
                .is_some());
        }
    }

    #[test]
    fn full_locking_concurrent_group_block_and_unblock_preserve_group_flag() {
        use crate::storage::FullLocking;
        use std::sync::{Arc, Barrier};
        use std::thread;

        for _ in 0..128 {
            let set: Arc<BlockedAccounts<FullLocking>> =
                Arc::new(BlockedAccounts::new(&StorageBuilder::new(FullLocking)));
            let groups = AccountGroups::new(&StorageBuilder::new(FullLocking));
            groups
                .register_group(&[account(1)], group(10))
                .expect("group registration must succeed");
            set.block_group(group(20), admin("old"))
                .expect("group block must succeed");

            let barrier = Arc::new(Barrier::new(3));
            thread::scope(|scope| {
                let block_set = Arc::clone(&set);
                let block_barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    block_barrier.wait();
                    block_set
                        .block_group(group(10), admin("new"))
                        .expect("group block must succeed");
                });

                let unblock_set = Arc::clone(&set);
                let unblock_barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    unblock_barrier.wait();
                    unblock_set
                        .unblock_group(group(20))
                        .expect("group unblock must succeed");
                });

                barrier.wait();
            });

            assert!(set
                .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
                .is_some());
        }
    }

    // Concurrency: under FullLocking (FullSync engines) reason replacement
    // takes the storage index exclusively, so it must serialize against
    // worker `check` reads without adding a whole-structure guard to every
    // check. Heavy interleaving of block/unblock/replace_reason with check
    // must neither panic nor leave the set inconsistent.
    #[test]
    fn full_locking_admin_mutations_race_check_without_corruption() {
        use crate::storage::FullLocking;
        use std::sync::Arc;
        use std::thread;

        let set: Arc<BlockedAccounts<FullLocking>> =
            Arc::new(BlockedAccounts::new(&StorageBuilder::new(FullLocking)));
        let groups: Arc<AccountGroups<FullLocking>> =
            Arc::new(AccountGroups::new(&StorageBuilder::new(FullLocking)));

        thread::scope(|scope| {
            // Writers churn the same contended account: block, replace its
            // (String-bearing) reason, and unblock, over and over.
            for tid in 0..4u64 {
                let set = Arc::clone(&set);
                scope.spawn(move || {
                    for round in 0..500 {
                        set.block_account(account(1), admin("blocked"));
                        // `replace_reason` overwrites the stored AccountBlock;
                        // this is the path that races `check`'s clone.
                        let _ =
                            set.replace_reason(account(1), admin(&format!("reason-{tid}-{round}")));
                        set.unblock_account(account(1));
                    }
                });
            }
            // Readers hammer `check` on the same account concurrently.
            for _ in 0..4 {
                let set = Arc::clone(&set);
                let groups = Arc::clone(&groups);
                scope.spawn(move || {
                    for _ in 0..500 {
                        // Either outcome is valid depending on interleaving;
                        // the point is that the clone must not race a write.
                        let _ = set.check(&groups, &AccountOrder(account(1)), RejectScope::Order);
                    }
                });
            }
        });

        // Deterministic terminal state: after a final unblock the account is
        // clear, and an account never touched by any thread is never blocked.
        set.unblock_account(account(1));
        assert!(set
            .check(&groups, &AccountOrder(account(1)), RejectScope::Order)
            .is_none());
        assert!(set
            .check(&groups, &AccountOrder(account(2)), RejectScope::Order)
            .is_none());
    }
}
