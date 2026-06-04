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

use crate::param::{AdjustmentAmount, PositionSize};

use super::error::{AdjustmentOverflowError, HoldError};

/// Triple of `available`, `held`, and `incoming` quantities for one asset slot.
///
/// `available` is free to be locked by new pre-trade reservations. `held`
/// is locked by pending reservations and is released back to `available`
/// on cancel or consumed on fill. `incoming` tracks expected future inflows
/// not yet settled and is managed exclusively through account adjustments.
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
    /// Returns a holdings with all fields at zero.
    pub fn zero() -> Self {
        Self {
            available: PositionSize::ZERO,
            held: PositionSize::ZERO,
            incoming: PositionSize::ZERO,
        }
    }

    /// Builds a holdings from available and held; incoming is set to zero.
    pub fn new(available: PositionSize, held: PositionSize) -> Self {
        Self {
            available,
            held,
            incoming: PositionSize::ZERO,
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
            available,
            held,
            incoming: self.incoming,
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
            available,
            held,
            incoming: self.incoming,
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
            available: self.available,
            held,
            incoming: self.incoming,
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
            available,
            held: self.held,
            incoming: self.incoming,
        })
    }

    /// Subtracts per-field deltas from the current slot in one atomic step.
    ///
    /// Intended for delta-based rollback of a prior `apply_adjustment` call:
    /// pass the deltas that were applied forward, and this method reverses them
    /// by subtracting each one from the corresponding field.
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
            available,
            held,
            incoming,
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
        Ok(match (target, amount) {
            (AdjustmentTarget::Available, AdjustmentAmount::Absolute(v)) => Self {
                available: v,
                held: self.held,
                incoming: self.incoming,
            },
            (AdjustmentTarget::Held, AdjustmentAmount::Absolute(v)) => Self {
                available: self.available,
                held: v,
                incoming: self.incoming,
            },
            (AdjustmentTarget::Incoming, AdjustmentAmount::Absolute(v)) => Self {
                available: self.available,
                held: self.held,
                incoming: v,
            },
            (AdjustmentTarget::Available, AdjustmentAmount::Delta(d)) => Self {
                available: self
                    .available
                    .checked_add(d)
                    .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?,
                held: self.held,
                incoming: self.incoming,
            },
            (AdjustmentTarget::Held, AdjustmentAmount::Delta(d)) => Self {
                available: self.available,
                held: self
                    .held
                    .checked_add(d)
                    .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?,
                incoming: self.incoming,
            },
            (AdjustmentTarget::Incoming, AdjustmentAmount::Delta(d)) => Self {
                available: self.available,
                held: self.held,
                incoming: self
                    .incoming
                    .checked_add(d)
                    .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?,
            },
        })
    }

    /// Returns `true` if all fields are zero.
    pub fn is_zero(&self) -> bool {
        self.available.is_zero() && self.held.is_zero() && self.incoming.is_zero()
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

    use crate::param::{AdjustmentAmount, PositionSize};

    use super::super::error::{AdjustmentOverflowError, HoldError};
    use super::{AdjustmentTarget, Holdings};

    fn ps(value: &str) -> PositionSize {
        PositionSize::from_str(value).expect("position size literal must be valid")
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
}
