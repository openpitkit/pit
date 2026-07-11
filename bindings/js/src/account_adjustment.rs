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

//! Account-adjustment payload bindings: `AccountAdjustment` and its groups.
//!
//! An adjustment carries an optional operation (a balance- or position-scoped
//! variant), an optional amount group, and optional bounds. Per the interop
//! "absent = Ok(None)" invariant, the binding performs no validation: every
//! field is stored as-is and the engine validates the assembled request.
//!
//! `AdjustmentAmount` is a tagged value that is either a signed `delta` or an
//! `absolute` target, each over a `PositionSize`.

use openpit::param::{AdjustmentAmount, Asset, Leverage, Pnl, PositionMode, PositionSize, Price};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::domain::{
    clone_wrapper_value, extract_cloned_wrapper, is_plain_object, parse_asset, read_field,
    read_optional_string, resolve_optional_leverage, resolve_optional_pnl,
    resolve_optional_position_size, resolve_optional_price, resolve_position_size, LeverageLike,
    OptionalPnlLike, OptionalPositionModeLike, OptionalPositionSizeLike, OptionalPriceLike,
    PositionSizeLike,
};
use crate::error::{make_error, ErrorKind};
use crate::param::enums::resolve_optional_position_mode;
use crate::param::leverage::JsLeverage;
use crate::param::value_types::{JsPositionSize, JsPrice};

/// Builds the error raised when an adjustment field is the wrong JS type.
fn invalid_field(message: &str) -> JsValue {
    make_error(ErrorKind::Type, message, None)
}

/// Reads an optional asset setter argument.
fn set_optional_asset(slot: &mut Option<Asset>, value: Option<String>) -> Result<(), JsValue> {
    *slot = match value {
        Some(text) => Some(parse_asset(&text)?),
        None => None,
    };
    Ok(())
}

#[wasm_bindgen(typescript_custom_section)]
const ACCOUNT_ADJUSTMENT_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link AccountAdjustmentAmount}.
 */
export interface AccountAdjustmentAmountInit {
  balance?: AdjustmentAmount;
  held?: AdjustmentAmount;
  incoming?: AdjustmentAmount;
}

/**
 * Plain-object form of {@link AccountAdjustmentBalanceOperation}.
 */
export interface AccountAdjustmentBalanceOperationInit {
  asset?: string;
  averageEntryPrice?: Price | string | number | bigint;
  realizedPnl?: Pnl | string | number | bigint;
}

/**
 * Plain-object form of {@link AccountAdjustmentPositionOperation}.
 */
export interface AccountAdjustmentPositionOperationInit {
  underlyingAsset?: string;
  settlementAsset?: string;
  collateralAsset?: string;
  averageEntryPrice?: Price | string | number | bigint;
  mode?: "netting" | "hedged";
  leverage?: Leverage | number | bigint | string;
}

/**
 * Plain-object form of {@link AccountAdjustmentBounds}. Every bound is optional.
 */
export interface AccountAdjustmentBoundsInit {
  balanceUpper?: PositionSize | string | number | bigint;
  balanceLower?: PositionSize | string | number | bigint;
  heldUpper?: PositionSize | string | number | bigint;
  heldLower?: PositionSize | string | number | bigint;
  incomingUpper?: PositionSize | string | number | bigint;
  incomingLower?: PositionSize | string | number | bigint;
}

/**
 * Plain-object form of {@link AccountAdjustment}. Each group accepts either the
 * typed wrapper or its own plain-object form. The `operation` accepts either
 * operation variant (balance- or position-scoped).
 */
export interface AccountAdjustmentInit {
  operation?:
    | AccountAdjustmentBalanceOperation
    | AccountAdjustmentBalanceOperationInit
    | AccountAdjustmentPositionOperation
    | AccountAdjustmentPositionOperationInit;
  amount?: AccountAdjustmentAmount | AccountAdjustmentAmountInit;
  bounds?: AccountAdjustmentBounds | AccountAdjustmentBoundsInit;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// An `AdjustmentAmount` wrapper or `null`/`undefined`.
    #[wasm_bindgen(typescript_type = "AdjustmentAmount | null | undefined")]
    pub type AdjustmentAmountLike;

    /// An `AccountAdjustmentAmount` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "AccountAdjustmentAmount | AccountAdjustmentAmountInit")]
    pub type AccountAdjustmentAmountLike;

