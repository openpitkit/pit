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

use super::{AccountBlock, PreTradeLock};
use crate::core::account_outcome::AccountAdjustmentOutcome;

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
/// use openpit::pretrade::policies::OrderValidationPolicy;
/// let engine = Engine::builder::<OrderOperation, (), ()>()
///     .no_sync()
///     .pre_trade(OrderValidationPolicy::new())
///     .build()?;
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
/// let lock = reservation.lock().clone();
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
    account_block: Option<AccountBlock>,
    account_adjustments: Vec<AccountAdjustmentOutcome>,
    lock: PreTradeLock,
    inner: Option<Box<dyn ReservationHandle>>,
}

/// Internal capability interface used by [`PreTradeReservation`].
///
/// Provides only finalization: commit or rollback. Lock context and account
/// adjustments are passed directly to [`PreTradeReservation::from_handle`] so
/// they are not stored twice.
pub(crate) trait ReservationHandle {
    /// Finalizes the reservation by applying commit mutations.
    fn commit(self: Box<Self>);
    /// Finalizes the reservation by applying rollback mutations.
    fn rollback(self: Box<Self>);
}

impl PreTradeReservation {
    /// Finalizes by applying commit mutations.
    ///
    /// The reservation owns its commit/rollback capability exactly once.
    /// After `commit` returns, the reservation is consumed and any further
    /// finalization call other than [`Self::rollback`] (which is a no-op
    /// after consumption) is a programmer error.
    ///
    /// # Panics
    ///
    /// Panics with `"pre-trade reservation already consumed"` if `commit`
    /// is called when the reservation has already been finalized. This
    /// happens when:
    ///
    /// - `commit` is called twice on the same reservation;
    /// - `commit` is called after [`Self::rollback`] has consumed the
    ///   reservation;
    /// - `commit` is called on a reservation whose
    ///   [`Drop`](std::ops::Drop) glue has already run the implicit
    ///   rollback (only reachable from raw FFI paths that retain a
    ///   pointer past the Rust scope).
    ///
    /// The panic is the API contract: each reservation must be finalized
    /// at most once and the caller is responsible for tracking ownership.
    /// Language bindings (Python, Go, C) that expose this method MUST
    /// wrap the call in [`std::panic::catch_unwind`] and translate the
    /// resulting unwind into the host language's idiomatic error type
    /// (Python: `RuntimeError` or a custom exception; Go: returned
    /// `error`; C: an out-parameter error code). Letting the panic
    /// propagate across the language boundary is undefined behaviour.
    pub fn commit(&mut self) {
        self.inner
            .take()
            .expect("pre-trade reservation already consumed")
            .commit();
    }

    /// Finalizes by applying rollback mutations.
    ///
    /// Unlike [`Self::commit`], calling `rollback` after the reservation
    /// has already been finalized is a no-op rather than a panic. This
    /// asymmetry is intentional: the destructor implicitly performs the
    /// same rollback when a reservation is dropped without explicit
    /// finalization, and a subsequent explicit `rollback` call by the
    /// owner must be safe so callers can defensively roll back without
    /// tracking whether they have already done so.
    ///
    /// # Panics
    ///
    /// This method does not panic on its own. Panics can only originate
    /// from inside individual rollback mutation closures registered by
    /// policies (for example, a deliberate `unreachable!` in a closure).
    /// The reservation API itself imposes no panic on double-rollback or
    /// rollback-after-commit; both are silent no-ops.
    ///
    /// Language bindings (Python, Go, C) that expose this method should
    /// still wrap the call in [`std::panic::catch_unwind`] because a
    /// misbehaving policy mutation closure can still unwind. Letting the
    /// panic propagate across the language boundary is undefined
    /// behaviour.
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

    /// Returns account position modifications grouped by [`super::PolicyGroupId`].
    ///
    /// Contains zero or more entries. Policies that share a group tag contribute
    /// to the same entry; policies that report nothing do not create an entry.
    /// Order within a group follows policy registration order.
    pub fn account_adjustments(&self) -> &[AccountAdjustmentOutcome] {
        &self.account_adjustments
    }

    /// Returns the winning account block produced by this reservation's pipeline.
    ///
    /// Regular accepted reservations carry none because an account block is an
    /// enforcing reject. A drop-copy reservation may carry the first block
    /// derived from an account-scoped reject that was deliberately not enforced,
    /// even when the account registry already contains an earlier block.
    pub fn account_block(&self) -> Option<&AccountBlock> {
        self.account_block.as_ref()
    }

