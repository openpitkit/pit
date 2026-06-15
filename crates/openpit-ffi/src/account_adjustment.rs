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

use openpit::param::{AdjustmentAmount, PositionSize};
use openpit::{AccountAdjustmentAmount, AccountAdjustmentBounds};
use openpit_interop::{
    AccountAdjustmentAmountAccess, AccountAdjustmentBoundsAccess, AccountAdjustmentOperationAccess,
    PopulatedAccountAdjustmentOperation, PopulatedBalanceOperation, PopulatedPositionOperation,
    RequestWithPayload,
};

use crate::define_optional;
use crate::instrument::{import_instrument, parse_asset_view, OpenPitInstrument};
use crate::last_error::{write_param_error_unspecified, OpenPitOutParamError};
use crate::param::{
    export_leverage, export_position_mode, import_leverage, import_position_mode,
    OpenPitParamAdjustmentAmountKind, OpenPitParamLeverage, OpenPitParamPnl,
    OpenPitParamPnlOptional, OpenPitParamPositionMode, OpenPitParamPositionSize,
    OpenPitParamPositionSizeOptional, OpenPitParamPrice, OpenPitParamPriceOptional,
};
use crate::string::OpenPitSharedString;
use crate::OpenPitStringView;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// One amount component inside an account adjustment.
///
/// The numeric value is interpreted according to `kind`:
/// - `Delta` means "change current state by this signed amount";
/// - `Absolute` means "set current state to this signed amount".
pub struct OpenPitParamAdjustmentAmount {
    /// Signed numeric value of the adjustment.
    pub value: OpenPitParamPositionSize,
    /// Interpretation mode for `value`.
    pub kind: OpenPitParamAdjustmentAmountKind,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Balance-operation payload for account adjustment.
pub struct OpenPitAccountAdjustmentBalanceOperation {
    /// Balance asset code.
    pub asset: OpenPitStringView,
    /// Optional force-set of the average entry price in account currency. No
    /// FX is applied by this adjustment.
    pub average_entry_price: OpenPitParamPriceOptional,
    /// Optional force-set of the slot's absolute realized PnL in account
    /// currency. No FX is applied by this adjustment.
    ///
    /// When set, the adjustment overwrites the slot's cumulative realized PnL
    /// with this caller-supplied account-currency value, the same way
    /// `average_entry_price` force-sets the average. The change is surfaced on
    /// the outcome's `realized_pnl` field as a delta/absolute pair, where
    /// `delta` is `new - prior` and `absolute` is this value; leaving it unset
    /// keeps the slot's realized PnL untouched and emits no outcome.
    pub realized_pnl: OpenPitParamPnlOptional,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Position-operation payload for account adjustment.
pub struct OpenPitAccountAdjustmentPositionOperation {
    /// Position instrument.
    pub instrument: OpenPitInstrument,
    /// Position collateral asset.
    pub collateral_asset: OpenPitStringView,
    /// Optional force-set of the average entry price in account currency. No
    /// FX is applied by this adjustment.
    pub average_entry_price: OpenPitParamPriceOptional,
    /// Optional leverage.
    pub leverage: OpenPitParamLeverage,
    /// Position mode.
    pub mode: OpenPitParamPositionMode,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Selects which account-adjustment operation payload is present.
///
/// At most one operation payload can be selected at a time:
/// - `Absent` means no operation is supplied;
/// - `Balance` selects the balance-operation payload;
/// - `Position` selects the position-operation payload.
pub enum OpenPitAccountAdjustmentOperationKind {
    /// No operation is supplied.
    #[default]
    Absent = 0,
    /// The balance-operation payload is selected.
    Balance = 1,
    /// The position-operation payload is selected.
    Position = 2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Account-adjustment operation as a single discriminated value.
///
/// `kind` selects which payload is meaningful; the payload not selected by
/// `kind` is ignored. Because a single discriminant chooses the payload,
/// supplying both a balance and a position operation at once is not
/// representable.
pub struct OpenPitAccountAdjustmentOperation {
    /// Selects which payload below is meaningful.
    pub kind: OpenPitAccountAdjustmentOperationKind,
    /// Balance-operation payload, meaningful only when `kind` is `Balance`.
    pub balance: OpenPitAccountAdjustmentBalanceOperation,
    /// Position-operation payload, meaningful only when `kind` is `Position`.
    pub position: OpenPitAccountAdjustmentPositionOperation,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional amount-change group for account adjustment.
///
/// The group is absent when every field is absent.
pub struct OpenPitAccountAdjustmentAmount {
    /// Requested balance change.
    pub balance: OpenPitParamAdjustmentAmount,
    /// Requested held-balance change.
    pub held: OpenPitParamAdjustmentAmount,
    /// Requested incoming-balance change.
    pub incoming: OpenPitParamAdjustmentAmount,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional bounds group for account adjustment.
///
/// The group is absent when every bound is absent.
pub struct OpenPitAccountAdjustmentBounds {
    /// Optional upper bound for balance.
    pub balance_upper: OpenPitParamPositionSizeOptional,
    /// Optional lower bound for balance.
    pub balance_lower: OpenPitParamPositionSizeOptional,
    /// Optional upper bound for held balance.
    pub held_upper: OpenPitParamPositionSizeOptional,
    /// Optional lower bound for held balance.
    pub held_lower: OpenPitParamPositionSizeOptional,
    /// Optional upper bound for incoming balance.
    pub incoming_upper: OpenPitParamPositionSizeOptional,
    /// Optional lower bound for incoming balance.
    pub incoming_lower: OpenPitParamPositionSizeOptional,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Full caller-owned account-adjustment payload.
pub struct OpenPitAccountAdjustment {
    /// Discriminated operation: at most one payload, selected by its kind.
    pub operation: OpenPitAccountAdjustmentOperation,
    /// Optional amount-change group.
    pub amount: OpenPitAccountAdjustmentAmountOptional,
    /// Optional bounds group.
    pub bounds: OpenPitAccountAdjustmentBoundsOptional,
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

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Result of `openpit_engine_apply_account_adjustment`.
pub enum OpenPitAccountAdjustmentApplyStatus {
    /// The call failed before the batch could be evaluated.
    #[default]
    Error = 0,
    /// The batch was accepted and applied.
    Applied = 1,
    /// The batch was evaluated and rejected by policy or validation logic.
    Rejected = 2,
}

define_optional!(
    optional = OpenPitAccountAdjustmentAmountOptional,
    value = OpenPitAccountAdjustmentAmount
);
define_optional!(
    optional = OpenPitAccountAdjustmentBoundsOptional,
    value = OpenPitAccountAdjustmentBounds
);

fn import_adjustment_amount(
    value: OpenPitParamAdjustmentAmount,
) -> Result<Option<AdjustmentAmount>, String> {
    match value.kind {
        OpenPitParamAdjustmentAmountKind::NotSet => Ok(None),
        OpenPitParamAdjustmentAmountKind::Delta => {
            Ok(Some(AdjustmentAmount::Delta(value.value.to_param()?)))
        }
        OpenPitParamAdjustmentAmountKind::Absolute => {
            Ok(Some(AdjustmentAmount::Absolute(value.value.to_param()?)))
        }
    }
}

/// Renders an adjustment amount into a caller-owned shared string.
///
/// Returns null and writes `out_error` when the amount is not set or its
/// numeric value cannot be decoded.
#[no_mangle]
pub unsafe extern "C" fn openpit_param_adjustment_amount_to_string(
    value: OpenPitParamAdjustmentAmount,
    out_error: OpenPitOutParamError,
) -> *mut OpenPitSharedString {
    match import_adjustment_amount(value) {
        Ok(Some(amount)) => OpenPitSharedString::new_handle(amount.to_string().as_str()),
        Ok(None) => {
            write_param_error_unspecified(out_error, "adjustment amount is not set");
            std::ptr::null_mut()
        }
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            std::ptr::null_mut()
        }
    }
}

fn export_adjustment_amount(value: Option<AdjustmentAmount>) -> OpenPitParamAdjustmentAmount {
    match value {
        Some(AdjustmentAmount::Delta(v)) => OpenPitParamAdjustmentAmount {
            kind: OpenPitParamAdjustmentAmountKind::Delta,
            value: OpenPitParamPositionSize(v.to_decimal().into()),
        },
        Some(AdjustmentAmount::Absolute(v)) => OpenPitParamAdjustmentAmount {
            kind: OpenPitParamAdjustmentAmountKind::Absolute,
            value: OpenPitParamPositionSize(v.to_decimal().into()),
        },
        _ => OpenPitParamAdjustmentAmount::default(),
    }
}

fn import_balance_operation(
    value: OpenPitAccountAdjustmentBalanceOperation,
) -> Result<PopulatedBalanceOperation, String> {
    let asset = parse_asset_view(value.asset, "account_adjustment.balance.asset")?;

    let average_entry_price = if value.average_entry_price.is_set {
        Some(value.average_entry_price.value.to_param()?)
    } else {
        None
    };

    let realized_pnl = if value.realized_pnl.is_set {
        Some(value.realized_pnl.value.to_param()?)
    } else {
        None
    };

    Ok(PopulatedBalanceOperation {
        asset,
        average_entry_price,
        realized_pnl,
    })
}

fn import_position_operation(
    value: OpenPitAccountAdjustmentPositionOperation,
) -> Result<PopulatedPositionOperation, String> {
    let instrument = import_instrument(&value.instrument)?;
    let collateral_asset = parse_asset_view(
        value.collateral_asset,
        "account_adjustment.position.collateral_asset",
    )?;
    let average_entry_price = if value.average_entry_price.is_set {
        Some(value.average_entry_price.value.to_param()?)
    } else {
        None
    };
    let mode = import_position_mode(value.mode);

    Ok(PopulatedPositionOperation {
        instrument,
        collateral_asset,
        average_entry_price,
        mode,
        leverage: import_leverage(value.leverage),
    })
}

fn import_operation(
    value: OpenPitAccountAdjustmentOperation,
) -> Result<AccountAdjustmentOperationAccess, String> {
    match value.kind {
        OpenPitAccountAdjustmentOperationKind::Absent => {
            Ok(AccountAdjustmentOperationAccess::Absent)
        }
        OpenPitAccountAdjustmentOperationKind::Balance => {
            Ok(AccountAdjustmentOperationAccess::Populated(
                PopulatedAccountAdjustmentOperation::Balance(import_balance_operation(
                    value.balance,
                )?),
            ))
        }
        OpenPitAccountAdjustmentOperationKind::Position => {
            Ok(AccountAdjustmentOperationAccess::Populated(
                PopulatedAccountAdjustmentOperation::Position(import_position_operation(
                    value.position,
                )?),
            ))
        }
    }
}

fn import_amount(
    value: OpenPitAccountAdjustmentAmountOptional,
) -> Result<AccountAdjustmentAmountAccess, String> {
    if !value.is_set {
        return Ok(AccountAdjustmentAmountAccess::Absent);
    }

    Ok(AccountAdjustmentAmountAccess::Populated(
        AccountAdjustmentAmount {
            balance: import_adjustment_amount(value.value.balance)?,
            held: import_adjustment_amount(value.value.held)?,
            incoming: import_adjustment_amount(value.value.incoming)?,
        },
    ))
}

fn import_bound(value: OpenPitParamPositionSizeOptional) -> Result<Option<PositionSize>, String> {
    if !value.is_set {
        return Ok(None);
    }
    Ok(Some(value.value.to_param()?))
}

fn import_bounds(
    value: OpenPitAccountAdjustmentBoundsOptional,
) -> Result<AccountAdjustmentBoundsAccess, String> {
    if !value.is_set {
        return Ok(AccountAdjustmentBoundsAccess::Absent);
    }

    Ok(AccountAdjustmentBoundsAccess::Populated(
        AccountAdjustmentBounds {
            balance_upper: import_bound(value.value.balance_upper)?,
            balance_lower: import_bound(value.value.balance_lower)?,
            held_upper: import_bound(value.value.held_upper)?,
            held_lower: import_bound(value.value.held_lower)?,
            incoming_upper: import_bound(value.value.incoming_upper)?,
            incoming_lower: import_bound(value.value.incoming_lower)?,
        },
    ))
}

fn export_balance_operation(
    value: &PopulatedBalanceOperation,
) -> OpenPitAccountAdjustmentBalanceOperation {
    OpenPitAccountAdjustmentBalanceOperation {
        asset: match &value.asset {
            Some(asset) => OpenPitStringView::from_utf8(asset.as_ref()),
            None => OpenPitStringView::default(),
        },
        average_entry_price: match value.average_entry_price {
            Some(v) => OpenPitParamPriceOptional {
                is_set: true,
                value: OpenPitParamPrice(v.to_decimal().into()),
            },
            None => OpenPitParamPriceOptional::default(),
        },
        realized_pnl: match value.realized_pnl {
            Some(v) => OpenPitParamPnlOptional {
                is_set: true,
                value: OpenPitParamPnl(v.to_decimal().into()),
            },
            None => OpenPitParamPnlOptional::default(),
        },
    }
}

fn export_position_operation(
    value: &PopulatedPositionOperation,
) -> OpenPitAccountAdjustmentPositionOperation {
    OpenPitAccountAdjustmentPositionOperation {
        instrument: match &value.instrument {
            Some(instrument) => OpenPitInstrument {
                underlying_asset: OpenPitStringView::from_utf8(
                    instrument.underlying_asset().as_ref(),
                ),
                settlement_asset: OpenPitStringView::from_utf8(
                    instrument.settlement_asset().as_ref(),
                ),
            },
            None => OpenPitInstrument::default(),
        },
        collateral_asset: match &value.collateral_asset {
            Some(collateral_asset) => OpenPitStringView::from_utf8(collateral_asset.as_ref()),
            None => OpenPitStringView::default(),
        },
        average_entry_price: match value.average_entry_price {
            Some(v) => OpenPitParamPriceOptional {
                is_set: true,
                value: OpenPitParamPrice(v.to_decimal().into()),
            },
            None => OpenPitParamPriceOptional::default(),
        },
        leverage: export_leverage(value.leverage),
        mode: match value.mode {
            Some(mode) => export_position_mode(mode),
            None => OpenPitParamPositionMode::default(),
        },
    }
}

fn export_operation(value: &AccountAdjustmentOperationAccess) -> OpenPitAccountAdjustmentOperation {
    match value {
        AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Balance(v),
        ) => OpenPitAccountAdjustmentOperation {
            kind: OpenPitAccountAdjustmentOperationKind::Balance,
            balance: export_balance_operation(v),
            position: OpenPitAccountAdjustmentPositionOperation::default(),
        },
        AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Position(v),
        ) => OpenPitAccountAdjustmentOperation {
            kind: OpenPitAccountAdjustmentOperationKind::Position,
            balance: OpenPitAccountAdjustmentBalanceOperation::default(),
            position: export_position_operation(v),
        },
        AccountAdjustmentOperationAccess::Absent => OpenPitAccountAdjustmentOperation::default(),
    }
}

fn export_amount(value: &AccountAdjustmentAmount) -> OpenPitAccountAdjustmentAmount {
    OpenPitAccountAdjustmentAmount {
        balance: export_adjustment_amount(value.balance),
        held: export_adjustment_amount(value.held),
        incoming: export_adjustment_amount(value.incoming),
    }
}

fn export_bound(value: Option<PositionSize>) -> OpenPitParamPositionSizeOptional {
    match value {
        Some(v) => OpenPitParamPositionSizeOptional {
            is_set: true,
            value: OpenPitParamPositionSize(v.to_decimal().into()),
        },
        None => OpenPitParamPositionSizeOptional::default(),
    }
}

fn export_bounds(value: &AccountAdjustmentBounds) -> OpenPitAccountAdjustmentBounds {
    OpenPitAccountAdjustmentBounds {
        balance_upper: export_bound(value.balance_upper),
        balance_lower: export_bound(value.balance_lower),
        held_upper: export_bound(value.held_upper),
        held_lower: export_bound(value.held_lower),
        incoming_upper: export_bound(value.incoming_upper),
        incoming_lower: export_bound(value.incoming_lower),
    }
}

pub(crate) fn import_account_adjustment(
    value: &OpenPitAccountAdjustment,
) -> Result<AccountAdjustment, String> {
    // The engine applies adjustments as owned domain values, so decoding a
    // borrowed adjustment view necessarily builds owned data here.
    let operation = import_operation(value.operation)?;

    Ok(RequestWithPayload::new(
        openpit_interop::AccountAdjustment {
            operation,
            amount: import_amount(value.amount)?,
            bounds: import_bounds(value.bounds)?,
        },
        value.user_data,
    ))
}

pub(crate) fn export_account_adjustment(value: &AccountAdjustment) -> OpenPitAccountAdjustment {
    OpenPitAccountAdjustment {
        operation: export_operation(&value.request.operation),
        amount: match &value.request.amount {
            AccountAdjustmentAmountAccess::Populated(v) => OpenPitAccountAdjustmentAmountOptional {
                value: export_amount(v),
                is_set: true,
            },
            AccountAdjustmentAmountAccess::Absent => {
                OpenPitAccountAdjustmentAmountOptional::default()
            }
        },
        bounds: match &value.request.bounds {
            AccountAdjustmentBoundsAccess::Populated(v) => OpenPitAccountAdjustmentBoundsOptional {
                value: export_bounds(v),
                is_set: true,
            },
            AccountAdjustmentBoundsAccess::Absent => {
                OpenPitAccountAdjustmentBoundsOptional::default()
            }
        },
        user_data: value.payload,
    }
}

/// FFI account-adjustment request paired with an opaque caller-defined token.
///
/// The token is stored in [`RequestWithPayload::payload`]. The SDK never
/// inspects, dereferences, or frees this value. Its meaning, lifetime, and
/// thread-safety are the caller's responsibility. A null pointer means
/// "not set". See the project Threading Contract for the full lifetime model.
///
/// The token is preserved unchanged across every engine callback that
/// receives the carrying value, including policy callbacks and adjustment
/// callbacks.
pub type AccountAdjustment = RequestWithPayload<openpit_interop::AccountAdjustment, *mut c_void>;

#[cfg(test)]
mod tests {
    use crate::OpenPitStringView;

