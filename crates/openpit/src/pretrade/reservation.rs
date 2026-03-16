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

use super::Lock;

/// Opaque capability object representing reserved state.
///
/// `Reservation` is the result of successful pre-trade execution. It owns the
/// commit/rollback capability for the mutations prepared by policies, and it
/// also carries the [`Lock`] produced while those mutations were built.
///
/// The lock is part of the reservation contract. It is the policy context that
/// describes what was actually locked and which values must survive beyond the
/// synchronous pre-trade phase. This matters when later reconciliation depends
/// on execution-report details, especially partial fills and terminal reports.
///
/// If a policy needs trade execution report fill details to finalize reserved
/// state, the caller must persist [`Reservation::lock`] together with the order
/// and keep it until the last execution report for that order has been
/// processed. A terminal order state alone is not sufficient if the policy also
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
/// - Keep the `Reservation` alive until the order is actually sent.
/// - Call [`Reservation::commit`] only after the venue accepted the order and
///   the reservation must become durable engine state.
/// - Call [`Reservation::rollback`] if submission fails and reserved state must
///   be reverted immediately.
/// - After commit, persist [`Reservation::lock`] if later execution-report
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
/// let engine = Engine::<OrderOperation, ()>::builder().build()?;
/// let order = OrderOperation {
///     instrument: Instrument::new(
///         Asset::new("AAPL")?,
///         Asset::new("USD")?,
///     ),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(
///         Quantity::from_str("10")?
///     ),
///     price: Some(Price::from_str("185")?),
/// };
/// let reservation = engine.start_pre_trade(order)?.execute()?;
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
pub struct Reservation {
    inner: Option<Box<dyn ReservationHandle>>,
    lock: Lock,
}

/// Internal capability interface used by [`Reservation`].
///
/// Implementations provide both finalization actions and the lock context that
/// must be exposed to the caller once reservation succeeds.
pub(crate) trait ReservationHandle {
    /// Finalizes the reservation by applying commit mutations.
    fn commit(self: Box<Self>);
    /// Finalizes the reservation by applying rollback mutations.
    fn rollback(self: Box<Self>);
    /// Returns the lock context attached to the reservation.
    fn lock(&self) -> Lock;
}

impl Reservation {
    /// Finalizes by applying commit mutations.
    pub fn commit(mut self) {
        if let Some(inner) = self.inner.take() {
            inner.commit();
        }
    }

    /// Finalizes by applying rollback mutations.
    pub fn rollback(mut self) {
        if let Some(inner) = self.inner.take() {
            inner.rollback();
        }
    }

    /// Returns the lock context attached to the reservation.
    ///
    /// Persist this value if post-trade reconciliation for the accepted order
    /// needs reservation-time policy context, such as fill-sensitive unlocking
    /// of the remaining reserved amount.
    pub fn lock(&self) -> &Lock {
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

impl Drop for Reservation {
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

    use super::{Lock, Reservation, ReservationHandle};
    use crate::param::{Asset, Price, Volume};
    use crate::pretrade::handles::ReservationHandleImpl;
    use crate::pretrade::{Mutation, RiskMutation};

    #[test]
    fn drop_without_explicit_finalize_rolls_back() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let calls_clone = Rc::clone(&calls);
        let apply = Box::new(move |mutation: &RiskMutation| {
            if let RiskMutation::SetKillSwitch { id, enabled } = mutation {
                calls_clone.borrow_mut().push((*id, *enabled));
            }
        });

        let reservation = Reservation::from_handle(Box::new(ReservationHandleImpl::new(
            vec![
                Mutation {
                    commit: RiskMutation::SetKillSwitch {
                        id: "m1",
                        enabled: true,
                    },
                    rollback: RiskMutation::SetKillSwitch {
                        id: "m1",
                        enabled: false,
                    },
                },
                Mutation {
                    commit: RiskMutation::ReserveNotional {
                        asset: Asset::new("USD").expect("asset code must be valid"),
                        amount: Volume::from_str("10").expect("volume must be valid"),
                    },
                    rollback: RiskMutation::ReserveNotional {
                        asset: Asset::new("USD").expect("asset code must be valid"),
                        amount: Volume::from_str("10").expect("volume must be valid"),
                    },
                },
            ],
            apply,
        )));

        drop(reservation);

        assert_eq!(&*calls.borrow(), &[("m1", false)]);
    }

    #[test]
    fn drop_without_explicit_finalize_can_ignore_non_kill_switch_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let calls_clone = Rc::clone(&calls);
        let apply = Box::new(move |mutation: &RiskMutation| {
            if let RiskMutation::SetKillSwitch { id, enabled } = mutation {
                calls_clone.borrow_mut().push((*id, *enabled));
            }
        });

        let reservation = Reservation::from_handle(Box::new(ReservationHandleImpl::new(
            vec![
                Mutation {
                    commit: RiskMutation::SetKillSwitch {
                        id: "m1",
                        enabled: true,
                    },
                    rollback: RiskMutation::SetKillSwitch {
                        id: "m1",
                        enabled: false,
                    },
                },
                Mutation {
                    commit: RiskMutation::ReserveNotional {
                        asset: Asset::new("USD").expect("asset code must be valid"),
                        amount: Volume::from_str("10").expect("volume must be valid"),
                    },
                    rollback: RiskMutation::ReserveNotional {
                        asset: Asset::new("USD").expect("asset code must be valid"),
                        amount: Volume::from_str("10").expect("volume must be valid"),
                    },
                },
            ],
            apply,
        )));

        drop(reservation);

        assert_eq!(&*calls.borrow(), &[("m1", false)]);
    }

    #[test]
    fn commit_is_noop_for_finalized_reservation() {
        let reservation = Reservation {
            inner: None,
            lock: Lock::default(),
        };
        reservation.commit();
    }

    #[test]
    fn rollback_is_noop_for_finalized_reservation() {
        let reservation = Reservation {
            inner: None,
            lock: Lock::default(),
        };
        reservation.rollback();
    }

    #[test]
    fn commit_with_locked_reservation_handle() {
        let reservation = Reservation::from_handle(Box::new(LockedReservationHandle {
            lock: Lock::new(None),
        }));
        reservation.commit();
    }

    #[test]
    fn lock_returns_handle_lock_with_some_price() {
        let price = Price::from_str("185").expect("price must be valid");
        let reservation = Reservation::from_handle(Box::new(LockedReservationHandle {
            lock: Lock::new(Some(price)),
        }));

        assert_eq!(reservation.lock().price(), Some(price));
    }

    #[test]
    fn lock_returns_handle_lock_with_none_price() {
        let reservation = Reservation::from_handle(Box::new(LockedReservationHandle {
            lock: Lock::new(None),
        }));

        assert_eq!(reservation.lock().price(), None);
    }

    struct LockedReservationHandle {
        lock: Lock,
    }

    impl ReservationHandle for LockedReservationHandle {
        fn commit(self: Box<Self>) {}

        fn rollback(self: Box<Self>) {}

        fn lock(&self) -> Lock {
            self.lock
        }
    }
}