    /// Either adjustment-operation wrapper or its plain-object form.
    #[wasm_bindgen(
        typescript_type = "AccountAdjustmentBalanceOperation | AccountAdjustmentBalanceOperationInit | AccountAdjustmentPositionOperation | AccountAdjustmentPositionOperationInit"
    )]
    pub type AccountAdjustmentOperationLike;

    /// An `AccountAdjustmentBounds` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "AccountAdjustmentBounds | AccountAdjustmentBoundsInit")]
    pub type AccountAdjustmentBoundsLike;

    /// The resolved operation variant returned by the `operation` getter.
    #[wasm_bindgen(
        typescript_type = "AccountAdjustmentBalanceOperation | AccountAdjustmentPositionOperation | undefined"
    )]
    pub type AccountAdjustmentOperationValue;
}

/// A tagged balance/position adjustment amount: `delta` or `absolute`.
#[wasm_bindgen(js_name = AdjustmentAmount)]
#[derive(Clone, Copy)]
pub struct JsAdjustmentAmount {
    inner: AdjustmentAmount,
}

#[wasm_bindgen(js_class = AdjustmentAmount)]
impl JsAdjustmentAmount {
    /// Builds a signed-difference (`delta`) adjustment amount.
    ///
    /// Accepts a `PositionSize` value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(js_name = delta)]
    pub fn delta(value: PositionSizeLike) -> Result<JsAdjustmentAmount, JsValue> {
        let size = resolve_position_size(value.into())?;
        Ok(Self {
            inner: AdjustmentAmount::Delta(size),
        })
    }

    /// Builds an absolute-target adjustment amount.
    ///
    /// Accepts a `PositionSize` value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(js_name = absolute)]
    pub fn absolute(value: PositionSizeLike) -> Result<JsAdjustmentAmount, JsValue> {
        let size = resolve_position_size(value.into())?;
        Ok(Self {
            inner: AdjustmentAmount::Absolute(size),
        })
    }

    /// Returns `true` when this amount is a signed difference.
    #[wasm_bindgen(getter, js_name = isDelta)]
    pub fn is_delta(&self) -> bool {
        matches!(self.inner, AdjustmentAmount::Delta(_))
    }

    /// Returns `true` when this amount is an absolute target.
    #[wasm_bindgen(getter, js_name = isAbsolute)]
    pub fn is_absolute(&self) -> bool {
        matches!(self.inner, AdjustmentAmount::Absolute(_))
    }

    /// Returns the delta size, or `undefined` when this is absolute.
    #[wasm_bindgen(getter, js_name = asDelta)]
    pub fn as_delta(&self) -> Option<JsPositionSize> {
        match self.inner {
            AdjustmentAmount::Delta(size) => Some(JsPositionSize::from_inner(size)),
            _ => None,
        }
    }

    /// Returns the absolute size, or `undefined` when this is a delta.
    #[wasm_bindgen(getter, js_name = asAbsolute)]
    pub fn as_absolute(&self) -> Option<JsPositionSize> {
        match self.inner {
            AdjustmentAmount::Absolute(size) => Some(JsPositionSize::from_inner(size)),
            _ => None,
        }
    }

    /// Returns a fresh `AdjustmentAmount` holding the same value.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAdjustmentAmount {
        *self
    }
}

impl JsAdjustmentAmount {
    /// Returns the wrapped core [`AdjustmentAmount`].
    pub fn inner(&self) -> AdjustmentAmount {
        self.inner
    }
}

/// Resolves an `AdjustmentAmount` setter argument.
fn resolve_adjustment_amount(value: JsValue) -> Result<AdjustmentAmount, JsValue> {
    extract_cloned_wrapper::<JsAdjustmentAmount>(&value)?
        .map(|wrapped| wrapped.inner())
        .ok_or_else(|| invalid_field("amount must be an AdjustmentAmount"))
}

/// Per-field adjustment amounts for balance, held, and incoming buckets.
#[wasm_bindgen(js_name = AccountAdjustmentAmount)]
#[derive(Clone, Copy, Default)]
pub struct JsAccountAdjustmentAmount {
    balance: Option<AdjustmentAmount>,
    held: Option<AdjustmentAmount>,
    incoming: Option<AdjustmentAmount>,
}