    use super::{
        export_account_adjustment, import_account_adjustment, OpenPitAccountAdjustment,
        OpenPitAccountAdjustmentAmount, OpenPitAccountAdjustmentAmountOptional,
        OpenPitAccountAdjustmentBalanceOperation, OpenPitAccountAdjustmentBounds,
        OpenPitAccountAdjustmentBoundsOptional, OpenPitAccountAdjustmentOperation,
        OpenPitAccountAdjustmentOperationKind, OpenPitAccountAdjustmentPositionOperation,
        OpenPitParamAdjustmentAmount,
    };
    use crate::instrument::OpenPitInstrument;
    use crate::param::{
        OpenPitParamAdjustmentAmountKind, OpenPitParamPnl, OpenPitParamPnlOptional,
        OpenPitParamPositionMode, OpenPitParamPositionSize, OpenPitParamPositionSizeOptional,
        OpenPitParamPrice,
    };
    use openpit::param::{
        AdjustmentAmount, Asset, Leverage, Pnl, PositionMode, PositionSize, Price,
    };
    use openpit::{AccountAdjustmentAmount, AccountAdjustmentBounds, Instrument};
    use openpit_interop::{
        AccountAdjustmentAmountAccess, AccountAdjustmentBoundsAccess,
        AccountAdjustmentOperationAccess, PopulatedAccountAdjustmentOperation,
        PopulatedBalanceOperation, PopulatedPositionOperation, RequestWithPayload,
    };

