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

use parking_lot::Mutex;

use super::reject::AccountBlock;
use crate::core::{AccountControl, AccountGroups, AccountGroupsHandle, GroupLookup};
use crate::param::{AccountGroupId, AccountId};
use crate::storage::{self, StorageBuilder};

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
    drop_copy_account_block: Mutex<Option<AccountBlock>>,
    drop_copy: bool,
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
        Self::with_groups_and_drop_copy(account_control, account_groups, account, false)
    }

    pub(crate) fn with_groups_and_drop_copy(
        account_control: Option<AccountControl<StorageFactory>>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: Option<AccountId>,
        drop_copy: bool,
    ) -> Self {
        Self {
            account_control,
            group_lookup: GroupLookup::new(account_groups, account),
            drop_copy_account_block: Mutex::new(None),
            drop_copy,
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
        self.drop_copy
    }

    pub(crate) fn record_drop_copy_account_block(&self, block: AccountBlock) {
        if !self.drop_copy {
            return;
        }
        let mut account_block = self.drop_copy_account_block.lock();
        if account_block.is_none() {
            *account_block = Some(block);
        }
    }

    pub(crate) fn take_drop_copy_account_block(&self) -> Option<AccountBlock> {
        if self.drop_copy {
            self.drop_copy_account_block.lock().take()
        } else {
            None
        }
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