#[wasm_bindgen(js_class = AccountAdjustmentAmount)]
impl JsAccountAdjustmentAmount {
    /// Constructs an empty amount group; populate it through the setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsAccountAdjustmentAmount {
        Self::default()
    }

    /// The balance-bucket amount, or `undefined`.
    #[wasm_bindgen(getter, js_name = balance)]
    pub fn balance(&self) -> Option<JsAdjustmentAmount> {
        self.balance.map(|inner| JsAdjustmentAmount { inner })
    }

    /// Sets the balance-bucket amount.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not an `AdjustmentAmount`.
    #[wasm_bindgen(setter, js_name = balance)]
    pub fn set_balance(&mut self, value: AdjustmentAmountLike) -> Result<(), JsValue> {
        self.balance = resolve_optional_adjustment_amount(value.into())?;
        Ok(())
    }

    /// The held-bucket amount, or `undefined`.
    #[wasm_bindgen(getter, js_name = held)]
    pub fn held(&self) -> Option<JsAdjustmentAmount> {
        self.held.map(|inner| JsAdjustmentAmount { inner })
    }

    /// Sets the held-bucket amount.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not an `AdjustmentAmount`.
    #[wasm_bindgen(setter, js_name = held)]
    pub fn set_held(&mut self, value: AdjustmentAmountLike) -> Result<(), JsValue> {
        self.held = resolve_optional_adjustment_amount(value.into())?;
        Ok(())
    }

    /// The incoming-bucket amount, or `undefined`.
    #[wasm_bindgen(getter, js_name = incoming)]
    pub fn incoming(&self) -> Option<JsAdjustmentAmount> {
        self.incoming.map(|inner| JsAdjustmentAmount { inner })
    }

    /// Sets the incoming-bucket amount.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not an `AdjustmentAmount`.
    #[wasm_bindgen(setter, js_name = incoming)]
    pub fn set_incoming(&mut self, value: AdjustmentAmountLike) -> Result<(), JsValue> {
        self.incoming = resolve_optional_adjustment_amount(value.into())?;
        Ok(())
    }

    /// Returns a fresh copy of this amount group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountAdjustmentAmount {
        *self
    }
}

impl JsAccountAdjustmentAmount {
    /// Builds an amount group from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut amount = Self::default();
        amount.set_balance(read_field(value, "balance")?.unchecked_into())?;
        amount.set_held(read_field(value, "held")?.unchecked_into())?;
        amount.set_incoming(read_field(value, "incoming")?.unchecked_into())?;
        Ok(amount)
    }
}

/// Resolves an optional `AdjustmentAmount` setter argument.
fn resolve_optional_adjustment_amount(value: JsValue) -> Result<Option<AdjustmentAmount>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    resolve_adjustment_amount(value).map(Some)
}

/// Balance-scoped adjustment operation: asset and average entry price.
#[wasm_bindgen(js_name = AccountAdjustmentBalanceOperation)]
#[derive(Clone, Default)]
pub struct JsAccountAdjustmentBalanceOperation {
    asset: Option<Asset>,
    average_entry_price: Option<Price>,
    realized_pnl: Option<Pnl>,
}

#[wasm_bindgen(js_class = AccountAdjustmentBalanceOperation)]
impl JsAccountAdjustmentBalanceOperation {
    /// Constructs an empty balance operation; populate it through the setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsAccountAdjustmentBalanceOperation {
        Self::default()
    }

    /// The asset, or `undefined`.
    #[wasm_bindgen(getter, js_name = asset)]
    pub fn asset(&self) -> Option<String> {
        self.asset.as_ref().map(|asset| asset.to_string())
    }

    /// Sets the asset.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when the asset string is empty.
    #[wasm_bindgen(setter, js_name = asset)]
    pub fn set_asset(&mut self, value: Option<String>) -> Result<(), JsValue> {
        set_optional_asset(&mut self.asset, value)
    }

    /// The average entry price, or `undefined`.
    #[wasm_bindgen(getter, js_name = averageEntryPrice)]
    pub fn average_entry_price(&self) -> Option<JsPrice> {
        self.average_entry_price.map(JsPrice::from_inner)
    }

    /// Sets the average entry price (accepts a value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = averageEntryPrice)]
    pub fn set_average_entry_price(&mut self, value: OptionalPriceLike) -> Result<(), JsValue> {
        self.average_entry_price = resolve_optional_price(value.into())?;
        Ok(())
    }

    /// The absolute realized P&L, or `undefined`.
    #[wasm_bindgen(getter, js_name = realizedPnl)]
    pub fn realized_pnl(&self) -> Option<crate::param::value_types::JsPnl> {
        self.realized_pnl
            .map(crate::param::value_types::JsPnl::from_inner)
    }

    /// Sets the absolute realized P&L (accepts a value object or DecimalInput).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = realizedPnl)]
    pub fn set_realized_pnl(&mut self, value: OptionalPnlLike) -> Result<(), JsValue> {
        self.realized_pnl = resolve_optional_pnl(value.into())?;
        Ok(())
    }

    /// Returns a deep copy of this balance operation.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountAdjustmentBalanceOperation {
        self.clone()
    }
}