    fn sample_balance_adjustment() -> OpenPitAccountAdjustment {
        OpenPitAccountAdjustment {
            operation: OpenPitAccountAdjustmentOperation {
                kind: OpenPitAccountAdjustmentOperationKind::Balance,
                balance: OpenPitAccountAdjustmentBalanceOperation {
                    asset: OpenPitStringView {
                        ptr: b"USD".as_ptr(),
                        len: 3,
                    },
                    average_entry_price: crate::param::OpenPitParamPriceOptional {
                        value: OpenPitParamPrice(
                            Price::from_str("10").expect("price").to_decimal().into(),
                        ),
                        is_set: true,
                    },
                    realized_pnl: OpenPitParamPnlOptional {
                        value: OpenPitParamPnl(
                            Pnl::from_str("7").expect("pnl").to_decimal().into(),
                        ),
                        is_set: true,
                    },
                },
                position: OpenPitAccountAdjustmentPositionOperation::default(),
            },
            amount: OpenPitAccountAdjustmentAmountOptional {
                is_set: true,
                value: OpenPitAccountAdjustmentAmount {
                    balance: OpenPitParamAdjustmentAmount {
                        value: OpenPitParamPositionSize(
                            PositionSize::from_str("1")
                                .expect("size")
                                .to_decimal()
                                .into(),
                        ),
                        kind: OpenPitParamAdjustmentAmountKind::Delta,
                    },
                    held: OpenPitParamAdjustmentAmount {
                        value: OpenPitParamPositionSize(
                            PositionSize::from_str("2")
                                .expect("size")
                                .to_decimal()
                                .into(),
                        ),
                        kind: OpenPitParamAdjustmentAmountKind::Absolute,
                    },
                    incoming: OpenPitParamAdjustmentAmount::default(),
                },
            },
            bounds: OpenPitAccountAdjustmentBoundsOptional {
                is_set: true,
                value: OpenPitAccountAdjustmentBounds {
                    balance_upper: OpenPitParamPositionSizeOptional {
                        is_set: true,
                        value: OpenPitParamPositionSize(
                            PositionSize::from_str("100")
                                .expect("size")
                                .to_decimal()
                                .into(),
                        ),
                    },
                    balance_lower: OpenPitParamPositionSizeOptional::default(),
                    held_upper: OpenPitParamPositionSizeOptional::default(),
                    held_lower: OpenPitParamPositionSizeOptional::default(),
                    incoming_upper: OpenPitParamPositionSizeOptional::default(),
                    incoming_lower: OpenPitParamPositionSizeOptional::default(),
                },
            },
            user_data: std::ptr::null_mut(),
        }
    }

