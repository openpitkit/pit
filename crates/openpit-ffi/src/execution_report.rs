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

use openpit::param::{AccountId, Trade};
use openpit_interop::{
    ExecutionReportFillAccess, ExecutionReportOperationAccess, ExecutionReportPositionImpactAccess,
    FinancialImpactAccess, PopulatedExecutionReportFill, PopulatedExecutionReportOperation,
    PopulatedExecutionReportPositionImpact, PopulatedFinancialImpact, RequestWithPayload,
};

use crate::define_optional;
use crate::instrument::{import_instrument, OpenPitInstrument};
use crate::param::{
    export_monetary_amount, export_position_effect, export_position_side, export_side,
    import_monetary_amount, import_position_effect, import_position_side, import_side,
    OpenPitParamAccountIdOptional, OpenPitParamFee, OpenPitParamFeeOptional,
    OpenPitParamMonetaryAmountOptional, OpenPitParamPnl, OpenPitParamPnlOptional,
    OpenPitParamPositionEffect, OpenPitParamPositionSide, OpenPitParamPrice, OpenPitParamQuantity,
    OpenPitParamQuantityOptional, OpenPitParamSide,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Populated operation-identification group for an execution report.
pub struct OpenPitExecutionReportOperation {
    /// Trading instrument (`underlying + settlement` asset pair).
    pub instrument: OpenPitInstrument,
    /// Account identifier associated with the report.
    pub account_id: OpenPitParamAccountIdOptional,
    /// Buy or sell direction of the affected order or trade.
    pub side: OpenPitParamSide,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Populated financial-impact group for an execution report.
pub struct OpenPitFinancialImpact {
    /// Profit-and-loss contribution carried by this report.
    pub pnl: OpenPitParamPnlOptional,
    /// Fee or rebate contribution carried by this report.
    pub fee: OpenPitParamFeeOptional,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Fill trade payload (`price + quantity`) for execution reports.
pub struct OpenPitExecutionReportTrade {
    /// Trade price.
    pub price: OpenPitParamPrice,
    /// Trade quantity.
    pub quantity: OpenPitParamQuantity,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Populated fill-details group for an execution report.
pub struct OpenPitExecutionReportFill {
    /// Optional latest trade payload.
    pub last_trade: OpenPitExecutionReportTradeOptional,
    /// Optional fill fee amount and currency.
    pub fee: OpenPitParamMonetaryAmountOptional,
    /// Remaining quantity after applying this report.
    pub leaves_quantity: OpenPitParamQuantityOptional,
    /// Pre-trade lock attached to the order.
    ///
    /// Optional field: null means the lock is absent (maps to `None`), not
    /// an empty lock. No lock record is fabricated on import for a null
    /// pointer, and an absent lock is exported as null.
    ///
    /// Ownership contract:
    /// - the caller owns the pointer when present (build it through
    ///   `openpit_pretrade_lock_*` functions) and remains responsible for
    ///   releasing it with `openpit_destroy_pretrade_pre_trade_lock`.
    pub lock: *const crate::pre_trade_lock::OpenPitPretradePreTradeLock,
    /// Whether this report closes the order's report stream.
    /// The order is filled, cancelled, or rejected.
    pub is_final: OpenPitExecutionReportIsFinalOptional,
}

impl Default for OpenPitExecutionReportFill {
    fn default() -> Self {
        Self {
            last_trade: OpenPitExecutionReportTradeOptional::default(),
            fee: OpenPitParamMonetaryAmountOptional::default(),
            leaves_quantity: OpenPitParamQuantityOptional::default(),
            lock: std::ptr::null(),
            is_final: OpenPitExecutionReportIsFinalOptional::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Populated position-impact group for an execution report.
pub struct OpenPitExecutionReportPositionImpact {
    /// Whether exposure is opened or closed.
    pub position_effect: OpenPitParamPositionEffect,
    /// Impacted side (long or short).
    pub position_side: OpenPitParamPositionSide,
}

define_optional!(
    optional = OpenPitExecutionReportOperationOptional,
    value = OpenPitExecutionReportOperation
);
define_optional!(
    optional = OpenPitFinancialImpactOptional,
    value = OpenPitFinancialImpact
);
define_optional!(
    optional = OpenPitExecutionReportTradeOptional,
    value = OpenPitExecutionReportTrade
);
define_optional!(
    optional = OpenPitExecutionReportIsFinalOptional,
    value = bool
);
define_optional!(
    optional = OpenPitExecutionReportFillOptional,
    value = OpenPitExecutionReportFill
);
define_optional!(
    optional = OpenPitExecutionReportPositionImpactOptional,
    value = OpenPitExecutionReportPositionImpact
);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Full caller-owned execution-report payload.
pub struct OpenPitExecutionReport {
    /// Optional operation-identification group.
    pub operation: OpenPitExecutionReportOperationOptional,
    /// Optional financial-impact group.
    pub financial_impact: OpenPitFinancialImpactOptional,
    /// Optional fill-details group.
    pub fill: OpenPitExecutionReportFillOptional,
    /// Optional position-impact group.
    pub position_impact: OpenPitExecutionReportPositionImpactOptional,
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

fn import_operation(
    value: OpenPitExecutionReportOperationOptional,
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
    value: OpenPitFinancialImpactOptional,
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

fn import_last_trade(value: OpenPitExecutionReportTradeOptional) -> Result<Option<Trade>, String> {
    if !value.is_set {
        return Ok(None);
    }

    Ok(Some(Trade {
        price: value.value.price.to_param()?,
        quantity: value.value.quantity.to_param()?,
    }))
}

fn import_fill(
    value: OpenPitExecutionReportFillOptional,
) -> Result<ExecutionReportFillAccess, String> {
    if !value.is_set {
        return Ok(ExecutionReportFillAccess::Absent);
    }

    let leaves_quantity = if value.value.leaves_quantity.is_set {
        Some(value.value.leaves_quantity.value.to_param()?)
    } else {
        None
    };

    let lock = if value.value.lock.is_null() {
        None
    } else {
        Some(unsafe { &*value.value.lock }.inner_clone())
    };

    Ok(ExecutionReportFillAccess::Populated(Box::new(
        PopulatedExecutionReportFill {
            last_trade: import_last_trade(value.value.last_trade)?,
            fee: if value.value.fee.is_set {
                Some(import_monetary_amount(value.value.fee.value)?)
            } else {
                None
            },
            leaves_quantity,
            lock,
            is_final: if value.value.is_final.is_set {
                Some(value.value.is_final.value)
            } else {
                None
            },
        },
    )))
}

fn import_position_impact(
    value: OpenPitExecutionReportPositionImpactOptional,
) -> ExecutionReportPositionImpactAccess {
    if !value.is_set {
        return ExecutionReportPositionImpactAccess::Absent;
    }

    ExecutionReportPositionImpactAccess::Populated(PopulatedExecutionReportPositionImpact {
        position_effect: import_position_effect(value.value.position_effect),
        position_side: import_position_side(value.value.position_side),
    })
}

fn export_operation(
    value: &ExecutionReportOperationAccess,
) -> OpenPitExecutionReportOperationOptional {
    match value {
        ExecutionReportOperationAccess::Populated(operation) => {
            let instrument = if let Some(instrument) = &operation.instrument {
                OpenPitInstrument {
                    underlying_asset: crate::OpenPitStringView::from_utf8(
                        instrument.underlying_asset().as_ref(),
                    ),
                    settlement_asset: crate::OpenPitStringView::from_utf8(
                        instrument.settlement_asset().as_ref(),
                    ),
                }
            } else {
                OpenPitInstrument::default()
            };

            OpenPitExecutionReportOperationOptional {
                is_set: true,
                value: OpenPitExecutionReportOperation {
                    instrument,
                    account_id: match operation.account_id {
                        Some(account_id) => OpenPitParamAccountIdOptional {
                            is_set: true,
                            value: account_id.as_u64(),
                        },
                        None => OpenPitParamAccountIdOptional::default(),
                    },
                    side: operation.side.map(export_side).unwrap_or_default(),
                },
            }
        }
        ExecutionReportOperationAccess::Absent => {
            OpenPitExecutionReportOperationOptional::default()
        }
    }
}

fn export_financial_impact(value: &FinancialImpactAccess) -> OpenPitFinancialImpactOptional {
    match value {
        FinancialImpactAccess::Populated(financial_impact) => OpenPitFinancialImpactOptional {
            is_set: true,
            value: OpenPitFinancialImpact {
                pnl: match financial_impact.pnl {
                    Some(v) => OpenPitParamPnlOptional {
                        is_set: true,
                        value: OpenPitParamPnl(v.to_decimal().into()),
                    },
                    None => OpenPitParamPnlOptional::default(),
                },
                fee: match financial_impact.fee {
                    Some(v) => OpenPitParamFeeOptional {
                        is_set: true,
                        value: OpenPitParamFee(v.to_decimal().into()),
                    },
                    None => OpenPitParamFeeOptional::default(),
                },
            },
        },
        FinancialImpactAccess::Absent => OpenPitFinancialImpactOptional::default(),
    }
}

fn export_last_trade(value: Option<Trade>) -> OpenPitExecutionReportTradeOptional {
    match value {
        Some(trade) => OpenPitExecutionReportTradeOptional {
            is_set: true,
            value: OpenPitExecutionReportTrade {
                price: OpenPitParamPrice(trade.price.to_decimal().into()),
                quantity: OpenPitParamQuantity(trade.quantity.to_decimal().into()),
            },
        },
        None => OpenPitExecutionReportTradeOptional::default(),
    }
}

fn export_fill(value: &ExecutionReportFillAccess) -> OpenPitExecutionReportFillOptional {
    match value {
        ExecutionReportFillAccess::Populated(fill) => OpenPitExecutionReportFillOptional {
            is_set: true,
            value: OpenPitExecutionReportFill {
                last_trade: export_last_trade(fill.last_trade),
                fee: fill.fee.as_ref().map_or_else(
                    OpenPitParamMonetaryAmountOptional::default,
                    |fee| OpenPitParamMonetaryAmountOptional {
                        is_set: true,
                        value: export_monetary_amount(fee),
                    },
                ),
                leaves_quantity: match fill.leaves_quantity {
                    Some(leaves_quantity) => OpenPitParamQuantityOptional {
                        is_set: true,
                        value: OpenPitParamQuantity(leaves_quantity.to_decimal().into()),
                    },
                    None => OpenPitParamQuantityOptional::default(),
                },
                lock: match &fill.lock {
                    Some(lock) => {
                        crate::pre_trade_lock::OpenPitPretradePreTradeLock::from_inner(lock.clone())
                    }
                    None => std::ptr::null(),
                },
                is_final: match fill.is_final {
                    Some(value) => OpenPitExecutionReportIsFinalOptional {
                        value,
                        is_set: true,
                    },
                    None => OpenPitExecutionReportIsFinalOptional::default(),
                },
            },
        },
        ExecutionReportFillAccess::Absent => OpenPitExecutionReportFillOptional::default(),
    }
}

fn export_position_impact(
    value: &ExecutionReportPositionImpactAccess,
) -> OpenPitExecutionReportPositionImpactOptional {
    match value {
        ExecutionReportPositionImpactAccess::Populated(position_impact) => {
            OpenPitExecutionReportPositionImpactOptional {
                is_set: true,
                value: OpenPitExecutionReportPositionImpact {
                    position_effect: position_impact
                        .position_effect
                        .map(export_position_effect)
                        .unwrap_or_default(),
                    position_side: position_impact
                        .position_side
                        .map(export_position_side)
                        .unwrap_or_default(),
                },
            }
        }
        ExecutionReportPositionImpactAccess::Absent => {
            OpenPitExecutionReportPositionImpactOptional::default()
        }
    }
}

pub(crate) fn import_execution_report(
    value: &OpenPitExecutionReport,
) -> Result<ExecutionReport, String> {
    // The engine applies reports as owned domain values, so decoding a
    // borrowed report view necessarily builds owned data here.
    Ok(RequestWithPayload::new(
        openpit_interop::ExecutionReport {
            operation: import_operation(value.operation)?,
            financial_impact: import_financial_impact(value.financial_impact)?,
            fill: import_fill(value.fill)?,
            position_impact: import_position_impact(value.position_impact),
        },
        value.user_data,
    ))
}

pub(crate) fn export_execution_report(value: &ExecutionReport) -> OpenPitExecutionReport {
    OpenPitExecutionReport {
        operation: export_operation(&value.request.operation),
        financial_impact: export_financial_impact(&value.request.financial_impact),
        fill: export_fill(&value.request.fill),
        position_impact: export_position_impact(&value.request.position_impact),
        user_data: value.payload,
    }
}

/// FFI execution-report request paired with an opaque caller-defined token.
///
/// The token is stored in [`RequestWithPayload::payload`]. The SDK never
/// inspects, dereferences, or frees this value. Its meaning, lifetime, and
/// thread-safety are the caller's responsibility. A null pointer means
/// "not set". See the project Threading Contract for the full lifetime model.
///
/// The token is preserved unchanged across every engine callback that
/// receives the carrying value, including policy callbacks and adjustment
/// callbacks.
pub type ExecutionReport = RequestWithPayload<openpit_interop::ExecutionReport, *mut c_void>;

#[cfg(test)]
mod tests {
    use super::{
        export_execution_report, import_execution_report, OpenPitExecutionReport,
        OpenPitExecutionReportFill, OpenPitExecutionReportFillOptional,
        OpenPitExecutionReportIsFinalOptional, OpenPitExecutionReportOperation,
        OpenPitExecutionReportOperationOptional, OpenPitExecutionReportPositionImpact,
        OpenPitExecutionReportPositionImpactOptional, OpenPitExecutionReportTrade,
        OpenPitExecutionReportTradeOptional, OpenPitFinancialImpact,
        OpenPitFinancialImpactOptional,
    };
    use crate::instrument::OpenPitInstrument;
    use crate::param::{
        OpenPitParamAccountIdOptional, OpenPitParamFee, OpenPitParamFeeOptional,
        OpenPitParamMonetaryAmount, OpenPitParamMonetaryAmountOptional, OpenPitParamPnl,
        OpenPitParamPnlOptional, OpenPitParamPositionEffect, OpenPitParamPositionSide,
        OpenPitParamPrice, OpenPitParamQuantity, OpenPitParamQuantityOptional, OpenPitParamSide,
    };
    use crate::OpenPitStringView;
    use openpit::param::{
        AccountId, Asset, Fee, MonetaryAmount, Pnl, PositionEffect, PositionSide, Price, Quantity,
        Side, Trade,
    };
    use openpit::pretrade::PreTradeLock;
    use openpit::Instrument;
    use openpit::{
        HasExecutionReportFillFee, HasExecutionReportIsFinal, HasExecutionReportPositionEffect,
        HasFee, HasInstrument, HasPnl, HasPreTradeLock,
    };
    use openpit_interop::{
        ExecutionReportFillAccess, ExecutionReportOperationAccess,
        ExecutionReportPositionImpactAccess, FinancialImpactAccess, PopulatedExecutionReportFill,
        PopulatedExecutionReportOperation, PopulatedExecutionReportPositionImpact,
        PopulatedFinancialImpact, RequestWithPayload,
    };

    fn instrument_view(underlying: &'static [u8], settlement: &'static [u8]) -> OpenPitInstrument {
        OpenPitInstrument {
            underlying_asset: OpenPitStringView {
                ptr: underlying.as_ptr(),
                len: underlying.len(),
            },
            settlement_asset: OpenPitStringView {
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
        let report = RequestWithPayload::new(
            openpit_interop::ExecutionReport {
                operation: populated_operation(),
                financial_impact: populated_financial_impact(),
                fill: ExecutionReportFillAccess::Populated(Box::new(
                    PopulatedExecutionReportFill {
                        last_trade: Some(Trade {
                            price: Price::from_str("101").expect("price must be valid"),
                            quantity: Quantity::from_str("3").expect("quantity must be valid"),
                        }),
                        fee: Some(MonetaryAmount {
                            amount: Fee::from_str("0.25").expect("fee amount must be valid"),
                            currency: Asset::new("USD").expect("asset code must be valid"),
                        }),
                        leaves_quantity: Some(
                            Quantity::from_str("1").expect("quantity must be valid"),
                        ),
                        lock: Some(PreTradeLock::from_entries([(
                            openpit::DEFAULT_POLICY_GROUP_ID,
                            Price::from_str("101").expect("price must be valid"),
                        )])),
                        is_final: Some(true),
                    },
                )),
                position_impact: ExecutionReportPositionImpactAccess::Populated(
                    PopulatedExecutionReportPositionImpact {
                        position_effect: Some(PositionEffect::Open),
                        position_side: Some(PositionSide::Long),
                    },
                ),
            },
            std::ptr::null_mut::<std::ffi::c_void>(),
        );

        if let ExecutionReportOperationAccess::Populated(operation) = &report.request.operation {
            assert_eq!(operation.side, Some(Side::Sell));
        } else {
            panic!("expected populated operation");
        }
        if let FinancialImpactAccess::Populated(financial_impact) = &report.request.financial_impact
        {
            assert_eq!(
                financial_impact.pnl,
                Some(Pnl::from_str("-10").expect("pnl must be valid"))
            );
        } else {
            panic!("expected populated financial impact");
        }
        assert!(report.is_final().expect("is_final"));
        assert_eq!(
            report.fill_fee().expect("fill fee"),
            Some(MonetaryAmount {
                amount: Fee::from_str("0.25").expect("fee amount must be valid"),
                currency: Asset::new("USD").expect("asset code must be valid"),
            })
        );
        assert_eq!(
            report.position_effect().expect("position_effect"),
            Some(PositionEffect::Open)
        );
    }

    #[test]
    fn execution_report_returns_absent_for_missing_groups() {
        let report = RequestWithPayload::new(
            openpit_interop::ExecutionReport {
                operation: ExecutionReportOperationAccess::Absent,
                financial_impact: FinancialImpactAccess::Absent,
                fill: ExecutionReportFillAccess::Absent,
                position_impact: ExecutionReportPositionImpactAccess::Absent,
            },
            std::ptr::null_mut::<std::ffi::c_void>(),
        );

        assert!(matches!(
            report.request.operation,
            ExecutionReportOperationAccess::Absent
        ));
        assert!(matches!(
            report.request.financial_impact,
            FinancialImpactAccess::Absent
        ));
        assert!(matches!(
            report.request.fill,
            ExecutionReportFillAccess::Absent
        ));
        assert!(matches!(
            report.request.position_impact,
            ExecutionReportPositionImpactAccess::Absent
        ));
    }

    #[test]
    fn import_execution_report_preserves_unset_leaves_quantity() {
        let report = OpenPitExecutionReport {
            operation: OpenPitExecutionReportOperationOptional::default(),
            financial_impact: OpenPitFinancialImpactOptional::default(),
            fill: OpenPitExecutionReportFillOptional {
                is_set: true,
                value: OpenPitExecutionReportFill {
                    last_trade: OpenPitExecutionReportTradeOptional::default(),
                    fee: OpenPitParamMonetaryAmountOptional::default(),
                    leaves_quantity: OpenPitParamQuantityOptional::default(),
                    lock: std::ptr::null(),
                    is_final: OpenPitExecutionReportIsFinalOptional::default(),
                },
            },
            position_impact: OpenPitExecutionReportPositionImpactOptional::default(),
            user_data: std::ptr::null_mut(),
        };

        let imported = import_execution_report(&report).expect("import");
        if let ExecutionReportFillAccess::Populated(fill) = &imported.request.fill {
            assert!(fill.leaves_quantity.is_none());
        } else {
            panic!("fill must be present");
        }
        assert!(imported.is_final().is_err());
    }

    #[test]
    fn import_execution_report_preserves_unset_is_final() {
        let report = OpenPitExecutionReport {
            operation: OpenPitExecutionReportOperationOptional::default(),
            financial_impact: OpenPitFinancialImpactOptional::default(),
            fill: OpenPitExecutionReportFillOptional {
                is_set: true,
                value: OpenPitExecutionReportFill {
                    last_trade: OpenPitExecutionReportTradeOptional::default(),
                    fee: OpenPitParamMonetaryAmountOptional::default(),
                    leaves_quantity: OpenPitParamQuantityOptional {
                        is_set: true,
                        value: OpenPitParamQuantity(
                            Quantity::from_str("1")
                                .expect("quantity")
                                .to_decimal()
                                .into(),
                        ),
                    },
                    lock: std::ptr::null(),
                    is_final: OpenPitExecutionReportIsFinalOptional::default(),
                },
            },
            position_impact: OpenPitExecutionReportPositionImpactOptional::default(),
            user_data: std::ptr::null_mut(),
        };

        let imported = import_execution_report(&report).expect("import");
        assert!(imported.is_final().is_err());
        if let ExecutionReportFillAccess::Populated(fill) = &imported.request.fill {
            assert_eq!(fill.is_final, None);
        } else {
            panic!("fill must be present");
        }
    }

    #[test]
    fn import_execution_report_null_lock_pointer_stays_absent() {
        let report = OpenPitExecutionReport {
            operation: OpenPitExecutionReportOperationOptional::default(),
            financial_impact: OpenPitFinancialImpactOptional::default(),
            fill: OpenPitExecutionReportFillOptional {
                is_set: true,
                value: OpenPitExecutionReportFill {
                    last_trade: OpenPitExecutionReportTradeOptional::default(),
                    fee: OpenPitParamMonetaryAmountOptional::default(),
                    leaves_quantity: OpenPitParamQuantityOptional::default(),
                    lock: std::ptr::null(),
                    is_final: OpenPitExecutionReportIsFinalOptional::default(),
                },
            },
            position_impact: OpenPitExecutionReportPositionImpactOptional::default(),
            user_data: std::ptr::null_mut(),
        };

        let imported = import_execution_report(&report).expect("import");
        if let ExecutionReportFillAccess::Populated(fill) = &imported.request.fill {
            assert_eq!(fill.lock, None);
        } else {
            panic!("fill must be present");
        }
        assert_eq!(
            imported
                .lock()
                .expect_err("null lock pointer must not fabricate an empty lock")
                .to_string(),
            "failed to access field 'fill.lock'"
        );

        let exported = export_execution_report(&imported);
        assert!(exported.fill.value.lock.is_null());
    }

    #[test]
    fn import_export_execution_report_roundtrip_exposes_trait_fields() {
        let report = RequestWithPayload::new(
            openpit_interop::ExecutionReport {
                operation: populated_operation(),
                financial_impact: populated_financial_impact(),
                fill: ExecutionReportFillAccess::Absent,
                position_impact: ExecutionReportPositionImpactAccess::Absent,
            },
            std::ptr::null_mut(),
        );
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
        let report = OpenPitExecutionReport {
            operation: OpenPitExecutionReportOperationOptional {
                is_set: true,
                value: OpenPitExecutionReportOperation {
                    instrument: instrument_view(b"AAPL", b"USD"),
                    account_id: OpenPitParamAccountIdOptional {
                        value: 42,
                        is_set: true,
                    },
                    side: OpenPitParamSide::Buy,
                },
            },
            financial_impact: OpenPitFinancialImpactOptional {
                is_set: true,
                value: OpenPitFinancialImpact {
                    pnl: OpenPitParamPnlOptional {
                        value: OpenPitParamPnl(
                            Pnl::from_str("10").expect("pnl").to_decimal().into(),
                        ),
                        is_set: true,
                    },
                    fee: OpenPitParamFeeOptional {
                        value: OpenPitParamFee(
                            Fee::from_str("1").expect("fee").to_decimal().into(),
                        ),
                        is_set: true,
                    },
                },
            },
            fill: OpenPitExecutionReportFillOptional {
                is_set: true,
                value: OpenPitExecutionReportFill {
                    last_trade: OpenPitExecutionReportTradeOptional {
                        is_set: true,
                        value: OpenPitExecutionReportTrade {
                            price: OpenPitParamPrice(
                                Price::from_str("100").expect("price").to_decimal().into(),
                            ),
                            quantity: OpenPitParamQuantity(
                                Quantity::from_str("2")
                                    .expect("quantity")
                                    .to_decimal()
                                    .into(),
                            ),
                        },
                    },
                    fee: OpenPitParamMonetaryAmountOptional {
                        is_set: true,
                        value: OpenPitParamMonetaryAmount {
                            amount: OpenPitParamFee(
                                Fee::from_str("0.50").expect("fee").to_decimal().into(),
                            ),
                            currency: OpenPitStringView {
                                ptr: b"USD".as_ptr(),
                                len: b"USD".len(),
                            },
                        },
                    },
                    leaves_quantity: OpenPitParamQuantityOptional {
                        is_set: true,
                        value: OpenPitParamQuantity(
                            Quantity::from_str("1")
                                .expect("quantity")
                                .to_decimal()
                                .into(),
                        ),
                    },
                    lock: crate::pre_trade_lock::OpenPitPretradePreTradeLock::from_inner(
                        PreTradeLock::from_entries([(
                            openpit::PolicyGroupId::new(7),
                            Price::from_str("101").expect("price must be valid"),
                        )]),
                    ),
                    is_final: OpenPitExecutionReportIsFinalOptional {
                        value: true,
                        is_set: true,
                    },
                },
            },
            position_impact: OpenPitExecutionReportPositionImpactOptional {
                is_set: true,
                value: OpenPitExecutionReportPositionImpact {
                    position_effect: OpenPitParamPositionEffect::Open,
                    position_side: OpenPitParamPositionSide::Long,
                },
            },
            user_data: std::ptr::null_mut(),
        };

        let imported = import_execution_report(&report).expect("import");
        let exported = export_execution_report(&imported);

        assert!(exported.operation.is_set);
        assert!(exported.financial_impact.is_set);
        assert!(exported.fill.is_set);
        assert!(exported.fill.value.fee.is_set);
        assert_eq!(
            imported.fill_fee().expect("fill fee"),
            Some(MonetaryAmount {
                amount: Fee::from_str("0.50").expect("fee"),
                currency: Asset::new("USD").expect("asset code must be valid"),
            })
        );
        assert!(exported.position_impact.is_set);

        crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(
            report.fill.value.lock as *mut _,
        );
        crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(
            exported.fill.value.lock as *mut _,
        );
    }
}
