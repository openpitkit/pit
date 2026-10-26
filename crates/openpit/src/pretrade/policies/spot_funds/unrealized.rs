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

//! Evaluation of unrealized and total P&L bounds from current holdings.

use crate::core::sync_mode::SyncMode;
use crate::marketdata::MarketDataSync;
use crate::param::{AccountGroupId, AccountId, Asset, Pnl, PositionSize, Price};
use crate::pretrade::holdings::PositionPnlState;
use crate::pretrade::AccountBlock;
use crate::{PnlHaltReason, PnlState};

use super::{
    SpotFundsPnlBoundsAxis, SpotFundsPnlBoundsBarrier, SpotFundsPolicy, SpotFundsSettings,
};

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: crate::storage::LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
{
    /// Evaluates every effective axis from `settings` for this account.
    pub(super) fn pnl_axis_block(
        &self,
        settings: &SpotFundsSettings,
        account_id: AccountId,
        account_group_id: Option<AccountGroupId>,
        account_currency: Option<&Asset>,
        carry_account_total_provenance: bool,
    ) -> Option<AccountBlock> {
        let barriers = SpotFundsPnlBoundsAxis::ALL.map(|axis| {
            settings.pnl_axis_barriers(axis).barrier_for(
                account_id,
                account_group_id,
                account_currency,
            )
        });
        self.pnl_axis_block_for_barriers(
            barriers,
            account_id,
            account_group_id,
            account_currency,
            carry_account_total_provenance,
        )
    }

    /// Evaluates the given barriers in `SpotFundsPnlBoundsAxis::ALL` order;
    /// the first block wins.
    ///
    /// No barrier means no holdings or market-data reads. Position values are
    /// full-close realized contributions; no value is persisted. Fill and
    /// retune callers carry record provenance on account total.
    pub(super) fn pnl_axis_block_for_barriers(
        &self,
        barriers: [Option<&SpotFundsPnlBoundsBarrier>; 4],
        account_id: AccountId,
        account_group_id: Option<AccountGroupId>,
        account_currency: Option<&Asset>,
        carry_account_total_provenance: bool,
    ) -> Option<AccountBlock> {
        if barriers.iter().all(Option::is_none) {
            return None;
        }

        let mut positions = Vec::new();
        if let Some(assets) = self.holdings.account_assets(&account_id) {
            for asset in assets {
                let Some(holdings) = self.holdings.get(&(account_id, asset.clone())) else {
                    continue;
                };
                if holdings.avg_entry_price().is_none() && !holdings.realized_pnl_is_halted() {
                    continue;
                }
                let unrealized = Self::owned_quantity(holdings).and_then(|owned| {
                    let price = account_currency
                        .and_then(|currency| {
                            self.account_currency_factor(
                                account_id,
                                account_group_id,
                                &asset,
                                currency,
                            )
                        })
                        .map(Price::new);
                    let mut contribution = Self::account_position_pnl_contribution(
                        holdings,
                        PositionSize::new(-owned.to_decimal()),
                        price,
                    );
                    if account_currency.is_none() && !matches!(contribution, Ok(None)) {
                        let mut halt_reason = contribution.err();
                        Self::select_pnl_halt_reason(
                            &mut halt_reason,
                            PnlHaltReason::MissingAccountCurrency,
                        );
                        if let Some(reason) = halt_reason {
                            contribution = Err(reason);
                        }
                    }
                    contribution
                });
                let unrealized = match unrealized {
                    Ok(Some(value)) => PnlState::Value(value),
                    Ok(None) => PnlState::Value(Pnl::ZERO),
                    Err(reason) => PnlState::Halted(reason),
                };
                let realized = match holdings.realized_pnl_outcome() {
                    Some(PositionPnlState::Pnl(value)) => PnlState::Value(value),
                    Some(PositionPnlState::Halted(reason)) => PnlState::Halted(reason),
                    // fallback(approved): nothing realized yet reads as zero
                    None => PnlState::Value(Pnl::ZERO),
                };
                positions.push((
                    asset,
                    unrealized,
                    Self::sum_pnl_states([realized, unrealized]),
                ));
            }
        }
        let account_unrealized = Self::sum_pnl_states(positions.iter().map(|(_, value, _)| *value));
        let (account_realized, assertion_token) = self
            .pnl
            .with(&account_id, |entry| (entry.state, entry.assertion_token))
            // fallback(approved): unset ledger reads as zero, as realized does
            .unwrap_or((PnlState::Value(Pnl::ZERO), None));
        let account_total = Self::sum_pnl_states([account_realized, account_unrealized]);

        for (axis, barrier) in SpotFundsPnlBoundsAxis::ALL.into_iter().zip(barriers) {
            let Some(barrier) = barrier else { continue };
            if let Some(block) =
                super::rejects::pnl_axis_currency_mismatch_block(axis, account_currency, barrier)
            {
                return Some(block);
            }
            match axis {
                SpotFundsPnlBoundsAxis::AccountUnrealized
                | SpotFundsPnlBoundsAxis::AccountTotal => {
                    let state = if axis == SpotFundsPnlBoundsAxis::AccountUnrealized {
                        account_unrealized
                    } else {
                        account_total
                    };
                    if let Some(mut block) =
                        super::rejects::pnl_axis_block_for_state(axis, None, state, barrier)
                    {
                        if axis == SpotFundsPnlBoundsAxis::AccountTotal
                            && carry_account_total_provenance
                        {
                            block.provenance = assertion_token;
                        }
                        return Some(block);
                    }
                }
                SpotFundsPnlBoundsAxis::PositionUnrealized
                | SpotFundsPnlBoundsAxis::PositionTotal => {
                    for (asset, unrealized, total) in &positions {
                        let state = if axis == SpotFundsPnlBoundsAxis::PositionUnrealized {
                            *unrealized
                        } else {
                            *total
                        };
                        if let Some(block) = super::rejects::pnl_axis_block_for_state(
                            axis,
                            Some(asset),
                            state,
                            barrier,
                        ) {
                            return Some(block);
                        }
                    }
                }
            }
        }
        None
    }

    fn sum_pnl_states(states: impl IntoIterator<Item = PnlState>) -> PnlState {
        let mut sum = Pnl::ZERO;
        let mut halt_reason = None;
        for state in states {
            match state {
                PnlState::Value(value) => match sum.checked_add(value) {
                    Ok(value) => sum = value,
                    Err(_) => Self::select_pnl_halt_reason(
                        &mut halt_reason,
                        PnlHaltReason::ArithmeticOverflow,
                    ),
                },
                PnlState::Halted(reason) => {
                    Self::select_pnl_halt_reason(&mut halt_reason, reason);
                }
            }
        }
        match halt_reason {
            Some(reason) => PnlState::Halted(reason),
            None => PnlState::Value(sum),
        }
    }
}