    #[test]
    fn import_account_adjustment_accepts_balance_payload() {
        let imported = import_account_adjustment(&sample_balance_adjustment()).expect("import");

        let operation =
            if let AccountAdjustmentOperationAccess::Populated(op) = &imported.request.operation {
                op
            } else {
                panic!("operation must be populated");
            };
        assert_eq!(
            *operation,
            PopulatedAccountAdjustmentOperation::Balance(PopulatedBalanceOperation {
                asset: Some(Asset::new("USD").expect("asset")),
                average_entry_price: Some(Price::from_str("10").expect("price")),
                realized_pnl: Some(Pnl::from_str("7").expect("pnl")),
            })
        );

        let amount = if let AccountAdjustmentAmountAccess::Populated(a) = &imported.request.amount {
            a
        } else {
            panic!("amount must be populated");
        };
        assert_eq!(
            *amount,
            AccountAdjustmentAmount {
                balance: Some(AdjustmentAmount::Delta(
                    PositionSize::from_str("1").expect("size"),
                )),
                held: Some(AdjustmentAmount::Absolute(
                    PositionSize::from_str("2").expect("size"),
                )),
                incoming: None,
            }
        );

        let bounds = if let AccountAdjustmentBoundsAccess::Populated(b) = &imported.request.bounds {
            b
        } else {
            panic!("bounds must be populated");
        };
        assert_eq!(
            *bounds,
            AccountAdjustmentBounds {
                balance_upper: Some(PositionSize::from_str("100").expect("size")),
                balance_lower: None,
                held_upper: None,
                held_lower: None,
                incoming_upper: None,
                incoming_lower: None,
            }
        );
    }

