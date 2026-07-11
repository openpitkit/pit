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
// Please see https://openpit.dev and the OWNERS file for details.

//! Order payload bindings: `Order` and its groups.
//!
//! The order is assembled from three optional groups: `OrderOperation`,
//! `OrderPosition`, and `OrderMargin`. Each group is a mutable builder with a
//! parameterless constructor plus camelCase getters and setters. Every field is
//! individually optional, with split `underlyingAsset`/`settlementAsset`
//! values.
//!
//! `TradeAmount` is a tagged amount that is either a `Quantity` or a `Volume`.

use openpit::param::{Asset, Leverage, PositionSide, Price, Side, TradeAmount};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::domain::{
    clone_wrapper_value, extract_cloned_wrapper, is_plain_object, parse_asset, read_field,
    read_optional_bool, read_optional_string, resolve_optional_account_id,
    resolve_optional_leverage, resolve_optional_price, resolve_quantity, resolve_volume,
    LeverageLike, OptionalAccountIdLike, OptionalPositionSideLike, OptionalPriceLike,
    OptionalSideLike, QuantityLike, VolumeLike,
};
use crate::error::{make_error, ErrorKind};
use crate::param::enums::{resolve_optional_position_side, resolve_optional_side};
use crate::param::ids::JsAccountId;
use crate::param::leverage::JsLeverage;
use crate::param::value_types::{JsPrice, JsQuantity, JsVolume};

/// Builds the error raised when an order group field is the wrong JS type.
fn invalid_field(message: &str) -> JsValue {
    make_error(ErrorKind::Type, message, None)
}

#[wasm_bindgen(typescript_custom_section)]
const ORDER_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link OrderOperation}. Every field is optional and
 * accepts both the typed wrapper and the idiomatic primitive (an `accountId`
 * number/bigint/string, a `side` string, a `price` `DecimalInput`, ...).
 */
export interface OrderOperationInit {
  underlyingAsset?: string;
  settlementAsset?: string;
  accountId?: AccountId | number | bigint | string;
  side?: "BUY" | "SELL";
  tradeAmount?: TradeAmount;
  price?: Price | string | number | bigint;
}

/**
 * Plain-object form of {@link OrderPosition}.
 */
export interface OrderPositionInit {
  positionSide?: "LONG" | "SHORT";
  reduceOnly?: boolean;
  closePosition?: boolean;
}

/**
 * Plain-object form of {@link OrderMargin}.
 */
export interface OrderMarginInit {
  leverage?: Leverage | number | bigint | string;
  collateralAsset?: string;
  autoBorrow?: boolean;
}

/**
 * Plain-object form of {@link Order}. Each group accepts either the typed
 * wrapper or its own plain-object form.
 */
export interface OrderInit {
  operation?: OrderOperation | OrderOperationInit;
  position?: OrderPosition | OrderPositionInit;
  margin?: OrderMargin | OrderMarginInit;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// An [`OrderOperation`](JsOrderOperation) wrapper or its plain-object
    /// form.
    #[wasm_bindgen(typescript_type = "OrderOperation | OrderOperationInit")]
    pub type OrderOperationLike;

    /// An [`OrderPosition`](JsOrderPosition) wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "OrderPosition | OrderPositionInit")]
    pub type OrderPositionLike;

    /// An [`OrderMargin`](JsOrderMargin) wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "OrderMargin | OrderMarginInit")]
    pub type OrderMarginLike;

    /// An [`Order`](JsOrder) wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "Order | OrderInit")]
    pub type OrderLike;

    /// A `TradeAmount` wrapper (no plain-object form: use the static
    /// factories).
    #[wasm_bindgen(typescript_type = "TradeAmount | null | undefined")]
    pub type TradeAmountLike;
}

/// Reads an optional asset setter argument, mapping `undefined`/`null` to
/// clearing the field.
fn set_optional_asset(slot: &mut Option<Asset>, value: Option<String>) -> Result<(), JsValue> {
    *slot = match value {
        Some(text) => Some(parse_asset(&text)?),
        None => None,
    };
    Ok(())
}

/// A tagged trade amount: either a `Quantity` or a `Volume`.
#[wasm_bindgen(js_name = TradeAmount)]
#[derive(Clone, Copy)]
pub struct JsTradeAmount {
    inner: TradeAmount,
}

