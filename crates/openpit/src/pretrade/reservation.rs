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

use super::PreTradeLock;

/// Opaque capability object representing reserved state.
///
/// `PreTradeReservation` is the result of successful pre-trade execution. It owns the
/// commit/rollback capability for the mutations prepared by policies, and it
/// also carries the [`PreTradeLock`] produced while those mutations were built.
///
/// The lock is part of the reservation contract. It is the policy context that
/// describes what was actually locked and which values must survive beyond the
/// synchronous pre-trade phase. This matters when later reconciliation depends
/// on execution-report details, especially partial fills and final reports.
///
/// If a policy needs trade execution report fill details to finalize reserved
/// state, the caller must persist [`PreTradeReservation::lock`] together with the order
/// and keep it until the last execution report for that order has been
/// processed. A final order state alone is not sufficient if the policy also
/// needs fill-by-fill data to determine how much of the reservation was truly
/// consumed and how much must be released.
///
/// Example: a policy may reserve quote notional using a pre-trade worst price.
/// When fills arrive, the engine may need that stored reservation context to
/// compute the unused remainder and unlock it correctly. If the lock is lost,
/// post-trade code no longer has the authoritative context produced by
/// pre-trade validation.
///
/// If dropped without explicit finalization, rollback is executed automatically.
///
/// # Lifecycle guidance
///
/// - Keep the `PreTradeReservation` alive until the order is actually sent.
/// - Call [`PreTradeReservation::commit`] only after the venue accepted the order and
///   the reservation must become durable engine state.
/// - Call [`PreTradeReservation::rollback`] if submission fails and reserved state must
///   be reverted immediately.
/// - After commit, persist [`PreTradeReservation::lock`] if later execution-report
///   processing depends on reservation-time policy context.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Price, Quantity, Side};
/// use openpit::{Engine, Instrument, OrderOperation};
/// use openpit::param::TradeAmount;
///
/// let engine = Engine::<OrderOperation>::builder().build()?;
/// let order = OrderOperation {
///     instrument: Instrument::new(
///         Asset::new("AAPL")?,
///         Asset::new("USD")?,
///     ),
///     account_id: openpit::param::AccountId::from_u64(99224416),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(
///         Quantity::from_str("10")?
///     ),
///     price: Some(Price::from_str("185")?),
/// };
/// let mut reservation = engine.start_pre_trade(order)?.execute()?;
/// let lock = *reservation.lock();
///
/// // Send order to venue. On success commit, on failure rollback.
/// reservation.commit(); // or reservation.rollback()
///
/// // If later reconciliation needs reservation context, persist `lock`
/// // together with the accepted order until the final execution report.
/// let _ = lock;
/// # Ok(())
/// # }
/// ```
pub struct PreTradeReservation {
    inner: Option<Box<dyn ReservationHandle>>,
    lock: PreTradeLock,
}

/// Internal capability interface used by [`PreTradeReservation`].
///
/// Implementations provide both finalization actions and the lock context that
/// must be exposed to the caller once reservation succeeds.
pub(crate) trait ReservationHandle {
    /// Finalizes the reservation by applying commit mutations.
    fn commit(self: Box<Self>);
    /// Finalizes the reservation by applying rollback mutations.
    fn rollback(self: Box<Self>);
    /// Returns the lock context attached to the reservation.
    fn lock(&self) -> PreTradeLock;
}

impl PreTradeReservation {
    /// Finalizes by applying commit mutations.
    /// Panics if the reservation has already been consumed.
    pub fn commit(&mut self) {
        self.inner
            .take()
            .expect("pre-trade reservation already consumed")
            .commit();
    }

