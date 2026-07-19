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
use openpit_interop::{
    OrderMarginAccess, OrderOperationAccess, OrderPositionAccess, PopulatedOrderMargin,
    PopulatedOrderOperation, PopulatedOrderPosition, RequestWithPayload,
};

use crate::instrument::{import_instrument, parse_asset_view, OpenPitInstrument};
use crate::param::{
    export_bool, export_leverage, export_position_side, export_side, export_trade_amount,
    import_bool, import_leverage, import_position_side, import_side, import_trade_amount,
    OpenPitParamAccountIdOptional, OpenPitParamLeverage, OpenPitParamPositionSide,
    OpenPitParamPriceOptional, OpenPitParamSide, OpenPitParamTradeAmount, OpenPitTriBool,
};
use crate::OpenPitStringView;

fn import_operation(
    value: OpenPitOrderOperationOptional,
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
        side: import_side(value.value.side)?,
        trade_amount: import_trade_amount(value.value.trade_amount)?,
        price: if value.value.price.is_set {
            Some(value.value.price.value.to_param()?)
        } else {
            None
        },
    }))
}

fn import_position(value: OpenPitOrderPositionOptional) -> Result<OrderPositionAccess, String> {
    if !value.is_set {
        return Ok(OrderPositionAccess::Absent);
    }

    Ok(OrderPositionAccess::Populated(PopulatedOrderPosition {
        position_side: import_position_side(value.value.position_side)?,
        reduce_only: import_bool(value.value.reduce_only)?.unwrap_or(false),
        close_position: import_bool(value.value.close_position)?.unwrap_or(false),
    }))
}