impl JsAccountAdjustmentBalanceOperation {
    /// Builds a balance operation from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut operation = Self::default();
        operation.set_asset(read_optional_string(value, "asset")?)?;
        operation
            .set_average_entry_price(read_field(value, "averageEntryPrice")?.unchecked_into())?;
        operation.set_realized_pnl(read_field(value, "realizedPnl")?.unchecked_into())?;
        Ok(operation)
    }
}

/// Position-scoped adjustment operation: instrument, collateral, price, mode,
/// and leverage.
#[wasm_bindgen(js_name = AccountAdjustmentPositionOperation)]
#[derive(Clone, Default)]
pub struct JsAccountAdjustmentPositionOperation {
    underlying_asset: Option<Asset>,
    settlement_asset: Option<Asset>,
    collateral_asset: Option<Asset>,
    average_entry_price: Option<Price>,
    mode: Option<PositionMode>,
    leverage: Option<Leverage>,
}

#[wasm_bindgen(js_class = AccountAdjustmentPositionOperation)]
impl JsAccountAdjustmentPositionOperation {
    /// Constructs an empty position operation; populate it through the setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsAccountAdjustmentPositionOperation {
        Self::default()
    }

    /// The underlying asset, or `undefined`.
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

    /// The average entry price, or `undefined`.
    #[wasm_bindgen(getter, js_name = averageEntryPrice)]
    pub fn average_entry_price(&self) -> Option<JsPrice> {
        self.average_entry_price.map(JsPrice::from_inner)
    }

    /// Sets the average entry price (accepts a value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = averageEntryPrice)]
    pub fn set_average_entry_price(&mut self, value: OptionalPriceLike) -> Result<(), JsValue> {
        self.average_entry_price = resolve_optional_price(value.into())?;
        Ok(())
    }

    /// The position mode wire string (`"netting"`/`"hedged"`), or `undefined`.
    #[wasm_bindgen(getter, js_name = mode)]
    pub fn mode(&self) -> Option<String> {
        self.mode.map(|mode| mode.to_string())
    }

    /// Sets the position mode from its wire string.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not `"netting"`/`"hedged"`.
    #[wasm_bindgen(setter, js_name = mode)]
    pub fn set_mode(&mut self, value: OptionalPositionModeLike) -> Result<(), JsValue> {
        self.mode = resolve_optional_position_mode(value.into())?;
        Ok(())
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

    /// Returns a deep copy of this position operation.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountAdjustmentPositionOperation {
        self.clone()
    }
}

impl JsAccountAdjustmentPositionOperation {
    /// Builds a position operation from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut operation = Self::default();
        operation.set_underlying_asset(read_optional_string(value, "underlyingAsset")?)?;
        operation.set_settlement_asset(read_optional_string(value, "settlementAsset")?)?;
        operation.set_collateral_asset(read_optional_string(value, "collateralAsset")?)?;
        operation
            .set_average_entry_price(read_field(value, "averageEntryPrice")?.unchecked_into())?;
        operation.set_mode(read_field(value, "mode")?.unchecked_into())?;
        operation.set_leverage(read_field(value, "leverage")?.unchecked_into())?;
        Ok(operation)
    }
}

