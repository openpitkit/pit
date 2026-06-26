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

use rust_decimal::Decimal;

use crate::param::{AdjustmentAmount, Pnl, PositionSize, Price};

use super::error::{AdjustmentOverflowError, HoldError};

/// Per-asset slot tracking `available`, `held`, and `incoming` quantities
/// plus the net position's `avg_entry_price` and cumulative `realized_pnl`.
///
/// `available` is free to be locked by new pre-trade reservations. `held`
/// is locked by pending reservations and is released back to `available`
/// on cancel or consumed on fill. `incoming` tracks expected future inflows
/// not yet settled: a pre-trade reservation projects the acquiring leg's
/// expected inflow into it ([`Holdings::reserve_incoming`]), a fill or cancel
/// drains the consumed/released portion ([`Holdings::consume_incoming`]), and
/// account adjustments may force it directly. `incoming` is purely
/// informational: it is never part of spendable capacity and never gates a
/// reservation (see [`Holdings::try_hold`]).
///
/// `avg_entry_price` is the average entry price of the current net owned
/// position (`available + held`), denominated in the account currency; it is
/// `None` when that net is flat or average tracking is unavailable.
/// `realized_pnl` is the cumulative realized profit and loss for this slot,
/// also denominated in the account currency. `None` means realized PnL is not
/// tracked. Missing account currency or missing FX clears both fields and does
/// not reject or block the fill. Both fields evolve online via
/// [`Holdings::realize_position_fill`] on the underlying leg of a fill and can
/// be force-set through account-adjustment balance operations; reservation and
/// cancel move funds between `available` and `held` without touching either
/// field.
///
/// `try_hold` is the only operation that enforces a financial invariant:
/// the reservation requires `amount <= available + min(held, 0)`. A
/// negative `held` (manager-initiated adjustment) reduces the spendable
/// capacity below `available`. All other mutating operations apply
/// arithmetic directly without non-negative guards — negative `amount`
/// inverts the direction — and only fail on decimal-range overflow.
///
/// Operations return a new `Holdings` (immutable update). This makes
/// rollback straightforward for the caller: capture the old value, write
/// the new value synchronously, and push a rollback `Mutation` that
/// restores the old value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Holdings {
    available: PositionSize,
    held: PositionSize,
    incoming: PositionSize,
    avg_entry_price: Option<Price>,
    realized_pnl: Option<Pnl>,
}

impl Default for Holdings {
    fn default() -> Self {
        Self::zero()
    }
}

/// Selects the field targeted by [`Holdings::apply_adjustment`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdjustmentTarget {
    /// Adjust `available`.
    Available,
    /// Adjust `held`.
    Held,
    /// Adjust `incoming`.
    Incoming,
}

impl Holdings {
    /// Returns a holdings with all quantities at zero and no tracked average
    /// entry price or realized PnL.
    pub fn zero() -> Self {
        Self {
            avg_entry_price: None,
            available: PositionSize::ZERO,
            held: PositionSize::ZERO,
            incoming: PositionSize::ZERO,
            realized_pnl: None,
        }
    }

    /// Builds a holdings from available and held; incoming is set to zero,
    /// the average entry price to `None`, and realized PnL is not tracked.
    pub fn new(available: PositionSize, held: PositionSize) -> Self {
        Self {
            avg_entry_price: None,
            available,
            held,
            incoming: PositionSize::ZERO,
            realized_pnl: None,
        }
    }

    pub fn available(&self) -> PositionSize {
        self.available
    }

    pub fn held(&self) -> PositionSize {
        self.held
    }

    pub fn incoming(&self) -> PositionSize {
        self.incoming
    }

    /// Average entry price of the current net owned position, or `None` when
    /// the net (`available + held`) is flat.
    pub fn avg_entry_price(&self) -> Option<Price> {
        self.avg_entry_price
    }

    /// Cumulative realized PnL for this slot, in the account currency.
    ///
    /// `None` means realized PnL is not tracked for the slot.
    pub fn realized_pnl(&self) -> Option<Pnl> {
        self.realized_pnl
    }

    /// Force-sets `realized_pnl` to an absolute account-currency value.
    ///
    /// Used by the account-adjustment path when a balance operation carries
    /// a realized-PnL override, mirroring how
    /// [`Holdings::with_avg_entry_price`] force-sets the average. Online fills
    /// never call this; they accrue realized PnL through
    /// [`Holdings::realize_position_fill`].
    pub fn with_realized_pnl(&self, realized_pnl: Pnl) -> Self {
        Self {
            realized_pnl: Some(realized_pnl),
            ..*self
        }
    }

    /// Force-sets `realized_pnl` to an absolute value or clears tracking.
    ///
    /// Unlike [`Holdings::with_realized_pnl`], which can only set `Some`, this
    /// accepts the full `Option<Pnl>` so a prior untracked state (`None`) can be
    /// restored exactly. Used to roll back an adjustment that force-set realized
    /// PnL, mirroring how [`Holdings::with_avg_entry_price`] restores the
    /// average: a `None` snapshot keeps the slot untracked and does not
    /// auto-resume on the next fill.
    pub fn with_realized_pnl_opt(&self, realized_pnl: Option<Pnl>) -> Self {
        Self {
            realized_pnl,
            ..*self
        }
    }

    /// Clears average-entry-price and realized-PnL tracking.
    pub fn without_position_tracking(&self) -> Self {
        Self {
            avg_entry_price: None,
            realized_pnl: None,
            ..*self
        }
    }

