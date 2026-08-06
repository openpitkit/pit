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

use crate::param::{AccountGroupId, AccountId, Asset};
use crate::storage::{self, StorageBuilder};

use super::{AccountControl, AccountGroups, AccountGroupsHandle, Accounts, GroupLookup};

pub(crate) struct AccountStateSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    accounts: Option<Accounts<StorageFactory>>,
    account: AccountId,
    group: Option<AccountGroupId>,
}

impl<StorageFactory> Clone for AccountStateSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn clone(&self) -> Self {
        Self {
            accounts: self.accounts.clone(),
            account: self.account,
            group: self.group,
        }
    }
}

impl<StorageFactory> AccountStateSnapshot<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn with_rollback<R>(
        &self,
        operation: impl FnOnce(Option<AccountGroupId>, Option<Asset>) -> R,
    ) -> R {
        let Some(accounts) = self.accounts.as_ref() else {
            return operation(self.group, None);
        };
        accounts.with_state_rollback(|| {
            let account_group = accounts.group_of(self.account);
            let account_currency = accounts.currency_of_in_group(self.account, account_group);
            operation(account_group, account_currency)
        })
    }
}

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
    accounts: Option<Accounts<StorageFactory>>,
    account: AccountId,
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
            accounts: None,
            account,
            group_lookup: GroupLookup::new(account_groups, Some(account)),
        }
    }

    pub(crate) fn with_accounts(
        account_control: AccountControl<StorageFactory>,
        accounts: Accounts<StorageFactory>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: AccountId,
    ) -> Self {
        Self {
            account_control,
            accounts: Some(accounts),
            account,
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
    /// The lookup is performed once and cached for the lifetime of this
    /// context, so repeated calls during one evaluation return the same group.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        self.group_lookup.group()
    }

    pub(crate) fn state_account_group(&self) -> Option<AccountGroupId> {
        self.accounts.as_ref().map_or_else(
            || self.group_lookup.group(),
            |accounts| accounts.group_of(self.account),
        )
    }

    /// Returns the effective currency for the adjusted account.
    ///
    /// The caller supplies `account_group`; this method resolves only the
    /// account -> group -> default currency cascade. Standalone contexts have
    /// no account registry, so they return `None`.
    pub(crate) fn account_currency(&self, account_group: Option<AccountGroupId>) -> Option<Asset> {
        self.accounts
            .as_ref()
            .and_then(|accounts| accounts.currency_of_in_group(self.account, account_group))
    }

    pub(crate) fn with_state_writer<R>(&self, operation: impl FnOnce() -> R) -> R {
        match self.accounts.as_ref() {
            Some(accounts) => accounts.with_state_writer(operation),
            None => operation(),
        }
    }

    pub(crate) fn with_state_writer_bypassing_transition<R>(
        &self,
        operation: impl FnOnce() -> R,
    ) -> R {
        match self.accounts.as_ref() {
            Some(accounts) => accounts.with_state_rollback(operation),
            None => operation(),
        }
    }

    pub(crate) fn state_snapshot(&self) -> AccountStateSnapshot<StorageFactory> {
        AccountStateSnapshot {
            accounts: self.accounts.clone(),
            account: self.account,
            group: self.state_account_group(),
        }
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
