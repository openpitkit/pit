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

use openpit::param::{Fee, Pnl, Quantity, Trade};
use openpit::pretrade::Lock;
use openpit::{ExecutionReportOperation, ExecutionReportPositionImpact};

/// Financial-impact extension fields for FFI report payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinancialImpactData {
    /// Realized trading result contributed by this report.
    pub pnl: Pnl,

    /// Fee/rebate associated with this report event.
    pub fee: Fee,
}

/// Fill-details extension fields for FFI report payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillDetailsData {
    /// Actual execution trade payload.
    pub last_trade: Option<Trade>,

    /// Remaining quantity after this report.
    pub leaves_quantity: Quantity,

    /// Reservation lock payload.
    pub lock: Lock,

    /// Whether this report closes the report stream for the order.
    pub is_terminal: bool,
}

/// Position-impact extension fields for FFI report payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionImpactData {
    /// Position impact payload.
    pub value: ExecutionReportPositionImpact,
}

/// Execution report payload used by FFI integrations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionReport {
    /// Operation-identification group (`instrument + side`).
    pub operation: Option<ExecutionReportOperation>,

    /// Financial-impact group (`pnl + fee`).
    pub financial_impact: Option<FinancialImpactData>,

    /// Fill-details group (`price + quantity + terminal/client id`).
    pub fill: Option<FillDetailsData>,

    /// Derivatives position-impact group.
    pub position_impact: Option<PositionImpactData>,
}

#[cfg(test)]
mod tests {
    use super::{ExecutionReport, FillDetailsData, FinancialImpactData, PositionImpactData};
    use openpit::param::{
        AccountId, Asset, Fee, Pnl, PositionEffect, PositionSide, Price, Quantity, Side, Trade,
    };
    use openpit::pretrade::Lock;
    use openpit::Instrument;
    use openpit::{ExecutionReportOperation, ExecutionReportPositionImpact};

    #[test]
    fn execution_report_exposes_all_groups() {
        let report = ExecutionReport {
            operation: Some(ExecutionReportOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Sell,
            }),
            financial_impact: Some(FinancialImpactData {
                pnl: Pnl::from_str("-10").expect("pnl must be valid"),
                fee: Fee::from_str("1").expect("fee must be valid"),
            }),
            fill: Some(FillDetailsData {
                last_trade: Some(Trade {
                    price: Price::from_str("101").expect("price must be valid"),
                    quantity: Quantity::from_str("3").expect("quantity must be valid"),
                }),
                leaves_quantity: Quantity::from_str("1").expect("quantity must be valid"),
                lock: Lock::new(Some(Price::from_str("101").expect("price must be valid"))),
                is_terminal: true,
            }),
            position_impact: Some(PositionImpactData {
                value: ExecutionReportPositionImpact {
                    position_effect: Some(PositionEffect::Open),
                    position_side: Some(PositionSide::Long),
                },
            }),
        };

        assert_eq!(
            report.operation.expect("operation must be present").side,
            Side::Sell
        );
        assert_eq!(
            report.financial_impact.expect("impact must be present").pnl,
            Pnl::from_str("-10").expect("pnl must be valid")
        );
        assert!(
            report
                .fill
                .as_ref()
                .expect("fill must be present")
                .is_terminal
        );
        assert_eq!(
            report
                .position_impact
                .expect("position impact must be present")
                .value
                .position_effect,
            Some(PositionEffect::Open)
        );
    }

    #[test]
    fn execution_report_returns_none_for_absent_optional_groups() {
        let report = ExecutionReport {
            operation: None,
            financial_impact: None,
            fill: None,
            position_impact: None,
        };

        assert!(report.operation.is_none());
        assert!(report.financial_impact.is_none());
        assert!(report.fill.is_none());
        assert!(report.position_impact.is_none());
    }

    #[test]
    fn execution_report_handles_partial_groups() {
        let report = ExecutionReport {
            operation: None,
            financial_impact: None,
            fill: Some(FillDetailsData {
                last_trade: None,
                leaves_quantity: Quantity::from_str("1").expect("quantity must be valid"),
                lock: Lock::default(),
                is_terminal: false,
            }),
            position_impact: Some(PositionImpactData {
                value: ExecutionReportPositionImpact {
                    position_effect: Some(PositionEffect::Close),
                    position_side: None,
                },
            }),
        };

        assert!(
            !report
                .fill
                .as_ref()
                .expect("fill must be present")
                .is_terminal
        );
        assert_eq!(
            report
                .position_impact
                .expect("position impact must be present")
                .value
                .position_effect,
            Some(PositionEffect::Close)
        );
    }
}
