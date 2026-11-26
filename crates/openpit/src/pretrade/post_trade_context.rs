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

#[cfg(test)]
use crate::core::{AccountBlockHandle, AccountCurrencies, BlockedAccounts};
use crate::core::{
    AccountGroups, AccountGroupsHandle, Accounts, OperationSnapshot, RequestFieldAccessError,
};
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
/// The report account is read exactly once at engine entry, including its
/// error. Group and currency are resolved lazily once for this operation.
pub struct PostTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    group_lookup: OperationSnapshot<StorageFactory>,
}

impl<StorageFactory> PostTradeContext<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + 'static,
{
    pub(crate) fn with_groups(
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: Result<AccountId, RequestFieldAccessError>,
    ) -> Self {
        Self {
            group_lookup: OperationSnapshot::new(account_groups, account, None),
        }
    }

    pub(crate) fn with_accounts(
        accounts: Accounts<StorageFactory>,
        account_groups: AccountGroupsHandle<StorageFactory>,
        account: Result<AccountId, RequestFieldAccessError>,
    ) -> Self {
        Self {
            group_lookup: OperationSnapshot::new(account_groups, account, Some(accounts)),
        }
    }

    /// Creates a standalone context for testing a [`PreTradePolicy`]'s
    /// post-trade hook outside an engine.
    ///
    /// The context is backed by an empty, private account-group registry and no
    /// currency registry, so [`account_group`](Self::account_group) returns
    /// `None`. The report's routing account, including an accessor error, is
    /// captured exactly once here. Pass this same report to the policy hook.
    /// Inside the engine the registry is the engine's shared one; this
    /// constructor exists so policy authors can drive a policy's hook directly
    /// in unit tests.
    ///
    /// [`PreTradePolicy`]: crate::pretrade::PreTradePolicy
    pub fn new<Report: crate::HasAccountId>(report: &Report) -> Self
    where
        StorageFactory: Default,
    {
        let builder = StorageBuilder::new(StorageFactory::default());
        let handle = AccountGroupsHandle::from_inner(StorageFactory::new_shared(
            AccountGroups::new(&builder),
        ));
        Self::with_groups(handle, report.account_id())
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
        let config_registry = StorageFactory::new_shared(crate::core::ConfigRegistry::empty());
        let accounts = Accounts::new(
            account_groups.clone(),
            block_handle,
            currencies,
            config_registry,
        );
        accounts.set_currency(account, currency);
        Self::with_accounts(accounts, account_groups, Ok(account))
    }

    /// Returns the group of the report's account, or `None` when the account is
    /// absent or unregistered.
    ///
    /// Resolved on first use and retained for this operation.
    pub fn account_group(&self) -> Option<AccountGroupId> {
        self.group_lookup.account_group()
    }

    /// Returns the operation account read once from the report at engine entry.
    ///
    /// # Errors
    /// Returns the original accessor error when the report account is
    /// unreadable.
    pub fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.group_lookup.account_id()
    }

    /// Returns the effective currency for the report's account.
    ///
    /// Resolved once from the operation's snapshotted group.
    pub(crate) fn account_currency(&self) -> Option<Asset> {
        self.group_lookup.account_currency()
    }

    /// Returns an owned read-only reader sharing this operation's first-use
    /// snapshot.
    pub fn operation_snapshot(&self) -> OperationSnapshot<StorageFactory> {
        self.group_lookup.clone()
    }
}

/// An unbound context whose account accessor returns an explicit error.
/// Use [`PostTradeContext::new`] with the report for report-aware policy hooks.
impl<StorageFactory> Default for PostTradeContext<StorageFactory>
where
    StorageFactory:
        storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId> + Default + 'static,
{
    fn default() -> Self {
        let builder = StorageBuilder::new(StorageFactory::default());
        let handle = AccountGroupsHandle::from_inner(StorageFactory::new_shared(
            AccountGroups::new(&builder),
        ));
        Self::with_groups(handle, Err(RequestFieldAccessError::new("account_id")))
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