#[wasm_bindgen(js_class = TradeAmount)]
impl JsTradeAmount {
    /// Builds a quantity-denominated trade amount.
    ///
    /// Accepts a `Quantity` value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(js_name = quantity)]
    pub fn quantity(value: QuantityLike) -> Result<JsTradeAmount, JsValue> {
        let quantity = resolve_quantity(value.into())?;
        Ok(Self {
            inner: TradeAmount::Quantity(quantity),
        })
    }

    /// Builds a volume-denominated trade amount.
    ///
    /// Accepts a `Volume` value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(js_name = volume)]
    pub fn volume(value: VolumeLike) -> Result<JsTradeAmount, JsValue> {
        let volume = resolve_volume(value.into())?;
        Ok(Self {
            inner: TradeAmount::Volume(volume),
        })
    }

    /// Returns `true` when this amount is a quantity.
    #[wasm_bindgen(getter, js_name = isQuantity)]
    pub fn is_quantity(&self) -> bool {
        matches!(self.inner, TradeAmount::Quantity(_))
    }

    /// Returns `true` when this amount is a volume.
    #[wasm_bindgen(getter, js_name = isVolume)]
    pub fn is_volume(&self) -> bool {
        matches!(self.inner, TradeAmount::Volume(_))
    }

    /// Returns the quantity, or `undefined` when this is a volume amount.
    #[wasm_bindgen(getter, js_name = asQuantity)]
    pub fn as_quantity(&self) -> Option<JsQuantity> {
        match self.inner {
            TradeAmount::Quantity(quantity) => Some(JsQuantity::from_inner(quantity)),
            _ => None,
        }
    }

    /// Returns the volume, or `undefined` when this is a quantity amount.
    #[wasm_bindgen(getter, js_name = asVolume)]
    pub fn as_volume(&self) -> Option<JsVolume> {
        match self.inner {
            TradeAmount::Volume(volume) => Some(JsVolume::from_inner(volume)),
            _ => None,
        }
    }

    /// Returns a fresh `TradeAmount` holding the same value.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsTradeAmount {
        *self
    }
}

impl JsTradeAmount {
    /// Returns the wrapped core [`TradeAmount`].
    pub fn inner(&self) -> TradeAmount {
        self.inner
    }
}

/// Resolves a `TradeAmount` setter argument.
fn resolve_trade_amount(value: JsValue) -> Result<TradeAmount, JsValue> {
    extract_cloned_wrapper::<JsTradeAmount>(&value)?
        .map(|wrapped| wrapped.inner())
        .ok_or_else(|| invalid_field("tradeAmount must be a TradeAmount"))
}

/// Order operation group: instrument, account, side, amount, and price.
#[wasm_bindgen(js_name = OrderOperation)]
#[derive(Clone, Default)]
pub struct JsOrderOperation {
    underlying_asset: Option<Asset>,
    settlement_asset: Option<Asset>,
    account_id: Option<openpit::param::AccountId>,
    side: Option<Side>,
    trade_amount: Option<TradeAmount>,
    price: Option<Price>,
}

