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

//! Rollback registration for [`SpotFundsPolicy`].

#[cfg(test)]
use crate::core::mutation::AccountPnlReconciliation;
use crate::core::mutation::MutationRollbackResult;
use crate::core::sync_mode::SyncMode;
use crate::core::AccountControl;
use crate::marketdata::MarketDataSync;
use crate::param::{AccountId, PositionSize, Price};
use crate::pretrade::holdings::{Holdings, PositionPnlState};
use crate::pretrade::{AccountBlock, RejectCode};
use crate::{Mutation, Mutations, PnlState};

use super::rejects::account_pnl_block_for_state;
use super::{
    AccountPnlEntry, AccountPnlLeaseGuard, HoldingsKey, SpotFundsPnlBoundsBarrier, SpotFundsPolicy,
    SPOT_FUNDS_POLICY_NAME,
};

/// Pre-adjustment average entry price to restore on rollback.
///
/// Wrapping the snapshot in an `Option<AvgRestore>` lets the forward path say
/// "this adjustment force-set the average, restore this value" (`Some`) versus
/// "leave the average untouched" (`None`). The `previous` field is the average
/// to restore (which may itself be `None` for a flat position).
#[derive(Clone, Copy)]
pub(super) struct AvgRestore {
    pub(super) previous: Option<Price>,
}

/// Pre-correction position PnL to restore on rollback.
#[derive(Clone, Copy)]
pub(super) struct PnlRestore {
    pub(super) previous: Option<PositionPnlState>,
}

/// Forward state needed to reverse an account adjustment.
///
/// Quantities reverse via inverse deltas. Average entry price and position PnL
/// restore their prior snapshots only while the complete asserted holdings
/// slot is still current.
#[derive(Clone, Copy)]
pub(super) struct AdjustmentRollback {
    pub(super) available_delta: PositionSize,
    pub(super) held_delta: PositionSize,
    pub(super) incoming_delta: PositionSize,
    pub(super) asserted: Holdings,
    pub(super) prior_avg: Option<AvgRestore>,
    pub(super) prior_realized: Option<PnlRestore>,
}

pub(super) struct AccountPnlAssertionRollback<StorageFactory>
where
    StorageFactory: crate::storage::LockingPolicyFactory
        + crate::storage::CreateStorageFor<AccountId>
        + 'static,
{
    pub(super) account_control: Option<AccountControl<StorageFactory>>,
    pub(super) account_id: AccountId,
    pub(super) previous: AccountPnlEntry,
    pub(super) asserted: PnlState,
    pub(super) token: u64,
    pub(super) barrier: Option<SpotFundsPnlBoundsBarrier>,
    pub(super) lease: AccountPnlLeaseGuard<StorageFactory>,
}

