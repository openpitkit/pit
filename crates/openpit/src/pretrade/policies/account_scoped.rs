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

use std::collections::HashMap;

use super::pnl_bounds;
use crate::marketdata::AccountInfo;
use crate::param::{AccountGroupId, AccountId};

#[derive(Clone, Debug)]
pub(super) struct AccountScoped<Value> {
    global: Value,
    groups: HashMap<AccountGroupId, Value>,
    accounts: HashMap<AccountId, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AccountScope {
    Account(AccountId),
    Group(AccountGroupId),
    Global,
}

impl<Value> AccountScoped<Value> {
    pub(super) fn new(global: Value) -> Self {
        Self {
            global,
            groups: HashMap::new(),
            accounts: HashMap::new(),
        }
    }

    pub(super) fn set_global(&mut self, value: Value) {
        self.global = value;
    }

    pub(super) fn global(&self) -> &Value {
        &self.global
    }

    pub(super) fn set_group(&mut self, group_id: AccountGroupId, value: Option<Value>) {
        pnl_bounds::set_or_clear(&mut self.groups, group_id, value);
    }

    pub(super) fn set_account(&mut self, account_id: AccountId, value: Option<Value>) {
        pnl_bounds::set_or_clear(&mut self.accounts, account_id, value);
    }

    pub(super) fn references_account(&self, account_id: AccountId) -> bool {
        self.accounts.contains_key(&account_id)
    }

    pub(super) fn set_groups(&mut self, groups: HashMap<AccountGroupId, Value>) {
        self.groups = groups;
    }

    pub(super) fn set_accounts(&mut self, accounts: HashMap<AccountId, Value>) {
        self.accounts = accounts;
    }

    pub(super) fn has_overrides(&self) -> bool {
        !self.groups.is_empty() || !self.accounts.is_empty()
    }

    pub(super) fn all_levels<'settings>(
        &'settings self,
        account_id: AccountId,
        account_info: &'settings impl AccountInfo,
    ) -> impl Iterator<Item = (AccountScope, &'settings Value)> {
        self.overrides(account_id, account_info)
            .chain(std::iter::once((AccountScope::Global, self.global())))
    }