#[wasm_bindgen(js_class = OrderOperation)]
impl JsOrderOperation {
    /// Constructs an empty operation group; populate it through the setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsOrderOperation {
        Self::default()
    }

    /// The underlying (traded) asset, or `undefined`.
    #[wasm_bindgen(getter, js_name = underlyingAsset)]
    pub fn underlying_asset(&self) -> Option<String> {
        self.underlying_asset
            .as_ref()
            .map(|asset| asset.to_string())
    }

    /// Sets the underlying asset.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when the asset string is empty.
    #[wasm_bindgen(setter, js_name = underlyingAsset)]
    pub fn set_underlying_asset(&mut self, value: Option<String>) -> Result<(), JsValue> {
        set_optional_asset(&mut self.underlying_asset, value)
    }

    /// The settlement asset, or `undefined`.
    #[wasm_bindgen(getter, js_name = settlementAsset)]
    pub fn settlement_asset(&self) -> Option<String> {
        self.settlement_asset
            .as_ref()
            .map(|asset| asset.to_string())
    }

    /// Sets the settlement asset.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when the asset string is empty.
    #[wasm_bindgen(setter, js_name = settlementAsset)]
    pub fn set_settlement_asset(&mut self, value: Option<String>) -> Result<(), JsValue> {
        set_optional_asset(&mut self.settlement_asset, value)
    }

    /// The account identifier, or `undefined`.
    #[wasm_bindgen(getter, js_name = accountId)]
    pub fn account_id(&self) -> Option<JsAccountId> {
        self.account_id.map(JsAccountId::from_inner)
    }

    /// Sets the account identifier.
    ///
    /// Accepts an `AccountId` object or a `number | bigint | string` (mirroring
    /// `AccountId.fromInt`/`AccountId.fromString`).
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid identifier value.
    #[wasm_bindgen(setter, js_name = accountId)]
    pub fn set_account_id(&mut self, value: OptionalAccountIdLike) -> Result<(), JsValue> {
        self.account_id = resolve_optional_account_id(value.into())?;
        Ok(())
    }

    /// The trading side wire string (`"BUY"`/`"SELL"`), or `undefined`.
    #[wasm_bindgen(getter, js_name = side)]
    pub fn side(&self) -> Option<String> {
        self.side.map(|side| side.to_string())
    }

    /// Sets the trading side from its wire string.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not `"BUY"`/`"SELL"`.
    #[wasm_bindgen(setter, js_name = side)]
    pub fn set_side(&mut self, value: OptionalSideLike) -> Result<(), JsValue> {
        self.side = resolve_optional_side(value.into())?;
        Ok(())
    }

    /// The trade amount, or `undefined`.
    #[wasm_bindgen(getter, js_name = tradeAmount)]
    pub fn trade_amount(&self) -> Option<JsTradeAmount> {
        self.trade_amount.map(|inner| JsTradeAmount { inner })
    }

    /// Sets the trade amount.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not a `TradeAmount`.
    #[wasm_bindgen(setter, js_name = tradeAmount)]
    pub fn set_trade_amount(&mut self, value: TradeAmountLike) -> Result<(), JsValue> {
        let value: JsValue = value.into();
        self.trade_amount = if value.is_undefined() || value.is_null() {
            None
        } else {
            Some(resolve_trade_amount(value)?)
        };
        Ok(())
    }

    /// The limit price, or `undefined` for a market order.
    #[wasm_bindgen(getter, js_name = price)]
    pub fn price(&self) -> Option<JsPrice> {
        self.price.map(JsPrice::from_inner)
    }

    /// Sets the limit price (accepts a `Price` object or a `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid price.
    #[wasm_bindgen(setter, js_name = price)]
    pub fn set_price(&mut self, value: OptionalPriceLike) -> Result<(), JsValue> {
        self.price = resolve_optional_price(value.into())?;
        Ok(())
    }

    /// Returns a deep copy of this operation group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderOperation {
        self.clone()
    }
}

impl JsOrderOperation {
    /// Builds an operation group from a plain object literal, reading each
    /// field through the corresponding setter.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut operation = Self::default();
        operation.set_underlying_asset(read_optional_string(value, "underlyingAsset")?)?;
        operation.set_settlement_asset(read_optional_string(value, "settlementAsset")?)?;
        operation.set_account_id(read_field(value, "accountId")?.unchecked_into())?;
        operation.set_side(read_field(value, "side")?.unchecked_into())?;
        operation.set_trade_amount(read_field(value, "tradeAmount")?.unchecked_into())?;
        operation.set_price(read_field(value, "price")?.unchecked_into())?;
        Ok(operation)
    }
}

impl JsOrderOperation {
    /// Builds the instrument from the asset pair, or `None` if either is unset.
    pub(crate) fn instrument(&self) -> Option<openpit::Instrument> {
        match (&self.underlying_asset, &self.settlement_asset) {
            (Some(underlying), Some(settlement)) => Some(openpit::Instrument::new(
                underlying.clone(),
                settlement.clone(),
            )),
            _ => None,
        }
    }

    /// Returns the wrapped account id.
    pub(crate) fn account_id_inner(&self) -> Option<openpit::param::AccountId> {
        self.account_id
    }

    /// Returns the wrapped side.
    pub(crate) fn side_inner(&self) -> Option<Side> {
        self.side
    }

    /// Returns the wrapped trade amount.
    pub(crate) fn trade_amount_inner(&self) -> Option<TradeAmount> {
        self.trade_amount
    }