/// Six optional `PositionSize` bounds across the balance, held, and incoming
/// buckets. The binding stores them without validation.
#[wasm_bindgen(js_name = AccountAdjustmentBounds)]
#[derive(Clone, Copy, Default)]
pub struct JsAccountAdjustmentBounds {
    balance_upper: Option<PositionSize>,
    balance_lower: Option<PositionSize>,
    held_upper: Option<PositionSize>,
    held_lower: Option<PositionSize>,
    incoming_upper: Option<PositionSize>,
    incoming_lower: Option<PositionSize>,
}

#[wasm_bindgen(js_class = AccountAdjustmentBounds)]
impl JsAccountAdjustmentBounds {
    /// Constructs an empty bounds group; populate it through the setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsAccountAdjustmentBounds {
        Self::default()
    }

    /// The upper balance bound, or `undefined`.
    #[wasm_bindgen(getter, js_name = balanceUpper)]
    pub fn balance_upper(&self) -> Option<JsPositionSize> {
        self.balance_upper.map(JsPositionSize::from_inner)
    }

    /// Sets the upper balance bound (value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = balanceUpper)]
    pub fn set_balance_upper(&mut self, value: OptionalPositionSizeLike) -> Result<(), JsValue> {
        self.balance_upper = resolve_optional_position_size(value.into())?;
        Ok(())
    }

    /// The lower balance bound, or `undefined`.
    #[wasm_bindgen(getter, js_name = balanceLower)]
    pub fn balance_lower(&self) -> Option<JsPositionSize> {
        self.balance_lower.map(JsPositionSize::from_inner)
    }

    /// Sets the lower balance bound (value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = balanceLower)]
    pub fn set_balance_lower(&mut self, value: OptionalPositionSizeLike) -> Result<(), JsValue> {
        self.balance_lower = resolve_optional_position_size(value.into())?;
        Ok(())
    }

    /// The upper held bound, or `undefined`.
    #[wasm_bindgen(getter, js_name = heldUpper)]
    pub fn held_upper(&self) -> Option<JsPositionSize> {
        self.held_upper.map(JsPositionSize::from_inner)
    }

    /// Sets the upper held bound (value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = heldUpper)]
    pub fn set_held_upper(&mut self, value: OptionalPositionSizeLike) -> Result<(), JsValue> {
        self.held_upper = resolve_optional_position_size(value.into())?;
        Ok(())
    }

    /// The lower held bound, or `undefined`.
    #[wasm_bindgen(getter, js_name = heldLower)]
    pub fn held_lower(&self) -> Option<JsPositionSize> {
        self.held_lower.map(JsPositionSize::from_inner)
    }

    /// Sets the lower held bound (value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = heldLower)]
    pub fn set_held_lower(&mut self, value: OptionalPositionSizeLike) -> Result<(), JsValue> {
        self.held_lower = resolve_optional_position_size(value.into())?;
        Ok(())
    }

    /// The upper incoming bound, or `undefined`.
    #[wasm_bindgen(getter, js_name = incomingUpper)]
    pub fn incoming_upper(&self) -> Option<JsPositionSize> {
        self.incoming_upper.map(JsPositionSize::from_inner)
    }

    /// Sets the upper incoming bound (value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = incomingUpper)]
    pub fn set_incoming_upper(&mut self, value: OptionalPositionSizeLike) -> Result<(), JsValue> {
        self.incoming_upper = resolve_optional_position_size(value.into())?;
        Ok(())
    }

    /// The lower incoming bound, or `undefined`.
    #[wasm_bindgen(getter, js_name = incomingLower)]
    pub fn incoming_lower(&self) -> Option<JsPositionSize> {
        self.incoming_lower.map(JsPositionSize::from_inner)
    }

    /// Sets the lower incoming bound (value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = incomingLower)]
    pub fn set_incoming_lower(&mut self, value: OptionalPositionSizeLike) -> Result<(), JsValue> {
        self.incoming_lower = resolve_optional_position_size(value.into())?;
        Ok(())
    }

    /// Returns a fresh copy of this bounds group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountAdjustmentBounds {
        *self
    }
}