    #[test]
    fn import_account_adjustment_ignores_unselected_payload() {
        // The discriminant selects the payload; a stray balance payload left in
        // the position-selected struct is structurally ignored, so "both set"
        // can never be observed by the importer.
        let mut input = sample_balance_adjustment();
        input.operation.kind = OpenPitAccountAdjustmentOperationKind::Position;
        input.operation.position = OpenPitAccountAdjustmentPositionOperation {
            instrument: OpenPitInstrument {
                underlying_asset: OpenPitStringView {
                    ptr: b"AAPL".as_ptr(),
                    len: 4,
                },
                settlement_asset: OpenPitStringView {
                    ptr: b"USD".as_ptr(),
                    len: 3,
                },
            },
            collateral_asset: OpenPitStringView {
                ptr: b"USD".as_ptr(),
                len: 3,
            },
            average_entry_price: crate::param::OpenPitParamPriceOptional {
                is_set: true,
                value: OpenPitParamPrice(Price::from_str("1").expect("price").to_decimal().into()),
            },
            leverage: 10,
            mode: OpenPitParamPositionMode::Netting,
        };

        let imported = import_account_adjustment(&input).expect("import");
        assert!(matches!(
            imported.request.operation,
            AccountAdjustmentOperationAccess::Populated(
                PopulatedAccountAdjustmentOperation::Position(_)
            )
        ));
    }