    /// Returns the wrapped price.
    pub(crate) fn price_inner(&self) -> Option<Price> {
        self.price
    }
}

/// Order position group: hedged/netting side and lifecycle flags.
#[wasm_bindgen(js_name = OrderPosition)]
#[derive(Clone, Default)]
pub struct JsOrderPosition {
    position_side: Option<PositionSide>,
    reduce_only: bool,
    close_position: bool,
}

#[wasm_bindgen(js_class = OrderPosition)]
impl JsOrderPosition {
    /// Constructs an empty position group (`reduceOnly`/`closePosition` false).
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsOrderPosition {
        Self::default()
    }

    /// The position side wire string (`"LONG"`/`"SHORT"`), or `undefined`.
    #[wasm_bindgen(getter, js_name = positionSide)]
    pub fn position_side(&self) -> Option<String> {
        self.position_side.map(|side| side.to_string())
    }

    /// Sets the position side from its wire string.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not `"LONG"`/`"SHORT"`.
    #[wasm_bindgen(setter, js_name = positionSide)]
    pub fn set_position_side(&mut self, value: OptionalPositionSideLike) -> Result<(), JsValue> {
        self.position_side = resolve_optional_position_side(value.into())?;
        Ok(())
    }

    /// Whether the order may only reduce an existing position.
    #[wasm_bindgen(getter, js_name = reduceOnly)]
    pub fn reduce_only(&self) -> bool {
        self.reduce_only
    }

    /// Sets the reduce-only flag.
    #[wasm_bindgen(setter, js_name = reduceOnly)]
    pub fn set_reduce_only(&mut self, value: bool) {
        self.reduce_only = value;
    }

    /// Whether the order closes the entire position.
    #[wasm_bindgen(getter, js_name = closePosition)]
    pub fn close_position(&self) -> bool {
        self.close_position
    }

    /// Sets the close-position flag.
    #[wasm_bindgen(setter, js_name = closePosition)]
    pub fn set_close_position(&mut self, value: bool) {
        self.close_position = value;
    }

    /// Returns a deep copy of this position group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderPosition {
        self.clone()
    }
}

impl JsOrderPosition {
    /// Builds a position group from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut position = Self::default();
        position.set_position_side(read_field(value, "positionSide")?.unchecked_into())?;
        if let Some(flag) = read_optional_bool(value, "reduceOnly")? {
            position.reduce_only = flag;
        }
        if let Some(flag) = read_optional_bool(value, "closePosition")? {
            position.close_position = flag;
        }
        Ok(position)
    }
}

impl JsOrderPosition {
    /// Returns the wrapped position side.
    pub(crate) fn position_side_inner(&self) -> Option<PositionSide> {
        self.position_side
    }

    /// Returns the reduce-only flag.
    pub(crate) fn reduce_only_inner(&self) -> bool {
        self.reduce_only
    }

    /// Returns the close-position flag.
    pub(crate) fn close_position_inner(&self) -> bool {
        self.close_position
    }
}

/// Order margin group: leverage, collateral, and auto-borrow.
#[wasm_bindgen(js_name = OrderMargin)]
#[derive(Clone, Default)]
pub struct JsOrderMargin {
    leverage: Option<Leverage>,
    collateral_asset: Option<Asset>,
    auto_borrow: bool,
}

#[wasm_bindgen(js_class = OrderMargin)]
impl JsOrderMargin {
    /// Constructs an empty margin group (`autoBorrow` false).
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsOrderMargin {
        Self::default()
    }

    /// The leverage multiplier, or `undefined`.
    #[wasm_bindgen(getter, js_name = leverage)]
    pub fn leverage(&self) -> Option<JsLeverage> {
        self.leverage.map(JsLeverage::from_inner)
    }

    /// Sets the leverage (accepts a `Leverage` object or a number/bigint/string
    /// multiplier).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an out-of-range leverage value.
    #[wasm_bindgen(setter, js_name = leverage)]
    pub fn set_leverage(&mut self, value: LeverageLike) -> Result<(), JsValue> {
        self.leverage = resolve_optional_leverage(value.into())?;
        Ok(())
    }

    /// The collateral asset, or `undefined`.
    #[wasm_bindgen(getter, js_name = collateralAsset)]
    pub fn collateral_asset(&self) -> Option<String> {
        self.collateral_asset
            .as_ref()
            .map(|asset| asset.to_string())
    }