    /// Finalizes by applying rollback mutations.
    /// Does nothing if the reservation has already been consumed.
    pub fn rollback(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.rollback();
        }
    }

    /// Returns the lock context attached to the reservation.
    ///
    /// Persist this value if post-trade reconciliation for the accepted order
    /// needs reservation-time policy context, such as fill-sensitive unlocking
    /// of the remaining reserved amount.
    pub fn lock(&self) -> &PreTradeLock {
        &self.lock
    }

    pub(crate) fn from_handle(inner: Box<dyn ReservationHandle>) -> Self {
        let lock = inner.lock();
        Self {
            inner: Some(inner),
            lock,
        }
    }
}

impl Drop for PreTradeReservation {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.rollback();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{PreTradeLock, PreTradeReservation, ReservationHandle};
    use crate::param::Price;
    use crate::pretrade::handle::ReservationHandleImpl;
    use crate::{Mutation, Mutations};

    fn noop_action() {}

    #[test]
    fn drop_without_explicit_finalize_rolls_back() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        let r1 = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            r1.borrow_mut().push("m1");
        }));
        let r2 = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            r2.borrow_mut().push("m2");
        }));

        let reservation =
            PreTradeReservation::from_handle(Box::new(ReservationHandleImpl::new(mutations)));

        drop(reservation);

        assert_eq!(&*calls.borrow(), &["m2", "m1"]);
    }

    #[test]
    fn drop_without_explicit_finalize_can_ignore_non_kill_switch_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        let rollback_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            rollback_calls.borrow_mut().push("rollback");
        }));
        mutations.push(Mutation::new(noop_action, noop_action));

        let reservation =
            PreTradeReservation::from_handle(Box::new(ReservationHandleImpl::new(mutations)));

        drop(reservation);

        assert_eq!(&*calls.borrow(), &["rollback"]);
    }

    #[test]
    #[should_panic(expected = "pre-trade reservation already consumed")]
    fn commit_panics_for_finalized_reservation() {
        let mut reservation = PreTradeReservation {
            inner: None,
            lock: PreTradeLock::default(),
        };
        reservation.commit();
    }

    #[test]
    fn rollback_is_noop_for_finalized_reservation() {
        let mut reservation = PreTradeReservation {
            inner: None,
            lock: PreTradeLock::default(),
        };
        reservation.rollback();
    }

    #[test]
    fn commit_with_locked_reservation_handle() {
        let mut reservation = PreTradeReservation::from_handle(Box::new(LockedReservationHandle {
            lock: PreTradeLock::new(None),
        }));
        reservation.commit();
    }

    #[test]
    fn lock_returns_handle_lock_with_some_price() {
        let price = Price::from_str("185").expect("price must be valid");
        let reservation = PreTradeReservation::from_handle(Box::new(LockedReservationHandle {
            lock: PreTradeLock::new(Some(price)),
        }));

        assert_eq!(reservation.lock().price(), Some(price));
    }

    #[test]
    fn lock_returns_handle_lock_with_none_price() {
        let reservation = PreTradeReservation::from_handle(Box::new(LockedReservationHandle {
            lock: PreTradeLock::new(None),
        }));

        assert_eq!(reservation.lock().price(), None);
    }

    #[test]
    fn commit_executes_commit_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        let commit_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(
            move || {
                commit_calls.borrow_mut().push("commit");
            },
            noop_action,
        ));

        let mut reservation =
            PreTradeReservation::from_handle(Box::new(ReservationHandleImpl::new(mutations)));
        reservation.commit();

        assert_eq!(&*calls.borrow(), &["commit"]);
    }

    #[test]
    fn rollback_executes_rollback_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        let rollback_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            rollback_calls.borrow_mut().push("rollback");
        }));

        let mut reservation =
            PreTradeReservation::from_handle(Box::new(ReservationHandleImpl::new(mutations)));
        reservation.rollback();

        assert_eq!(&*calls.borrow(), &["rollback"]);
    }

    struct LockedReservationHandle {
        lock: PreTradeLock,
    }

    impl ReservationHandle for LockedReservationHandle {
        fn commit(self: Box<Self>) {}

        fn rollback(self: Box<Self>) {}

        fn lock(&self) -> PreTradeLock {
            self.lock
        }
    }
}
