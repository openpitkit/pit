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

#![allow(clippy::missing_safety_doc, clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::c_void;

use openpit::param::{AccountId, Quantity, Trade};
use openpit::pretrade::PreTradeLock;
use openpit::ExecutionReportPositionImpact;
use openpit::{HasFee, HasInstrument, HasPnl, RequestFieldAccessError};
use pit_interop::{
    ExecutionReportOperationAccess, FinancialImpactAccess, PopulatedExecutionReportOperation,
    PopulatedFinancialImpact,
};

use crate::define_optional;
use crate::instrument::{import_instrument, PitInstrument};
use crate::param::{
    export_position_effect, export_position_side, export_side, import_position_effect,
    import_position_side, import_side, PitParamAccountIdOptional, PitParamFee, PitParamFeeOptional,
    PitParamPnl, PitParamPnlOptional, PitParamPositionEffect, PitParamPositionSide, PitParamPrice,
    PitParamPriceOptional, PitParamQuantity, PitParamQuantityOptional, PitParamSide,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Populated operation-identification group for an execution report.
pub struct PitExecutionReportOperation {
    /// Trading instrument (`underlying + settlement` asset pair).
    pub instrument: PitInstrument,
    /// Account identifier associated with the report.
    pub account_id: PitParamAccountIdOptional,
    /// Buy or sell direction of the affected order or trade.
    pub side: PitParamSide,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Populated financial-impact group for an execution report.
pub struct PitFinancialImpact {
    /// Profit-and-loss contribution carried by this report.
    pub pnl: PitParamPnlOptional,
    /// Fee or rebate contribution carried by this report.
    pub fee: PitParamFeeOptional,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Fill trade payload (`price + quantity`) for execution reports.
pub struct PitExecutionReportTrade {
    /// Trade price.
    pub price: PitParamPrice,
    /// Trade quantity.
    pub quantity: PitParamQuantity,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Populated fill-details group for an execution report.
pub struct PitExecutionReportFill {
    /// Optional latest trade payload.
    pub last_trade: PitExecutionReportTradeOptional,
    /// Remaining quantity after applying this report.
    pub leaves_quantity: PitParamQuantityOptional,
    /// Optional lock price associated with the report.
    pub lock_price: PitParamPriceOptional,
    /// Whether this report ends the report stream for the order.
    pub is_terminal: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Populated position-impact group for an execution report.
pub struct PitExecutionReportPositionImpact {
    /// Whether exposure is opened or closed.
    pub position_effect: PitParamPositionEffect,
    /// Impacted side (long or short).
    pub position_side: PitParamPositionSide,
}

define_optional!(
    optional = PitExecutionReportOperationOptional,
    value = PitExecutionReportOperation
);
define_optional!(
    optional = PitFinancialImpactOptional,
    value = PitFinancialImpact
);
define_optional!(
    optional = PitExecutionReportTradeOptional,
    value = PitExecutionReportTrade
);
define_optional!(
    optional = PitExecutionReportFillOptional,
    value = PitExecutionReportFill
);
define_optional!(
    optional = PitExecutionReportPositionImpactOptional,
    value = PitExecutionReportPositionImpact
);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Full caller-owned execution-report payload.
pub struct PitExecutionReport {
    /// Optional operation-identification group.
    pub operation: PitExecutionReportOperationOptional,
    /// Optional financial-impact group.
    pub financial_impact: PitFinancialImpactOptional,
    /// Optional fill-details group.
    pub fill: PitExecutionReportFillOptional,
    /// Optional position-impact group.
    pub position_impact: PitExecutionReportPositionImpactOptional,
    /// Opaque caller-defined token.
    ///
    /// The SDK never inspects, dereferences, or frees this value. Its meaning,
    /// lifetime, and thread-safety are the caller's responsibility. `0` / null
    /// means "not set". See the project Threading Contract for the full lifetime
    /// model.
    ///
    /// The token is preserved unchanged across every engine callback that
    /// receives the carrying value, including policy callbacks and adjustment
    /// callbacks.
    pub user_data: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Aggregated post-trade processing result.
pub struct PitPretradePostTradeResult {
    /// Whether the report triggered some kill-switch policy.
    pub kill_switch_triggered: bool,
}

fn import_operation(
    value: PitExecutionReportOperationOptional,
) -> Result<ExecutionReportOperationAccess, String> {
    if !value.is_set {
        return Ok(ExecutionReportOperationAccess::Absent);
    }

    Ok(ExecutionReportOperationAccess::Populated(
        PopulatedExecutionReportOperation {
            instrument: import_instrument(&value.value.instrument)?,
            account_id: if value.value.account_id.is_set {
                Some(AccountId::from_u64(value.value.account_id.value))
            } else {
                None
            },
            side: import_side(value.value.side),
        },
    ))
}

fn import_financial_impact(
    value: PitFinancialImpactOptional,
) -> Result<FinancialImpactAccess, String> {
    if !value.is_set {
        return Ok(FinancialImpactAccess::Absent);
    }

    Ok(FinancialImpactAccess::Populated(PopulatedFinancialImpact {
        pnl: if value.value.pnl.is_set {
            Some(value.value.pnl.value.to_param()?)
        } else {
            None
        },
        fee: if value.value.fee.is_set {
            Some(value.value.fee.value.to_param()?)
        } else {
            None
        },
    }))
}

fn import_last_trade(value: PitExecutionReportTradeOptional) -> Result<Option<Trade>, String> {
    if !value.is_set {
        return Ok(None);
    }

    Ok(Some(Trade {
        price: value.value.price.to_param()?,
        quantity: value.value.quantity.to_param()?,
    }))
}

fn import_fill(value: PitExecutionReportFillOptional) -> Result<Option<FillDetailsData>, String> {
    if !value.is_set {
        return Ok(None);
    }

    let leaves_quantity = if value.value.leaves_quantity.is_set {
        value.value.leaves_quantity.value.to_param()?
    } else {
        return Err("execution_report.fill.leaves_quantity is not set".to_string());
    };

    let lock = if value.value.lock_price.is_set {
        PreTradeLock::new(Some(value.value.lock_price.value.to_param()?))
    } else {
        PreTradeLock::new(None)
    };

    Ok(Some(FillDetailsData {
        last_trade: import_last_trade(value.value.last_trade)?,
        leaves_quantity,
        lock,
        is_terminal: value.value.is_terminal,
    }))
}

fn import_position_impact(
    value: PitExecutionReportPositionImpactOptional,
) -> Option<PositionImpactData> {
    if !value.is_set {
        return None;
    }

    Some(PositionImpactData {
        value: ExecutionReportPositionImpact {
            position_effect: import_position_effect(value.value.position_effect),
            position_side: import_position_side(value.value.position_side),
        },
    })
}

fn export_operation(value: &ExecutionReportOperationAccess) -> PitExecutionReportOperationOptional {
    match value {
        ExecutionReportOperationAccess::Populated(operation) => {
            let instrument = if let Some(instrument) = &operation.instrument {
                PitInstrument {
                    underlying_asset: crate::PitStringView::from_utf8(
                        instrument.underlying_asset().as_ref(),
                    ),
                    settlement_asset: crate::PitStringView::from_utf8(
                        instrument.settlement_asset().as_ref(),
                    ),
                }
            } else {
                PitInstrument::default()
            };

            PitExecutionReportOperationOptional {
                is_set: true,
                value: PitExecutionReportOperation {
                    instrument,
                    account_id: match operation.account_id {
                        Some(account_id) => PitParamAccountIdOptional {
                            is_set: true,
                            value: account_id.as_u64(),
                        },
                        None => PitParamAccountIdOptional::default(),
                    },
                    side: operation.side.map(export_side).unwrap_or_default(),
                },
            }
        }
        ExecutionReportOperationAccess::Absent => PitExecutionReportOperationOptional::default(),
    }
}

fn export_financial_impact(value: &FinancialImpactAccess) -> PitFinancialImpactOptional {
    match value {
        FinancialImpactAccess::Populated(financial_impact) => PitFinancialImpactOptional {
            is_set: true,
            value: PitFinancialImpact {
                pnl: match financial_impact.pnl {
                    Some(v) => PitParamPnlOptional {
                        is_set: true,
                        value: PitParamPnl(v.to_decimal().into()),
                    },
                    None => PitParamPnlOptional::default(),
                },
                fee: match financial_impact.fee {
                    Some(v) => PitParamFeeOptional {
                        is_set: true,
                        value: PitParamFee(v.to_decimal().into()),
                    },
                    None => PitParamFeeOptional::default(),
                },
            },
        },
        FinancialImpactAccess::Absent => PitFinancialImpactOptional::default(),
    }
}

fn export_last_trade(value: Option<Trade>) -> PitExecutionReportTradeOptional {
    match value {
        Some(trade) => PitExecutionReportTradeOptional {
            is_set: true,
            value: PitExecutionReportTrade {
                price: PitParamPrice(trade.price.to_decimal().into()),
                quantity: PitParamQuantity(trade.quantity.to_decimal().into()),
            },
        },
        None => PitExecutionReportTradeOptional::default(),
    }
}

fn export_fill(value: &Option<FillDetailsData>) -> PitExecutionReportFillOptional {
    match value {
        Some(fill) => PitExecutionReportFillOptional {
            is_set: true,
            value: PitExecutionReportFill {
                last_trade: export_last_trade(fill.last_trade),
                leaves_quantity: PitParamQuantityOptional {
                    is_set: true,
                    value: PitParamQuantity(fill.leaves_quantity.to_decimal().into()),
                },
                lock_price: match fill.lock.price() {
                    Some(price) => PitParamPriceOptional {
                        is_set: true,
                        value: PitParamPrice(price.to_decimal().into()),
                    },
                    None => PitParamPriceOptional::default(),
                },
                is_terminal: fill.is_terminal,
            },
        },
        None => PitExecutionReportFillOptional::default(),
    }
}

fn export_position_impact(
    value: &Option<PositionImpactData>,
) -> PitExecutionReportPositionImpactOptional {
    match value {
        Some(position_impact) => PitExecutionReportPositionImpactOptional {
            is_set: true,
            value: PitExecutionReportPositionImpact {
                position_effect: position_impact
                    .value
                    .position_effect
                    .map(export_position_effect)
                    .unwrap_or_default(),
                position_side: position_impact
                    .value
                    .position_side
                    .map(export_position_side)
                    .unwrap_or_default(),
            },
        },
        None => PitExecutionReportPositionImpactOptional::default(),
    }
}

pub(crate) fn import_execution_report(
    value: &PitExecutionReport,
) -> Result<ExecutionReport, String> {
    // The engine applies reports as owned domain values, so decoding a
    // borrowed report view necessarily builds owned data here.
    Ok(ExecutionReport {
        operation: import_operation(value.operation)?,
        financial_impact: import_financial_impact(value.financial_impact)?,
        fill: import_fill(value.fill)?,
        position_impact: import_position_impact(value.position_impact),
        user_data: value.user_data,
    })
}

pub(crate) fn export_execution_report(value: &ExecutionReport) -> PitExecutionReport {
    PitExecutionReport {
        operation: export_operation(&value.operation),
        financial_impact: export_financial_impact(&value.financial_impact),
        fill: export_fill(&value.fill),
        position_impact: export_position_impact(&value.position_impact),
        user_data: value.user_data,
    }
}

/// Fill-details payload carrying validated domain values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillDetailsData {
    /// Actual execution trade payload.
    pub last_trade: Option<Trade>,

    /// Remaining quantity after this report.
    pub leaves_quantity: Quantity,

    /// PreTradeReservation lock payload.
    pub lock: PreTradeLock,

    /// Whether this report closes the report stream for the order.
    pub is_terminal: bool,
}

/// Position-impact payload carrying validated domain values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionImpactData {
    /// Position impact payload.
    pub value: ExecutionReportPositionImpact,
}

/// Execution-report payload carrying validated domain values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionReport {
    /// Operation-identification group.
    pub operation: ExecutionReportOperationAccess,

    /// Financial-impact group.
    pub financial_impact: FinancialImpactAccess,

    /// Optional fill-details group.
    pub fill: Option<FillDetailsData>,

    /// Optional position-impact group.
    pub position_impact: Option<PositionImpactData>,

    /// Opaque caller-defined token.
    ///
    /// The SDK never inspects, dereferences, or frees this value. Its meaning,
    /// lifetime, and thread-safety are the caller's responsibility. `0` / null
    /// means "not set". See the project Threading Contract for the full lifetime
    /// model.
    ///
    /// The token is preserved unchanged across every engine callback that
    /// receives the carrying value, including policy callbacks and adjustment
    /// callbacks.
    pub user_data: *mut c_void,
}

impl HasInstrument for ExecutionReport {
    fn instrument(&self) -> Result<&openpit::Instrument, RequestFieldAccessError> {
        self.operation.instrument()
    }
}

impl HasPnl for ExecutionReport {
    fn pnl(&self) -> Result<openpit::param::Pnl, RequestFieldAccessError> {
        self.financial_impact.pnl()
    }
}

impl HasFee for ExecutionReport {
    fn fee(&self) -> Result<openpit::param::Fee, RequestFieldAccessError> {
        self.financial_impact.fee()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        export_execution_report, import_execution_report, ExecutionReport, FillDetailsData,
        PitExecutionReport, PitExecutionReportFill, PitExecutionReportFillOptional,
        PitExecutionReportOperation, PitExecutionReportOperationOptional,
        PitExecutionReportPositionImpact, PitExecutionReportPositionImpactOptional,
        PitExecutionReportTrade, PitExecutionReportTradeOptional, PitFinancialImpact,
        PitFinancialImpactOptional, PositionImpactData,
    };
    use crate::instrument::PitInstrument;
    use crate::param::{
        PitParamAccountIdOptional, PitParamFee, PitParamFeeOptional, PitParamPnl,
        PitParamPnlOptional, PitParamPositionEffect, PitParamPositionSide, PitParamPrice,
        PitParamPriceOptional, PitParamQuantity, PitParamQuantityOptional, PitParamSide,
    };
    use crate::PitStringView;
    use openpit::param::{
        AccountId, Asset, Fee, Pnl, PositionEffect, PositionSide, Price, Quantity, Side, Trade,
    };
    use openpit::pretrade::PreTradeLock;
    use openpit::ExecutionReportPositionImpact;
    use openpit::Instrument;
    use openpit::{HasFee, HasInstrument, HasPnl};
    use pit_interop::{
        ExecutionReportOperationAccess, FinancialImpactAccess, PopulatedExecutionReportOperation,
        PopulatedFinancialImpact,
    };

    fn instrument_view(underlying: &'static [u8], settlement: &'static [u8]) -> PitInstrument {
        PitInstrument {
            underlying_asset: PitStringView {
                ptr: underlying.as_ptr(),
                len: underlying.len(),
            },
            settlement_asset: PitStringView {
                ptr: settlement.as_ptr(),
                len: settlement.len(),
            },
        }
    }

    fn populated_operation() -> ExecutionReportOperationAccess {
        ExecutionReportOperationAccess::Populated(PopulatedExecutionReportOperation {
            instrument: Some(Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            )),
            account_id: Some(AccountId::from_u64(99224416)),
            side: Some(Side::Sell),
        })
    }

    fn populated_financial_impact() -> FinancialImpactAccess {
        FinancialImpactAccess::Populated(PopulatedFinancialImpact {
            pnl: Some(Pnl::from_str("-10").expect("pnl must be valid")),
            fee: Some(Fee::from_str("1").expect("fee must be valid")),
        })
    }

    #[test]
    fn execution_report_exposes_all_groups() {
        let report = ExecutionReport {
            operation: populated_operation(),
            financial_impact: populated_financial_impact(),
            fill: Some(FillDetailsData {
                last_trade: Some(Trade {
                    price: Price::from_str("101").expect("price must be valid"),
                    quantity: Quantity::from_str("3").expect("quantity must be valid"),
                }),
                leaves_quantity: Quantity::from_str("1").expect("quantity must be valid"),
                lock: PreTradeLock::new(Some(Price::from_str("101").expect("price must be valid"))),
                is_terminal: true,
            }),
            position_impact: Some(PositionImpactData {
                value: ExecutionReportPositionImpact {
                    position_effect: Some(PositionEffect::Open),
                    position_side: Some(PositionSide::Long),
                },
            }),
            user_data: std::ptr::null_mut(),
        };

        if let ExecutionReportOperationAccess::Populated(operation) = &report.operation {
            assert_eq!(operation.side, Some(Side::Sell));
        } else {
            panic!("expected populated operation");
        }
        if let FinancialImpactAccess::Populated(financial_impact) = &report.financial_impact {
            assert_eq!(
                financial_impact.pnl,
                Some(Pnl::from_str("-10").expect("pnl must be valid"))
            );
        } else {
            panic!("expected populated financial impact");
        }
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
    fn execution_report_returns_absent_for_missing_groups() {
        let report = ExecutionReport {
            operation: ExecutionReportOperationAccess::Absent,
            financial_impact: FinancialImpactAccess::Absent,
            fill: None,
            position_impact: None,
            user_data: std::ptr::null_mut(),
        };

        assert!(matches!(
            report.operation,
            ExecutionReportOperationAccess::Absent
        ));
        assert!(matches!(
            report.financial_impact,
            FinancialImpactAccess::Absent
        ));
        assert!(report.fill.is_none());
        assert!(report.position_impact.is_none());
    }

    #[test]
    fn import_execution_report_rejects_fill_without_leaves_quantity() {
        let report = PitExecutionReport {
            operation: PitExecutionReportOperationOptional::default(),
            financial_impact: PitFinancialImpactOptional::default(),
            fill: PitExecutionReportFillOptional {
                is_set: true,
                value: PitExecutionReportFill {
                    last_trade: PitExecutionReportTradeOptional::default(),
                    leaves_quantity: PitParamQuantityOptional::default(),
                    lock_price: PitParamPriceOptional::default(),
                    is_terminal: false,
                },
            },
            position_impact: PitExecutionReportPositionImpactOptional::default(),
            user_data: std::ptr::null_mut(),
        };

        let error = import_execution_report(&report).expect_err("import must fail");
        assert_eq!(error, "execution_report.fill.leaves_quantity is not set");
    }

    #[test]
    fn import_export_execution_report_roundtrip_exposes_trait_fields() {
        let report = ExecutionReport {
            operation: populated_operation(),
            financial_impact: populated_financial_impact(),
            fill: None,
            position_impact: None,
            user_data: std::ptr::null_mut(),
        };
        let exported = export_execution_report(&report);
        let imported = import_execution_report(&exported).expect("import");

        let instrument = imported.instrument().expect("instrument");
        assert_eq!(instrument.underlying_asset().as_ref(), "AAPL");
        assert_eq!(
            imported.pnl().expect("pnl"),
            Pnl::from_str("-10").expect("pnl")
        );
        assert_eq!(
            imported.fee().expect("fee"),
            Fee::from_str("1").expect("fee")
        );
    }

    #[test]
    fn ffi_execution_report_by_value_roundtrip() {
        let report = PitExecutionReport {
            operation: PitExecutionReportOperationOptional {
                is_set: true,
                value: PitExecutionReportOperation {
                    instrument: instrument_view(b"AAPL", b"USD"),
                    account_id: PitParamAccountIdOptional {
                        value: 42,
                        is_set: true,
                    },
                    side: PitParamSide::Buy,
                },
            },
            financial_impact: PitFinancialImpactOptional {
                is_set: true,
                value: PitFinancialImpact {
                    pnl: PitParamPnlOptional {
                        value: PitParamPnl(Pnl::from_str("10").expect("pnl").to_decimal().into()),
                        is_set: true,
                    },
                    fee: PitParamFeeOptional {
                        value: PitParamFee(Fee::from_str("1").expect("fee").to_decimal().into()),
                        is_set: true,
                    },
                },
            },
            fill: PitExecutionReportFillOptional {
                is_set: true,
                value: PitExecutionReportFill {
                    last_trade: PitExecutionReportTradeOptional {
                        is_set: true,
                        value: PitExecutionReportTrade {
                            price: PitParamPrice(
                                Price::from_str("100").expect("price").to_decimal().into(),
                            ),
                            quantity: PitParamQuantity(
                                Quantity::from_str("2")
                                    .expect("quantity")
                                    .to_decimal()
                                    .into(),
                            ),
                        },
                    },
                    leaves_quantity: PitParamQuantityOptional {
                        is_set: true,
                        value: PitParamQuantity(
                            Quantity::from_str("1")
                                .expect("quantity")
                                .to_decimal()
                                .into(),
                        ),
                    },
                    lock_price: PitParamPriceOptional {
                        is_set: true,
                        value: PitParamPrice(
                            Price::from_str("101").expect("price").to_decimal().into(),
                        ),
                    },
                    is_terminal: true,
                },
            },
            position_impact: PitExecutionReportPositionImpactOptional {
                is_set: true,
                value: PitExecutionReportPositionImpact {
                    position_effect: PitParamPositionEffect::Open,
                    position_side: PitParamPositionSide::Long,
                },
            },
            user_data: std::ptr::null_mut(),
        };

        let imported = import_execution_report(&report).expect("import");
        let exported = export_execution_report(&imported);

        assert!(exported.operation.is_set);
        assert!(exported.financial_impact.is_set);
        assert!(exported.fill.is_set);
        assert!(exported.position_impact.is_set);
    }
}
