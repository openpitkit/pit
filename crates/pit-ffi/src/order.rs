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

use openpit::param::AccountId;
use openpit::{
    HasInstrument, HasOrderPrice, HasTradeAmount, OrderMargin, OrderPosition,
    RequestFieldAccessError,
};
use pit_interop::{OrderOperationAccess, PopulatedOrderOperation};

use crate::instrument::{import_instrument, parse_asset_view, PitInstrument};
use crate::param::{
    export_bool, export_leverage, export_position_side, export_side, export_trade_amount,
    import_bool, import_leverage, import_position_side, import_side, import_trade_amount,
    PitParamAccountIdOptional, PitParamLeverage, PitParamPositionSide, PitParamPriceOptional,
    PitParamSide, PitParamTradeAmount, PitTriBool,
};
use crate::PitStringView;

fn import_operation(
    value: PitOrderOperationOptional,
) -> Result<Option<PopulatedOrderOperation>, String> {
    if !value.is_set {
        return Ok(None);
    }

    Ok(Some(PopulatedOrderOperation {
        instrument: import_instrument(&value.value.instrument)?,
        account_id: if value.value.account_id.is_set {
            Some(AccountId::from_u64(value.value.account_id.value))
        } else {
            None
        },
        side: import_side(value.value.side),
        trade_amount: import_trade_amount(value.value.trade_amount)?,
        price: if value.value.price.is_set {
            Some(value.value.price.value.to_param()?)
        } else {
            None
        },
    }))
}

fn import_position(value: PitOrderPositionOptional) -> Option<OrderPosition> {
    if !value.is_set {
        return None;
    }

    Some(OrderPosition {
        position_side: import_position_side(value.value.position_side),
        reduce_only: import_bool(value.value.reduce_only).unwrap_or(false),
        close_position: import_bool(value.value.close_position).unwrap_or(false),
    })
}