    /// Sets the collateral asset.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when the asset string is empty.
    #[wasm_bindgen(setter, js_name = collateralAsset)]
    pub fn set_collateral_asset(&mut self, value: Option<String>) -> Result<(), JsValue> {
        set_optional_asset(&mut self.collateral_asset, value)
    }

    /// Whether borrowing is permitted to satisfy margin.
    #[wasm_bindgen(getter, js_name = autoBorrow)]
    pub fn auto_borrow(&self) -> bool {
        self.auto_borrow
    }

    /// Sets the auto-borrow flag.
    #[wasm_bindgen(setter, js_name = autoBorrow)]
    pub fn set_auto_borrow(&mut self, value: bool) {
        self.auto_borrow = value;
    }

    /// Returns a deep copy of this margin group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderMargin {
        self.clone()
    }
}

impl JsOrderMargin {
    /// Builds a margin group from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut margin = Self::default();
        margin.set_leverage(read_field(value, "leverage")?.unchecked_into())?;
        margin.set_collateral_asset(read_optional_string(value, "collateralAsset")?)?;
        if let Some(flag) = read_optional_bool(value, "autoBorrow")? {
            margin.auto_borrow = flag;
        }
        Ok(margin)
    }
}

impl JsOrderMargin {
    /// Returns the wrapped leverage.
    pub(crate) fn leverage_inner(&self) -> Option<Leverage> {
        self.leverage
    }

    /// Returns the wrapped collateral asset.
    pub(crate) fn collateral_asset_inner(&self) -> Option<Asset> {
        self.collateral_asset.clone()
    }

    /// Returns the auto-borrow flag.
    pub(crate) fn auto_borrow_inner(&self) -> bool {
        self.auto_borrow
    }
}

/// Top-level order assembled from the operation, position, and margin groups.
#[wasm_bindgen(js_name = Order)]
#[derive(Clone, Default)]
pub struct JsOrder {
    operation: Option<JsOrderOperation>,
    position: Option<JsOrderPosition>,
    margin: Option<JsOrderMargin>,
}

#[wasm_bindgen(js_class = Order)]
impl JsOrder {
    /// Constructs an empty order; populate it through the group setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsOrder {
        Self::default()
    }

    /// The operation group, or `undefined`.
    #[wasm_bindgen(getter, js_name = operation)]
    pub fn operation(&self) -> Option<JsOrderOperation> {
        self.operation.clone()
    }

    /// Sets the operation group.
    ///
    /// Accepts an `OrderOperation` object or a plain `OrderOperationInit`
    /// literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = operation)]
    pub fn set_operation(&mut self, value: OrderOperationLike) -> Result<(), JsValue> {
        self.operation = coerce_operation(value.into())?;
        Ok(())
    }

    /// The position group, or `undefined`.
    #[wasm_bindgen(getter, js_name = position)]
    pub fn position(&self) -> Option<JsOrderPosition> {
        self.position.clone()
    }

    /// Sets the position group.
    ///
    /// Accepts an `OrderPosition` object or a plain `OrderPositionInit`
    /// literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = position)]
    pub fn set_position(&mut self, value: OrderPositionLike) -> Result<(), JsValue> {
        self.position = coerce_position(value.into())?;
        Ok(())
    }

    /// The margin group, or `undefined`.
    #[wasm_bindgen(getter, js_name = margin)]
    pub fn margin(&self) -> Option<JsOrderMargin> {
        self.margin.clone()
    }

    /// Sets the margin group.
    ///
    /// Accepts an `OrderMargin` object or a plain `OrderMarginInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = margin)]
    pub fn set_margin(&mut self, value: OrderMarginLike) -> Result<(), JsValue> {
        self.margin = coerce_margin(value.into())?;
        Ok(())
    }

    /// Returns a deep copy of this order, including its groups.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrder {
        self.clone()
    }
}