    #[test]
    fn import_account_adjustment_accepts_absent_operation() {
        let input = OpenPitAccountAdjustment {
            operation: OpenPitAccountAdjustmentOperation::default(),
            amount: OpenPitAccountAdjustmentAmountOptional::default(),
            bounds: OpenPitAccountAdjustmentBoundsOptional::default(),
            user_data: std::ptr::null_mut(),
        };

        let imported = import_account_adjustment(&input).expect("import");
        assert_eq!(
            imported.request.operation,
            AccountAdjustmentOperationAccess::Absent
        );
    }

    #[test]
    fn import_account_adjustment_passes_absent_position_fields_through() {
        let input = OpenPitAccountAdjustment {
            operation: OpenPitAccountAdjustmentOperation {
                kind: OpenPitAccountAdjustmentOperationKind::Position,
                balance: OpenPitAccountAdjustmentBalanceOperation::default(),
                position: OpenPitAccountAdjustmentPositionOperation {
                    instrument: OpenPitInstrument::default(),
                    collateral_asset: OpenPitStringView::not_set(),
                    average_entry_price: crate::param::OpenPitParamPriceOptional::default(),
                    leverage: 10,
                    mode: OpenPitParamPositionMode::Hedged,
                },
            },
            amount: OpenPitAccountAdjustmentAmountOptional::default(),
            bounds: OpenPitAccountAdjustmentBoundsOptional::default(),
            user_data: std::ptr::null_mut(),
        };

        // The FFI layer is a pure proxy: absent required fields are forwarded as
        // `None`, and required-on-demand validation happens in the interop layer.
        let imported = import_account_adjustment(&input).expect("import");
        assert_eq!(
            imported.request.operation,
            AccountAdjustmentOperationAccess::Populated(
                PopulatedAccountAdjustmentOperation::Position(PopulatedPositionOperation {
                    instrument: None,
                    collateral_asset: None,
                    average_entry_price: None,
                    mode: Some(PositionMode::Hedged),
                    leverage: Leverage::from_raw(10).ok(),
                }),
            )
        );
    }

