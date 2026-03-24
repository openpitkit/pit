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

use openpit::{
    AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
    AccountAdjustmentPositionOperation,
};

/// Operation payload used by account-adjustment FFI records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountAdjustmentOperation {
    /// Physical-balance adjustment operation.
    Balance(AccountAdjustmentBalanceOperation),
    /// Derivatives-like position adjustment operation.
    Position(AccountAdjustmentPositionOperation),
}

/// Account-adjustment payload used by FFI integrations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAdjustment {
    /// Operation group (`balance` or `position`).
    pub operation: Option<AccountAdjustmentOperation>,

    /// Amount group (`total + reserved + pending`).
    pub amount: Option<AccountAdjustmentAmount>,

    /// Bounds group (`*_upper_bound + *_lower_bound`).
    pub bounds: Option<AccountAdjustmentBounds>,
}

#[cfg(test)]
mod tests {
    use super::{AccountAdjustment, AccountAdjustmentOperation};
    use openpit::param::{AccountId, AdjustmentAmount, Asset, PositionMode, PositionSize, Price};
    use openpit::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        AccountAdjustmentPositionOperation, Instrument,
    };

    #[test]
    fn absent_optional_groups_remain_none() {
        let payload = AccountAdjustment {
            operation: None,
            amount: None,
            bounds: None,
        };

        assert!(payload.operation.is_none());
        assert!(payload.amount.is_none());
        assert!(payload.bounds.is_none());
    }

    #[test]
    fn partial_payload_works() {
        let payload = AccountAdjustment {
            operation: None,
            amount: Some(AccountAdjustmentAmount {
                total: Some(AdjustmentAmount::Absolute(
                    PositionSize::from_str("5").expect("must be valid"),
                )),
                reserved: None,
                pending: Some(AdjustmentAmount::Delta(
                    PositionSize::from_str("1").expect("must be valid"),
                )),
            }),
            bounds: Some(AccountAdjustmentBounds {
                total_upper_bound: Some(PositionSize::from_str("10").expect("must be valid")),
                total_lower_bound: None,
                reserved_upper_bound: None,
                reserved_lower_bound: None,
                pending_upper_bound: None,
                pending_lower_bound: None,
            }),
        };

        assert!(payload.amount.is_some());
        assert!(payload.bounds.is_some());
    }

    #[test]
    fn account_adjustment_balance_full_payload_roundtrip() {
        let payload = AccountAdjustment {
            operation: Some(AccountAdjustmentOperation::Balance(
                AccountAdjustmentBalanceOperation {
                    account_id: AccountId::from_u64(99),
                    asset: Asset::new("CHF").expect("asset code must be valid"),
                    average_entry_price: None,
                },
            )),
            amount: Some(AccountAdjustmentAmount {
                total: Some(AdjustmentAmount::Delta(
                    PositionSize::from_str("2").expect("must be valid"),
                )),
                reserved: Some(AdjustmentAmount::Absolute(
                    PositionSize::from_str("3").expect("must be valid"),
                )),
                pending: None,
            }),
            bounds: Some(AccountAdjustmentBounds {
                total_upper_bound: Some(PositionSize::from_str("10").expect("must be valid")),
                total_lower_bound: Some(PositionSize::from_str("-10").expect("must be valid")),
                reserved_upper_bound: Some(PositionSize::from_str("5").expect("must be valid")),
                reserved_lower_bound: Some(PositionSize::from_str("0").expect("must be valid")),
                pending_upper_bound: Some(PositionSize::from_str("4").expect("must be valid")),
                pending_lower_bound: Some(PositionSize::from_str("-4").expect("must be valid")),
            }),
        };

        let clone = payload.clone();
        assert_eq!(payload, clone);
        assert_ne!(format!("{payload:?}"), "");

        let operation = payload.operation.expect("operation must be present");
        assert_eq!(
            operation,
            AccountAdjustmentOperation::Balance(AccountAdjustmentBalanceOperation {
                account_id: AccountId::from_u64(99),
                asset: Asset::new("CHF").expect("asset code must be valid"),
                average_entry_price: None,
            })
        );

        let amount = payload.amount.expect("amount must be present");
        assert!(amount.total.is_some());
        assert!(amount.reserved.is_some());
        assert!(amount.pending.is_none());

        let bounds = payload.bounds.expect("bounds must be present");
        assert!(bounds.total_upper_bound.is_some());
        assert!(bounds.total_lower_bound.is_some());
        assert!(bounds.reserved_upper_bound.is_some());
        assert!(bounds.reserved_lower_bound.is_some());
        assert!(bounds.pending_upper_bound.is_some());
        assert!(bounds.pending_lower_bound.is_some());
    }

    #[test]
    fn account_adjustment_position_full_payload_roundtrip() {
        let payload = AccountAdjustment {
            operation: Some(AccountAdjustmentOperation::Position(
                AccountAdjustmentPositionOperation {
                    account_id: AccountId::from_u64(100),
                    instrument: Instrument::new(
                        Asset::new("XRP").expect("asset code must be valid"),
                        Asset::new("USD").expect("asset code must be valid"),
                    ),
                    collateral_asset: Asset::new("USDT").expect("asset code must be valid"),
                    average_entry_price: Price::from_str("0.5").expect("must be valid"),
                    mode: PositionMode::Hedged,
                    leverage: None,
                },
            )),
            amount: Some(AccountAdjustmentAmount {
                total: None,
                reserved: None,
                pending: Some(AdjustmentAmount::Delta(
                    PositionSize::from_str("1").expect("must be valid"),
                )),
            }),
            bounds: Some(AccountAdjustmentBounds {
                total_upper_bound: None,
                total_lower_bound: None,
                reserved_upper_bound: None,
                reserved_lower_bound: None,
                pending_upper_bound: Some(PositionSize::from_str("2").expect("must be valid")),
                pending_lower_bound: Some(PositionSize::from_str("-2").expect("must be valid")),
            }),
        };

        let clone = payload.clone();
        assert_eq!(payload, clone);
        assert_ne!(format!("{payload:?}"), "");

        let operation = payload.operation.expect("operation must be present");
        assert_eq!(
            operation,
            AccountAdjustmentOperation::Position(AccountAdjustmentPositionOperation {
                account_id: AccountId::from_u64(100),
                instrument: Instrument::new(
                    Asset::new("XRP").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                collateral_asset: Asset::new("USDT").expect("asset code must be valid"),
                average_entry_price: Price::from_str("0.5").expect("must be valid"),
                mode: PositionMode::Hedged,
                leverage: None,
            })
        );

        let amount = payload.amount.expect("amount must be present");
        assert!(amount.total.is_none());
        assert!(amount.reserved.is_none());
        assert!(amount.pending.is_some());

        let bounds = payload.bounds.expect("bounds must be present");
        assert!(bounds.total_upper_bound.is_none());
        assert!(bounds.total_lower_bound.is_none());
        assert!(bounds.reserved_upper_bound.is_none());
        assert!(bounds.reserved_lower_bound.is_none());
        assert!(bounds.pending_upper_bound.is_some());
        assert!(bounds.pending_lower_bound.is_some());
    }
}