fn import_margin(value: PitOrderMarginOptional) -> Result<Option<OrderMargin>, String> {
    if !value.is_set {
        return Ok(None);
    }

    Ok(Some(OrderMargin {
        leverage: import_leverage(value.value.leverage),
        collateral_asset: parse_asset_view(
            value.value.collateral_asset,
            "margin.collateral_asset",
        )?,
        auto_borrow: import_bool(value.value.auto_borrow).unwrap_or(false),
    }))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional operation group for an order.
///
/// The group is absent when all fields are absent.
pub struct PitOrderOperation {
    /// Optional trade amount payload.
    pub trade_amount: PitParamTradeAmount,
    /// Trading instrument.
    pub instrument: PitInstrument,
    /// Optional limit price.
    pub price: PitParamPriceOptional,
    /// Optional account identifier.
    pub account_id: PitParamAccountIdOptional,
    /// Optional buy/sell direction.
    pub side: PitParamSide,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional position-management group for an order.
///
/// The group is absent when every field is `NotSet`.
pub struct PitOrderPosition {
    /// Optional long/short side.
    pub position_side: PitParamPositionSide,
    /// Reduce-only flag.
    pub reduce_only: PitTriBool,
    /// Close-position flag.
    pub close_position: PitTriBool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional margin group for an order.
///
/// The group is absent when every field is `NotSet`.
pub struct PitOrderMargin {
    /// Optional collateral asset.
    pub collateral_asset: PitStringView,
    /// Auto-borrow flag.
    pub auto_borrow: PitTriBool,
    /// Optional leverage value.
    pub leverage: PitParamLeverage,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Full caller-owned order payload.
pub struct PitOrder {
    /// Optional main operation group.
    pub operation: PitOrderOperationOptional,
    /// Optional margin group.
    pub margin: PitOrderMarginOptional,
    /// Optional position-management group.
    pub position: PitOrderPositionOptional,
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

define_optional!(
    optional = PitOrderOperationOptional,
    value = PitOrderOperation
);
define_optional!(optional = PitOrderMarginOptional, value = PitOrderMargin);
define_optional!(
    optional = PitOrderPositionOptional,
    value = PitOrderPosition
);

pub(crate) fn import_order(value: &PitOrder) -> Result<Order, String> {
    // The engine works with owned domain objects, so decoding a borrowed order
    // view necessarily builds owned values here.
    Ok(Order {
        operation: match import_operation(value.operation)? {
            Some(v) => OrderOperationAccess::Populated(v),
            None => OrderOperationAccess::Absent,
        },
        position: import_position(value.position),
        margin: import_margin(value.margin)?,
        user_data: value.user_data,
    })
}

pub(crate) fn export_order(value: &Order) -> PitOrder {
    let operation = match &value.operation {
        OrderOperationAccess::Populated(v) => {
            let instrument = if let Some(instrument) = &v.instrument {
                PitInstrument {
                    underlying_asset: PitStringView::from_utf8(
                        instrument.underlying_asset().as_ref(),
                    ),
                    settlement_asset: PitStringView::from_utf8(
                        instrument.settlement_asset().as_ref(),
                    ),
                }
            } else {
                PitInstrument::default()
            };

            PitOrderOperationOptional {
                is_set: true,
                value: PitOrderOperation {
                    instrument,
                    price: match v.price {
                        Some(price) => PitParamPriceOptional {
                            value: crate::param::PitParamPrice(price.to_decimal().into()),
                            is_set: true,
                        },
                        None => PitParamPriceOptional::default(),
                    },
                    trade_amount: export_trade_amount(v.trade_amount),
                    account_id: match v.account_id {
                        Some(account_id) => PitParamAccountIdOptional {
                            value: account_id.as_u64(),
                            is_set: true,
                        },
                        None => PitParamAccountIdOptional::default(),
                    },
                    side: v.side.map(export_side).unwrap_or_default(),
                },
            }
        }
        OrderOperationAccess::Absent => PitOrderOperationOptional::default(),
    };

    let position = match &value.position {
        Some(position) => PitOrderPositionOptional {
            is_set: true,
            value: PitOrderPosition {
                position_side: position
                    .position_side
                    .map(export_position_side)
                    .unwrap_or_default(),
                reduce_only: export_bool(position.reduce_only),
                close_position: export_bool(position.close_position),
            },
        },
        None => PitOrderPositionOptional::default(),
    };

    let margin = match &value.margin {
        Some(margin) => {
            let collateral_asset = if let Some(asset) = margin.collateral_asset.as_ref() {
                PitStringView::from_utf8(asset.as_ref())
            } else {
                PitStringView::not_set()
            };
            PitOrderMarginOptional {
                is_set: true,
                value: PitOrderMargin {
                    collateral_asset,
                    leverage: export_leverage(margin.leverage),
                    auto_borrow: export_bool(margin.auto_borrow),
                },
            }
        }
        None => PitOrderMarginOptional::default(),
    };

    PitOrder {
        operation,
        margin,
        position,
        user_data: value.user_data,
    }
}

/// Order payload carrying validated domain values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    pub operation: OrderOperationAccess,
    pub position: Option<OrderPosition>,
    pub margin: Option<OrderMargin>,
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

impl Default for Order {
    fn default() -> Self {
        Self {
            operation: OrderOperationAccess::Absent,
            position: None,
            margin: None,
            user_data: std::ptr::null_mut(),
        }
    }
}

impl HasInstrument for Order {
    fn instrument(&self) -> Result<&openpit::Instrument, RequestFieldAccessError> {
        self.operation.instrument()
    }
}

impl HasTradeAmount for Order {
    fn trade_amount(&self) -> Result<openpit::param::TradeAmount, RequestFieldAccessError> {
        self.operation.trade_amount()
    }
}

impl HasOrderPrice for Order {
    fn price(&self) -> Result<Option<openpit::param::Price>, RequestFieldAccessError> {
        self.operation.price()
    }
}

#[cfg(test)]
mod tests {
    use openpit::param::{Asset, Price, Quantity, Side, Volume};
    use openpit::{
        HasInstrument, HasOrderPrice, HasTradeAmount, Instrument, OrderMargin, OrderPosition,
    };
    use pit_interop::OrderOperationAccess;

    use super::{
        export_order, import_order, Order, PitOrder, PitOrderMargin, PitOrderMarginOptional,
        PitOrderOperation, PitOrderOperationOptional, PitOrderPosition, PitOrderPositionOptional,
    };
    use crate::param::{
        PitParamAccountIdOptional, PitParamLeverage, PitParamPositionSide, PitParamPrice,
        PitParamPriceOptional, PitParamQuantity, PitParamSide, PitParamTradeAmount,
        PitParamTradeAmountKind, PitTriBool,
    };
    use crate::{instrument::PitInstrument, PitStringView};

    fn sample_order() -> PitOrder {
        PitOrder {
            operation: PitOrderOperationOptional {
                is_set: true,
                value: PitOrderOperation {
                    instrument: PitInstrument {
                        underlying_asset: PitStringView {
                            ptr: b"SPX".as_ptr(),
                            len: 3,
                        },
                        settlement_asset: PitStringView {
                            ptr: b"USD".as_ptr(),
                            len: 3,
                        },
                    },
                    price: PitParamPriceOptional {
                        value: PitParamPrice(
                            openpit::param::Price::from_str("100")
                                .expect("valid")
                                .to_decimal()
                                .into(),
                        ),
                        is_set: true,
                    },
                    trade_amount: PitParamTradeAmount {
                        value: PitParamQuantity(
                            openpit::param::Quantity::from_str("2")
                                .expect("valid")
                                .to_decimal()
                                .into(),
                        )
                        .0,
                        kind: PitParamTradeAmountKind::Quantity,
                    },
                    account_id: PitParamAccountIdOptional {
                        value: 7,
                        is_set: true,
                    },
                    side: PitParamSide::Buy,
                },
            },
            position: PitOrderPositionOptional {
                is_set: true,
                value: PitOrderPosition {
                    position_side: PitParamPositionSide::Long,
                    reduce_only: PitTriBool::True,
                    close_position: PitTriBool::False,
                },
            },
            margin: PitOrderMarginOptional {
                is_set: true,
                value: PitOrderMargin {
                    collateral_asset: PitStringView {
                        ptr: b"USD".as_ptr(),
                        len: 3,
                    },
                    leverage: 200 as PitParamLeverage,
                    auto_borrow: PitTriBool::True,
                },
            },
            user_data: std::ptr::null_mut(),
        }
    }

    #[test]
    fn import_order_roundtrips_pod_payload() {
        let imported = import_order(&sample_order()).expect("order must import");
        assert_eq!(
            imported
                .instrument()
                .expect("instrument")
                .underlying_asset()
                .as_ref(),
            "SPX"
        );
        assert_eq!(
            imported
                .instrument()
                .expect("instrument")
                .settlement_asset()
                .as_ref(),
            "USD"
        );
        assert_eq!(
            imported.trade_amount().expect("amount"),
            openpit::param::TradeAmount::Quantity(Quantity::from_str("2").expect("valid"))
        );
        assert_eq!(
            imported.price().expect("price"),
            Some(Price::from_str("100").expect("valid"))
        );
        assert_eq!(
            imported.position,
            Some(OrderPosition {
                position_side: Some(openpit::param::PositionSide::Long),
                reduce_only: true,
                close_position: false,
            })
        );
        assert_eq!(
            imported.margin,
            Some(OrderMargin {
                leverage: openpit::param::Leverage::from_raw(200).ok(),
                collateral_asset: Some(Asset::new("USD").expect("valid")),
                auto_borrow: true,
            })
        );
    }

    #[test]
    fn import_order_rejects_partial_instrument() {
        let mut order = sample_order();
        order.operation.value.instrument.settlement_asset = PitStringView::not_set();
        let err = import_order(&order).expect_err("partial instrument must fail");
        assert!(err.contains("both underlying_asset and settlement_asset"));
    }

    #[test]
    fn export_order_produces_readable_views() {
        let order = Order {
            operation: OrderOperationAccess::Populated(pit_interop::PopulatedOrderOperation {
                instrument: Some(Instrument::new(
                    Asset::new("AAPL").expect("valid"),
                    Asset::new("USD").expect("valid"),
                )),
                account_id: Some(openpit::param::AccountId::from_u64(3)),
                side: Some(Side::Sell),
                trade_amount: Some(openpit::param::TradeAmount::Volume(
                    Volume::from_str("1500").expect("valid"),
                )),
                price: None,
            }),
            position: None,
            margin: Some(OrderMargin {
                leverage: None,
                collateral_asset: Some(Asset::new("USD").expect("valid")),
                auto_borrow: false,
            }),
            user_data: std::ptr::null_mut(),
        };

        let exported = export_order(&order);
        assert!(exported.operation.is_set);
        assert_eq!(exported.operation.value.side, PitParamSide::Sell);
        assert_eq!(
            exported.operation.value.trade_amount,
            PitParamTradeAmount {
                value: Volume::from_str("1500").expect("valid").to_decimal().into(),
                kind: PitParamTradeAmountKind::Volume
            }
        );
        assert_eq!(exported.operation.value.instrument.underlying_asset.len, 4);
        assert_eq!(exported.margin.value.collateral_asset.len, 3);
        assert_eq!(exported.margin.value.auto_borrow, PitTriBool::False);
    }

    #[test]
    fn import_order_preserves_present_false_boolean_groups() {
        let order = PitOrder {
            operation: PitOrderOperationOptional::default(),
            margin: PitOrderMarginOptional {
                is_set: true,
                value: PitOrderMargin {
                    collateral_asset: PitStringView::not_set(),
                    leverage: PitParamLeverage::default(),
                    auto_borrow: PitTriBool::False,
                },
            },
            position: PitOrderPositionOptional {
                is_set: true,
                value: PitOrderPosition {
                    position_side: PitParamPositionSide::NotSet,
                    reduce_only: PitTriBool::False,
                    close_position: PitTriBool::False,
                },
            },
            user_data: std::ptr::null_mut(),
        };

        let imported = import_order(&order).expect("order must import");
        assert_eq!(
            imported.position,
            Some(OrderPosition {
                position_side: None,
                reduce_only: false,
                close_position: false,
            })
        );
        assert_eq!(
            imported.margin,
            Some(OrderMargin {
                leverage: None,
                collateral_asset: None,
                auto_borrow: false,
            })
        );
    }
}