fn import_margin(value: OpenPitOrderMarginOptional) -> Result<OrderMarginAccess, String> {
    if !value.is_set {
        return Ok(OrderMarginAccess::Absent);
    }

    Ok(OrderMarginAccess::Populated(PopulatedOrderMargin {
        leverage: import_leverage(value.value.leverage),
        collateral_asset: parse_asset_view(
            value.value.collateral_asset,
            "margin.collateral_asset",
        )?,
        auto_borrow: import_bool(value.value.auto_borrow)?.unwrap_or(false),
    }))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional operation group for an order.
///
/// The group is absent when all fields are absent.
pub struct OpenPitOrderOperation {
    /// Optional trade amount payload.
    pub trade_amount: OpenPitParamTradeAmount,
    /// Trading instrument.
    pub instrument: OpenPitInstrument,
    /// Optional limit price.
    pub price: OpenPitParamPriceOptional,
    /// Optional account identifier.
    pub account_id: OpenPitParamAccountIdOptional,
    /// Optional buy/sell direction.
    pub side: OpenPitParamSide,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional position-management group for an order.
///
/// The group is absent when every field is `NotSet`.
pub struct OpenPitOrderPosition {
    /// Optional long/short side.
    pub position_side: OpenPitParamPositionSide,
    /// Reduce-only flag.
    pub reduce_only: OpenPitTriBool,
    /// Close-position flag.
    pub close_position: OpenPitTriBool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Optional margin group for an order.
///
/// The group is absent when every field is `NotSet`.
pub struct OpenPitOrderMargin {
    /// Optional collateral asset.
    pub collateral_asset: OpenPitStringView,
    /// Auto-borrow flag.
    pub auto_borrow: OpenPitTriBool,
    /// Optional leverage value.
    pub leverage: OpenPitParamLeverage,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Full caller-owned order payload.
pub struct OpenPitOrder {
    /// Optional main operation group.
    pub operation: OpenPitOrderOperationOptional,
    /// Optional margin group.
    pub margin: OpenPitOrderMarginOptional,
    /// Optional position-management group.
    pub position: OpenPitOrderPositionOptional,
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
    optional = OpenPitOrderOperationOptional,
    value = OpenPitOrderOperation
);
define_optional!(
    optional = OpenPitOrderMarginOptional,
    value = OpenPitOrderMargin
);
define_optional!(
    optional = OpenPitOrderPositionOptional,
    value = OpenPitOrderPosition
);

pub(crate) fn import_order(value: &OpenPitOrder) -> Result<Order, String> {
    // The engine works with owned domain objects, so decoding a borrowed order
    // view necessarily builds owned values here.
    Ok(RequestWithPayload::new(
        openpit_interop::Order {
            operation: match import_operation(value.operation)? {
                Some(v) => OrderOperationAccess::Populated(v),
                None => OrderOperationAccess::Absent,
            },
            position: import_position(value.position)?,
            margin: import_margin(value.margin)?,
        },
        value.user_data,
    ))
}

pub(crate) fn export_order(value: &Order) -> OpenPitOrder {
    let operation = match &value.request.operation {
        OrderOperationAccess::Populated(v) => {
            let instrument = if let Some(instrument) = &v.instrument {
                OpenPitInstrument {
                    underlying_asset: OpenPitStringView::from_utf8(
                        instrument.underlying_asset().as_ref(),
                    ),
                    settlement_asset: OpenPitStringView::from_utf8(
                        instrument.settlement_asset().as_ref(),
                    ),
                }
            } else {
                OpenPitInstrument::default()
            };

            OpenPitOrderOperationOptional {
                is_set: true,
                value: OpenPitOrderOperation {
                    instrument,
                    price: match v.price {
                        Some(price) => OpenPitParamPriceOptional {
                            value: crate::param::OpenPitParamPrice(price.to_decimal().into()),
                            is_set: true,
                        },
                        None => OpenPitParamPriceOptional::default(),
                    },
                    trade_amount: export_trade_amount(v.trade_amount),
                    account_id: match v.account_id {
                        Some(account_id) => OpenPitParamAccountIdOptional {
                            value: account_id.as_u64(),
                            is_set: true,
                        },
                        None => OpenPitParamAccountIdOptional::default(),
                    },
                    side: v.side.map(export_side).unwrap_or_default(),
                },
            }
        }
        OrderOperationAccess::Absent => OpenPitOrderOperationOptional::default(),
    };

    let position = match &value.request.position {
        OrderPositionAccess::Populated(position) => OpenPitOrderPositionOptional {
            is_set: true,
            value: OpenPitOrderPosition {
                position_side: position
                    .position_side
                    .map(export_position_side)
                    .unwrap_or_default(),
                reduce_only: export_bool(position.reduce_only),
                close_position: export_bool(position.close_position),
            },
        },
        OrderPositionAccess::Absent => OpenPitOrderPositionOptional::default(),
    };

    let margin = match &value.request.margin {
        OrderMarginAccess::Populated(margin) => {
            let collateral_asset = if let Some(asset) = margin.collateral_asset.as_ref() {
                OpenPitStringView::from_utf8(asset.as_ref())
            } else {
                OpenPitStringView::not_set()
            };
            OpenPitOrderMarginOptional {
                is_set: true,
                value: OpenPitOrderMargin {
                    collateral_asset,
                    leverage: export_leverage(margin.leverage),
                    auto_borrow: export_bool(margin.auto_borrow),
                },
            }
        }
        OrderMarginAccess::Absent => OpenPitOrderMarginOptional::default(),
    };

    OpenPitOrder {
        operation,
        margin,
        position,
        user_data: value.payload,
    }
}

/// FFI order request paired with an opaque caller-defined token.
///
/// The token is stored in [`RequestWithPayload::payload`]. The SDK never
/// inspects, dereferences, or frees this value. Its meaning, lifetime, and
/// thread-safety are the caller's responsibility. A null pointer means
/// "not set". See the project Threading Contract for the full lifetime model.
///
/// The token is preserved unchanged across every engine callback that
/// receives the carrying value, including policy callbacks and adjustment
/// callbacks.
pub type Order = RequestWithPayload<openpit_interop::Order, *mut c_void>;

#[cfg(test)]
mod tests {
    use openpit::param::{Asset, Price, Quantity, Side, Volume};
    use openpit::{HasInstrument, HasOrderPrice, HasTradeAmount};
    use openpit_interop::{
        OrderMarginAccess, OrderOperationAccess, OrderPositionAccess, PopulatedOrderMargin,
        PopulatedOrderOperation, PopulatedOrderPosition, RequestWithPayload,
    };

    use super::{
        export_order, import_order, OpenPitOrder, OpenPitOrderMargin, OpenPitOrderMarginOptional,
        OpenPitOrderOperation, OpenPitOrderOperationOptional, OpenPitOrderPosition,
        OpenPitOrderPositionOptional,
    };
    use crate::param::{
        OpenPitParamAccountIdOptional, OpenPitParamLeverage, OpenPitParamPrice,
        OpenPitParamPriceOptional, OpenPitParamQuantity, OpenPitParamTradeAmount,
    };
    use crate::{instrument::OpenPitInstrument, OpenPitStringView};

    fn sample_order() -> OpenPitOrder {
        OpenPitOrder {
            operation: OpenPitOrderOperationOptional {
                is_set: true,
                value: OpenPitOrderOperation {
                    instrument: OpenPitInstrument {
                        underlying_asset: OpenPitStringView {
                            ptr: b"SPX".as_ptr(),
                            len: 3,
                        },
                        settlement_asset: OpenPitStringView {
                            ptr: b"USD".as_ptr(),
                            len: 3,
                        },
                    },
                    price: OpenPitParamPriceOptional {
                        value: OpenPitParamPrice(
                            openpit::param::Price::from_str("100")
                                .expect("valid")
                                .to_decimal()
                                .into(),
                        ),
                        is_set: true,
                    },
                    trade_amount: OpenPitParamTradeAmount {
                        value: OpenPitParamQuantity(
                            openpit::param::Quantity::from_str("2")
                                .expect("valid")
                                .to_decimal()
                                .into(),
                        )
                        .0,
                        kind: crate::param::OPENPIT_PARAM_TRADE_AMOUNT_KIND_QUANTITY,
                    },
                    account_id: OpenPitParamAccountIdOptional {
                        value: 7,
                        is_set: true,
                    },
                    side: crate::param::OPENPIT_PARAM_SIDE_BUY,
                },
            },
            position: OpenPitOrderPositionOptional {
                is_set: true,
                value: OpenPitOrderPosition {
                    position_side: crate::param::OPENPIT_PARAM_POSITION_SIDE_LONG,
                    reduce_only: crate::param::OPENPIT_TRI_BOOL_TRUE,
                    close_position: crate::param::OPENPIT_TRI_BOOL_FALSE,
                },
            },
            margin: OpenPitOrderMarginOptional {
                is_set: true,
                value: OpenPitOrderMargin {
                    collateral_asset: OpenPitStringView {
                        ptr: b"USD".as_ptr(),
                        len: 3,
                    },
                    leverage: 200 as OpenPitParamLeverage,
                    auto_borrow: crate::param::OPENPIT_TRI_BOOL_TRUE,
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
            imported.request.position,
            OrderPositionAccess::Populated(PopulatedOrderPosition {
                position_side: Some(openpit::param::PositionSide::Long),
                reduce_only: true,
                close_position: false,
            })
        );
        assert_eq!(
            imported.request.margin,
            OrderMarginAccess::Populated(PopulatedOrderMargin {
                leverage: openpit::param::Leverage::from_raw(200).ok(),
                collateral_asset: Some(Asset::new("USD").expect("valid")),
                auto_borrow: true,
            })
        );
    }

    #[test]
    fn import_order_rejects_partial_instrument() {
        let mut order = sample_order();
        order.operation.value.instrument.settlement_asset = OpenPitStringView::not_set();
        let err = import_order(&order).expect_err("partial instrument must fail");
        assert!(err.contains("both underlying_asset and settlement_asset"));
    }

    #[test]
    fn export_order_produces_readable_views() {
        let order = RequestWithPayload::new(
            openpit_interop::Order {
                operation: OrderOperationAccess::Populated(PopulatedOrderOperation {
                    instrument: Some(openpit::Instrument::new(
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
                position: OrderPositionAccess::Absent,
                margin: OrderMarginAccess::Populated(PopulatedOrderMargin {
                    leverage: None,
                    collateral_asset: Some(Asset::new("USD").expect("valid")),
                    auto_borrow: false,
                }),
            },
            std::ptr::null_mut(),
        );

        let exported = export_order(&order);
        assert!(exported.operation.is_set);
        assert_eq!(
            exported.operation.value.side,
            crate::param::OPENPIT_PARAM_SIDE_SELL
        );
        assert_eq!(
            exported.operation.value.trade_amount,
            OpenPitParamTradeAmount {
                value: Volume::from_str("1500").expect("valid").to_decimal().into(),
                kind: crate::param::OPENPIT_PARAM_TRADE_AMOUNT_KIND_VOLUME
            }
        );
        assert_eq!(exported.operation.value.instrument.underlying_asset.len, 4);
        assert_eq!(exported.margin.value.collateral_asset.len, 3);
        assert_eq!(
            exported.margin.value.auto_borrow,
            crate::param::OPENPIT_TRI_BOOL_FALSE
        );
    }

    #[test]
    fn import_order_preserves_present_false_boolean_groups() {
        let order = OpenPitOrder {
            operation: OpenPitOrderOperationOptional::default(),
            margin: OpenPitOrderMarginOptional {
                is_set: true,
                value: OpenPitOrderMargin {
                    collateral_asset: OpenPitStringView::not_set(),
                    leverage: OpenPitParamLeverage::default(),
                    auto_borrow: crate::param::OPENPIT_TRI_BOOL_FALSE,
                },
            },
            position: OpenPitOrderPositionOptional {
                is_set: true,
                value: OpenPitOrderPosition {
                    position_side: crate::param::OPENPIT_PARAM_POSITION_SIDE_NOT_SET,
                    reduce_only: crate::param::OPENPIT_TRI_BOOL_FALSE,
                    close_position: crate::param::OPENPIT_TRI_BOOL_FALSE,
                },
            },
            user_data: std::ptr::null_mut(),
        };

        let imported = import_order(&order).expect("order must import");
        assert_eq!(
            imported.request.position,
            OrderPositionAccess::Populated(PopulatedOrderPosition {
                position_side: None,
                reduce_only: false,
                close_position: false,
            })
        );
        assert_eq!(
            imported.request.margin,
            OrderMarginAccess::Populated(PopulatedOrderMargin {
                leverage: None,
                collateral_asset: None,
                auto_borrow: false,
            })
        );
    }
}
