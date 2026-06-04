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

//! Rollback registration for [`SpotFundsPolicy`].

use crate::core::sync_mode::SyncMode;
use crate::core::AccountControl;
use crate::marketdata::MarketDataSync;
use crate::param::{AccountId, PositionSize};
use crate::pretrade::holdings::Holdings;
use crate::pretrade::{AccountBlock, RejectCode};
use crate::{Mutation, Mutations};

use super::{HoldingsKey, SpotFundsPolicy};

/// Records an arithmetic overflow encountered during a rollback closure via
/// [`AccountControl`] captured from the operation context.
///
/// The block uses [`RejectCode::ArithmeticOverflow`] so subsequent pre-trade
/// requests for the account are rejected exactly like any other kill-switch
/// block. The detail string is built lazily so non-overflow paths pay nothing.
/// When `account_control` is `None` the overflow cannot be attributed to a
/// specific account and is silently dropped; in practice rollback closures are
/// only registered when an account control is available.
fn record_rollback_overflow<StorageFactory>(
    account_control: &Option<AccountControl<StorageFactory>>,
    details: impl FnOnce() -> String,
) where
    StorageFactory: crate::storage::LockingPolicyFactory
        + crate::storage::CreateStorageFor<AccountId>
        + 'static,
{
    if let Some(ctrl) = account_control {
        let block = AccountBlock::new(
            SpotFundsPolicyName::NAME,
            RejectCode::ArithmeticOverflow,
            "rollback overflow: slot left inconsistent",
            details(),
        );
        ctrl.block(block);
    }
}

/// Local alias for the policy name to avoid carrying generic parameters into a
/// free function. Mirrors `SpotFundsPolicy::NAME` without requiring a
/// type-bound caller.
struct SpotFundsPolicyName;
impl SpotFundsPolicyName {
    const NAME: &'static str = "SpotFundsPolicy";
}

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
{
    pub(super) fn register_hold_rollback(
        &self,
        mutations: &mut Mutations,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        key: HoldingsKey,
        amount: PositionSize,
    ) where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let holdings_arc = self.holdings.clone();
        mutations.push(Mutation::new(
            // Commit is intentionally a no-op: the hold was written
            // synchronously inside `perform_pre_trade_check` so that
            // any subsequent policy check in the same pipeline observes
            // the reservation. In a multi-policy setup there is no
            // guarantee that no other check runs between our check and
            // our commit, and every later check must see funds already
            // held by earlier checks - otherwise the same 100 USD could
            // be reserved twice. Rollback reverses the delta.
            || {},
            move || {
                // Use `with_mut` (not `with_mut_if_present`) because a
                // concurrent adjustment may have driven the slot to zero and
                // pruned it between hold and rollback; without re-insertion
                // the rollback would silently lose the funds that the hold
                // moved into held. Releasing the recorded amount from a
                // freshly created zero placeholder restores exactly the
                // pre-hold state when no concurrent change happened, and
                // undoes only our delta otherwise.
                let key_for_remove = key.clone();
                let account_id = key.0;
                let asset_for_diagnostic = key.1.clone();
                let became_zero = holdings_arc.with_mut(key, Holdings::zero, |slot, _| {
                    match slot.release(amount) {
                        Ok(undone) => {
                            *slot = undone;
                            undone.is_zero()
                        }
                        // Overflow during rollback is practically unreachable
                        // for real balances. The slot is left unchanged and
                        // the account is recorded on the engine's blocked-
                        // accounts sink so the failure is visible end to end
                        // rather than silently swallowed.
                        Err(_) => {
                            record_rollback_overflow(&account_control, || {
                                format!(
                                    "hold rollback overflow: account {account_id}, \
                                     asset {asset_for_diagnostic}, release {amount}, \
                                     slot {slot:?}",
                                )
                            });
                            slot.is_zero()
                        }
                    }
                });
                if became_zero {
                    holdings_arc.remove_if_zero(&key_for_remove);
                }
            },
        ));
    }

    pub(super) fn register_adjustment_rollback(
        &self,
        mutations: &mut Mutations,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        key: HoldingsKey,
        available_delta: PositionSize,
        held_delta: PositionSize,
        incoming_delta: PositionSize,
    ) where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let holdings_arc = self.holdings.clone();
        mutations.push(Mutation::new(
            // Commit is a no-op: the new value was written synchronously
            // inside `apply_account_adjustment` so that later policies and
            // checks in the same pipeline observe the adjustment. See the
            // hold-rollback comment for the underlying reason.
            || {},
            move || {
                // Apply the inverse of the forward delta to whatever the slot
                // holds right now, so concurrent changes by other threads are
                // not overwritten. `with_mut` (not `with_mut_if_present`) is
                // used here because the adjustment may have produced a zero
                // result and the main path may have pruned the entry via
                // `remove_if_zero`; without re-insertion the rollback would
                // silently lose the previous balance.
                let key_for_remove = key.clone();
                let account_id = key.0;
                let asset_for_diagnostic = key.1.clone();
                let became_zero = holdings_arc.with_mut(key, Holdings::zero, |slot, _| {
                    match slot.apply_delta_rollback(available_delta, held_delta, incoming_delta) {
                        Ok(rolled_back) => {
                            *slot = rolled_back;
                            rolled_back.is_zero()
                        }
                        // Overflow during rollback is practically unreachable
                        // for real balances. The slot is left unchanged and
                        // the account is recorded on the engine's blocked-
                        // accounts sink so the failure is visible end to end
                        // rather than silently swallowed.
                        Err(_) => {
                            record_rollback_overflow(&account_control, || {
                                format!(
                                    "adjustment rollback overflow: account {account_id}, \
                                     asset {asset_for_diagnostic}, \
                                     available_delta {available_delta}, \
                                     held_delta {held_delta}, \
                                     incoming_delta {incoming_delta}, \
                                     slot {slot:?}",
                                )
                            });
                            slot.is_zero()
                        }
                    }
                });
                if became_zero {
                    holdings_arc.remove_if_zero(&key_for_remove);
                }
            },
        ));
    }
}