impl JsAccountAdjustmentBounds {
    /// Builds a bounds group from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut bounds = Self::default();
        bounds.set_balance_upper(read_field(value, "balanceUpper")?.unchecked_into())?;
        bounds.set_balance_lower(read_field(value, "balanceLower")?.unchecked_into())?;
        bounds.set_held_upper(read_field(value, "heldUpper")?.unchecked_into())?;
        bounds.set_held_lower(read_field(value, "heldLower")?.unchecked_into())?;
        bounds.set_incoming_upper(read_field(value, "incomingUpper")?.unchecked_into())?;
        bounds.set_incoming_lower(read_field(value, "incomingLower")?.unchecked_into())?;
        Ok(bounds)
    }
}

/// The adjustment operation variant: balance-scoped or position-scoped.
#[derive(Clone)]
enum AdjustmentOperation {
    /// A balance-scoped operation.
    Balance(JsAccountAdjustmentBalanceOperation),
    /// A position-scoped operation.
    Position(JsAccountAdjustmentPositionOperation),
}

/// Top-level account adjustment: operation, amount, and bounds.
#[wasm_bindgen(js_name = AccountAdjustment)]
#[derive(Clone, Default)]
pub struct JsAccountAdjustment {
    operation: Option<AdjustmentOperation>,
    amount: Option<JsAccountAdjustmentAmount>,
    bounds: Option<JsAccountAdjustmentBounds>,
}

#[wasm_bindgen(js_class = AccountAdjustment)]
impl JsAccountAdjustment {
    /// Constructs an empty adjustment; populate it through the group setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsAccountAdjustment {
        Self::default()
    }

    /// The operation group (a balance or position operation), or `undefined`.
    #[wasm_bindgen(getter, js_name = operation)]
    pub fn operation(&self) -> AccountAdjustmentOperationValue {
        let value = match &self.operation {
            Some(AdjustmentOperation::Balance(op)) => JsValue::from(op.clone()),
            Some(AdjustmentOperation::Position(op)) => JsValue::from(op.clone()),
            None => JsValue::UNDEFINED,
        };
        value.unchecked_into()
    }

    /// Sets the operation group.
    ///
    /// Accepts an `AccountAdjustmentBalanceOperation` or an
    /// `AccountAdjustmentPositionOperation` object, or a plain object literal
    /// of either shape (a
    /// `mode`/`leverage`/`underlyingAsset`/`settlementAsset`/`collateralAsset`
    /// field selects the position form; otherwise the balance form).
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when `value` is neither operation type
    /// nor a valid literal.
    #[wasm_bindgen(setter, js_name = operation)]
    pub fn set_operation(&mut self, value: AccountAdjustmentOperationLike) -> Result<(), JsValue> {
        let value: JsValue = value.into();
        self.operation = if value.is_undefined() || value.is_null() {
            None
        } else {
            Some(resolve_operation(value)?)
        };
        Ok(())
    }

    /// The amount group, or `undefined`.
    #[wasm_bindgen(getter, js_name = amount)]
    pub fn amount(&self) -> Option<JsAccountAdjustmentAmount> {
        self.amount
    }

    /// Sets the amount group.
    ///
    /// Accepts an `AccountAdjustmentAmount` object or a plain
    /// `AccountAdjustmentAmountInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = amount)]
    pub fn set_amount(&mut self, value: AccountAdjustmentAmountLike) -> Result<(), JsValue> {
        self.amount = coerce_amount(value.into())?;
        Ok(())
    }

    /// The bounds group, or `undefined`.
    #[wasm_bindgen(getter, js_name = bounds)]
    pub fn bounds(&self) -> Option<JsAccountAdjustmentBounds> {
        self.bounds
    }

    /// Sets the bounds group.
    ///
    /// Accepts an `AccountAdjustmentBounds` object or a plain
    /// `AccountAdjustmentBoundsInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = bounds)]
    pub fn set_bounds(&mut self, value: AccountAdjustmentBoundsLike) -> Result<(), JsValue> {
        self.bounds = coerce_bounds(value.into())?;
        Ok(())
    }

    /// Returns a deep copy of this adjustment, including its groups.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountAdjustment {
        self.clone()
    }
}