    /// Moves `amount` from `available` to `held`.
    ///
    /// Negative `amount` inverts the direction (moves funds from `held`
    /// back to `available`). The financial reject fires when
    /// `amount > available + min(held, 0)`: a negative `held`
    /// (set by a manager-initiated adjustment) reduces the spendable
    /// capacity below `available`, because those funds are owed back.
    ///
    /// # Errors
    ///
    /// - [`HoldError::InsufficientAvailable`] if
    ///   `amount > available + min(held, 0)`.
    /// - [`HoldError::ArithmeticOverflow`] if the underlying decimal
    ///   addition or subtraction overflows the value range.
    pub fn try_hold(&self, amount: PositionSize) -> Result<Self, HoldError> {
        let spendable = if self.held < PositionSize::ZERO {
            self.available
                .checked_add(self.held)
                .map_err(|_| HoldError::ArithmeticOverflow)?
        } else {
            self.available
        };
        if amount > spendable {
            return Err(HoldError::InsufficientAvailable {
                available: spendable,
                requested: amount,
            });
        }

        let available = self
            .available
            .checked_sub(amount)
            .map_err(|_| HoldError::ArithmeticOverflow)?;
        let held = self
            .held
            .checked_add(amount)
            .map_err(|_| HoldError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available,
            held,
            incoming: self.incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Moves `amount` from `available` to `held` without the solvency gate.
    ///
    /// Sibling of [`Holdings::try_hold`] for the spot-funds track-only mode: it
    /// performs the same checked `available - amount` / `held + amount`
    /// arithmetic but never refuses on insufficiency, so `available` is allowed
    /// to go negative. Decimal-range overflow is still an integrity failure and
    /// is surfaced, never silently absorbed.
    ///
    /// # Errors
    ///
    /// - [`HoldError::ArithmeticOverflow`] if the underlying decimal addition or
    ///   subtraction overflows the value range.
    pub fn hold_allow_negative(&self, amount: PositionSize) -> Result<Self, HoldError> {
        let available = self
            .available
            .checked_sub(amount)
            .map_err(|_| HoldError::ArithmeticOverflow)?;
        let held = self
            .held
            .checked_add(amount)
            .map_err(|_| HoldError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available,
            held,
            incoming: self.incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Moves `amount` from `held` back to `available`.
    ///
    /// Negative `amount` inverts the direction. The result may have
    /// negative `held` or negative `available` when the caller asks for
    /// it; that is intentional. The only failure is decimal-range overflow.
    ///
    /// # Errors
    ///
    /// - [`AdjustmentOverflowError::ArithmeticOverflow`] if the underlying decimal
    ///   addition or subtraction overflows the value range.
    pub fn release(&self, amount: PositionSize) -> Result<Self, AdjustmentOverflowError> {
        let available = self
            .available
            .checked_add(amount)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        let held = self
            .held
            .checked_sub(amount)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available,
            held,
            incoming: self.incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Subtracts `amount` from `held` without enforcing the
    /// non-negative invariant.
    ///
    /// Use this when the venue execution report is authoritative and
    /// the engine must record the fact even when the actual fill
    /// exceeds the reserved `held`. The resulting `held` may be
    /// negative; the engine accepts this as evidence of divergence
    /// between the reservation estimate and the venue truth.
    ///
    /// `amount` may carry any sign; the routine performs a plain
    /// `held - amount` and returns the result.
    ///
    /// # Errors
    ///
    /// Returns [`AdjustmentOverflowError::ArithmeticOverflow`] when
    /// the underlying decimal subtraction overflows the value range.
    pub fn apply_fill_outflow(
        &self,
        amount: PositionSize,
    ) -> Result<Self, AdjustmentOverflowError> {
        let held = self
            .held
            .checked_sub(amount)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available: self.available,
            held,
            incoming: self.incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Adds `amount` to `available` without enforcing the
    /// non-negative invariant.
    ///
    /// Use this when the venue execution report is authoritative
    /// (inflow side of a fill, price-improvement savings credit-back).
    /// `amount` may carry any sign; the routine performs a plain
    /// `available + amount` and returns the result.
    ///
    /// # Errors
    ///
    /// Returns [`AdjustmentOverflowError::ArithmeticOverflow`] when
    /// the underlying decimal addition overflows the value range.
    pub fn apply_fill_inflow(&self, amount: PositionSize) -> Result<Self, AdjustmentOverflowError> {
        let available = self
            .available
            .checked_add(amount)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available,
            held: self.held,
            incoming: self.incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Adds `amount` to `incoming`, projecting an acquiring leg's expected
    /// inflow alongside the existing `held` outflow leg.
    ///
    /// Used by the pre-trade reserve path (a buy's base leg, a priced sell's
    /// quote leg). `incoming` is purely informational and has no solvency gate,
    /// so this never rejects on insufficiency - the only failure is decimal-range
    /// overflow, which the pre-trade caller maps to a reject. `available` and
    /// `held` are untouched, so spendable capacity is unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`AdjustmentOverflowError::ArithmeticOverflow`] when the
    /// underlying decimal addition overflows the value range.
    pub fn reserve_incoming(&self, amount: PositionSize) -> Result<Self, AdjustmentOverflowError> {
        let incoming = self
            .incoming
            .checked_add(amount)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available: self.available,
            held: self.held,
            incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Subtracts `amount` from `incoming`, draining the projected inflow as a
    /// fill consumes it or a cancel releases the unfilled remainder.
    ///
    /// `amount` may carry any sign and the result may go negative when the venue
    /// fill diverges from the reservation estimate (mirroring how
    /// [`Holdings::apply_fill_outflow`] allows a negative `held`); the engine
    /// accepts this as informational divergence. `available` and `held` are
    /// untouched, so this never feeds the available credit and never changes
    /// spendable capacity. The execution path maps an overflow to an account
    /// block.
    ///
    /// # Errors
    ///
    /// Returns [`AdjustmentOverflowError::ArithmeticOverflow`] when the
    /// underlying decimal subtraction overflows the value range.
    pub fn consume_incoming(&self, amount: PositionSize) -> Result<Self, AdjustmentOverflowError> {
        let incoming = self
            .incoming
            .checked_sub(amount)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available: self.available,
            held: self.held,
            incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Applies one underlying-leg fill to the average-entry-price / realized-PnL
    /// state and returns the updated holdings together with the realized PnL
    /// produced by this fill.
    ///
    /// This is signed weighted-average-cost accounting with full long/short
    /// support including flips. `signed_qty` is the signed base flow of the
    /// fill (`> 0` for a buy/inflow, `< 0` for a sell/outflow) and `price` is
    /// the fill price converted into the account currency. Only the underlying
    /// (base) leg of a fill calls this; the settlement leg never touches
    /// average price or realized PnL. The returned holdings differs from
    /// `self` only in `avg_entry_price` and `realized_pnl`; the caller applies
    /// the quantity mutation (`available` / `held`) separately, within the same
    /// slot update.
    ///
    /// Let `owned = available + held` be the net base position *before* this
    /// leg's quantity mutation, `avg` the prior average entry price, `Δ` the
    /// `signed_qty`, `p` the fill `price`, and `new_owned = owned + Δ`. The
    /// realized delta and the new average are:
    ///
    /// 1. `owned == 0` (open from flat): `new_avg = Some(p)`, realized `0`.
    /// 2. same sign as `owned` (add to position): the position-weighted average
    ///    `new_avg = (owned*avg + Δ*p) / new_owned`, realized `0`.
    /// 3. opposite sign, `|Δ| <= |owned|` (reduce/close): realized
    ///    `(p - avg) * (-Δ)`; `new_avg = avg`, or `None` when `new_owned == 0`.
    /// 4. opposite sign, `|Δ| > |owned|` (flip): realized `(p - avg) * owned`
    ///    closes the whole prior position, and the remainder opens the opposite
    ///    side at `p`, so `new_avg = Some(p)`.
    ///
    /// Tracking is optional: opening from a flat slot starts tracking with
    /// `avg_entry_price = Some(p)` and `realized_pnl = Some(0)`. A non-flat
    /// slot whose `realized_pnl` is `None` has lost its account-currency basis;
    /// it stays untracked and does not auto-resume even when later fills pass a
    /// converted price. Force-set both fields through account adjustment to
    /// re-arm tracking.
    ///
    /// Realized PnL accumulates while tracked:
    /// `realized_pnl_new = realized_pnl + realized`. Sign sanity: a long
    /// (`owned > 0`) sold at `p > avg` yields positive PnL; a short
    /// (`owned < 0`) bought back at `p < avg` also yields positive PnL.
    ///
    /// # Errors
    ///
    /// Returns [`AdjustmentOverflowError::ArithmeticOverflow`] if any decimal
    /// multiplication, addition, subtraction, or division overflows the value
    /// range (a zero `new_owned` divisor cannot occur in case 2, which is the
    /// only branch that divides).
    pub fn realize_position_fill(
        &self,
        signed_qty: PositionSize,
        price: Price,
    ) -> Result<(Self, Option<Pnl>), AdjustmentOverflowError> {
        let owned = self
            .available
            .checked_add(self.held)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        let new_owned = owned
            .checked_add(signed_qty)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;

        if !owned.is_zero() && self.realized_pnl.is_none() {
            return Ok((self.without_position_tracking(), None));
        }

        let owned_dec = owned.to_decimal();
        let delta_dec = signed_qty.to_decimal();
        let price_dec = price.to_decimal();
        let zero = Decimal::ZERO;

        let (new_avg, realized_dec) = if owned_dec == zero {
            // Case 1: opening from flat. A zero-quantity fill leaves the slot
            // flat with no average; a non-zero fill seeds the average at `p`.
            let avg = if delta_dec == zero { None } else { Some(price) };
            (avg, zero)
        } else if (owned_dec > zero) == (delta_dec > zero) {
            // Case 2: same direction, growing the position. `new_owned` is
            // non-zero (same-sign add never crosses 0).
            match self.avg_entry_price {
                // Position-weighted average against the prior basis.
                Some(avg) => {
                    let weighted_existing = owned_dec
                        .checked_mul(avg.to_decimal())
                        .ok_or(AdjustmentOverflowError::ArithmeticOverflow)?;
                    let weighted_fill = delta_dec
                        .checked_mul(price_dec)
                        .ok_or(AdjustmentOverflowError::ArithmeticOverflow)?;
                    let numerator = weighted_existing
                        .checked_add(weighted_fill)
                        .ok_or(AdjustmentOverflowError::ArithmeticOverflow)?;
                    let new_avg_dec = numerator
                        .checked_div(new_owned.to_decimal())
                        .ok_or(AdjustmentOverflowError::ArithmeticOverflow)?;
                    (Some(Price::new(new_avg_dec)), zero)
                }
                // No prior basis to weight against: stay basis-less.
                None => (None, zero),
            }
        } else {
            // Cases 3 & 4: opposite direction, reducing/closing/flipping.
            match self.avg_entry_price {
                Some(avg) => {
                    let price_minus_avg = price_dec
                        .checked_sub(avg.to_decimal())
                        .ok_or(AdjustmentOverflowError::ArithmeticOverflow)?;
                    if delta_dec.abs() <= owned_dec.abs() {
                        // Case 3: reduce or exact close. Realized over the closed
                        // quantity `-Δ` (sign-correct for both long and short).
                        // `Decimal` negation is infallible.
                        let closed_qty = -delta_dec;
                        let realized = price_minus_avg
                            .checked_mul(closed_qty)
                            .ok_or(AdjustmentOverflowError::ArithmeticOverflow)?;
                        let avg = if new_owned.is_zero() { None } else { Some(avg) };
                        (avg, realized)
                    } else {
                        // Case 4: flip. Close the whole prior `owned` (realized
                        // over `owned`), then open the opposite side at `p`.
                        let realized = price_minus_avg
                            .checked_mul(owned_dec)
                            .ok_or(AdjustmentOverflowError::ArithmeticOverflow)?;
                        (Some(price), realized)
                    }
                }
                // No prior basis: nothing to realize against. A reduce/close
                // keeps no average; a flip opens the remainder at `p`.
                None => {
                    let new_avg = if delta_dec.abs() <= owned_dec.abs() {
                        None
                    } else {
                        Some(price)
                    };
                    (new_avg, zero)
                }
            }
        };

        let realized_delta = Pnl::new(realized_dec);
        let realized_pnl = match self.realized_pnl {
            Some(realized_pnl) => Some(
                realized_pnl
                    .checked_add(realized_delta)
                    .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?,
            ),
            None => Some(realized_delta),
        };

        Ok((
            Self {
                avg_entry_price: new_avg,
                available: self.available,
                held: self.held,
                incoming: self.incoming,
                realized_pnl,
            },
            Some(realized_delta),
        ))
    }

    /// Subtracts per-quantity deltas from the current slot in one atomic step.
    ///
    /// Intended for delta-based rollback of a prior `apply_adjustment` call:
    /// pass the deltas that were applied forward, and this method reverses them
    /// by subtracting each one from the corresponding quantity field. Applying
    /// the inverse delta (rather than restoring a snapshot) keeps concurrent
    /// changes by other threads intact for the quantity fields.
    ///
    /// Average entry price and realized PnL are intentionally left untouched
    /// here: neither is delta-reversible (the weighted-average cost is
    /// path-dependent, and a forced realized value may overwrite an untracked
    /// `None`), so the rollback path restores both absolutely from a snapshot.
    ///
    /// All three subtractions are checked; returns
    /// [`AdjustmentOverflowError::ArithmeticOverflow`] if any of them would
    /// overflow the decimal range. A caller that treats rollback as best-effort
    /// should leave the slot unchanged on error.
    pub fn apply_delta_rollback(
        &self,
        available_delta: PositionSize,
        held_delta: PositionSize,
        incoming_delta: PositionSize,
    ) -> Result<Self, AdjustmentOverflowError> {
        let available = self
            .available
            .checked_sub(available_delta)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        let held = self
            .held
            .checked_sub(held_delta)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        let incoming = self
            .incoming
            .checked_sub(incoming_delta)
            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
        Ok(Self {
            avg_entry_price: self.avg_entry_price,
            available,
            held,
            incoming,
            realized_pnl: self.realized_pnl,
        })
    }

    /// Applies an `AdjustmentAmount` to the chosen field.
    ///
    /// - `AdjustmentAmount::Absolute(v)` sets the field to `v`
    ///   unconditionally; negative values are permitted for
    ///   manager-initiated overrides.
    /// - `AdjustmentAmount::Delta(d)` adds `d` to the field; the
    ///   result may be negative.
    ///
    /// # Errors
    ///
    /// Returns [`AdjustmentOverflowError::ArithmeticOverflow`] when
    /// the underlying decimal addition overflows the value range
    /// (delta variant only).
    pub fn apply_adjustment(
        &self,
        target: AdjustmentTarget,
        amount: AdjustmentAmount,
    ) -> Result<Self, AdjustmentOverflowError> {
        // Start from a copy so `avg_entry_price` and `realized_pnl` carry
        // through unchanged; only the targeted quantity field is rewritten.
        let mut new = *self;
        let field = match target {
            AdjustmentTarget::Available => &mut new.available,
            AdjustmentTarget::Held => &mut new.held,
            AdjustmentTarget::Incoming => &mut new.incoming,
        };
        *field = match amount {
            AdjustmentAmount::Absolute(v) => v,
            AdjustmentAmount::Delta(d) => field
                .checked_add(d)
                .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?,
        };
        Ok(new)
    }

    /// Sets the average entry price of the current net position.
    ///
    /// Used by the account-adjustment path when a balance operation carries an
    /// account-currency average entry price. Realized PnL is never touched
    /// here.
    pub fn with_avg_entry_price(&self, avg_entry_price: Option<Price>) -> Self {
        Self {
            avg_entry_price,
            ..*self
        }
    }

    /// Returns `true` only when the slot carries no economic state at all:
    /// every quantity is zero, realized PnL is absent or zero, and there is no
    /// average entry price.
    ///
    /// Realized PnL and a residual average entry price keep the slot alive so
    /// the online PnL accumulated from fills is never silently pruned.
    pub fn is_zero(&self) -> bool {
        self.available.is_zero()
            && self.held.is_zero()
            && self.incoming.is_zero()
            && self.realized_pnl.map_or(true, |pnl| pnl.is_zero())
            && self.avg_entry_price.is_none()
    }

    /// Returns `true` if `available` is within the given inclusive bounds.
    ///
    /// `None` on either side means that bound is unconstrained.
    pub fn available_within_bounds(
        &self,
        lower: Option<PositionSize>,
        upper: Option<PositionSize>,
    ) -> bool {
        !lower.is_some_and(|b| self.available < b) && !upper.is_some_and(|b| self.available > b)
    }

    /// Returns `true` if `held` is within the given inclusive bounds.
    ///
    /// `None` on either side means that bound is unconstrained.
    pub fn held_within_bounds(
        &self,
        lower: Option<PositionSize>,
        upper: Option<PositionSize>,
    ) -> bool {
        !lower.is_some_and(|b| self.held < b) && !upper.is_some_and(|b| self.held > b)
    }

    /// Returns `true` if `incoming` is within the given inclusive bounds.
    ///
    /// `None` on either side means that bound is unconstrained.
    pub fn incoming_within_bounds(
        &self,
        lower: Option<PositionSize>,
        upper: Option<PositionSize>,
    ) -> bool {
        !lower.is_some_and(|b| self.incoming < b) && !upper.is_some_and(|b| self.incoming > b)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::param::{AdjustmentAmount, Pnl, PositionSize, Price};

    use super::super::error::{AdjustmentOverflowError, HoldError};
    use super::{AdjustmentTarget, Holdings};

    fn ps(value: &str) -> PositionSize {
        PositionSize::from_str(value).expect("position size literal must be valid")
    }

    fn pnl(value: &str) -> Pnl {
        Pnl::from_str(value).expect("pnl literal must be valid")
    }

    fn px(value: &str) -> Price {
        Price::from_str(value).expect("price literal must be valid")
    }

    fn holdings(available: &str, held: &str) -> Holdings {
        Holdings::new(ps(available), ps(held))
    }

    fn max_ps() -> PositionSize {
        PositionSize::new(Decimal::MAX)
    }

    fn min_ps() -> PositionSize {
        PositionSize::new(Decimal::MIN)
    }

    #[test]
    fn zero_returns_empty_components() {
        let value = Holdings::zero();

        assert_eq!(value.available(), PositionSize::ZERO);
        assert_eq!(value.held(), PositionSize::ZERO);
        assert_eq!(value.incoming(), PositionSize::ZERO);
    }

    #[test]
    fn new_stores_explicit_components() {
        let value = Holdings::new(ps("5"), ps("3"));

        assert_eq!(value.available(), ps("5"));
        assert_eq!(value.held(), ps("3"));
        assert_eq!(value.incoming(), PositionSize::ZERO);

        assert_eq!(
            Holdings::new(PositionSize::ZERO, PositionSize::ZERO),
            Holdings::zero(),
        );
    }

    #[test]
    fn new_accepts_negative_components() {
        let value = Holdings::new(ps("-1"), ps("-2"));

        assert_eq!(value.available(), ps("-1"));
        assert_eq!(value.held(), ps("-2"));
        assert_eq!(value.incoming(), PositionSize::ZERO);
    }

    #[test]
    fn accessors_return_constructor_values() {
        let value = holdings("7", "4");

        assert_eq!(value.available(), ps("7"));
        assert_eq!(value.held(), ps("4"));
        assert_eq!(value.incoming(), PositionSize::ZERO);
    }

    #[test]
    fn try_hold_moves_available_to_held() {
        let value = holdings("10", "0");
        let updated = value.try_hold(ps("5")).expect("must hold");

        assert_eq!(updated.available(), ps("5"));
        assert_eq!(updated.held(), ps("5"));
    }

    #[test]
    fn try_hold_all_available() {
        let value = holdings("10", "0");
        let updated = value.try_hold(ps("10")).expect("must hold");

        assert_eq!(updated.available(), PositionSize::ZERO);
        assert_eq!(updated.held(), ps("10"));
    }

    #[test]
    fn try_hold_rejects_insufficient_available_without_changing_original() {
        let value = holdings("10", "0");
        let err = value.try_hold(ps("15")).expect_err("must fail");

        assert_eq!(
            err,
            HoldError::InsufficientAvailable {
                available: ps("10"),
                requested: ps("15"),
            }
        );
        assert_eq!(value, holdings("10", "0"));
    }

    #[test]
    fn try_hold_negative_amount_inverts_as_arithmetic() {
        let value = holdings("10", "5");
        let updated = value.try_hold(ps("-3")).expect("must succeed");

        assert_eq!(updated.available(), ps("13"));
        assert_eq!(updated.held(), ps("2"));
    }

    #[test]
    fn try_hold_reports_arithmetic_overflow_when_held_would_overflow() {
        let value = Holdings::new(max_ps(), max_ps());
        let err = value.try_hold(max_ps()).expect_err("must fail");

        assert_eq!(err, HoldError::ArithmeticOverflow);
    }

    #[test]
    fn try_hold_respects_negative_held() {
        // Manager set held=-2000, balance=2000; net spendable is 0.
        let value = Holdings::new(ps("2000"), ps("-2000"));
        let err = value.try_hold(ps("1")).expect_err("must reject");

        assert_eq!(
            err,
            HoldError::InsufficientAvailable {
                available: PositionSize::ZERO,
                requested: ps("1"),
            }
        );
    }

    #[test]
    fn try_hold_succeeds_when_negative_held_covered_by_available() {
        // held=-2000, available=5000 → spendable=3000.
        let value = Holdings::new(ps("5000"), ps("-2000"));

        value
            .try_hold(ps("3000"))
            .expect("must succeed within spendable");

        let err = value
            .try_hold(ps("3001"))
            .expect_err("must reject one over");
        assert_eq!(
            err,
            HoldError::InsufficientAvailable {
                available: ps("3000"),
                requested: ps("3001"),
            }
        );
    }

    #[test]
    fn try_hold_positive_held_does_not_change_spendable() {
        // positive held does not reduce spendable.
        let value = holdings("10", "5");
        value
            .try_hold(ps("10"))
            .expect("must succeed - held is positive, spendable = available");
    }

    #[test]
    fn hold_allow_negative_moves_available_to_held_within_balance() {
        let value = holdings("10", "0");
        let updated = value.hold_allow_negative(ps("5")).expect("must hold");

        assert_eq!(updated.available(), ps("5"));
        assert_eq!(updated.held(), ps("5"));
    }

    #[test]
    fn hold_allow_negative_drives_available_negative_without_rejecting() {
        let value = holdings("1000", "0");
        let updated = value.hold_allow_negative(ps("2000")).expect("must hold");

        // No solvency gate: available goes negative, held records the full hold.
        assert_eq!(updated.available(), ps("-1000"));
        assert_eq!(updated.held(), ps("2000"));
    }

    #[test]
    fn hold_allow_negative_preserves_incoming_and_position_tracking() {
        let value = holdings("10", "5")
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(pnl("7"))
            .reserve_incoming(ps("4"))
            .expect("seed must succeed");
        let updated = value.hold_allow_negative(ps("20")).expect("must hold");

        assert_eq!(updated.incoming(), ps("4"));
        assert_eq!(updated.avg_entry_price(), Some(px("100")));
        assert_eq!(updated.realized_pnl(), Some(pnl("7")));
    }

    #[test]
    fn hold_allow_negative_reports_arithmetic_overflow() {
        // held + amount overflows the value range even though insufficiency is
        // never enforced.
        let value = Holdings::new(max_ps(), max_ps());
        let err = value.hold_allow_negative(max_ps()).expect_err("must fail");

        assert_eq!(err, HoldError::ArithmeticOverflow);
    }

    #[test]
    fn release_moves_held_to_available() {
        let value = holdings("2", "10");
        let updated = value.release(ps("4")).expect("must release");

        assert_eq!(updated.available(), ps("6"));
        assert_eq!(updated.held(), ps("6"));
    }

    #[test]
    fn release_all_held() {
        let value = holdings("2", "10");
        let updated = value.release(ps("10")).expect("must release");

        assert_eq!(updated.available(), ps("12"));
        assert_eq!(updated.held(), PositionSize::ZERO);
    }

    #[test]
    fn release_amount_exceeding_held_drives_held_negative() {
        let value = holdings("2", "10");
        let updated = value.release(ps("15")).expect("must succeed");

        assert_eq!(updated.available(), ps("17"));
        assert_eq!(updated.held(), ps("-5"));
    }

    #[test]
    fn release_negative_amount_inverts_as_arithmetic() {
        let value = holdings("10", "5");
        let updated = value.release(ps("-3")).expect("must succeed");

        assert_eq!(updated.available(), ps("7"));
        assert_eq!(updated.held(), ps("8"));
    }

    #[test]
    fn release_reports_arithmetic_overflow_when_available_would_overflow() {
        let value = Holdings::new(max_ps(), max_ps());
        let err = value.release(max_ps()).expect_err("must fail");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn apply_fill_outflow_subtracts_held_only() {
        let value = holdings("10", "5");
        let updated = value.apply_fill_outflow(ps("3")).expect("must subtract");

        assert_eq!(updated.available(), ps("10"));
        assert_eq!(updated.held(), ps("2"));
    }

    #[test]
    fn apply_fill_outflow_drives_held_negative_when_amount_exceeds_held() {
        let value = holdings("10", "5");
        let updated = value.apply_fill_outflow(ps("8")).expect("must subtract");

        assert_eq!(updated.available(), ps("10"));
        assert_eq!(updated.held(), ps("-3"));
    }

    #[test]
    fn apply_fill_outflow_negative_amount_adds_to_held() {
        let value = holdings("10", "5");
        let updated = value.apply_fill_outflow(ps("-3")).expect("must succeed");

        assert_eq!(updated.available(), ps("10"));
        assert_eq!(updated.held(), ps("8"));
    }

    #[test]
    fn apply_fill_outflow_reports_arithmetic_overflow() {
        // held - amount overflows when amount is very negative and
        // held is near the positive end of the value range.
        let value = Holdings::new(PositionSize::ZERO, max_ps());
        let err = value
            .apply_fill_outflow(min_ps())
            .expect_err("must overflow");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn apply_fill_inflow_zero_amount_is_no_change() {
        let value = holdings("10", "2");
        let updated = value
            .apply_fill_inflow(PositionSize::ZERO)
            .expect("must succeed");

        assert_eq!(updated, value);
    }

    #[test]
    fn apply_fill_inflow_adds_to_available_only() {
        let value = holdings("10", "5");
        let updated = value.apply_fill_inflow(ps("3")).expect("must add");

        assert_eq!(updated.available(), ps("13"));
        assert_eq!(updated.held(), ps("5"));
    }

    #[test]
    fn apply_fill_inflow_accepts_negative_amount_driving_available_negative() {
        let value = holdings("3", "5");
        let updated = value.apply_fill_inflow(ps("-7")).expect("must add");

        assert_eq!(updated.available(), ps("-4"));
        assert_eq!(updated.held(), ps("5"));
    }

    #[test]
    fn apply_fill_inflow_reports_arithmetic_overflow() {
        let value = Holdings::new(max_ps(), PositionSize::ZERO);
        let err = value
            .apply_fill_inflow(max_ps())
            .expect_err("must overflow");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn apply_adjustment_sets_available_absolute_values() {
        let value = holdings("5", "11");

        assert_eq!(
            value
                .apply_adjustment(
                    AdjustmentTarget::Available,
                    AdjustmentAmount::Absolute(ps("7"))
                )
                .expect("absolute must succeed")
                .available(),
            ps("7")
        );
        assert_eq!(
            value
                .apply_adjustment(
                    AdjustmentTarget::Available,
                    AdjustmentAmount::Absolute(ps("0"))
                )
                .expect("absolute must succeed")
                .available(),
            PositionSize::ZERO
        );
        assert_eq!(
            value
                .apply_adjustment(
                    AdjustmentTarget::Available,
                    AdjustmentAmount::Absolute(ps("7"))
                )
                .expect("absolute must succeed")
                .held(),
            ps("11")
        );
        let neg = value
            .apply_adjustment(
                AdjustmentTarget::Available,
                AdjustmentAmount::Absolute(ps("-1")),
            )
            .expect("absolute must succeed");
        assert_eq!(neg.available(), ps("-1"));
        assert_eq!(neg.held(), ps("11"));
    }

    #[test]
    fn apply_adjustment_sets_held_absolute_values() {
        let value = holdings("11", "5");

        assert_eq!(
            value
                .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Absolute(ps("7")))
                .expect("absolute must succeed")
                .held(),
            ps("7")
        );
        assert_eq!(
            value
                .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Absolute(ps("0")))
                .expect("absolute must succeed")
                .held(),
            PositionSize::ZERO
        );
        assert_eq!(
            value
                .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Absolute(ps("7")))
                .expect("absolute must succeed")
                .available(),
            ps("11")
        );
        let neg = value
            .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Absolute(ps("-1")))
            .expect("absolute must succeed");
        assert_eq!(neg.held(), ps("-1"));
        assert_eq!(neg.available(), ps("11"));
    }

    #[test]
    fn apply_adjustment_applies_available_deltas() {
        let value = holdings("5", "11");

        assert_eq!(
            value
                .apply_adjustment(
                    AdjustmentTarget::Available,
                    AdjustmentAmount::Delta(ps("3"))
                )
                .expect("delta must succeed"),
            holdings("8", "11")
        );
        assert_eq!(
            value
                .apply_adjustment(
                    AdjustmentTarget::Available,
                    AdjustmentAmount::Delta(ps("0"))
                )
                .expect("delta must succeed"),
            value
        );
        assert_eq!(
            value
                .apply_adjustment(
                    AdjustmentTarget::Available,
                    AdjustmentAmount::Delta(ps("-3"))
                )
                .expect("delta must succeed"),
            holdings("2", "11")
        );
        assert_eq!(
            value
                .apply_adjustment(
                    AdjustmentTarget::Available,
                    AdjustmentAmount::Delta(ps("-5"))
                )
                .expect("delta must succeed"),
            holdings("0", "11")
        );
        let neg = value
            .apply_adjustment(
                AdjustmentTarget::Available,
                AdjustmentAmount::Delta(ps("-6")),
            )
            .expect("delta must succeed");
        assert_eq!(neg.available(), ps("-1"));
        assert_eq!(neg.held(), ps("11"));
    }

    #[test]
    fn apply_adjustment_applies_held_deltas() {
        let value = holdings("11", "5");

        assert_eq!(
            value
                .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Delta(ps("3")))
                .expect("delta must succeed"),
            holdings("11", "8")
        );
        assert_eq!(
            value
                .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Delta(ps("0")))
                .expect("delta must succeed"),
            value
        );
        assert_eq!(
            value
                .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Delta(ps("-3")))
                .expect("delta must succeed"),
            holdings("11", "2")
        );
        assert_eq!(
            value
                .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Delta(ps("-5")))
                .expect("delta must succeed"),
            holdings("11", "0")
        );
        let neg = value
            .apply_adjustment(AdjustmentTarget::Held, AdjustmentAmount::Delta(ps("-6")))
            .expect("delta must succeed");
        assert_eq!(neg.held(), ps("-1"));
        assert_eq!(neg.available(), ps("11"));
    }

    #[test]
    fn apply_adjustment_reports_arithmetic_overflow_for_delta() {
        let value = Holdings::new(max_ps(), PositionSize::ZERO);
        let err = value
            .apply_adjustment(
                AdjustmentTarget::Available,
                AdjustmentAmount::Delta(max_ps()),
            )
            .expect_err("must overflow");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn apply_adjustment_sets_incoming_absolute_values() {
        let value = holdings("5", "11");

        let set = value
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Absolute(ps("7")),
            )
            .expect("absolute must succeed");
        assert_eq!(set.incoming(), ps("7"));
        assert_eq!(set.available(), ps("5"));
        assert_eq!(set.held(), ps("11"));

        let zero = value
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Absolute(ps("0")),
            )
            .expect("absolute must succeed");
        assert_eq!(zero.incoming(), PositionSize::ZERO);

        let neg = value
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Absolute(ps("-3")),
            )
            .expect("absolute must succeed");
        assert_eq!(neg.incoming(), ps("-3"));
        assert_eq!(neg.available(), ps("5"));
        assert_eq!(neg.held(), ps("11"));
    }

    #[test]
    fn apply_adjustment_applies_incoming_deltas() {
        let mut base = holdings("5", "11");
        // give it a non-zero incoming to start
        base = base
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Absolute(ps("10")),
            )
            .expect("seed must succeed");

        assert_eq!(
            base.apply_adjustment(AdjustmentTarget::Incoming, AdjustmentAmount::Delta(ps("3")))
                .expect("delta must succeed")
                .incoming(),
            ps("13")
        );
        assert_eq!(
            base.apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Delta(ps("-4"))
            )
            .expect("delta must succeed")
            .incoming(),
            ps("6")
        );
        let neg = base
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Delta(ps("-15")),
            )
            .expect("delta must succeed");
        assert_eq!(neg.incoming(), ps("-5"));
        assert_eq!(neg.available(), ps("5"));
        assert_eq!(neg.held(), ps("11"));
    }

