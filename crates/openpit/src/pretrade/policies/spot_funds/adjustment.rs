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

//! Account-adjustment path for [`SpotFundsPolicy`].

use crate::core::account_outcome::OutcomeAmount;
use crate::core::sync_mode::SyncMode;
use crate::core::{
    AccountControl, AccountOutcomeEntry, HasAccountAdjustmentBalance,
    HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceUpperBound,
    HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
    HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
    HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound,
    HasBalanceAsset,
};
use crate::marketdata::MarketDataSync;
use crate::param::{AccountId, PositionSize};
use crate::pretrade::holdings::{AdjustmentTarget, Holdings};
use crate::pretrade::policy::missing_required_field_account_adjustment_reject;
use crate::pretrade::{RejectScope, Rejects};
use crate::Mutations;

use super::rejects::{
    account_adjustment_bounds_exceeded_reject, adj_field, arithmetic_overflow_reject,
};
use super::views::AdjustmentRequestView;
use super::SpotFundsPolicy;

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
{
    pub(super) fn read_adjustment_request<AccountAdjustment>(
        &self,
        adjustment: &AccountAdjustment,
    ) -> Result<AdjustmentRequestView, Rejects>
    where
        AccountAdjustment: HasBalanceAsset
            + HasAccountAdjustmentBalance
            + HasAccountAdjustmentBalanceLowerBound
            + HasAccountAdjustmentBalanceUpperBound
            + HasAccountAdjustmentHeld
            + HasAccountAdjustmentHeldLowerBound
            + HasAccountAdjustmentHeldUpperBound
            + HasAccountAdjustmentIncoming
            + HasAccountAdjustmentIncomingLowerBound
            + HasAccountAdjustmentIncomingUpperBound,
    {
        let asset = adjustment
            .balance_asset()
            .map_err(|e| {
                Rejects::from(missing_required_field_account_adjustment_reject(
                    self,
                    "balance asset",
                    &e,
                ))
            })?
            .clone();
        let balance = adj_field(self, "balance", adjustment.balance())?;
        let balance_lower = adj_field(self, "balance lower bound", adjustment.balance_lower())?;
        let balance_upper = adj_field(self, "balance upper bound", adjustment.balance_upper())?;
        let held = adj_field(self, "held", adjustment.held())?;
        let held_lower = adj_field(self, "held lower bound", adjustment.held_lower())?;
        let held_upper = adj_field(self, "held upper bound", adjustment.held_upper())?;
        let incoming = adj_field(self, "incoming", adjustment.incoming())?;
        let incoming_lower = adj_field(self, "incoming lower bound", adjustment.incoming_lower())?;
        let incoming_upper = adj_field(self, "incoming upper bound", adjustment.incoming_upper())?;
        Ok(AdjustmentRequestView {
            asset,
            balance,
            balance_lower,
            balance_upper,
            held,
            held_lower,
            held_upper,
            incoming,
            incoming_lower,
            incoming_upper,
        })
    }

    pub(super) fn apply_account_adjustment_impl<AccountAdjustment>(
        &self,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<Vec<AccountOutcomeEntry>, Rejects>
    where
        AccountAdjustment: HasBalanceAsset
            + HasAccountAdjustmentBalance
            + HasAccountAdjustmentBalanceLowerBound
            + HasAccountAdjustmentBalanceUpperBound
            + HasAccountAdjustmentHeld
            + HasAccountAdjustmentHeldLowerBound
            + HasAccountAdjustmentHeldUpperBound
            + HasAccountAdjustmentIncoming
            + HasAccountAdjustmentIncomingLowerBound
            + HasAccountAdjustmentIncomingUpperBound,
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let request = self.read_adjustment_request(adjustment)?;
        if request.balance.is_none() && request.held.is_none() && request.incoming.is_none() {
            return Ok(Vec::new());
        }

        let key = (account_id, request.asset.clone());
        let (new, available_delta, held_delta, incoming_delta) = self.holdings.with_mut_or_insert(
            key.clone(),
            Holdings::zero,
            |slot, _is_new| -> Result<(Holdings, PositionSize, PositionSize, PositionSize), Rejects> {
                let current = *slot;
                let mut new = current;

                if let Some(amount) = request.balance {
                    new = new
                        .apply_adjustment(AdjustmentTarget::Available, amount)
                        .map_err(|_| {
                            Rejects::from(arithmetic_overflow_reject(
                                Self::NAME,
                                RejectScope::Account,
                                format!(
                                    "account adjustment overflow: account {account_id}, \
                                     asset {asset}, field balance, current {val}, applied {amount}",
                                    asset = request.asset,
                                    val = new.available(),
                                ),
                            ))
                        })?;
                    if !new.available_within_bounds(request.balance_lower, request.balance_upper) {
                        return Err(Rejects::from(account_adjustment_bounds_exceeded_reject(
                            Self::NAME,
                            account_id,
                            &request.asset,
                            "balance",
                            new.available(),
                            request.balance_lower,
                            request.balance_upper,
                        )));
                    }
                }

                if let Some(amount) = request.held {
                    new = new
                        .apply_adjustment(AdjustmentTarget::Held, amount)
                        .map_err(|_| {
                            Rejects::from(arithmetic_overflow_reject(
                                Self::NAME,
                                RejectScope::Account,
                                format!(
                                    "account adjustment overflow: account {account_id}, \
                                     asset {asset}, field held, current {val}, applied {amount}",
                                    asset = request.asset,
                                    val = new.held(),
                                ),
                            ))
                        })?;
                    if !new.held_within_bounds(request.held_lower, request.held_upper) {
                        return Err(Rejects::from(account_adjustment_bounds_exceeded_reject(
                            Self::NAME,
                            account_id,
                            &request.asset,
                            "held",
                            new.held(),
                            request.held_lower,
                            request.held_upper,
                        )));
                    }
                }

                if let Some(amount) = request.incoming {
                    new = new
                        .apply_adjustment(AdjustmentTarget::Incoming, amount)
                        .map_err(|_| {
                            Rejects::from(arithmetic_overflow_reject(
                                Self::NAME,
                                RejectScope::Account,
                                format!(
                                    "account adjustment overflow: account {account_id}, \
                                     asset {asset}, field incoming, current {val}, applied {amount}",
                                    asset = request.asset,
                                    val = new.incoming(),
                                ),
                            ))
                        })?;
                    if !new.incoming_within_bounds(request.incoming_lower, request.incoming_upper) {
                        return Err(Rejects::from(account_adjustment_bounds_exceeded_reject(
                            Self::NAME,
                            account_id,
                            &request.asset,
                            "incoming",
                            new.incoming(),
                            request.incoming_lower,
                            request.incoming_upper,
                        )));
                    }
                }

                // Compute per-field deltas with checked arithmetic before writing
                // the slot. This serves two purposes:
                //   1. Outcome reporting: delta = actual applied change per field.
                //   2. Rollback: the inverse delta is later applied to whatever
                //      the slot holds at rollback time (safe for FullSync).
                // For Delta adjustments overflow here is practically impossible
                // (new was derived from current via checked_add); for Absolute
                // adjustments with extreme opposing values it can fail, and we
                // reject before writing so no partial state escapes.
                let available_delta = new
                    .available()
                    .checked_sub(current.available())
                    .map_err(|_| {
                        Rejects::from(arithmetic_overflow_reject(
                            Self::NAME,
                            RejectScope::Account,
                            format!(
                                "account adjustment delta overflow: account {account_id}, \
                                 asset {asset}, field balance",
                                asset = request.asset,
                            ),
                        ))
                    })?;
                let held_delta = new
                    .held()
                    .checked_sub(current.held())
                    .map_err(|_| {
                        Rejects::from(arithmetic_overflow_reject(
                            Self::NAME,
                            RejectScope::Account,
                            format!(
                                "account adjustment delta overflow: account {account_id}, \
                                 asset {asset}, field held",
                                asset = request.asset,
                            ),
                        ))
                    })?;
                let incoming_delta = new
                    .incoming()
                    .checked_sub(current.incoming())
                    .map_err(|_| {
                        Rejects::from(arithmetic_overflow_reject(
                            Self::NAME,
                            RejectScope::Account,
                            format!(
                                "account adjustment delta overflow: account {account_id}, \
                                 asset {asset}, field incoming",
                                asset = request.asset,
                            ),
                        ))
                    })?;

                *slot = new; // synchronous write, see register_*_rollback comments
                Ok((new, available_delta, held_delta, incoming_delta))
            },
        )?;
        if new.is_zero() {
            self.holdings.remove_if_zero(&key);
        }
        self.register_adjustment_rollback(
            mutations,
            account_control,
            key,
            available_delta,
            held_delta,
            incoming_delta,
        );

        let balance_outcome = request.balance.map(|_| OutcomeAmount {
            delta: available_delta,
            absolute: new.available(),
        });
        let held_outcome = request.held.map(|_| OutcomeAmount {
            delta: held_delta,
            absolute: new.held(),
        });
        let incoming_outcome = request.incoming.map(|_| OutcomeAmount {
            delta: incoming_delta,
            absolute: new.incoming(),
        });

        Ok(vec![AccountOutcomeEntry {
            asset: request.asset,
            balance: balance_outcome,
            held: held_outcome,
            incoming: incoming_outcome,
        }])
    }
}