    #[test]
    fn export_account_adjustment_produces_operation_specific_group() {
        let domain = RequestWithPayload::new(
            openpit_interop::AccountAdjustment {
                operation: AccountAdjustmentOperationAccess::Populated(
                    PopulatedAccountAdjustmentOperation::Position(PopulatedPositionOperation {
                        instrument: Some(Instrument::new(
                            Asset::new("SPX").expect("asset"),
                            Asset::new("USD").expect("asset"),
                        )),
                        collateral_asset: Some(Asset::new("EUR").expect("asset")),
                        average_entry_price: Some(Price::from_str("5").expect("price")),
                        mode: Some(PositionMode::Hedged),
                        leverage: None,
                    }),
                ),
                amount: AccountAdjustmentAmountAccess::Absent,
                bounds: AccountAdjustmentBoundsAccess::Absent,
            },
            std::ptr::null_mut(),
        );

        let exported = export_account_adjustment(&domain);
        assert_eq!(
            exported.operation.kind,
            OpenPitAccountAdjustmentOperationKind::Position
        );
        assert_eq!(
            exported.operation.balance,
            OpenPitAccountAdjustmentBalanceOperation::default()
        );
        assert_eq!(
            exported.operation.position.instrument.underlying_asset.len,
            3
        );
        assert_eq!(
            exported.operation.position.instrument.settlement_asset.len,
            3
        );
        assert_eq!(exported.operation.position.collateral_asset.len, 3);
        assert!(exported.operation.position.average_entry_price.is_set);
        assert_eq!(
            exported.operation.position.mode,
            OpenPitParamPositionMode::Hedged
        );
    }

    #[test]
    fn import_export_account_adjustment_roundtrip() {
        let domain = RequestWithPayload::new(
            openpit_interop::AccountAdjustment {
                operation: AccountAdjustmentOperationAccess::Absent,
                amount: AccountAdjustmentAmountAccess::Populated(AccountAdjustmentAmount {
                    balance: Some(AdjustmentAmount::Absolute(
                        PositionSize::from_str("4").expect("size"),
                    )),
                    held: None,
                    incoming: Some(AdjustmentAmount::Delta(
                        PositionSize::from_str("1").expect("size"),
                    )),
                }),
                bounds: AccountAdjustmentBoundsAccess::Populated(AccountAdjustmentBounds {
                    balance_upper: Some(PositionSize::from_str("8").expect("size")),
                    balance_lower: None,
                    held_upper: None,
                    held_lower: None,
                    incoming_upper: None,
                    incoming_lower: Some(PositionSize::from_str("-2").expect("size")),
                }),
            },
            std::ptr::null_mut(),
        );

        let exported = export_account_adjustment(&domain);
        let imported = import_account_adjustment(&exported).expect("import");
        assert_eq!(imported, domain);
    }
}
