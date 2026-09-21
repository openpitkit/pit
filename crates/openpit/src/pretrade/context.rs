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

use std::cell::RefCell;
use std::rc::Rc;

use super::reject::AccountBlock;
use crate::core::account_control::DeferredAccountOperations;
use crate::core::{
    AccountControl, AccountGroups, AccountGroupsHandle, Accounts, BlockedAccounts, GroupLookup,
    HasAccountId,
};
use crate::param::{AccountGroupId, AccountId, Asset};
use crate::storage::{self, StorageBuilder};
use crate::{Mutation, Mutations, RequestFieldAccessError};

/// Drop-copy state of a [`PreTradeContext`].
///
/// The account is not optional: the engine rejects a drop-copy order whose
/// account cannot be read before it builds the pipeline, so a deferred block
/// always has an account to land on.
struct DropCopyState<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    // `PreTradeContext::account_control` is `Option` because an ordinary
    // order may carry no readable account id, but drop-copy always has one
    // (see the struct doc above). This copy is un-`Option`-wrapped, so
    // `account_control.block(block)` needs no unwrap and no reachable
    // panic path.
    account_control: AccountControl<StorageFactory>,
    account_operations: DeferredAccountOperations,
    start_mutations: DropCopyStartMutationRecorder,
}

/// Callback-scoped recorder for atomic drop-copy start-stage mutations.
#[doc(hidden)]
#[derive(Clone)]
pub struct DropCopyStartMutationRecorder {
    mutations: Rc<RefCell<Option<Mutations>>>,
}

impl DropCopyStartMutationRecorder {
    fn new() -> Self {
        Self {
            mutations: Rc::new(RefCell::new(Some(Mutations::new()))),
        }
    }

    /// Records `mutation` while the owning drop-copy operation is active.
    ///
    /// Returns the mutation to the caller after the engine has finalized the
    /// operation.
    #[doc(hidden)]
    pub fn record(&self, mutation: Mutation) -> Result<(), Mutation> {
        let mut mutations = self.mutations.borrow_mut();
        let Some(mutations) = mutations.as_mut() else {
            return Err(mutation);
        };
        mutations.push(mutation);
        Ok(())
    }

    /// Creates and records a mutation only while the operation is active.
    ///
    /// This preserves caller ownership when constructing the mutation itself
    /// transfers ownership to the engine.
    #[doc(hidden)]
    pub fn record_with(&self, create: impl FnOnce() -> Mutation) -> bool {
        let mut mutations = self.mutations.borrow_mut();
        let Some(mutations) = mutations.as_mut() else {
            return false;
        };
        mutations.push(create());
        true
    }

    /// Returns whether the owning drop-copy operation still accepts mutations.
    #[doc(hidden)]
    pub fn is_active(&self) -> bool {
        self.mutations.borrow().is_some()
    }

    fn take(&self) -> Mutations {
        self.mutations.borrow_mut().take().unwrap_or_default()
    }
}

/// Context of the current pre-trade operation.
///
/// Carries the operation's account, read from the order exactly once when the
/// operation starts and available to every policy through
/// [`account_id`](Self::account_id). That single read is what the engine routes
/// and blocks on, so a policy that reads the order again is not reading the
/// same operation's account.
///
/// Carries an [`AccountControl`] bound to that account so rollback closures can
/// record overflow blocks without repeating the account identifier. The control
/// is `None` when the account identifier could not be read; in that case
/// rollback closures must not call [`AccountControl::block`].
///
/// Also exposes the order account's
/// [`AccountGroupId`](crate::param::AccountGroupId) through
/// [`account_group`](Self::account_group).
///
/// Order data the engine does not need itself, and mutations, are passed as
/// explicit method arguments and intentionally do not live inside this context.
pub struct PreTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Per-account control bound to the operation's account, or `None` when the
    /// account identifier could not be read from the order.
    pub account_control: Option<AccountControl<StorageFactory>>,
    accounts: Option<Accounts<StorageFactory>>,
    account: Result<AccountId, RequestFieldAccessError>,
    group_lookup: GroupLookup<StorageFactory>,
    drop_copy: Option<Box<DropCopyState<StorageFactory>>>,
}

