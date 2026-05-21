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

//! Kill-switch-triggered account blocking for [`Engine`](crate::Engine).
//!
//! When a policy's `apply_execution_report` returns `true` (kill switch), the
//! engine records the affected account here. All subsequent pre-trade requests
//! for that account are rejected immediately, before any policy is invoked.

use crate::core::HasAccountId;
use crate::param::AccountId;
use crate::pretrade::{AccountBlock, Reject, RejectCode, RejectScope, Rejects};
use crate::storage::{self, IndexFlag, Storage, StorageBuilder};

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

fn new_unverifiable_blocked_rejects(scope: RejectScope) -> Rejects {
    Rejects::new(vec![Reject::new(
        "Engine",
        scope,
        RejectCode::MissingRequiredField,
        "account could not be verified as account ID is missing",
        "unable to check account for blocking".to_owned(),
    )])
}

// ─── BlockedAccounts ─────────────────────────────────────────────────────────

/// Per-engine storage for kill-switch-blocked accounts.
///
/// Uses:
/// - `any_flag`: write-once flag for the level-1 fast path (set when the first
///   account is blocked or a global block fires; never reset).
/// - `all_flag`: write-once flag set when `block_all()` is called.
/// - `accounts`: per-account storage mapping each blocked `AccountId` to the
///   first `AccountBlock` that triggered the kill-switch for that account.
///   Synchronization is delegated entirely to the `Storage` infrastructure
///   matching the engine's synchronization mode.
pub(crate) struct BlockedAccounts<StorageFactory>
where
    StorageFactory: storage::LockingPolicyFactory + storage::CreateStorageFor<AccountId>,
{
    any_flag: <StorageFactory as storage::LockingPolicyFactory>::IndexFlag,
    all_flag: <StorageFactory as storage::LockingPolicyFactory>::IndexFlag,
    accounts: Storage<AccountId, AccountBlock, StorageFactory::Policy>,
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
            any_flag:
                <StorageLockingPolicyFactory as storage::LockingPolicyFactory>::IndexFlag::new(
                    false,
                ),
            all_flag:
                <StorageLockingPolicyFactory as storage::LockingPolicyFactory>::IndexFlag::new(
                    false,
                ),
            accounts: builder.create(),
        }
    }

    /// Checks whether a pre-trade request should be rejected.
    ///
    /// Returns `None` when the order may proceed. Returns `Some(Rejects)` when:
    /// - the order's account is individually blocked;
    /// - a global block is active;
    /// - something is blocked but the account cannot be identified.
    ///
    /// Callers should return the `Rejects` immediately without running any
    /// policies.
    pub(crate) fn check<Order: HasAccountId>(
        &self,
        order: &Order,
        operation_scope: RejectScope,
    ) -> Option<Rejects> {
        if !self.any_flag.load() {
            debug_assert!(!self.all_flag.load());
            return None;
        }
        match order.account_id() {
            Err(_) => Some(new_unverifiable_blocked_rejects(operation_scope)),
            Ok(id) => {
                if let Some(rejects) = self
                    .accounts
                    .with(&id, |b| Rejects::new(vec![Reject::from(b.clone())]))
                {
                    return Some(rejects);
                }
                if self.all_flag.load() {
                    return Some(new_account_blocked_rejects());
                }
                None
            }
        }
    }

    /// Records a kill-switch event from an execution report.
    ///
    /// Extracts the account from `report` and blocks it, storing `cause` as
    /// the reason returned for all future pre-trade requests on that account.
    /// If the report carries no account identifier, activates a global block
    /// instead. The first cause recorded for an account wins; subsequent calls
    /// for the same account are no-ops.
    pub(crate) fn record<Report: HasAccountId>(&self, report: &Report, cause: AccountBlock) {
        match report.account_id() {
            Ok(id) => self.block_account(id, cause),
            Err(_) => self.block_all(),
        }
    }

    fn block_account(&self, id: AccountId, cause: AccountBlock) {
        self.accounts.with_mut(id, || cause, |_, _| ());
        self.any_flag.store(true);
    }

    fn block_all(&self) {
        self.all_flag.store(true);
        self.any_flag.store(true);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::HasAccountId;
    use crate::param::AccountId;
    use crate::pretrade::RejectCode;
    use crate::storage::{NoLocking, StorageBuilder};
    use crate::RequestFieldAccessError;

    fn new_set() -> BlockedAccounts<NoLocking> {
        BlockedAccounts::new(&StorageBuilder::new(NoLocking))
    }

    fn cause(policy: &str, code: RejectCode) -> AccountBlock {
        AccountBlock::new(policy, code, "test block", "details")
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

    #[test]
    fn initially_nothing_blocked() {
        let set = new_set();
        assert!(set
            .check(&AccountOrder(account(1)), RejectScope::Order)
            .is_none());
        assert!(set.check(&NoAccountOrder, RejectScope::Order).is_none());
    }

    #[test]
    fn record_account_blocks_that_account() {
        let set = new_set();
        set.record(
            &AccountOrder(account(1)),
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&AccountOrder(account(1)), RejectScope::Order)
            .is_some());
    }

    #[test]
    fn record_account_does_not_block_other_accounts() {
        let set = new_set();
        set.record(
            &AccountOrder(account(1)),
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&AccountOrder(account(2)), RejectScope::Order)
            .is_none());
    }

    #[test]
    fn record_no_account_blocks_every_account() {
        let set = new_set();
        set.record(
            &NoAccountOrder,
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set
            .check(&AccountOrder(account(1)), RejectScope::Order)
            .is_some());
        assert!(set
            .check(&AccountOrder(account(99)), RejectScope::Order)
            .is_some());
    }

    #[test]
    fn record_no_account_blocks_unidentifiable_orders() {
        let set = new_set();
        set.record(
            &NoAccountOrder,
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set.check(&NoAccountOrder, RejectScope::Order).is_some());
    }

    #[test]
    fn record_account_blocks_unidentifiable_orders() {
        let set = new_set();
        set.record(
            &AccountOrder(account(1)),
            cause("Policy", RejectCode::PnlKillSwitchTriggered),
        );
        assert!(set.check(&NoAccountOrder, RejectScope::Order).is_some());
    }

    #[test]
    fn initially_unidentifiable_order_is_allowed() {
        let set = new_set();
        assert!(set.check(&NoAccountOrder, RejectScope::Order).is_none());
    }

    #[test]
    fn check_returns_cause_for_blocked_account() {
        let set = new_set();
        set.record(
            &AccountOrder(account(1)),
            cause("KillSwitch", RejectCode::PnlKillSwitchTriggered),
        );
        let rejects = set
            .check(&AccountOrder(account(1)), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "KillSwitch");
        assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(rejects[0].scope, RejectScope::Account);
    }

    #[test]
    fn first_cause_wins_on_repeated_block() {
        let set = new_set();
        set.record(
            &AccountOrder(account(1)),
            cause("First", RejectCode::PnlKillSwitchTriggered),
        );
        set.record(
            &AccountOrder(account(1)),
            cause("Second", RejectCode::Other),
        );
        let rejects = set
            .check(&AccountOrder(account(1)), RejectScope::Order)
            .expect("blocked account must return rejects");
        assert_eq!(rejects[0].policy, "First");
    }
}
