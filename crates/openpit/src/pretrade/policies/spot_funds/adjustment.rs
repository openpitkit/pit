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

//! Account-adjustment path for [`SpotFundsPolicy`].

use crate::core::account_outcome::{OutcomeAmount, PnlOutcomeAmount};
use crate::core::sync_mode::SyncMode;
use crate::core::{
    AccountControl, AccountOutcomeEntry, HasAccountAdjustmentBalance,
    HasAccountAdjustmentBalanceAverageEntryPrice, HasAccountAdjustmentBalanceLowerBound,
    HasAccountAdjustmentBalanceUpperBound, HasAccountAdjustmentHeld,
    HasAccountAdjustmentHeldLowerBound, HasAccountAdjustmentHeldUpperBound,
    HasAccountAdjustmentIncoming, HasAccountAdjustmentIncomingLowerBound,
    HasAccountAdjustmentIncomingUpperBound, HasAccountAdjustmentPnlOperation, HasBalanceAsset,
};
use crate::marketdata::MarketDataSync;
use crate::param::{AccountGroupId, AccountId};
use crate::pretrade::holdings::{AdjustmentTarget, Holdings, PositionPnlState};
use crate::pretrade::policy::missing_required_field_account_adjustment_reject;
use crate::pretrade::{PolicyAccountAdjustmentResult, RejectScope, Rejects};
use crate::{Mutations, PnlState};

use super::rejects::{
    account_adjustment_bounds_exceeded_reject, account_pnl_block_for_state, adj_field,
    arithmetic_overflow_reject,
};
use super::rollback::{AccountPnlAssertionRollback, AdjustmentRollback, AvgRestore, PnlRestore};
use super::views::AdjustmentRequestView;
use super::SpotFundsPolicy;