/// Resolves an adjustment-operation argument to its variant.
///
/// A typed wrapper of either variant is taken as-is. A plain object literal is
/// dispatched by shape: a position-specific field selects the position form;
/// otherwise the balance form.
///
/// # Errors
///
/// Throws `ParamError`/`AssetError` when the value is neither a wrapper nor a
/// plain object, or on an invalid literal field.
fn resolve_operation(value: JsValue) -> Result<AdjustmentOperation, JsValue> {
    if let Some(balance) = extract_cloned_wrapper::<JsAccountAdjustmentBalanceOperation>(&value)? {
        return Ok(AdjustmentOperation::Balance(balance));
    }
    if let Some(position) = extract_cloned_wrapper::<JsAccountAdjustmentPositionOperation>(&value)?
    {
        return Ok(AdjustmentOperation::Position(position));
    }
    if is_plain_object(&value) {
        return if has_position_field(&value)? {
            JsAccountAdjustmentPositionOperation::from_object(&value)
                .map(AdjustmentOperation::Position)
        } else {
            JsAccountAdjustmentBalanceOperation::from_object(&value)
                .map(AdjustmentOperation::Balance)
        };
    }
    Err(invalid_field(
        "operation must be an AccountAdjustmentBalanceOperation, an \
         AccountAdjustmentPositionOperation, or a plain object",
    ))
}

/// Returns `true` when a plain operation literal carries a position-specific
/// field, selecting the position-scoped form over the balance-scoped form.
fn has_position_field(value: &JsValue) -> Result<bool, JsValue> {
    const POSITION_FIELDS: [&str; 5] = [
        "underlyingAsset",
        "settlementAsset",
        "collateralAsset",
        "mode",
        "leverage",
    ];
    for field in POSITION_FIELDS {
        if !read_field(value, field)?.is_undefined() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Resolves an optional
/// `AccountAdjustmentAmount | AccountAdjustmentAmountInit`.
///
/// # Errors
///
/// Throws `ParamError` on an invalid literal field or a non-object value.
fn coerce_amount(value: JsValue) -> Result<Option<JsAccountAdjustmentAmount>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<JsAccountAdjustmentAmount>(&value)? {
        return Ok(Some(wrapped));
    }
    if is_plain_object(&value) {
        return JsAccountAdjustmentAmount::from_object(&value).map(Some);
    }
    Err(invalid_field(
        "amount must be an AccountAdjustmentAmount or a plain object",
    ))
}

/// Resolves an optional
/// `AccountAdjustmentBounds | AccountAdjustmentBoundsInit`.
///
/// # Errors
///
/// Throws `ParamError` on an invalid literal field or a non-object value.
fn coerce_bounds(value: JsValue) -> Result<Option<JsAccountAdjustmentBounds>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<JsAccountAdjustmentBounds>(&value)? {
        return Ok(Some(wrapped));
    }
    if is_plain_object(&value) {
        return JsAccountAdjustmentBounds::from_object(&value).map(Some);
    }
    Err(invalid_field(
        "bounds must be an AccountAdjustmentBounds or a plain object",
    ))
}

impl JsAccountAdjustmentBalanceOperation {
    /// Returns the wrapped asset.
    pub(crate) fn asset_inner(&self) -> Option<Asset> {
        self.asset.clone()
    }

    /// Returns the wrapped average entry price.
    pub(crate) fn average_entry_price_inner(&self) -> Option<Price> {
        self.average_entry_price
    }

    /// Returns the wrapped absolute realized P&L.
    pub(crate) fn realized_pnl_inner(&self) -> Option<Pnl> {
        self.realized_pnl
    }
}

impl JsAccountAdjustmentPositionOperation {
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

    /// Returns the wrapped collateral asset.
    pub(crate) fn collateral_asset_inner(&self) -> Option<Asset> {
        self.collateral_asset.clone()
    }

    /// Returns the wrapped average entry price.
    pub(crate) fn average_entry_price_inner(&self) -> Option<Price> {
        self.average_entry_price
    }

    /// Returns the wrapped position mode.
    pub(crate) fn mode_inner(&self) -> Option<PositionMode> {
        self.mode
    }

    /// Returns the wrapped leverage.
    pub(crate) fn leverage_inner(&self) -> Option<Leverage> {
        self.leverage
    }
}

impl JsAccountAdjustmentAmount {
    /// Returns the wrapped balance-bucket amount.
    pub(crate) fn balance_inner(&self) -> Option<AdjustmentAmount> {
        self.balance
    }

    /// Returns the wrapped held-bucket amount.
    pub(crate) fn held_inner(&self) -> Option<AdjustmentAmount> {
        self.held
    }

