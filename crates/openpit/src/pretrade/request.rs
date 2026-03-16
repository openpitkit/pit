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

use super::reject::Rejects;
use super::reservation::Reservation;

/// Opaque deferred pre-trade capability produced by `start_pre_trade`.
///
/// Created by [`crate::Engine::start_pre_trade`] after start-stage policies pass.
/// Holds a single-use capability: once [`Request::execute`] is called, the
/// object is consumed and cannot be reused.
///
/// The capability is single-use: once [`Request::execute`] is called, the
/// request is consumed and cannot be reused.
///
/// The request does not expose the underlying order to the caller;
/// those values are visible only to the engine and the policies.
pub struct Request<O> {
    inner: Box<dyn RequestHandle<O>>,
}

/// Internal capability interface used by [`Request`].
pub(crate) trait RequestHandle<O> {
    /// Executes deferred main-stage pre-trade checks.
    fn execute(self: Box<Self>) -> Result<Reservation, Rejects>;
}

impl<O> std::fmt::Debug for Request<O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request").finish_non_exhaustive()
    }
}

impl<O> Request<O> {
    /// Executes deferred pre-trade checks.
    ///
    /// The call is single-use by type semantics because `self` is consumed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openpit::param::{Asset, Price, Quantity, Side};
    /// use openpit::{Engine, Instrument};
    /// use openpit::OrderOperation;
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
    /// let request = engine.start_pre_trade(order)?;
    /// let reservation = request.execute()?;
    /// reservation.commit();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Rejects`] when any main-stage policy rejects the order.
    /// All policies run before returning, and all registered mutations are
    /// rolled back in reverse order.
    pub fn execute(self) -> Result<Reservation, Rejects> {
        self.inner.execute()
    }

    pub(crate) fn from_handle(inner: Box<dyn RequestHandle<O>>) -> Self {
        Self { inner }
    }
}

#[cfg(test)]
mod tests {
    use super::Request;
    use crate::pretrade::handles::RequestHandleImpl;
    use crate::pretrade::Reservation;

    #[test]
    fn execute_consumes_request_and_delegates_to_handle() {
        let request =
            Request::<()>::from_handle(Box::new(RequestHandleImpl::new(Box::new(|| {
                Ok(Reservation::from_handle(Box::new(NoopReservationHandle)))
            }))));

        let reservation = request.execute().expect("request execution must succeed");
        reservation.commit();
    }

    #[test]
    fn execute_can_finalize_returned_reservation_with_rollback() {
        let request =
            Request::<()>::from_handle(Box::new(RequestHandleImpl::new(Box::new(|| {
                Ok(Reservation::from_handle(Box::new(NoopReservationHandle)))
            }))));

        let reservation = request.execute().expect("request execution must succeed");
        reservation.rollback();
    }

    #[test]
    fn debug_format_is_opaque() {
        let request =
            Request::<()>::from_handle(Box::new(RequestHandleImpl::new(Box::new(|| {
                Ok(Reservation::from_handle(Box::new(NoopReservationHandle)))
            }))));
        assert!(format!("{request:?}").contains("Request"));
        drop(request.execute());
    }

    struct NoopReservationHandle;

    impl crate::pretrade::reservation::ReservationHandle for NoopReservationHandle {
        fn commit(self: Box<Self>) {}

        fn rollback(self: Box<Self>) {}

        fn lock(&self) -> crate::pretrade::Lock {
            crate::pretrade::Lock::default()
        }
    }
}
