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

use crate::core::sync_mode::SyncMode;
use crate::core::AccountRetirementRefusal;
use crate::marketdata::MarketDataSync;
use crate::param::{AccountId, Pnl};
use crate::pretrade::holdings::Holdings;
use crate::storage::{ConfigCell, LockingPolicyFactory};
use crate::{Mutation, Mutations, PnlState};

use super::SpotFundsPolicy;

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
    <Sync::StorageLockingPolicyFactory as LockingPolicyFactory>::Policy: 'static,
{
    pub(super) fn retire_account_state(
        &self,
        account_id: AccountId,
        mutations: &mut Mutations,
    ) -> Result<(), AccountRetirementRefusal> {
        if matches!(
            self.active_holdings_mutations.with(&account_id, |count| *count),
            Some(count) if count != 0
        ) || matches!(
            self.pnl_leases.with(&account_id, |lease| *lease),
            Some(Some(_))
        ) || matches!(
            self.pnl.with(&account_id, |entry| entry.assertion_token),
            Some(Some(_))
        ) {
            return Err(AccountRetirementRefusal::OperationInProgress);
        }

        if matches!(
            self.pnl.with(&account_id, |entry| entry.state),
            Some(state) if state != PnlState::Value(Pnl::ZERO)
        ) {
            return Err(AccountRetirementRefusal::NonZeroState);
        }

        // Retirement is rare, so scan all holdings keys without an account
        // index.
        let holdings_keys: Vec<_> = self
            .holdings
            .keys()
            .into_iter()
            .filter(|(account, _)| *account == account_id)
            .collect();
        if holdings_keys
            .iter()
            .any(|key| matches!(self.holdings.get(key), Some(holdings) if !holdings.is_retirable()))
        {
            return Err(AccountRetirementRefusal::NonZeroState);
        }

        if self
            .settings
            .with(|settings| settings.references_account(account_id))
        {
            return Err(AccountRetirementRefusal::ConfigurationReferencesAccount);
        }

        for key in holdings_keys {
            let holdings = self.holdings.clone();
            mutations.push(Mutation::new_reporting(
                move || holdings.remove_if(&key, Holdings::is_retirable),
                Default::default,
            ));
        }
        if self.pnl.with(&account_id, |_| ()).is_some() {
            let pnl = self.pnl.clone();
            mutations.push(Mutation::new_reporting(
                move || {
                    pnl.remove_if(&account_id, |entry| {
                        entry.state == PnlState::Value(Pnl::ZERO) && entry.assertion_token.is_none()
                    })
                },
                Default::default,
            ));
        }
        let pnl_leases = self.pnl_leases.clone();
        mutations.push(Mutation::new(
            move || {
                pnl_leases.remove(&account_id);
            },
            || {},
        ));
        Ok(())
    }
}