    #[test]
    fn apply_adjustment_incoming_overflow_returns_error() {
        let mut value = Holdings::new(PositionSize::ZERO, PositionSize::ZERO);
        value = value
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Absolute(max_ps()),
            )
            .expect("seed must succeed");
        let err = value
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Delta(max_ps()),
            )
            .expect_err("must overflow");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn trading_operations_do_not_touch_incoming() {
        let mut base = holdings("10", "5");
        base = base
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Absolute(ps("7")),
            )
            .expect("seed must succeed");

        assert_eq!(
            base.try_hold(ps("3")).expect("must hold").incoming(),
            ps("7")
        );
        assert_eq!(
            base.release(ps("2")).expect("must release").incoming(),
            ps("7")
        );
        assert_eq!(
            base.apply_fill_outflow(ps("2"))
                .expect("must outflow")
                .incoming(),
            ps("7")
        );
        assert_eq!(
            base.apply_fill_inflow(ps("2"))
                .expect("must inflow")
                .incoming(),
            ps("7")
        );
    }

    #[test]
    fn reserve_incoming_adds_only_incoming() {
        let base = holdings("10", "5")
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(pnl("7"));
        let updated = base.reserve_incoming(ps("3")).expect("must reserve");

        assert_eq!(updated.incoming(), ps("3"));
        assert_eq!(updated.available(), ps("10"));
        assert_eq!(updated.held(), ps("5"));
        assert_eq!(updated.avg_entry_price(), Some(px("100")));
        assert_eq!(updated.realized_pnl(), Some(pnl("7")));
    }

    #[test]
    fn reserve_incoming_accumulates() {
        let base = holdings("0", "0")
            .reserve_incoming(ps("4"))
            .expect("first reserve");
        let updated = base.reserve_incoming(ps("6")).expect("second reserve");

        assert_eq!(updated.incoming(), ps("10"));
    }

    #[test]
    fn reserve_incoming_reports_arithmetic_overflow() {
        let mut value = Holdings::zero();
        value = value.reserve_incoming(max_ps()).expect("seed must succeed");
        let err = value.reserve_incoming(max_ps()).expect_err("must overflow");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn consume_incoming_subtracts_only_incoming() {
        let base = holdings("10", "5")
            .reserve_incoming(ps("8"))
            .expect("seed must succeed");
        let updated = base.consume_incoming(ps("3")).expect("must consume");

        assert_eq!(updated.incoming(), ps("5"));
        assert_eq!(updated.available(), ps("10"));
        assert_eq!(updated.held(), ps("5"));
    }

    #[test]
    fn consume_incoming_can_drive_incoming_negative() {
        let base = holdings("10", "5")
            .reserve_incoming(ps("2"))
            .expect("seed must succeed");
        let updated = base.consume_incoming(ps("5")).expect("must consume");

        assert_eq!(updated.incoming(), ps("-3"));
        assert_eq!(updated.available(), ps("10"));
        assert_eq!(updated.held(), ps("5"));
    }

    #[test]
    fn consume_incoming_reports_arithmetic_overflow() {
        // incoming - amount overflows when amount is very negative and incoming
        // is near the positive end of the value range.
        let mut value = Holdings::zero();
        value = value.reserve_incoming(max_ps()).expect("seed must succeed");
        let err = value.consume_incoming(min_ps()).expect_err("must overflow");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn incoming_operations_do_not_touch_available_held_or_pnl() {
        let base = holdings("10", "5")
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(pnl("7"));

        let reserved = base.reserve_incoming(ps("4")).expect("must reserve");
        assert_eq!(reserved.available(), ps("10"));
        assert_eq!(reserved.held(), ps("5"));
        assert_eq!(reserved.avg_entry_price(), Some(px("100")));
        assert_eq!(reserved.realized_pnl(), Some(pnl("7")));

        let consumed = reserved.consume_incoming(ps("4")).expect("must consume");
        assert_eq!(consumed.incoming(), PositionSize::ZERO);
        assert_eq!(consumed.available(), ps("10"));
        assert_eq!(consumed.held(), ps("5"));
        assert_eq!(consumed.avg_entry_price(), Some(px("100")));
        assert_eq!(consumed.realized_pnl(), Some(pnl("7")));
    }

    #[test]
    fn try_hold_spendable_ignores_incoming() {
        // Reserved incoming must never enter spendable capacity: a slot with
        // available 10 and incoming 1000 still rejects a hold above 10.
        let base = holdings("10", "0")
            .reserve_incoming(ps("1000"))
            .expect("seed must succeed");

        base.try_hold(ps("10")).expect("must hold within available");
        let err = base
            .try_hold(ps("11"))
            .expect_err("must reject over available");
        assert_eq!(
            err,
            HoldError::InsufficientAvailable {
                available: ps("10"),
                requested: ps("11"),
            }
        );
    }

    #[test]
    fn available_within_bounds_accepts_missing_bounds() {
        assert!(holdings("5", "0").available_within_bounds(None, None));
    }

    #[test]
    fn available_within_bounds_checks_lower_inclusively() {
        assert!(holdings("5", "0").available_within_bounds(Some(ps("3")), None));
        assert!(!holdings("2", "0").available_within_bounds(Some(ps("3")), None));
        assert!(holdings("3", "0").available_within_bounds(Some(ps("3")), None));
    }

    #[test]
    fn available_within_bounds_checks_upper_inclusively() {
        assert!(holdings("5", "0").available_within_bounds(None, Some(ps("7"))));
        assert!(!holdings("8", "0").available_within_bounds(None, Some(ps("7"))));
        assert!(holdings("7", "0").available_within_bounds(None, Some(ps("7"))));
    }

    #[test]
    fn available_within_bounds_checks_both_bounds() {
        assert!(holdings("5", "0").available_within_bounds(Some(ps("3")), Some(ps("7"))));
        assert!(!holdings("2", "0").available_within_bounds(Some(ps("3")), Some(ps("7"))));
        assert!(!holdings("8", "0").available_within_bounds(Some(ps("3")), Some(ps("7"))));
    }

    #[test]
    fn available_within_bounds_handles_negative_bounds() {
        assert!(holdings("0", "0").available_within_bounds(Some(ps("-3")), None));
        assert!(!holdings("0", "0").available_within_bounds(Some(ps("1")), None));
    }

    #[test]
    fn held_within_bounds_checks_inclusively() {
        let h = holdings("0", "5");
        assert!(h.held_within_bounds(None, None));
        assert!(h.held_within_bounds(Some(ps("3")), None));
        assert!(!h.held_within_bounds(Some(ps("6")), None));
        assert!(h.held_within_bounds(Some(ps("5")), None));
        assert!(h.held_within_bounds(None, Some(ps("7"))));
        assert!(!h.held_within_bounds(None, Some(ps("4"))));
        assert!(h.held_within_bounds(None, Some(ps("5"))));
        assert!(h.held_within_bounds(Some(ps("3")), Some(ps("7"))));
        assert!(!h.held_within_bounds(Some(ps("6")), Some(ps("9"))));
    }

    #[test]
    fn incoming_within_bounds_checks_inclusively() {
        let mut base = holdings("0", "0");
        base = base
            .apply_adjustment(
                AdjustmentTarget::Incoming,
                AdjustmentAmount::Absolute(ps("5")),
            )
            .expect("seed must succeed");

        assert!(base.incoming_within_bounds(None, None));
        assert!(base.incoming_within_bounds(Some(ps("3")), None));
        assert!(!base.incoming_within_bounds(Some(ps("6")), None));
        assert!(base.incoming_within_bounds(Some(ps("5")), None));
        assert!(base.incoming_within_bounds(None, Some(ps("7"))));
        assert!(!base.incoming_within_bounds(None, Some(ps("4"))));
        assert!(base.incoming_within_bounds(None, Some(ps("5"))));
        assert!(base.incoming_within_bounds(Some(ps("3")), Some(ps("7"))));
        assert!(!base.incoming_within_bounds(Some(ps("6")), Some(ps("9"))));
    }

    #[test]
    fn holdings_is_copy() {
        let original = holdings("10", "5");
        let copied = original;

        assert_eq!(copied, original);
    }

    #[test]
    fn mutating_operations_return_new_values() {
        let original = holdings("10", "5");

        let held = original.try_hold(ps("3")).expect("must hold");
        let released = original.release(ps("2")).expect("must release");
        let outflow = original.apply_fill_outflow(ps("2")).expect("must subtract");
        let inflow = original.apply_fill_inflow(ps("2")).expect("must add");

        assert_eq!(original, holdings("10", "5"));
        assert_eq!(held, holdings("7", "8"));
        assert_eq!(released, holdings("12", "3"));
        assert_eq!(outflow, holdings("10", "3"));
        assert_eq!(inflow, holdings("12", "5"));
    }

    // ── average entry price / realized PnL ─────────────────────────────────

    #[test]
    fn new_and_zero_have_no_avg_and_untracked_pnl() {
        let zero = Holdings::zero();
        assert_eq!(zero.avg_entry_price(), None);
        assert_eq!(zero.realized_pnl(), None);

        let made = Holdings::new(ps("5"), ps("3"));
        assert_eq!(made.avg_entry_price(), None);
        assert_eq!(made.realized_pnl(), None);
    }

    #[test]
    fn realize_open_from_flat_seeds_avg_and_realizes_nothing() {
        let flat = Holdings::zero();
        let (updated, realized) = flat
            .realize_position_fill(ps("10"), px("100"))
            .expect("must realize");

        assert_eq!(realized, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), Some(px("100")));
        assert_eq!(updated.realized_pnl(), Some(Pnl::ZERO));
    }

    #[test]
    fn realize_open_short_from_flat_seeds_avg() {
        let flat = Holdings::zero();
        let (updated, realized) = flat
            .realize_position_fill(ps("-4"), px("50"))
            .expect("must realize");

        assert_eq!(realized, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), Some(px("50")));
    }

    #[test]
    fn realize_zero_qty_from_flat_keeps_avg_none() {
        let flat = Holdings::zero();
        let (updated, realized) = flat
            .realize_position_fill(PositionSize::ZERO, px("100"))
            .expect("must realize");

        assert_eq!(realized, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), None);
    }

    #[test]
    fn realize_add_to_long_weights_average() {
        // owned = 10 @ 100, buy 10 more @ 200 → avg = (10*100 + 10*200)/20 = 150.
        let long = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = long
            .realize_position_fill(ps("10"), px("200"))
            .expect("must realize");

        assert_eq!(realized, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), Some(px("150")));
    }

    #[test]
    fn realize_add_to_short_weights_average() {
        // owned = -10 @ 100, sell 10 more @ 200 → avg = (-10*100 + -10*200)/-20 = 150.
        let short = Holdings::new(ps("-10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = short
            .realize_position_fill(ps("-10"), px("200"))
            .expect("must realize");

        assert_eq!(realized, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), Some(px("150")));
    }

    #[test]
    fn realize_partial_close_long_realizes_positive_when_price_above_avg() {
        // long 10 @ 100, sell 4 @ 130 → realized = (130-100)*4 = 120, avg unchanged.
        let long = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = long
            .realize_position_fill(ps("-4"), px("130"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("120")));
        assert_eq!(updated.avg_entry_price(), Some(px("100")));
        assert_eq!(updated.realized_pnl(), Some(pnl("120")));
    }

    #[test]
    fn realize_partial_close_short_realizes_positive_when_price_below_avg() {
        // short -10 @ 100, buy 4 @ 70 → realized = (70-100)*-(4) = 120, avg unchanged.
        let short = Holdings::new(ps("-10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = short
            .realize_position_fill(ps("4"), px("70"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("120")));
        assert_eq!(updated.avg_entry_price(), Some(px("100")));
    }

    #[test]
    fn realize_exact_close_long_resets_avg_to_none_and_keeps_pnl() {
        // long 10 @ 100, sell all 10 @ 130 → realized = 300, new_owned = 0 → avg None.
        let long = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = long
            .realize_position_fill(ps("-10"), px("130"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("300")));
        assert_eq!(updated.avg_entry_price(), None);
        assert_eq!(updated.realized_pnl(), Some(pnl("300")));
    }

    #[test]
    fn realize_flip_long_to_short_closes_then_reopens_at_price() {
        // long 10 @ 100, sell 15 @ 130 → close 10: realized = (130-100)*10 = 300;
        // remainder opens short -5 at 130 → avg = 130.
        let long = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = long
            .realize_position_fill(ps("-15"), px("130"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("300")));
        assert_eq!(updated.avg_entry_price(), Some(px("130")));
    }

    #[test]
    fn realize_flip_short_to_long_closes_then_reopens_at_price() {
        // short -10 @ 100, buy 15 @ 70 → close 10: realized = (70-100)*-10 = 300;
        // remainder opens long +5 at 70 → avg = 70.
        let short = Holdings::new(ps("-10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = short
            .realize_position_fill(ps("15"), px("70"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("300")));
        assert_eq!(updated.avg_entry_price(), Some(px("70")));
    }

    #[test]
    fn realize_partial_close_long_at_loss_is_negative() {
        // long 10 @ 100, sell 4 @ 80 → realized = (80-100)*4 = -80.
        let long = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (_updated, realized) = long
            .realize_position_fill(ps("-4"), px("80"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("-80")));
    }

    #[test]
    fn realize_owned_uses_available_plus_held() {
        // available 6 + held 4 = owned 10 @ 100, sell 10 @ 130 → realized 300.
        let long = Holdings::new(ps("6"), ps("4"))
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, realized) = long
            .realize_position_fill(ps("-10"), px("130"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("300")));
        assert_eq!(updated.avg_entry_price(), None);
    }

    #[test]
    fn realize_reduce_without_basis_realizes_nothing_and_keeps_no_average() {
        // owned 10 without tracking, sell 4 @ 200: tracking stays absent.
        let basis_less = Holdings::new(ps("10"), PositionSize::ZERO);
        let (updated, realized) = basis_less
            .realize_position_fill(ps("-4"), px("200"))
            .expect("must realize");

        assert_eq!(realized, None);
        assert_eq!(updated.avg_entry_price(), None);
        assert_eq!(updated.realized_pnl(), None);
    }

    #[test]
    fn realize_exact_close_without_basis_realizes_nothing() {
        // owned -10 without tracking, buy 10 @ 70: tracking stays absent.
        let basis_less = Holdings::new(ps("-10"), PositionSize::ZERO);
        let (updated, realized) = basis_less
            .realize_position_fill(ps("10"), px("70"))
            .expect("must realize");

        assert_eq!(realized, None);
        assert_eq!(updated.avg_entry_price(), None);
    }

    #[test]
    fn realize_add_without_basis_stays_basis_less() {
        // owned 10 without tracking, buy 5 @ 200: still untracked.
        let basis_less = Holdings::new(ps("10"), PositionSize::ZERO);
        let (updated, realized) = basis_less
            .realize_position_fill(ps("5"), px("200"))
            .expect("must realize");

        assert_eq!(realized, None);
        assert_eq!(updated.avg_entry_price(), None);
    }

    #[test]
    fn realize_flip_without_basis_opens_remainder_at_price() {
        // owned 10 without tracking, sell 15 @ 130: do not auto-resume even
        // though the fill flips the position.
        let basis_less = Holdings::new(ps("10"), PositionSize::ZERO);
        let (updated, realized) = basis_less
            .realize_position_fill(ps("-15"), px("130"))
            .expect("must realize");

        assert_eq!(realized, None);
        assert_eq!(updated.avg_entry_price(), None);
    }

    #[test]
    fn realize_after_rollback_to_none_stays_untracked() {
        // A slot whose realized PnL was restored to `None` by an adjustment
        // rollback (modelled via `with_realized_pnl_opt(None)`) has lost its
        // basis; a subsequent non-flat fill must short-circuit and not
        // auto-resume tracking, exactly like any other untracked slot.
        let rolled_back = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl_opt(None);
        let (updated, realized) = rolled_back
            .realize_position_fill(ps("-4"), px("130"))
            .expect("must realize");

        assert_eq!(realized, None);
        assert_eq!(updated.realized_pnl(), None);
        assert_eq!(updated.avg_entry_price(), None);
    }

    #[test]
    fn realize_accumulates_realized_pnl_across_fills() {
        let long = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(pnl("50"));
        let (updated, realized) = long
            .realize_position_fill(ps("-4"), px("130"))
            .expect("must realize");

        assert_eq!(realized, Some(pnl("120")));
        assert_eq!(updated.realized_pnl(), Some(pnl("170")));
    }

    #[test]
    fn realize_position_fill_leaves_quantities_untouched() {
        let long = Holdings::new(ps("10"), ps("2"))
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(Pnl::ZERO);
        let (updated, _realized) = long
            .realize_position_fill(ps("-4"), px("130"))
            .expect("must realize");

        assert_eq!(updated.available(), ps("10"));
        assert_eq!(updated.held(), ps("2"));
        assert_eq!(updated.incoming(), PositionSize::ZERO);
    }

    #[test]
    fn realize_position_fill_reports_overflow() {
        let long = Holdings::new(max_ps(), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("2")))
            .with_realized_pnl(Pnl::ZERO);
        // owned*avg overflows on the weighted-average branch.
        let err = long
            .realize_position_fill(max_ps(), px("2"))
            .expect_err("must overflow");

        assert_eq!(err, AdjustmentOverflowError::ArithmeticOverflow);
    }

    #[test]
    fn is_zero_requires_no_avg_and_zero_realized_pnl() {
        assert!(Holdings::zero().is_zero());

        // Realized PnL alone keeps the slot alive.
        let with_pnl = Holdings::zero().with_realized_pnl(pnl("5"));
        assert!(!with_pnl.is_zero());

        let with_zero_pnl = Holdings::zero().with_realized_pnl(Pnl::ZERO);
        assert!(with_zero_pnl.is_zero());

        // A residual average entry price alone keeps the slot alive.
        let with_avg = Holdings::zero().with_avg_entry_price(Some(px("100")));
        assert!(!with_avg.is_zero());
    }

    #[test]
    fn reservation_and_cancel_preserve_avg_and_pnl() {
        let base = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(pnl("7"));

        let held = base.try_hold(ps("4")).expect("must hold");
        assert_eq!(held.avg_entry_price(), Some(px("100")));
        assert_eq!(held.realized_pnl(), Some(pnl("7")));

        let released = held.release(ps("4")).expect("must release");
        assert_eq!(released.avg_entry_price(), Some(px("100")));
        assert_eq!(released.realized_pnl(), Some(pnl("7")));
    }

    #[test]
    fn apply_delta_rollback_reverses_quantity_deltas() {
        // Reversing the forward quantity deltas subtracts each one from the
        // current slot, leaving concurrent contributions to other fields intact.
        let slot = Holdings::new(ps("10"), ps("2"))
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(pnl("90"));
        let rolled = slot
            .apply_delta_rollback(ps("3"), ps("1"), PositionSize::ZERO)
            .expect("rollback must succeed");

        assert_eq!(rolled.available(), ps("7"));
        assert_eq!(rolled.held(), ps("1"));
        assert_eq!(rolled.incoming(), PositionSize::ZERO);
        // Average and realized PnL are not delta-reversed here; the rollback
        // path restores them from a snapshot instead.
        assert_eq!(rolled.avg_entry_price(), Some(px("100")));
        assert_eq!(rolled.realized_pnl(), Some(pnl("90")));
    }

    #[test]
    fn apply_delta_rollback_leaves_avg_and_pnl_untouched() {
        let slot = Holdings::new(ps("5"), ps("0"))
            .with_avg_entry_price(Some(px("42")))
            .with_realized_pnl(pnl("42"));
        let rolled = slot
            .apply_delta_rollback(ps("3"), PositionSize::ZERO, PositionSize::ZERO)
            .expect("rollback must succeed");

        assert_eq!(rolled.available(), ps("2"));
        assert_eq!(rolled.avg_entry_price(), Some(px("42")));
        assert_eq!(rolled.realized_pnl(), Some(pnl("42")));
    }

    #[test]
    fn with_realized_pnl_opt_restores_untracked_state() {
        // A non-flat slot whose realized PnL was force-set can be returned to the
        // untracked `None` state, exactly as a snapshot rollback would.
        let tracked = Holdings::new(ps("10"), ps("0")).with_realized_pnl(pnl("5"));
        let untracked = tracked.with_realized_pnl_opt(None);
        assert_eq!(untracked.realized_pnl(), None);
        assert_eq!(untracked.available(), ps("10"));

        let retracked = untracked.with_realized_pnl_opt(Some(pnl("-3")));
        assert_eq!(retracked.realized_pnl(), Some(pnl("-3")));
    }

    #[test]
    fn with_realized_pnl_force_sets_absolute_value() {
        let slot = Holdings::new(ps("10"), ps("0")).with_realized_pnl(pnl("7"));
        assert_eq!(
            slot.with_realized_pnl(pnl("-3")).realized_pnl(),
            Some(pnl("-3"))
        );
        // Quantities and average are untouched by the force-set.
        let with_avg = slot.with_avg_entry_price(Some(px("100")));
        let forced = with_avg.with_realized_pnl(pnl("99"));
        assert_eq!(forced.realized_pnl(), Some(pnl("99")));
        assert_eq!(forced.avg_entry_price(), Some(px("100")));
        assert_eq!(forced.available(), ps("10"));
    }

    #[test]
    fn quantity_adjustments_preserve_avg_and_pnl() {
        let base = Holdings::new(ps("10"), PositionSize::ZERO)
            .with_avg_entry_price(Some(px("100")))
            .with_realized_pnl(pnl("7"));

        let adjusted = base
            .apply_adjustment(
                AdjustmentTarget::Available,
                AdjustmentAmount::Delta(ps("3")),
            )
            .expect("must adjust");
        assert_eq!(adjusted.available(), ps("13"));
        assert_eq!(adjusted.avg_entry_price(), Some(px("100")));
        assert_eq!(adjusted.realized_pnl(), Some(pnl("7")));
    }

    #[test]
    fn realize_tracked_pnl_same_side_fill_without_avg_stays_basis_less() {
        // Degenerate state: realized PnL is Some but avg_entry_price is None
        // on a non-flat slot. A same-side add must not establish a basis and
        // must contribute 0 to the delta (nothing to weight against).
        let slot = Holdings::new(ps("10"), PositionSize::ZERO).with_realized_pnl(pnl("30"));
        assert_eq!(slot.avg_entry_price(), None);

        let (updated, delta) = slot
            .realize_position_fill(ps("5"), px("200"))
            .expect("must not overflow");

        assert_eq!(delta, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), None);
        assert_eq!(updated.realized_pnl(), Some(pnl("30")));
    }

    #[test]
    fn realize_tracked_pnl_opposite_side_fill_without_avg_stays_basis_less() {
        // Degenerate state: realized Some but avg None; opposite-side partial
        // close realizes 0 (nothing to close against) and avg stays None.
        let slot = Holdings::new(ps("10"), PositionSize::ZERO).with_realized_pnl(pnl("30"));

        let (updated, delta) = slot
            .realize_position_fill(ps("-4"), px("150"))
            .expect("must not overflow");

        assert_eq!(delta, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), None);
        assert_eq!(updated.realized_pnl(), Some(pnl("30")));
    }

    #[test]
    fn realize_tracked_pnl_flip_without_avg_seeds_avg_at_price() {
        // Degenerate state: realized Some but avg None; a flip (|sell| > |owned|)
        // seeds avg at the fill price for the new opposite position, realizing 0.
        let slot = Holdings::new(ps("5"), PositionSize::ZERO).with_realized_pnl(pnl("30"));

        let (updated, delta) = slot
            .realize_position_fill(ps("-10"), px("150"))
            .expect("must not overflow");

        assert_eq!(delta, Some(Pnl::ZERO));
        assert_eq!(updated.avg_entry_price(), Some(px("150")));
        assert_eq!(updated.realized_pnl(), Some(pnl("30")));
    }
}