/// Resolves an optional `OrderOperation | OrderOperationInit` argument.
///
/// `undefined`/`null` maps to `None`; a wrapper instance is taken as-is; a
/// plain object literal is assembled field by field.
///
/// # Errors
///
/// Throws `ParamError`/`AssetError` on an invalid literal field, or
/// `ParamError` when the value is neither a wrapper nor a plain object.
fn coerce_operation(value: JsValue) -> Result<Option<JsOrderOperation>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<JsOrderOperation>(&value)? {
        return Ok(Some(wrapped));
    }
    if is_plain_object(&value) {
        return JsOrderOperation::from_object(&value).map(Some);
    }
    Err(invalid_field(
        "operation must be an OrderOperation or a plain object",
    ))
}

/// Resolves an optional `OrderPosition | OrderPositionInit` argument.
///
/// # Errors
///
/// Throws `ParamError` on an invalid literal field or a non-object value.
fn coerce_position(value: JsValue) -> Result<Option<JsOrderPosition>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<JsOrderPosition>(&value)? {
        return Ok(Some(wrapped));
    }
    if is_plain_object(&value) {
        return JsOrderPosition::from_object(&value).map(Some);
    }
    Err(invalid_field(
        "position must be an OrderPosition or a plain object",
    ))
}

/// Resolves an optional `OrderMargin | OrderMarginInit` argument.
///
/// # Errors
///
/// Throws `ParamError`/`AssetError` on an invalid literal field or a non-object
/// value.
fn coerce_margin(value: JsValue) -> Result<Option<JsOrderMargin>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<JsOrderMargin>(&value)? {
        return Ok(Some(wrapped));
    }
    if is_plain_object(&value) {
        return JsOrderMargin::from_object(&value).map(Some);
    }
    Err(invalid_field(
        "margin must be an OrderMargin or a plain object",
    ))
}

impl JsOrder {
    /// Builds an order from a plain object literal `{ operation?, position?,
    /// margin? }`, coercing each group from its wrapper or its own literal
    /// form.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        Ok(Self {
            operation: coerce_operation(read_field(value, "operation")?)?,
            position: coerce_position(read_field(value, "position")?)?,
            margin: coerce_margin(read_field(value, "margin")?)?,
        })
    }

    /// Resolves an `Order | OrderInit` argument into an owned [`JsOrder`].
    ///
    /// A wrapper instance is cloned at the JS level before extraction, so the
    /// engine borrows a copy and the caller's `Order` stays usable; a plain
    /// object literal is assembled. Both paths are non-consuming.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` on an invalid literal field, or
    /// `ParamError` when the value is neither an `Order` nor a plain object.
    pub(crate) fn coerce(value: JsValue) -> Result<JsOrder, JsValue> {
        if let Some(cloned) = clone_wrapper_value(&value)? {
            if let Ok(wrapped) = JsOrder::try_from_js_value(cloned) {
                return Ok(wrapped);
            }
        }
        if is_plain_object(&value) {
            return JsOrder::from_object(&value);
        }
        Err(invalid_field("order must be an Order or a plain object"))
    }

    /// Converts this order into the engine-facing interop request.
    ///
    /// Each absent group maps to the interop `Absent` access variant; a present
    /// group is copied field by field into the `Populated` variant.
    pub(crate) fn to_interop(&self) -> openpit_interop::Order {
        use openpit_interop::{
            OrderMarginAccess, OrderOperationAccess, OrderPositionAccess, PopulatedOrderMargin,
            PopulatedOrderOperation, PopulatedOrderPosition,
        };

        let operation = match &self.operation {
            None => OrderOperationAccess::Absent,
            Some(op) => OrderOperationAccess::Populated(PopulatedOrderOperation {
                instrument: op.instrument(),
                account_id: op.account_id_inner(),
                side: op.side_inner(),
                trade_amount: op.trade_amount_inner(),
                price: op.price_inner(),
            }),
        };

        let position = match &self.position {
            None => OrderPositionAccess::Absent,
            Some(pos) => OrderPositionAccess::Populated(PopulatedOrderPosition {
                position_side: pos.position_side_inner(),
                reduce_only: pos.reduce_only_inner(),
                close_position: pos.close_position_inner(),
            }),
        };

        let margin = match &self.margin {
            None => OrderMarginAccess::Absent,
            Some(m) => OrderMarginAccess::Populated(PopulatedOrderMargin {
                leverage: m.leverage_inner(),
                collateral_asset: m.collateral_asset_inner(),
                auto_borrow: m.auto_borrow_inner(),
            }),
        };

        openpit_interop::Order {
            operation,
            position,
            margin,
        }
    }
}
