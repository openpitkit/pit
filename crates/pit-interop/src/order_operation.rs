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

//! Runtime wrapper for the order operation group.

use openpit::param::{AccountId, Price, Side, TradeAmount};
use openpit::{
    HasAccountId, HasInstrument, HasOrderPrice, HasSide, HasTradeAmount, Instrument,
    RequestFieldAccessError,
};

/// Populated order-operation group with individually-optional fields.
///
/// Each field is stored as [`Option`]. A `Some` value returns `Ok`; a `None`
/// required field returns `Err(RequestFieldAccessError)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedOrderOperation {
    /// Trading instrument (`underlying + settlement` asset pair).
    pub instrument: Option<Instrument>,
    /// Account identifier for the order.
    pub account_id: Option<AccountId>,
    /// Buy or sell direction.
    pub side: Option<Side>,
    /// Amount to trade (quantity or volume).
    pub trade_amount: Option<TradeAmount>,
    /// Limit price, or `None` for a market order.
    pub price: Option<Price>,
}

impl HasInstrument for PopulatedOrderOperation {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.instrument
            .as_ref()
            .ok_or_else(|| RequestFieldAccessError::new("operation.instrument"))
    }
}

impl HasAccountId for PopulatedOrderOperation {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.account_id
            .ok_or_else(|| RequestFieldAccessError::new("operation.account_id"))
    }
}

impl HasSide for PopulatedOrderOperation {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        self.side
            .ok_or_else(|| RequestFieldAccessError::new("operation.side"))
    }
}

impl HasTradeAmount for PopulatedOrderOperation {
    fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
        self.trade_amount
            .ok_or_else(|| RequestFieldAccessError::new("operation.trade_amount"))
    }
}

impl HasOrderPrice for PopulatedOrderOperation {
    fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        Ok(self.price)
    }
}

/// Runtime access to an order's operation group.
///
/// Use [`OrderOperationAccess::Populated`] when the group is present,
/// [`OrderOperationAccess::Absent`] when it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderOperationAccess {
    /// The operation group is present.
    Populated(PopulatedOrderOperation),
    /// The operation group is absent.
    Absent,
}

impl HasInstrument for OrderOperationAccess {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.instrument(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.instrument")),
        }
    }
}

impl HasAccountId for OrderOperationAccess {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.account_id(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.account_id")),
        }
    }
}

impl HasSide for OrderOperationAccess {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.side(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.side")),
        }
    }
}

impl HasTradeAmount for OrderOperationAccess {
    fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.trade_amount(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.trade_amount")),
        }
    }
}

impl HasOrderPrice for OrderOperationAccess {
    fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.price(),
            Self::Absent => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{Asset, Quantity, Side};
    use openpit::{Instrument, OrderOperation};

    fn sample_op() -> OrderOperation {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("SPX").expect("valid"),
                Asset::new("USD").expect("valid"),
            ),
            account_id: AccountId::from_u64(1),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("1").expect("valid")),
            price: None,
        }
    }

    #[test]
    fn populated_all_some_returns_ok() {
        let op = sample_op();
        let populated = PopulatedOrderOperation {
            instrument: Some(op.instrument.clone()),
            account_id: Some(op.account_id),
            side: Some(op.side),
            trade_amount: Some(op.trade_amount),
            price: op.price,
        };
        assert!(populated.instrument().is_ok());
        assert!(populated.account_id().is_ok());
        assert!(populated.side().is_ok());
        assert!(populated.trade_amount().is_ok());
        assert_eq!(populated.price().unwrap(), None);
    }

    #[test]
    fn populated_missing_required_returns_err() {
        let populated = PopulatedOrderOperation {
            instrument: None,
            account_id: None,
            side: None,
            trade_amount: None,
            price: None,
        };
        assert!(populated.instrument().is_err());
        assert!(populated.account_id().is_err());
        assert!(populated.side().is_err());
        assert!(populated.trade_amount().is_err());
        assert_eq!(populated.price().unwrap(), None);
    }

    #[test]
    fn absent_required_fields_return_err() {
        let access = OrderOperationAccess::Absent;
        assert!(access.instrument().is_err());
        assert!(access.account_id().is_err());
        assert!(access.side().is_err());
        assert!(access.trade_amount().is_err());
        assert_eq!(access.price().unwrap(), None);
    }

    #[test]
    fn populated_access_delegates_correctly() {
        let op = sample_op();
        let access = OrderOperationAccess::Populated(PopulatedOrderOperation {
            instrument: Some(op.instrument.clone()),
            account_id: Some(op.account_id),
            side: Some(op.side),
            trade_amount: Some(op.trade_amount),
            price: op.price,
        });
        assert_eq!(access.instrument().unwrap(), &op.instrument);
        assert_eq!(access.account_id().unwrap(), op.account_id);
        assert_eq!(access.side().unwrap(), op.side);
        assert_eq!(access.trade_amount().unwrap(), op.trade_amount);
        assert_eq!(access.price().unwrap(), op.price);
    }
}