impl<StorageFactory> PreTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn with_groups(
        account_control: Option<AccountControl<StorageFactory>>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: Result<AccountId, RequestFieldAccessError>,
    ) -> Self {
        let lookup_account = account.as_ref().ok().copied();
        Self {
            account_control,
            accounts: None,
            account,
            group_lookup: GroupLookup::new(account_groups, lookup_account),
            drop_copy: None,
        }
    }

    pub(crate) fn with_accounts(
        account_control: Option<AccountControl<StorageFactory>>,
        accounts: Accounts<StorageFactory>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: Result<AccountId, RequestFieldAccessError>,
    ) -> Self {
        let lookup_account = account.as_ref().ok().copied();
        Self {
            account_control,
            accounts: Some(accounts),
            account,
            group_lookup: GroupLookup::new(account_groups, lookup_account),
            drop_copy: None,
        }
    }

    /// Creates an engine-backed drop-copy context bound to `account`.
    ///
    /// Both the account and its control are mandatory here: drop-copy defers
    /// policy blocks until the operation succeeds, and a deferred block with no
    /// account would be reported to the caller yet never reach the blocked set.
    pub(crate) fn with_accounts_and_drop_copy(
        account_control: AccountControl<StorageFactory>,
        accounts: Accounts<StorageFactory>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: AccountId,
    ) -> Self {
        let account_operations = DeferredAccountOperations::default();
        let account_control = account_control.with_deferred_operations(account_operations.clone());
        Self {
            account_control: Some(account_control.clone()),
            accounts: Some(accounts),
            account: Ok(account),
            group_lookup: GroupLookup::new(account_groups, Some(account)),
            drop_copy: Some(Box::new(DropCopyState {
                account_control,
                account_operations,
                start_mutations: DropCopyStartMutationRecorder::new(),
            })),
        }
    }

    /// Creates a standalone context for testing a [`PreTradePolicy`] outside an
    /// engine.
    ///
    /// `order` is read exactly once, here, for the fields the engine itself
    /// needs - the same single read the engine performs when an operation
    /// starts. Those fields are the bound on `Order`, and a field the engine
    /// starts reading for itself later joins it: a test order is expected to
    /// implement the whole engine-read accessor set, not only the accessors the
    /// policy under test happens to use. Taking the order rather than a
    /// pre-read value is deliberate - it is what makes the context and the
    /// order agree by construction.
    ///
    /// The context is backed by an empty, private account-group registry, so
    /// [`account_group`](Self::account_group) returns `None`; inside the engine
    /// the registry is the engine's shared one. This constructor exists so
    /// policy authors can drive a policy's hooks directly in unit tests.
    ///
    /// [`PreTradePolicy`]: crate::pretrade::PreTradePolicy
    pub fn new<Order>(
        account_control: Option<AccountControl<StorageFactory>>,
        order: &Order,
    ) -> Self
    where
        StorageFactory: Default,
        Order: HasAccountId,
    {
        let builder = StorageBuilder::new(StorageFactory::default());
        let handle = AccountGroupsHandle::from_inner(StorageFactory::new_shared(
            AccountGroups::new(&builder),
        ));
        Self::with_groups(account_control, handle, order.account_id())
    }

    /// Returns the operation's account.
    ///
    /// The engine reads the order's account identifier once, when the operation
    /// starts, and every account decision of that operation - the blocked-set
    /// check, [`account_control`](Self::account_control), the recorded
    /// kill-switch block - is made on this value. A policy that needs the
    /// account takes it from here: reading it from the order again is a second,
    /// independent read of a caller-implemented accessor, and nothing requires
    /// that accessor to answer twice the same.
    ///
    /// # Errors
    ///
    /// Returns the error the order's accessor returned for that single read.
    /// An unreadable account reaches a policy only when the blocked set could
    /// clear the request without one: with any account, group, or global block
    /// armed, the engine rejects an unidentifiable account with
    /// [`RejectCode::MissingRequiredField`](crate::pretrade::RejectCode) before
    /// any policy runs. Past that point the operation continues, and an account
    /// is required only by the policies that need one.
    pub fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.account.clone()
    }

    /// Returns the group of the order's account, or `None` when the account is
    /// absent or unregistered.
    ///
    /// The lookup is performed once and cached for the lifetime of this
    /// context, so repeated calls during one evaluation return the same group.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        self.group_lookup.group()
    }

    pub(crate) fn state_account_group(&self) -> Option<AccountGroupId> {
        match (self.accounts.as_ref(), self.known_account()) {
            (Some(accounts), Some(account)) => accounts.group_of(account),
            _ => self.group_lookup.group(),
        }
    }

    /// The operation's account when it was readable, for the engine's own
    /// internal paths that treat an unreadable account as simply absent.
    fn known_account(&self) -> Option<AccountId> {
        self.account.as_ref().ok().copied()
    }

    /// Returns the effective currency for the order's account.
    ///
    /// The caller supplies `account_group`; this method resolves only the
    /// account -> group -> default currency cascade. Standalone contexts have
    /// no account registry, so they return `None`.
    pub(crate) fn account_currency(&self, account_group: Option<AccountGroupId>) -> Option<Asset> {
        self.known_account().and_then(|account| {
            self.accounts
                .as_ref()?
                .currency_of_in_group(account, account_group)
        })
    }

    pub(crate) fn with_state_writer<R>(&self, operation: impl FnOnce() -> R) -> R {
        match self.accounts.as_ref() {
            Some(accounts) => accounts.with_state_writer(operation),
            None => operation(),
        }
    }

    pub(crate) fn state_accounts(&self) -> Option<Accounts<StorageFactory>> {
        self.accounts.clone()
    }

    /// Returns whether policy rejects are non-blocking for this operation.
    ///
    /// Policies normally run identically in both modes. A policy needs this
    /// flag only when enforcing a boundary would prevent its regular
    /// bookkeeping, such as recording a negative available balance.
    pub fn is_drop_copy(&self) -> bool {
        self.drop_copy.is_some()
    }

    pub(crate) fn record_drop_copy_account_block(&self, block: AccountBlock) {
        if let Some(state) = &self.drop_copy {
            state.account_control.block(block);
        }
    }

    /// The operation's account when this is a drop copy, derived rather than
    /// stored: a drop-copy context is only ever built with a readable account,
    /// so the two can never disagree.
    pub(crate) fn drop_copy_account_id(&self) -> Option<AccountId> {
        self.drop_copy
            .is_some()
            .then(|| self.known_account())
            .flatten()
    }

    /// Records a start-stage mutation owned by an atomic drop-copy operation.
    ///
    /// The producer applies tentative state before registration and supplies a
    /// rollback that reverses it. On evaluation failure the engine runs every
    /// recorded rollback in reverse order; otherwise the mutation is handed to
    /// the returned operation and finalized by its owner.
    ///
    /// Returns the mutation unchanged when this is not an active drop-copy
    /// context. The caller still owns the tentative state in that case and must
    /// roll it back or register it elsewhere.
    pub fn record_drop_copy_start_mutation(&self, mutation: Mutation) -> Result<(), Mutation> {
        let Some(drop_copy) = &self.drop_copy else {
            return Err(mutation);
        };
        drop_copy.start_mutations.record(mutation)
    }

    /// Returns a callback-scoped recorder for drop-copy start-stage mutations.
    ///
    /// Bindings use this handle to preserve the same transactional seam as
    /// [`Self::record_drop_copy_start_mutation`] without retaining this context.
    #[doc(hidden)]
    pub fn drop_copy_start_mutation_recorder(&self) -> Option<DropCopyStartMutationRecorder> {
        self.drop_copy
            .as_ref()
            .map(|state| state.start_mutations.clone())
    }

    pub(crate) fn drop_copy_account_block(&self) -> Option<AccountBlock> {
        self.drop_copy
            .as_ref()
            .and_then(|state| state.account_operations.first_block())
    }

    pub(crate) fn apply_drop_copy_account_operations(
        &self,
        blocked_accounts: &BlockedAccounts<StorageFactory>,
    ) {
        if let Some(state) = &self.drop_copy {
            state.account_operations.apply(blocked_accounts);
        }
    }

    pub(crate) fn abandon_drop_copy_account_operations(&self) {
        if let Some(state) = &self.drop_copy {
            state.account_operations.abandon();
        }
    }

    pub(crate) fn take_drop_copy_start_mutations(&self) -> Mutations {
        self.drop_copy
            .as_ref()
            .map(|state| state.start_mutations.take())
            .unwrap_or_default()
    }
}

impl<StorageFactory> crate::marketdata::AccountInfo for PreTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Delegates to [`PreTradeContext::account_group`]; the order account's
    /// group is the source consulted for group-level quote/TTL resolution.
    fn group(&self) -> Option<AccountGroupId> {
        self.account_group()
    }
}
