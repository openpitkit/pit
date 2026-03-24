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

/// Immutable request data available during main-stage pre-trade checks.
///
/// The engine passes this context into [`crate::pretrade::Policy`] so policies
/// can inspect the original order without taking ownership of it. The context
/// cannot be constructed directly by user code.
pub struct Context<'a, O> {
    order: &'a O,
}

impl<'a, O> Context<'a, O> {
    /// Returns the original order passed to `start_pre_trade`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openpit::{HasInstrument, OrderOperation};
    /// use openpit::param::{Asset, Side, TradeAmount, Quantity};
    /// use openpit::pretrade::{Context, Mutations, Policy, Reject};
    ///
    /// struct InspectPolicy;
    ///
    /// impl<O, R> Policy<O, R> for InspectPolicy
    /// where
    ///     O: HasInstrument,
    /// {
    ///     fn name(&self) -> &'static str { "InspectPolicy" }
    ///
    ///     fn perform_pre_trade_check(
    ///         &self,
    ///         ctx: &Context<'_, O>,
    ///         _mutations: &mut Mutations,
    ///         _rejects: &mut Vec<Reject>,
    ///     ) {
    ///         let _ = ctx.order().instrument().expect("instrument must be present").settlement_asset();
    ///     }
    ///
    ///     fn apply_execution_report(&self, _report: &R) -> bool {
    ///         false
    ///     }
    /// }
    ///
    /// let order = OrderOperation {
    ///     instrument: openpit::Instrument::new(
    ///         Asset::new("AAPL")?,
    ///         Asset::new("USD")?,
    ///     ),
    ///     account_id: openpit::param::AccountId::from_u64(99224416),
    ///     side: Side::Buy,
    ///     trade_amount: TradeAmount::Quantity(
    ///         Quantity::from_str("1")?,
    ///     ),
    ///     price: None,
    /// };
    /// let _ = order;
    /// # Ok(())
    /// # }
    /// ```
    pub fn order(&self) -> &O {
        self.order
    }

    pub(crate) fn new(order: &'a O) -> Self {
        Self { order }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::OrderOperation;
    use crate::param::{AccountId, Asset, Quantity, Side, TradeAmount};

    use super::Context;

    #[test]
    fn stores_order_reference() {
        let order = OrderOperation {
            instrument: crate::Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("10").expect("quantity must be valid"),
            ),
            price: None,
        };

        let ctx = Context::new(&order);

        assert_eq!(ctx.order().trade_amount, order.trade_amount);
    }
}
