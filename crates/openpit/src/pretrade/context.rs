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
    AccountControl, AccountGroups, AccountGroupsHandle, BlockedAccounts, GroupLookup,
};
use crate::param::{AccountGroupId, AccountId};
use crate::storage::{self, StorageBuilder};
use crate::{Mutation, Mutations};

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
    account: AccountId,
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
/// Carries an [`AccountControl`] bound to the order's account so rollback
/// closures can record overflow blocks without repeating the account
/// identifier. The control is `None` when the order carries no recognizable
/// account identifier; in that case rollback closures must not call
/// [`AccountControl::block`].
///
/// Also exposes the order account's
/// [`AccountGroupId`](crate::param::AccountGroupId) through
/// [`account_group`](Self::account_group).
///
/// Operation arguments (order data, mutations) are passed as explicit method
/// arguments and intentionally do not live inside this context.
pub struct PreTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Per-account control bound to the order's account, or `None` when the
    /// account identifier could not be extracted from the order.
    pub account_control: Option<AccountControl<StorageFactory>>,
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
        account: Option<AccountId>,
    ) -> Self {
        Self {
            account_control,
            group_lookup: GroupLookup::new(account_groups, account),
            drop_copy: None,
        }
    }

    /// Creates a drop-copy context bound to `account`.
    ///
    /// Both the account and its control are mandatory here: drop-copy defers
    /// policy blocks until the operation succeeds, and a deferred block with no
    /// account would be reported to the caller yet never reach the blocked set.
    pub(crate) fn with_groups_and_drop_copy(
        account_control: AccountControl<StorageFactory>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: AccountId,
    ) -> Self {
        let account_operations = DeferredAccountOperations::default();
        let account_control = account_control.with_deferred_operations(account_operations.clone());
        Self {
            account_control: Some(account_control.clone()),
            group_lookup: GroupLookup::new(account_groups, Some(account)),
            drop_copy: Some(Box::new(DropCopyState {
                account_control,
                account_operations,
                start_mutations: DropCopyStartMutationRecorder::new(),
                account,
            })),
        }
    }

    /// Creates a standalone context for testing a [`PreTradePolicy`] outside an
    /// engine.
    ///
    /// The context is backed by an empty, private account-group registry and no
    /// bound account, so [`account_group`](Self::account_group) returns `None`.
    /// Inside the engine the registry is the engine's shared one; this
    /// constructor exists so policy authors can drive a policy's hooks directly
    /// in unit tests.
    ///
    /// [`PreTradePolicy`]: crate::pretrade::PreTradePolicy
    pub fn new(account_control: Option<AccountControl<StorageFactory>>) -> Self
    where
        StorageFactory: Default,
    {
        let builder = StorageBuilder::new(StorageFactory::default());
        let handle = AccountGroupsHandle::from_inner(StorageFactory::new_shared(
            AccountGroups::new(&builder),
        ));
        Self::with_groups(account_control, handle, None)
    }

    /// Returns the group of the order's account, or `None` when the account is
    /// absent or unregistered.
    ///
    /// The lookup is performed once and cached for the lifetime of this context.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        self.group_lookup.group()
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

    pub(crate) fn drop_copy_account_id(&self) -> Option<AccountId> {
        self.drop_copy.as_ref().map(|state| state.account)
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