fn restore_adjusted_snapshots(
    current_before_rollback: Holdings,
    asserted: Holdings,
    rolled_back: Holdings,
    prior_avg: Option<AvgRestore>,
    prior_realized: Option<PnlRestore>,
) -> Holdings {
    // Account-adjustment rollback is expected to be rare and usually signals
    // an unstable external workflow, such as failed order submission or
    // persistence. Serializing every fill on the hot path is not justified, so
    // restore absolute fields only if the complete asserted slot is still
    // current. Field equality alone cannot prove that a fill did not change a
    // quantity while leaving average entry price and PnL unchanged.
    // Equality is an ABA check, not a version check: a slot mutated away from
    // and back to exactly the asserted value reads as untouched, and the
    // restore then runs over a genuinely newer value.
    if current_before_rollback != asserted {
        return rolled_back;
    }
    let restored = match prior_avg {
        Some(AvgRestore { previous }) => rolled_back.with_avg_entry_price(previous),
        None => rolled_back,
    };
    match prior_realized {
        Some(PnlRestore { previous }) => restored.with_realized_pnl_state(previous),
        None => restored,
    }
}

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
            SPOT_FUNDS_POLICY_NAME,
            RejectCode::ArithmeticOverflow,
            "rollback overflow: slot left inconsistent",
            details(),
        );
        ctrl.block(block);
    }
}

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: crate::storage::LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
{
    pub(super) fn register_account_pnl_adjustment_rollback(
        &self,
        mutations: &mut Mutations,
        rollback: AccountPnlAssertionRollback<
            <Sync as SyncMode>::StorageLockingPolicyFactory,
        >,
    ) where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let AccountPnlAssertionRollback {
            account_control,
            account_id,
            previous,
            asserted,
            token,
            barrier,
            lease,
        } = rollback;
        let commit_pnl = self.pnl.clone();
        let rollback_pnl = self.pnl.clone();
        mutations.push(Mutation::new_reporting_with_guard(
            move || {
                commit_pnl.with_mut_if_present(&account_id, |entry| {
                    if entry.assertion_token == Some(token) {
                        entry.assertion_token = None;
                    }
                });
            },
            move || {
                #[cfg(test)]
                let mut reconciliation = None;
                let final_state =
                    rollback_pnl.with_mut(account_id, AccountPnlEntry::zero, |entry, _| {
                        if entry.assertion_token != Some(token) {
                            return entry.state;
                        }

                        let restored = match (previous.state, asserted, entry.state) {
                            (
                                PnlState::Value(previous_value),
                                PnlState::Value(asserted_value),
                                PnlState::Value(current_value),
                            ) => current_value
                                .checked_sub(asserted_value)
                                .and_then(|delta| delta.checked_add(previous_value))
                                .map(PnlState::Value)
                                .unwrap_or(PnlState::Halted(
                                    crate::PnlHaltReason::ArithmeticOverflow,
                                )),
                            (
                                PnlState::Halted(_),
                                PnlState::Value(asserted_value),
                                PnlState::Value(current_value),
                            ) => {
                                #[cfg(test)]
                                {
                                    reconciliation = Some(AccountPnlReconciliation {
                                        account_id,
                                        discarded_delta: current_value
                                            .checked_sub(asserted_value)
                                            .ok(),
                                    });
                                }
                                #[cfg(not(test))]
                                let _ = (asserted_value, current_value);
                                previous.state
                            }
                            // A rejected halt assertion cannot accept numeric
                            // deltas. Restoring the prior entry is exact.
                            (_, PnlState::Halted(_), _)
                            | (_, PnlState::Value(_), PnlState::Halted(_)) => previous.state,
                        };
                        *entry = AccountPnlEntry {
                            state: restored,
                            assertion_token: previous.assertion_token,
                        };
                        restored
                    });

                #[cfg(test)]
                let mut result = MutationRollbackResult::default();
                #[cfg(not(test))]
                let result = MutationRollbackResult::default();
                if let Some(control) = &account_control {
                    let invalidated = control.invalidate_provenance(token);
                    #[cfg(test)]
                    if let Some(block) = invalidated {
                        result.report.invalidated_account_blocks.push(block);
                    }
                    #[cfg(not(test))]
                    drop(invalidated);
                }
                if let Some(barrier) = barrier.as_ref() {
                    if let Some(block) =
                        account_pnl_block_for_state(account_id, final_state, barrier, None)
                    {
                        #[cfg(test)]
                        match &account_control {
                            Some(control) => {
                                result.report.account_blocks.push(block.clone());
                                control.block(block);
                            }
                            None => {
                                result.report.account_blocks.push(block);
                            }
                        }
                        #[cfg(not(test))]
                        if let Some(control) = &account_control {
                            control.block(block);
                        }
                    }
                }
                #[cfg(test)]
                if let Some(reconciliation) = reconciliation {
                    result.report.reconciliations.push(reconciliation);
                }
                result
            },
            lease,
        ));
    }

    /// Registers the rollback that reverses one reserved asset leg.
    ///
    /// A reservation moves `available -= held_amount`, `held += held_amount`,
    /// and `incoming += incoming_amount` for the asset. To reverse it the
    /// rollback applies the inverse deltas through
    /// [`Holdings::apply_delta_rollback`], which subtracts each forward delta
    /// from the current slot. The forward deltas are therefore
    /// `available_delta = -held_amount` (available went down), `held_delta =
    /// +held_amount`, and `incoming_delta = +incoming_amount`; subtracting them
    /// restores available up, held down, and incoming down. Reversing inverse
    /// deltas (rather than a snapshot) keeps any concurrent change on the same
    /// slot intact.
    pub(super) fn register_hold_rollback(
        &self,
        mutations: &mut Mutations,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        key: HoldingsKey,
        held_amount: PositionSize,
        incoming_amount: PositionSize,
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
                // moved into held/incoming. Applying the inverse deltas to a
                // freshly created zero placeholder restores exactly the
                // pre-hold state when no concurrent change happened, and
                // undoes only our delta otherwise.
                let key_for_remove = key.clone();
                let asset_for_diagnostic = key.1.clone();
                let became_zero = holdings_arc.with_mut(key, Holdings::zero, |slot, _| {
                    match slot.apply_delta_rollback(-held_amount, held_amount, incoming_amount) {
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
                                    "hold rollback overflow: asset {asset_for_diagnostic}, \
                                     held {held_amount}, \
                                     incoming {incoming_amount}, slot {slot:?}",
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
        rollback: AdjustmentRollback,
    ) where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let AdjustmentRollback {
            available_delta,
            held_delta,
            incoming_delta,
            asserted,
            prior_avg,
            prior_realized,
        } = rollback;
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
                let asset_for_diagnostic = key.1.clone();
                let became_zero = holdings_arc.with_mut(key, Holdings::zero, |slot, _| {
                    let current_before_rollback = *slot;
                    match slot.apply_delta_rollback(available_delta, held_delta, incoming_delta) {
                        Ok(rolled_back) => {
                            // Quantities roll back via the concurrency-safe
                            // inverse delta above, so a concurrent fill on the
                            // same slot keeps its quantity contribution.
                            //
                            let restored = restore_adjusted_snapshots(
                                current_before_rollback,
                                asserted,
                                rolled_back,
                                prior_avg,
                                prior_realized,
                            );
                            *slot = restored;
                            restored.is_zero()
                        }
                        // Overflow during rollback is practically unreachable
                        // for real balances. The slot is left unchanged and
                        // the account is recorded on the engine's blocked-
                        // accounts sink so the failure is visible end to end
                        // rather than silently swallowed.
                        Err(_) => {
                            record_rollback_overflow(&account_control, || {
                                format!(
                                    "adjustment rollback overflow: asset {asset_for_diagnostic}, \
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

#[cfg(test)]
mod tests {
    use crate::param::{Pnl, PositionSize, Price};
    use crate::pretrade::holdings::{Holdings, PositionPnlState};

    use super::{restore_adjusted_snapshots, AvgRestore, PnlRestore};

    fn price(value: &str) -> Price {
        Price::from_str(value).expect("price literal must be valid")
    }

    fn pnl(value: &str) -> Pnl {
        Pnl::from_str(value).expect("PnL literal must be valid")
    }

    fn tracked(avg: &str, realized: &str) -> Holdings {
        Holdings::new(PositionSize::ZERO, PositionSize::ZERO)
            .with_avg_entry_price(Some(price(avg)))
            .with_realized_pnl(pnl(realized))
    }

    #[test]
    fn asserted_position_snapshots_restore_when_still_current() {
        let asserted = tracked("200", "50");
        let restored = restore_adjusted_snapshots(
            asserted,
            asserted,
            asserted,
            Some(AvgRestore {
                previous: Some(price("100")),
            }),
            Some(PnlRestore {
                previous: Some(PositionPnlState::Pnl(pnl("10"))),
            }),
        );

        assert_eq!(restored.avg_entry_price(), Some(price("100")));
        assert_eq!(restored.realized_pnl(), Some(pnl("10")));
    }

    #[test]
    fn parallel_position_snapshot_changes_survive_rollback() {
        let current = tracked("250", "60");
        let restored = restore_adjusted_snapshots(
            current,
            tracked("200", "50"),
            current,
            Some(AvgRestore {
                previous: Some(price("100")),
            }),
            Some(PnlRestore {
                previous: Some(PositionPnlState::Pnl(pnl("10"))),
            }),
        );

        assert_eq!(restored.avg_entry_price(), Some(price("250")));
        assert_eq!(restored.realized_pnl(), Some(pnl("60")));
    }

    #[test]
    fn quantity_change_prevents_restore_when_snapshot_fields_are_equal() {
        let asserted = tracked("200", "50");
        let current = Holdings::new(
            PositionSize::from_str("1").expect("position size must be valid"),
            PositionSize::ZERO,
        )
        .with_avg_entry_price(Some(price("200")))
        .with_realized_pnl(pnl("50"));
        let rolled_back = current
            .apply_delta_rollback(
                PositionSize::from_str("2").expect("position size must be valid"),
                PositionSize::ZERO,
                PositionSize::ZERO,
            )
            .expect("quantity rollback must succeed");
        let restored = restore_adjusted_snapshots(
            current,
            asserted,
            rolled_back,
            Some(AvgRestore {
                previous: Some(price("100")),
            }),
            Some(PnlRestore {
                previous: Some(PositionPnlState::Pnl(pnl("10"))),
            }),
        );

        assert_eq!(
            restored.available(),
            PositionSize::from_str("-1").expect("position size must be valid")
        );
        assert_eq!(restored.avg_entry_price(), Some(price("200")));
        assert_eq!(restored.realized_pnl(), Some(pnl("50")));
    }
}