    fn overrides<'settings, 'info, Info: AccountInfo>(
        &'settings self,
        account_id: AccountId,
        account_info: &'info Info,
    ) -> impl Iterator<Item = (AccountScope, &'settings Value)> + use<'settings, 'info, Value, Info>
    {
        self.accounts
            .get(&account_id)
            .map(|value| (AccountScope::Account(account_id), value))
            .into_iter()
            .chain(
                std::iter::once_with(move || {
                    let group_id = account_info.group()?;
                    self.groups
                        .get(&group_id)
                        .map(|value| (AccountScope::Group(group_id), value))
                })
                .flatten(),
            )
    }

    pub(super) fn most_specific(
        &self,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> &Value {
        match self.overrides(account_id, account_info).next() {
            Some((_, value)) => value,
            None => self.global(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::pretrade::SpotFundsLimitMode::{Enforce, ReviewWithHold, TrackOnly};

    struct CountedAccountInfo {
        group: AccountGroupId,
        calls: Cell<usize>,
    }

    impl AccountInfo for CountedAccountInfo {
        fn group(&self) -> Option<AccountGroupId> {
            self.calls.set(self.calls.get() + 1);
            Some(self.group)
        }
    }

    #[test]
    fn all_levels_visits_account_group_and_global_in_order() {
        let account = AccountId::from_u64(1001);
        let group = AccountGroupId::from_u32(7).expect("valid group");
        let mut modes = AccountScoped::new(Enforce);
        modes.set_group(group, Some(TrackOnly));
        modes.set_account(account, Some(ReviewWithHold));
        assert_eq!(
            modes.all_levels(account, &Some(group)).collect::<Vec<_>>(),
            vec![
                (AccountScope::Account(account), &ReviewWithHold),
                (AccountScope::Group(group), &TrackOnly),
                (AccountScope::Global, &Enforce),
            ]
        );
    }

    #[test]
    fn all_levels_resolves_group_only_after_consuming_account() {
        let account = AccountId::from_u64(1001);
        let info = CountedAccountInfo {
            group: AccountGroupId::from_u32(7).expect("valid group"),
            calls: Cell::new(0),
        };
        let mut modes = AccountScoped::new(Enforce);
        modes.set_account(account, Some(ReviewWithHold));
        modes.set_group(info.group, Some(TrackOnly));
        let mut levels = modes.all_levels(account, &info);
        assert_eq!(
            info.calls.get(),
            0,
            "constructing the iterator must be lazy"
        );
        assert_eq!(
            levels.next(),
            Some((AccountScope::Account(account), &ReviewWithHold))
        );
        assert_eq!(
            info.calls.get(),
            0,
            "an account match must not resolve its group"
        );
        assert_eq!(
            levels.next(),
            Some((AccountScope::Group(info.group), &TrackOnly))
        );
        assert_eq!(info.calls.get(), 1);
        assert_eq!(levels.next(), Some((AccountScope::Global, &Enforce)));
        assert_eq!(levels.next(), None);
        assert_eq!(info.calls.get(), 1);
    }

    #[test]
    fn all_levels_without_a_group_visits_only_account_and_global() {
        let account = AccountId::from_u64(1001);
        let mut modes = AccountScoped::new(Enforce);
        assert_eq!(
            modes.all_levels(account, &None).collect::<Vec<_>>(),
            vec![(AccountScope::Global, &Enforce)]
        );
        modes.set_account(account, Some(TrackOnly));
        assert_eq!(
            modes.all_levels(account, &None).collect::<Vec<_>>(),
            vec![
                (AccountScope::Account(account), &TrackOnly),
                (AccountScope::Global, &Enforce),
            ]
        );
    }

    #[test]
    fn most_specific_mode_wins_for_the_accounts_group() {
        let account = AccountId::from_u64(7);
        let group = AccountGroupId::from_u32(3).expect("valid group");
        let other_group = AccountGroupId::from_u32(9).expect("valid other group");
        let mut modes = AccountScoped::new(Enforce);
        assert_eq!(*modes.global(), Enforce);
        assert_eq!(*modes.most_specific(account, &Some(group)), Enforce);

        modes.set_global(TrackOnly);
        modes.set_group(group, Some(ReviewWithHold));
        assert_eq!(*modes.global(), TrackOnly);
        assert_eq!(*modes.most_specific(account, &None), TrackOnly);
        assert_eq!(*modes.most_specific(account, &Some(other_group)), TrackOnly);
        assert_eq!(*modes.most_specific(account, &Some(group)), ReviewWithHold);

        modes.set_account(account, Some(Enforce));
        assert_eq!(*modes.most_specific(account, &Some(group)), Enforce);
        assert_eq!(*modes.most_specific(account, &None), Enforce);
        assert_eq!(*modes.most_specific(account, &Some(other_group)), Enforce);
        modes.set_account(account, Some(TrackOnly));
        assert_eq!(*modes.most_specific(account, &Some(group)), TrackOnly);
    }

    #[test]
    fn clearing_overrides_restores_the_next_applicable_mode() {
        let account = AccountId::from_u64(7);
        let group = AccountGroupId::from_u32(3).expect("valid group");
        let mut modes = AccountScoped::new(Enforce);
        modes.set_group(group, Some(TrackOnly));
        modes.set_account(account, Some(ReviewWithHold));

        modes.set_account(account, None);
        assert_eq!(*modes.most_specific(account, &Some(group)), TrackOnly);
        modes.set_group(group, Some(ReviewWithHold));
        assert_eq!(*modes.most_specific(account, &Some(group)), ReviewWithHold);
        modes.set_group(group, None);
        assert_eq!(*modes.most_specific(account, &Some(group)), Enforce);
        modes.set_account(account, None);
        modes.set_group(group, None);
        assert_eq!(*modes.most_specific(account, &Some(group)), Enforce);
    }

    #[test]
    fn only_an_account_override_keeps_an_account_referenced() {
        let account = AccountId::from_u64(7);
        let other_account = AccountId::from_u64(8);
        let group = AccountGroupId::from_u32(3).expect("valid group");
        let mut modes = AccountScoped::new(Enforce);
        assert!(!modes.references_account(account));
        modes.set_group(group, Some(TrackOnly));
        assert!(!modes.references_account(account));
        modes.set_account(account, Some(ReviewWithHold));
        assert!(modes.references_account(account));
        assert!(!modes.references_account(other_account));
        modes.set_account(account, None);
        assert!(!modes.references_account(account));
    }

    #[test]
    fn an_account_override_does_not_resolve_the_group() {
        let account = AccountId::from_u64(7);
        let account_info = CountedAccountInfo {
            group: AccountGroupId::from_u32(3).expect("valid group"),
            calls: Cell::new(0),
        };
        let mut modes = AccountScoped::new(Enforce);
        modes.set_group(account_info.group, Some(TrackOnly));
        modes.set_account(account, Some(ReviewWithHold));
        assert_eq!(*modes.most_specific(account, &account_info), ReviewWithHold);
        assert_eq!(account_info.calls.get(), 0);

        modes.set_account(account, None);
        assert_eq!(*modes.most_specific(account, &account_info), TrackOnly);
        assert_eq!(account_info.calls.get(), 1);
    }
}
