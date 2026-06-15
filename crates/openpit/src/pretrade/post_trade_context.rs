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

#[cfg(test)]
use crate::core::{AccountBlockHandle, AccountCurrencies, BlockedAccounts};
use crate::core::{AccountGroups, AccountGroupsHandle, Accounts, GroupLookup};
use crate::param::{AccountGroupId, AccountId, Asset};
use crate::storage::{self, StorageBuilder};

/// Context of the current post-trade (execution-report) operation.
///
/// Exposes lazy account-group accessors and account-currency access for the
/// report's account. Unlike
/// [`PreTradeContext`](crate::pretrade::PreTradeContext) and
/// [`AccountAdjustmentContext`](crate::AccountAdjustmentContext) it carries no
/// `account_control`: post-trade processing reports account blocks through the
/// [`PostTradeResult`](crate::pretrade::PostTradeResult) return value instead.
///
/// The bound account is the report's account; it is `None` when the report
/// carries no recognizable account identifier, in which case
/// [`account_group`](Self::account_group) returns `None`.
pub struct PostTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    accounts: Option<Accounts<StorageFactory>>,
    account: Option<AccountId>,
    group_lookup: GroupLookup<StorageFactory>,
}

impl<StorageFactory> PostTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn with_groups(
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: Option<AccountId>,
    ) -> Self {
        Self {
            accounts: None,
            account,
            group_lookup: GroupLookup::new(account_groups, account),
        }
    }

    pub(crate) fn with_accounts(
        accounts: Accounts<StorageFactory>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: Option<AccountId>,
    ) -> Self {
        Self {
            accounts: Some(accounts),
            account,
            group_lookup: GroupLookup::new(account_groups, account),
        }
    }

    /// Creates a standalone context for testing a [`PreTradePolicy`]'s
    /// post-trade hook outside an engine.
    ///
    /// The context is backed by an empty, private account-group registry and no
    /// bound account, so [`account_group`](Self::account_group) returns `None`.
    /// Inside the engine the registry is the engine's shared one; this
    /// constructor exists so policy authors can drive a policy's hook directly
    /// in unit tests.
    ///
    /// [`PreTradePolicy`]: crate::pretrade::PreTradePolicy
    pub fn new() -> Self
    where
        StorageFactory: Default,
    {
        let builder = StorageBuilder::new(StorageFactory::default());
        let handle = AccountGroupsHandle::from_inner(StorageFactory::new_shared(
            AccountGroups::new(&builder),
        ));
        Self::with_groups(handle, None)
    }

    #[cfg(test)]
    pub(crate) fn with_account_currency(account: AccountId, currency: Asset) -> Self
    where
        StorageFactory: Default + storage::CreateStorageFor<AccountGroupId>,
    {
        let builder = StorageBuilder::new(StorageFactory::default());
        let account_groups = AccountGroupsHandle::from_inner(StorageFactory::new_shared(
            AccountGroups::new(&builder),
        ));
        let block_handle = AccountBlockHandle::from_inner(StorageFactory::new_shared(
            BlockedAccounts::new(&builder),
        ));
        let currencies = StorageFactory::new_shared(AccountCurrencies::new(&builder));
        let accounts = Accounts::new(account_groups.clone(), block_handle, currencies);
        accounts.set_currency(account, currency);
        Self::with_accounts(accounts, account_groups, Some(account))
    }

    /// Returns the group of the report's account, or `None` when the account is
    /// absent or unregistered.
    ///
    /// The lookup is performed once and cached for the lifetime of this context.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        self.group_lookup.group()
    }

    /// Returns the currency resolved for the report's account.
    ///
    /// The engine-backed context uses the same account -> group -> default
    /// cascade as [`Accounts::currency_of`]. Standalone test contexts have no
    /// account registry, so they return `None`.
    pub(crate) fn account_currency(&self) -> Option<Asset> {
        self.account
            .and_then(|account| self.accounts.as_ref()?.currency_of(account))
    }
}

impl<StorageFactory> Default for PostTradeContext<StorageFactory>
where
    StorageFactory:
        storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + Default + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<StorageFactory> crate::marketdata::AccountInfo for PostTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    fn group(&self) -> Option<AccountGroupId> {
        self.account_group()
    }
}