    pub(crate) fn from_handle(
        inner: Box<dyn ReservationHandle>,
        lock: PreTradeLock,
        account_adjustments: Vec<AccountAdjustmentOutcome>,
    ) -> Self {
        Self {
            account_block: None,
            account_adjustments,
            lock,
            inner: Some(inner),
        }
    }

    pub(crate) fn from_handle_with_account_block(
        inner: Box<dyn ReservationHandle>,
        lock: PreTradeLock,
        account_adjustments: Vec<AccountAdjustmentOutcome>,
        account_block: Option<AccountBlock>,
    ) -> Self {
        Self {
            account_block,
            account_adjustments,
            lock,
            inner: Some(inner),
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
    use crate::core::DEFAULT_POLICY_GROUP_ID;
    use crate::param::Price;
    use crate::pretrade::handle::ReservationHandleImpl;
    use crate::{Mutation, Mutations};

    fn noop_action() {}

    #[test]
    fn drop_without_explicit_finalize_rolls_back() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(2);
        let r1 = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            r1.borrow_mut().push("m1");
        }));
        let r2 = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            r2.borrow_mut().push("m2");
        }));

        let reservation = PreTradeReservation::from_handle(
            Box::new(ReservationHandleImpl::new(mutations)),
            PreTradeLock::default(),
            Vec::new(),
        );

        drop(reservation);

        assert_eq!(&*calls.borrow(), &["m2", "m1"]);
    }

    #[test]
    fn drop_without_explicit_finalize_can_ignore_non_kill_switch_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(2);
        let rollback_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            rollback_calls.borrow_mut().push("rollback");
        }));
        mutations.push(Mutation::new(noop_action, noop_action));

        let reservation = PreTradeReservation::from_handle(
            Box::new(ReservationHandleImpl::new(mutations)),
            PreTradeLock::default(),
            Vec::new(),
        );

        drop(reservation);

        assert_eq!(&*calls.borrow(), &["rollback"]);
    }

    #[test]
    #[should_panic(expected = "pre-trade reservation already consumed")]
    fn commit_panics_for_finalized_reservation() {
        let mut reservation = PreTradeReservation {
            account_block: None,
            account_adjustments: Vec::new(),
            lock: PreTradeLock::default(),
            inner: None,
        };
        reservation.commit();
    }

    #[test]
    fn rollback_is_noop_for_finalized_reservation() {
        let mut reservation = PreTradeReservation {
            account_block: None,
            account_adjustments: Vec::new(),
            lock: PreTradeLock::default(),
            inner: None,
        };
        reservation.rollback();
    }

    #[test]
    fn commit_with_locked_reservation_handle() {
        let mut reservation = PreTradeReservation::from_handle(
            Box::new(LockedReservationHandle),
            PreTradeLock::new(),
            Vec::new(),
        );
        reservation.commit();
    }

    #[test]
    fn lock_returns_reservation_lock_with_some_price() {
        let price = Price::from_str("185").expect("price must be valid");
        let reservation = PreTradeReservation::from_handle(
            Box::new(LockedReservationHandle),
            PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price)]),
            Vec::new(),
        );

        let prices: Vec<_> = reservation
            .lock()
            .prices_of(DEFAULT_POLICY_GROUP_ID)
            .collect();
        assert_eq!(prices, vec![price]);
    }

    #[test]
    fn lock_returns_reservation_lock_with_none_price() {
        let reservation = PreTradeReservation::from_handle(
            Box::new(LockedReservationHandle),
            PreTradeLock::new(),
            Vec::new(),
        );

        assert!(reservation
            .lock()
            .prices_of(DEFAULT_POLICY_GROUP_ID)
            .next()
            .is_none());
    }

    #[test]
    fn commit_executes_commit_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(1);
        let commit_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(
            move || {
                commit_calls.borrow_mut().push("commit");
            },
            noop_action,
        ));

        let mut reservation = PreTradeReservation::from_handle(
            Box::new(ReservationHandleImpl::new(mutations)),
            PreTradeLock::default(),
            Vec::new(),
        );
        reservation.commit();

        assert_eq!(&*calls.borrow(), &["commit"]);
    }

    #[test]
    fn rollback_executes_rollback_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(1);
        let rollback_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            rollback_calls.borrow_mut().push("rollback");
        }));

        let mut reservation = PreTradeReservation::from_handle(
            Box::new(ReservationHandleImpl::new(mutations)),
            PreTradeLock::default(),
            Vec::new(),
        );
        reservation.rollback();

        assert_eq!(&*calls.borrow(), &["rollback"]);
    }

    struct LockedReservationHandle;

    impl ReservationHandle for LockedReservationHandle {
        fn commit(self: Box<Self>) {}

        fn rollback(self: Box<Self>) {}
    }
}