/// Payload computed while mutating a holdings slot during an account
/// adjustment: the new holdings plus the named rollback payload.
///
/// Quantity deltas feed the concurrency-safe inverse-delta rollback. Absolute
/// fields carry both their prior and asserted values so rollback can avoid
/// overwriting a parallel change.
struct AdjustmentSlotUpdate {
    new: Holdings,
    rollback: AdjustmentRollback,
}

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: crate::storage::LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
{
    pub(super) fn read_adjustment_request<AccountAdjustment>(
        &self,
        adjustment: &AccountAdjustment,
    ) -> Result<AdjustmentRequestView, Rejects>
    where
        AccountAdjustment: HasBalanceAsset
            + HasAccountAdjustmentBalance
            + HasAccountAdjustmentBalanceAverageEntryPrice
            + HasAccountAdjustmentBalanceLowerBound
            + HasAccountAdjustmentBalanceUpperBound
            + HasAccountAdjustmentHeld
            + HasAccountAdjustmentHeldLowerBound
            + HasAccountAdjustmentHeldUpperBound
            + HasAccountAdjustmentIncoming
            + HasAccountAdjustmentIncomingLowerBound
            + HasAccountAdjustmentIncomingUpperBound
            + HasAccountAdjustmentPnlOperation,
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
        let balance_average_entry_price = adj_field(
            self,
            "balance average entry price",
            adjustment.balance_average_entry_price(),
        )?;
        let balance_realized_pnl = adj_field(
            self,
            "balance realized pnl",
            adjustment.balance_realized_pnl(),
        )?;
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
            balance_average_entry_price,
            balance_realized_pnl,
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
        account_group_id: Option<AccountGroupId>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<PolicyAccountAdjustmentResult, Rejects>
    where
        AccountAdjustment: HasBalanceAsset
            + HasAccountAdjustmentBalance
            + HasAccountAdjustmentBalanceAverageEntryPrice
            + HasAccountAdjustmentBalanceLowerBound
            + HasAccountAdjustmentBalanceUpperBound
            + HasAccountAdjustmentHeld
            + HasAccountAdjustmentHeldLowerBound
            + HasAccountAdjustmentHeldUpperBound
            + HasAccountAdjustmentIncoming
            + HasAccountAdjustmentIncomingLowerBound
            + HasAccountAdjustmentIncomingUpperBound
            + HasAccountAdjustmentPnlOperation,
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let account_pnl = adj_field(
            self,
            "account pnl operation",
            adjustment.account_adjustment_pnl_operation(),
        )?;
        if let Some(state) = account_pnl {
            return self.apply_account_pnl_adjustment(
                account_control,
                account_group_id,
                account_id,
                state,
                mutations,
            );
        }

        let request = self.read_adjustment_request(adjustment)?;
        if request.balance.is_none()
            && request.balance_average_entry_price.is_none()
            && request.balance_realized_pnl.is_none()
            && request.held.is_none()
            && request.incoming.is_none()
        {
            return Ok(PolicyAccountAdjustmentResult::default());
        }

        let key = (account_id, request.asset.clone());
        let update =
            self.holdings.with_mut_or_insert(
            key.clone(),
            Holdings::zero,
            |slot, _is_new| -> Result<AdjustmentSlotUpdate, Rejects> {
                let current = *slot;
                let prior_avg = current.avg_entry_price();
                let prior_realized = current.realized_pnl_outcome();
                let mut new = current;

                if let Some(amount) = request.balance {
                    new = new
                        .apply_adjustment(AdjustmentTarget::Available, amount)
                        .map_err(|_| {
                            Rejects::from(arithmetic_overflow_reject(
                                Self::NAME,
                                RejectScope::Account,
                                format!(
                                    "account adjustment overflow: asset {asset}, field balance, current {val}, applied {amount}",
                                    asset = request.asset,
                                    val = new.available(),
                                ),
                            ))
                        })?;
                    if !new.available_within_bounds(request.balance_lower, request.balance_upper) {
                        return Err(Rejects::from(account_adjustment_bounds_exceeded_reject(
                            Self::NAME,
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
                                    "account adjustment overflow: asset {asset}, field held, current {val}, applied {amount}",
                                    asset = request.asset,
                                    val = new.held(),
                                ),
                            ))
                        })?;
                    if !new.held_within_bounds(request.held_lower, request.held_upper) {
                        return Err(Rejects::from(account_adjustment_bounds_exceeded_reject(
                            Self::NAME,
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
                                    "account adjustment overflow: asset {asset}, field incoming, current {val}, applied {amount}",
                                    asset = request.asset,
                                    val = new.incoming(),
                                ),
                            ))
                        })?;
                    if !new.incoming_within_bounds(request.incoming_lower, request.incoming_upper) {
                        return Err(Rejects::from(account_adjustment_bounds_exceeded_reject(
                            Self::NAME,
                            &request.asset,
                            "incoming",
                            new.incoming(),
                            request.incoming_lower,
                            request.incoming_upper,
                        )));
                    }
                }

                if let Some(average_entry_price) = request.balance_average_entry_price {
                    new = new.with_avg_entry_price(Some(average_entry_price));
                }

                if let Some(state) = request.balance_realized_pnl {
                    new = match state {
                        PnlState::Value(value) => new.with_realized_pnl(value),
                        PnlState::Halted(reason) => new.halt_realized_pnl(reason).holdings(),
                    };
                }

                let avg_may_change = request.balance_average_entry_price.is_some()
                    || ((request.balance.is_some() || request.held.is_some())
                        && current.avg_entry_price().is_some());
                let clear_avg_on_flat = if avg_may_change {
                    let net_owned = new.available().checked_add(new.held()).map_err(|_| {
                        Rejects::from(arithmetic_overflow_reject(
                            Self::NAME,
                            RejectScope::Account,
                            format!(
                                "account adjustment net-position overflow: asset {asset}",
                                asset = request.asset,
                            ),
                        ))
                    })?;
                    net_owned.is_zero()
                } else {
                    false
                };
                if clear_avg_on_flat {
                    new = new.with_avg_entry_price(None);
                }
                let restore_avg_on_rollback = request.balance_average_entry_price.is_some()
                    || (clear_avg_on_flat && current.avg_entry_price().is_some());

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
                                "account adjustment delta overflow: asset {asset}, field balance",
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
                                "account adjustment delta overflow: asset {asset}, field held",
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
                                "account adjustment delta overflow: asset {asset}, field incoming",
                                asset = request.asset,
                            ),
                        ))
                    })?;
                *slot = new; // synchronous write, see register_*_rollback comments
                Ok(AdjustmentSlotUpdate {
                    new,
                    rollback: AdjustmentRollback {
                        available_delta,
                        held_delta,
                        incoming_delta,
                        asserted: new,
                        prior_avg: restore_avg_on_rollback.then_some(AvgRestore {
                            previous: prior_avg,
                        }),
                        prior_realized: request.balance_realized_pnl.map(|_| PnlRestore {
                            previous: prior_realized,
                        }),
                    },
                })
            },
        )?;
        let new = update.new;
        let rollback = update.rollback;
        if new.is_zero() {
            self.holdings.remove_if_zero(&key);
        }
        self.register_adjustment_rollback(mutations, account_control, key, rollback);

        let balance_outcome = request.balance.map(|_| OutcomeAmount {
            delta: rollback.available_delta,
            absolute: new.available(),
        });
        let held_outcome = request.held.map(|_| OutcomeAmount {
            delta: rollback.held_delta,
            absolute: new.held(),
        });
        let incoming_outcome = request.incoming.map(|_| OutcomeAmount {
            delta: rollback.incoming_delta,
            absolute: new.incoming(),
        });
        let realized_pnl = match request.balance_realized_pnl {
            Some(PnlState::Value(absolute)) => {
                let delta = match rollback
                    .prior_realized
                    .as_ref()
                    .and_then(|restore| restore.previous.as_ref())
                    .and_then(|outcome| match outcome {
                        PositionPnlState::Pnl(value) => Some(*value),
                        PositionPnlState::Halted(_) => None,
                    }) {
                    Some(previous) => absolute.checked_sub(previous).map_err(|_| {
                        Rejects::from(arithmetic_overflow_reject(
                            Self::NAME,
                            RejectScope::Account,
                            format!(
                                "account adjustment delta overflow: asset {}, field realized pnl",
                                request.asset
                            ),
                        ))
                    })?,
                    None => absolute,
                };
                Some(Ok(PnlOutcomeAmount { delta, absolute }))
            }
            Some(PnlState::Halted(reason)) => Some(Err(reason)),
            None => None,
        };
        let average_entry_price = (request.balance.is_some()
            || request.balance_average_entry_price.is_some()
            || request.balance_realized_pnl.is_some())
        .then_some(new.avg_entry_price())
        .flatten();
        Ok(PolicyAccountAdjustmentResult {
            account_adjustments: vec![AccountOutcomeEntry {
                asset: request.asset,
                balance: balance_outcome,
                held: held_outcome,
                incoming: incoming_outcome,
                realized_pnl,
                average_entry_price,
            }],
            account_blocks: Vec::new(),
        })
    }

    fn apply_account_pnl_adjustment(
        &self,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        account_group_id: Option<AccountGroupId>,
        account_id: AccountId,
        state: PnlState,
        mutations: &mut Mutations,
    ) -> Result<PolicyAccountAdjustmentResult, Rejects>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let owner_id = mutations.owner_id();
        let barrier = self.pnl_barrier_for(account_id, account_group_id);
        let (previous, token, lease) =
            self.acquire_account_pnl_assertion(account_id, owner_id, state);
        self.register_account_pnl_adjustment_rollback(
            mutations,
            AccountPnlAssertionRollback {
                account_control,
                account_id,
                previous,
                asserted: state,
                token,
                barrier: barrier.clone(),
                lease,
            },
        );
        // Untagged by construction: the engine discards these blocks when the
        // batch rejects and only records them once every mutation is committed,
        // so by the time the cause exists its assertion is over and no rollback
        // is left that could invalidate a provenance tag.
        let account_blocks = barrier
            .as_ref()
            .and_then(|barrier| account_pnl_block_for_state(account_id, state, barrier, None))
            .into_iter()
            .collect();
        Ok(PolicyAccountAdjustmentResult {
            account_adjustments: Vec::new(),
            account_blocks,
        })
    }
}
