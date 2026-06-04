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

use crate::param::{AccountGroupId, AccountId};
use crate::storage::{self, StorageBuilder};

use super::{AccountControl, AccountGroups, AccountGroupsHandle, GroupLookup};

/// Context of the current account-adjustment operation.
///
/// Carries an [`AccountControl`] bound to the account being adjusted so
/// rollback closures can record overflow blocks without repeating the
/// account identifier.
///
/// Also exposes the adjusted account's
/// [`AccountGroupId`](crate::param::AccountGroupId) through
/// [`account_group`](Self::account_group).
pub struct AccountAdjustmentContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    /// Per-account control bound to the account being adjusted.
    pub account_control: AccountControl<StorageFactory>,
    group_lookup: GroupLookup<StorageFactory>,
}

impl<StorageFactory> AccountAdjustmentContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn with_groups(
        account_control: AccountControl<StorageFactory>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: AccountId,
    ) -> Self {
        Self {
            account_control,
            group_lookup: GroupLookup::new(account_groups, Some(account)),
        }
    }

    /// Creates a standalone context for testing a [`PreTradePolicy`]'s
    /// account-adjustment hook outside an engine.
    ///
    /// `account` is the adjusted account. The context is backed by an empty,
    /// private account-group registry, so [`account_group`](Self::account_group)
    /// returns `None`. Inside the engine the registry is the engine's shared
    /// one; this constructor exists so policy authors can drive a policy's hook
    /// directly in unit tests.
    ///
    /// [`PreTradePolicy`]: crate::pretrade::PreTradePolicy
    pub fn new(account_control: AccountControl<StorageFactory>, account: AccountId) -> Self
    where
        StorageFactory: Default,
    {
        let builder = StorageBuilder::new(StorageFactory::default());
        let handle = AccountGroupsHandle::from_inner(StorageFactory::new_shared(
            AccountGroups::new(&builder),
        ));
        Self::with_groups(account_control, handle, account)
    }

    /// Returns the group of the adjusted account, or `None` when it is not
    /// registered.
    ///
    /// The lookup is performed once and cached for the lifetime of this context.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        self.group_lookup.group()
    }

    /// Test-only constructor with a placeholder bound account.
    #[cfg(test)]
    pub(crate) fn new_test(account_control: AccountControl<StorageFactory>) -> Self
    where
        StorageFactory: Default,
    {
        Self::new(account_control, AccountId::from_u64(0))
    }
}