    /// Returns the wrapped incoming-bucket amount.
    pub(crate) fn incoming_inner(&self) -> Option<AdjustmentAmount> {
        self.incoming
    }
}

impl JsAccountAdjustmentBounds {
    /// Builds the core bounds record from the six optional limits.
    pub(crate) fn to_inner(self) -> openpit::AccountAdjustmentBounds {
        openpit::AccountAdjustmentBounds {
            balance_upper: self.balance_upper,
            balance_lower: self.balance_lower,
            held_upper: self.held_upper,
            held_lower: self.held_lower,
            incoming_upper: self.incoming_upper,
            incoming_lower: self.incoming_lower,
        }
    }
}

impl JsAccountAdjustment {
    /// Builds an adjustment from a plain object literal `{ operation?, amount?,
    /// bounds? }`.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let operation = match read_field(value, "operation")? {
            field if field.is_undefined() || field.is_null() => None,
            field => Some(resolve_operation(field)?),
        };
        Ok(Self {
            operation,
            amount: coerce_amount(read_field(value, "amount")?)?,
            bounds: coerce_bounds(read_field(value, "bounds")?)?,
        })
    }

    /// Resolves an `AccountAdjustment | AccountAdjustmentInit` argument into an
    /// owned adjustment.
    ///
    /// A wrapper instance is cloned at the JS level before extraction, so the
    /// caller's adjustment stays usable (and remains valid as the request
    /// payload); a plain object literal is assembled. Both paths are
    /// non-consuming.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` on an invalid literal field, or
    /// `ParamError` when the value is neither an `AccountAdjustment` nor a
    /// plain object.
    pub(crate) fn coerce(value: JsValue) -> Result<JsAccountAdjustment, JsValue> {
        if let Some(cloned) = clone_wrapper_value(&value)? {
            if let Ok(wrapped) = JsAccountAdjustment::try_from_js_value(cloned) {
                return Ok(wrapped);
            }
        }
        if is_plain_object(&value) {
            return JsAccountAdjustment::from_object(&value);
        }
        Err(invalid_field(
            "each adjustment must be an AccountAdjustment or a plain object",
        ))
    }

    /// Converts this adjustment into the engine-facing interop request.
    ///
    /// Absent groups become the interop `Absent` access variant, and the
    /// operation variant is dispatched to the matching populated
    /// balance/position form.
    pub(crate) fn to_interop(&self) -> openpit_interop::AccountAdjustment {
        use openpit_interop::{
            AccountAdjustmentAmountAccess, AccountAdjustmentBoundsAccess,
            AccountAdjustmentOperationAccess, PopulatedAccountAdjustmentOperation,
            PopulatedBalanceOperation, PopulatedPositionOperation,
        };

        let operation = match &self.operation {
            None => AccountAdjustmentOperationAccess::Absent,
            Some(AdjustmentOperation::Balance(balance)) => {
                AccountAdjustmentOperationAccess::Populated(
                    PopulatedAccountAdjustmentOperation::Balance(PopulatedBalanceOperation {
                        asset: balance.asset_inner(),
                        average_entry_price: balance.average_entry_price_inner(),
                        realized_pnl: balance.realized_pnl_inner(),
                    }),
                )
            }
            Some(AdjustmentOperation::Position(position)) => {
                AccountAdjustmentOperationAccess::Populated(
                    PopulatedAccountAdjustmentOperation::Position(PopulatedPositionOperation {
                        instrument: position.instrument(),
                        collateral_asset: position.collateral_asset_inner(),
                        average_entry_price: position.average_entry_price_inner(),
                        mode: position.mode_inner(),
                        leverage: position.leverage_inner(),
                    }),
                )
            }
        };

        let amount = match &self.amount {
            None => AccountAdjustmentAmountAccess::Absent,
            Some(value) => {
                AccountAdjustmentAmountAccess::Populated(openpit::AccountAdjustmentAmount {
                    balance: value.balance_inner(),
                    held: value.held_inner(),
                    incoming: value.incoming_inner(),
                })
            }
        };

        let bounds = match &self.bounds {
            None => AccountAdjustmentBoundsAccess::Absent,
            Some(value) => AccountAdjustmentBoundsAccess::Populated(value.to_inner()),
        };

        openpit_interop::AccountAdjustment {
            operation,
            amount,
            bounds,
        }
    }
}
