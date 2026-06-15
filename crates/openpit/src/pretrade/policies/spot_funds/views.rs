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

//! View structs that flatten policy inputs into compact, locally-named bundles
//! so the per-stage helpers do not have to thread trait-object accessors.

use crate::core::instrument::Instrument;
use crate::param::{
    AccountId, AdjustmentAmount, Asset, Pnl, PositionSize, Price, Quantity, Side, Trade,
    TradeAmount,
};
use crate::pretrade::holdings::Holdings;
use crate::pretrade::PreTradeLock;

/// Identifies which side of an instrument an asset leg settles.
///
/// A spot order touches at most two legs: the underlying it delivers or
/// receives, and the settlement cash it pays or receives. Reservation,
/// fill and cancel are computed independently per leg with the same signed
/// arithmetic, so the side only selects the asset and the per-unit outflow
/// sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LegKind {
    /// The instrument's underlying asset (delivered on buy fills, received on
    /// sell fills, sign convention `+1` outflow per unit when given away).
    Underlying,
    /// The instrument's settlement asset (cash paid or received).
    Settlement,
}

/// View over an order required for pre-trade reservation.
pub(super) struct OrderRequestView<'i> {
    pub(super) instrument: &'i Instrument,
    pub(super) account_id: AccountId,
    pub(super) side: Side,
    pub(super) trade_amount: TradeAmount,
    pub(super) price: Option<Price>,
}

/// View over an account adjustment payload, with every field already read.
pub(super) struct AdjustmentRequestView {
    pub(super) asset: Asset,
    pub(super) balance: Option<AdjustmentAmount>,
    pub(super) balance_average_entry_price: Option<Price>,
    pub(super) balance_realized_pnl: Option<Pnl>,
    pub(super) balance_lower: Option<PositionSize>,
    pub(super) balance_upper: Option<PositionSize>,
    pub(super) held: Option<AdjustmentAmount>,
    pub(super) held_lower: Option<PositionSize>,
    pub(super) held_upper: Option<PositionSize>,
    pub(super) incoming: Option<AdjustmentAmount>,
    pub(super) incoming_lower: Option<PositionSize>,
    pub(super) incoming_upper: Option<PositionSize>,
}

/// View over an execution report required for post-trade settlement.
pub(super) struct ExecutionRequestView<'i> {
    pub(super) instrument: &'i Instrument,
    pub(super) account_id: AccountId,
    pub(super) side: Side,
    pub(super) last_trade: Option<Trade>,
    pub(super) leaves_quantity: Quantity,
    pub(super) is_final: bool,
    pub(super) lock: PreTradeLock,
}

/// Per-asset accumulator for the `held`, `balance`, and realized-PnL changes
/// produced by an execution report.
///
/// A single asset can be touched by both the fill and the cancel phase (and,
/// for a two-leg order, by both legs), so the deltas accumulate additively and
/// the snapshot tracks the most recent post-mutation `Holdings`. `final_holdings`
/// is `None` until the asset has been touched at least once. `pnl_delta` sums
/// the realized PnL across this report's tracked underlying-leg fills, or is
/// `None` when PnL was not tracked; the current average entry price is read
/// back from `final_holdings` rather than tracked separately, since every
/// holdings mutation carries it through.
#[derive(Clone, Copy)]
pub(super) struct LegDelta {
    pub(super) held_delta: PositionSize,
    pub(super) balance_delta: PositionSize,
    pub(super) pnl_delta: Option<Pnl>,
    pub(super) final_holdings: Option<Holdings>,
}

impl LegDelta {
    fn new() -> Self {
        Self {
            held_delta: PositionSize::ZERO,
            balance_delta: PositionSize::ZERO,
            pnl_delta: None,
            final_holdings: None,
        }
    }
}

/// Accumulator carried across an execution report's two settlement legs.
///
/// Each leg (underlying, settlement) reconciles independently: a fill consumes
/// the reserved `held` and credits/debits `balance` in the signed flow
/// direction; a cancel releases the unfilled reserved remainder. The two
/// per-asset accumulators feed the final [`crate::core::AccountAdjustmentOutcome`]
/// entries.
pub(super) struct FillCancelDeltas {
    pub(super) underlying: LegDelta,
    pub(super) settlement: LegDelta,
}

impl FillCancelDeltas {
    pub(super) fn new() -> Self {
        Self {
            underlying: LegDelta::new(),
            settlement: LegDelta::new(),
        }
    }

    pub(super) fn leg_mut(&mut self, kind: LegKind) -> &mut LegDelta {
        match kind {
            LegKind::Underlying => &mut self.underlying,
            LegKind::Settlement => &mut self.settlement,
        }
    }
}
