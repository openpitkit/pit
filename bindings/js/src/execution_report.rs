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

//! Execution-report payload bindings: `ExecutionReport` and its groups.
//!
//! An execution report is assembled from four optional groups:
//! `ExecutionReportOperation`, `FinancialImpact`, `ExecutionReportFillDetails`,
//! and `ExecutionReportPositionImpact`. Each group is a mutable builder with a
//! camelCase getter/setter surface.
//!
//! `Trade` is an immutable `(price, quantity)` pair. The fill-details group
//! carries optional boundary values that the core validates according to the
//! policies consuming the report.

use openpit::param::{
    AccountId, Asset, Fee, MonetaryAmount, Pnl, PositionEffect, PositionSide, Quantity, Side, Trade,
};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::domain::{
    clone_wrapper_value, extract_cloned_wrapper, is_plain_object, parse_asset, read_field,
    read_optional_bool, read_optional_string, resolve_fee, resolve_optional_account_id,
    resolve_optional_quantity, resolve_pnl, resolve_price, resolve_quantity, FeeLike,
    OptionalAccountIdLike, OptionalPositionEffectLike, OptionalPositionSideLike,
    OptionalQuantityLike, OptionalSideLike, PnlLike, PriceLike, QuantityLike,
};
use crate::error::{make_error, ErrorKind};
use crate::lock::JsLock;
use crate::param::enums::{
    resolve_optional_position_effect, resolve_optional_position_side, resolve_optional_side,
};
use crate::param::ids::JsAccountId;
use crate::param::monetary_amount::{JsMonetaryAmount, MonetaryAmountLike};
use crate::param::value_types::{JsFee, JsPnl, JsPrice, JsQuantity};

/// Builds the error raised when a report group field is the wrong JS type.
fn invalid_field(message: &str) -> JsValue {
    make_error(ErrorKind::Type, message, None)
}

#[wasm_bindgen(typescript_custom_section)]
const EXECUTION_REPORT_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link ExecutionReportOperation}.
 */
export interface ExecutionReportOperationInit {
  underlyingAsset?: string;
  settlementAsset?: string;
  accountId?: AccountId | number | bigint | string;
  side?: "BUY" | "SELL";
}

/**
 * Plain-object form of {@link FinancialImpact}.
 */
export interface FinancialImpactInit {
  pnl?: Pnl | string | number | bigint;
  fee?: Fee | string | number | bigint;
}

/**
 * Plain-object form of {@link ExecutionReportFillDetails}.
 */
export interface ExecutionReportFillDetailsInit {
  lock?: Lock;
  fee?: MonetaryAmount | MonetaryAmountInit;
  lastTrade?: Trade | TradeInit;
  leavesQuantity?: Quantity | string | number | bigint;
  isFinal?: boolean;
}

/**
 * Plain-object form of {@link ExecutionReportPositionImpact}.
 */
export interface ExecutionReportPositionImpactInit {
  positionEffect?: "OPEN" | "CLOSE";
  positionSide?: "LONG" | "SHORT";
}

/**
 * Plain-object form of {@link ExecutionReport}. Each group accepts either the
 * typed wrapper or its own plain-object form.
 */
export interface ExecutionReportInit {
  operation?: ExecutionReportOperation | ExecutionReportOperationInit;
  financialImpact?: FinancialImpact | FinancialImpactInit;
  fill?: ExecutionReportFillDetails | ExecutionReportFillDetailsInit;
  positionImpact?:
    | ExecutionReportPositionImpact
    | ExecutionReportPositionImpactInit;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// An `ExecutionReportOperation` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "ExecutionReportOperation | ExecutionReportOperationInit")]
    pub type ExecutionReportOperationLike;

    /// A `FinancialImpact` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "FinancialImpact | FinancialImpactInit")]
    pub type FinancialImpactLike;

    /// An `ExecutionReportFillDetails` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "ExecutionReportFillDetails | ExecutionReportFillDetailsInit")]
    pub type ExecutionReportFillDetailsLike;

    /// An `ExecutionReportPositionImpact` wrapper or its plain-object form.
    #[wasm_bindgen(
        typescript_type = "ExecutionReportPositionImpact | ExecutionReportPositionImpactInit"
    )]
    pub type ExecutionReportPositionImpactLike;

    /// An `ExecutionReport` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "ExecutionReport | ExecutionReportInit")]
    pub type ExecutionReportLike;

    /// A `Trade` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "Trade | TradeInit | null | undefined")]
    pub type TradeLike;
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
const TRADE_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link Trade}. Both fields are required.
 */
export interface TradeInit {
  price: Price | string | number | bigint;
  quantity: Quantity | string | number | bigint;
}
"#;

/// Immutable executed trade: a `(price, quantity)` pair.
#[wasm_bindgen(js_name = Trade)]
#[derive(Clone, Copy)]
pub struct JsTrade {
    inner: Trade,
}

#[wasm_bindgen(js_class = Trade)]
impl JsTrade {
    /// Constructs a trade from a price and quantity.
    ///
    /// Each argument accepts a value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid price or quantity.
    #[wasm_bindgen(constructor)]
    pub fn new(price: PriceLike, quantity: QuantityLike) -> Result<JsTrade, JsValue> {
        Ok(Self {
            inner: Trade {
                price: resolve_price(price.into())?,
                quantity: resolve_quantity(quantity.into())?,
            },
        })
    }

    /// The trade price.
    #[wasm_bindgen(getter, js_name = price)]
    pub fn price(&self) -> JsPrice {
        JsPrice::from_inner(self.inner.price)
    }

    /// The trade quantity.
    #[wasm_bindgen(getter, js_name = quantity)]
    pub fn quantity(&self) -> JsQuantity {
        JsQuantity::from_inner(self.inner.quantity)
    }

    /// Returns a fresh `Trade` holding the same price and quantity.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsTrade {
        *self
    }
}

impl JsTrade {
    /// Returns the wrapped core [`Trade`].
    pub fn inner(&self) -> Trade {
        self.inner
    }

    /// Builds a trade from a plain object literal `{ price, quantity }`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid or missing `price`/`quantity`.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        Ok(Self {
            inner: Trade {
                price: resolve_price(read_field(value, "price")?)?,
                quantity: resolve_quantity(read_field(value, "quantity")?)?,
            },
        })
    }
}

/// Resolves an optional `Trade | TradeInit` setter argument.
///
/// `undefined`/`null` maps to `None`; a wrapper is taken as-is; a plain object
/// literal is assembled.
///
/// # Errors
///
/// Throws `ParamError` on an invalid literal field or a non-object value.
fn resolve_optional_trade(value: JsValue) -> Result<Option<Trade>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<JsTrade>(&value)? {
        return Ok(Some(wrapped.inner()));
    }
    if is_plain_object(&value) {
        return JsTrade::from_object(&value).map(|trade| Some(trade.inner()));
    }
    Err(invalid_field("lastTrade must be a Trade or a plain object"))
}

/// Execution-report operation group: instrument, account, and side.
#[wasm_bindgen(js_name = ExecutionReportOperation)]
#[derive(Clone, Default)]
pub struct JsExecutionReportOperation {
    underlying_asset: Option<Asset>,
    settlement_asset: Option<Asset>,
    account_id: Option<AccountId>,
    side: Option<Side>,
}

#[wasm_bindgen(js_class = ExecutionReportOperation)]
impl JsExecutionReportOperation {
    /// Constructs an empty operation group; populate it through the setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsExecutionReportOperation {
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

    /// The account identifier, or `undefined`.
    #[wasm_bindgen(getter, js_name = accountId)]
    pub fn account_id(&self) -> Option<JsAccountId> {
        self.account_id.map(JsAccountId::from_inner)
    }

    /// Sets the account identifier.
    ///
    /// Accepts an `AccountId` object or a `number | bigint | string`.
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

    /// Returns a deep copy of this operation group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsExecutionReportOperation {
        self.clone()
    }
}

impl JsExecutionReportOperation {
    /// Builds an operation group from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut operation = Self::default();
        operation.set_underlying_asset(read_optional_string(value, "underlyingAsset")?)?;
        operation.set_settlement_asset(read_optional_string(value, "settlementAsset")?)?;
        operation.set_account_id(read_field(value, "accountId")?.unchecked_into())?;
        operation.set_side(read_field(value, "side")?.unchecked_into())?;
        Ok(operation)
    }
}

/// Financial impact group: realized P&L and fee.
#[wasm_bindgen(js_name = FinancialImpact)]
#[derive(Clone, Copy)]
pub struct JsFinancialImpact {
    pnl: Option<Pnl>,
    fee: Option<Fee>,
}

#[wasm_bindgen(js_class = FinancialImpact)]
impl JsFinancialImpact {
    /// Constructs a financial-impact group from a P&L and a fee.
    ///
    /// Each argument accepts a value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(constructor)]
    pub fn new(pnl: PnlLike, fee: FeeLike) -> Result<JsFinancialImpact, JsValue> {
        Ok(Self {
            pnl: Some(resolve_pnl(pnl.into())?),
            fee: Some(resolve_fee(fee.into())?),
        })
    }

    /// The realized P&L, or `undefined` when omitted from a plain object.
    #[wasm_bindgen(getter, js_name = pnl)]
    pub fn pnl(&self) -> Option<JsPnl> {
        self.pnl.map(JsPnl::from_inner)
    }

    /// Sets the realized P&L.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = pnl)]
    pub fn set_pnl(&mut self, value: PnlLike) -> Result<(), JsValue> {
        self.pnl = Some(resolve_pnl(value.into())?);
        Ok(())
    }

    /// The fee, or `undefined` when omitted from a plain object.
    #[wasm_bindgen(getter, js_name = fee)]
    pub fn fee(&self) -> Option<JsFee> {
        self.fee.map(JsFee::from_inner)
    }

    /// Sets the fee.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = fee)]
    pub fn set_fee(&mut self, value: FeeLike) -> Result<(), JsValue> {
        self.fee = Some(resolve_fee(value.into())?);
        Ok(())
    }

    /// Returns a fresh `FinancialImpact` holding the same P&L and fee.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsFinancialImpact {
        *self
    }
}

impl JsFinancialImpact {
    /// Builds a financial-impact group from a plain object literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid present `pnl`/`fee`.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let pnl = read_field(value, "pnl")?;
        let fee = read_field(value, "fee")?;
        Ok(Self {
            pnl: if pnl.is_undefined() || pnl.is_null() {
                None
            } else {
                Some(resolve_pnl(pnl)?)
            },
            fee: if fee.is_undefined() || fee.is_null() {
                None
            } else {
                Some(resolve_fee(fee)?)
            },
        })
    }
}

/// Fill-details group: lock, fee, remaining quantity, last trade, and finality.
#[wasm_bindgen(js_name = ExecutionReportFillDetails)]
#[derive(Clone)]
pub struct JsExecutionReportFillDetails {
    last_trade: Option<Trade>,
    fee: Option<MonetaryAmount>,
    leaves_quantity: Option<Quantity>,
    lock: Option<JsLock>,
    is_final: Option<bool>,
}

#[wasm_bindgen(js_class = ExecutionReportFillDetails)]
impl JsExecutionReportFillDetails {
    /// Constructs a fill-details group around the required `lock`.
    ///
    /// The lock is mandatory; the other fields default to absent and are set
    /// through their setters.
    #[wasm_bindgen(constructor)]
    pub fn new(lock: &JsLock) -> JsExecutionReportFillDetails {
        Self {
            last_trade: None,
            fee: None,
            leaves_quantity: None,
            lock: Some(JsLock::from_inner(lock.inner())),
            is_final: None,
        }
    }

    /// The reservation lock for this fill, or `undefined` when omitted from a
    /// plain object.
    #[wasm_bindgen(getter, js_name = lock)]
    pub fn lock(&self) -> Option<JsLock> {
        self.lock
            .as_ref()
            .map(|lock| JsLock::from_inner(lock.inner()))
    }

    /// Sets the reservation lock.
    #[wasm_bindgen(setter, js_name = lock)]
    pub fn set_lock(&mut self, value: &JsLock) {
        self.lock = Some(JsLock::from_inner(value.inner()));
    }

    /// The fee amount and currency for this fill, or `undefined`.
    #[wasm_bindgen(getter, js_name = fee)]
    pub fn fee(&self) -> Option<JsMonetaryAmount> {
        self.fee.clone().map(JsMonetaryAmount::from_inner)
    }

    /// Sets or clears the fee amount and currency.
    ///
    /// Accepts a `MonetaryAmount` object or a `MonetaryAmountInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when a present value cannot be
    /// marshalled into the core value types.
    #[wasm_bindgen(setter, js_name = fee)]
    pub fn set_fee(&mut self, value: MonetaryAmountLike) -> Result<(), JsValue> {
        self.fee = JsMonetaryAmount::resolve_optional(value.into())?;
        Ok(())
    }

    /// The remaining (unfilled) quantity, or `undefined`.
    #[wasm_bindgen(getter, js_name = leavesQuantity)]
    pub fn leaves_quantity(&self) -> Option<JsQuantity> {
        self.leaves_quantity.map(JsQuantity::from_inner)
    }

    /// Sets the remaining quantity (accepts a value object or `DecimalInput`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(setter, js_name = leavesQuantity)]
    pub fn set_leaves_quantity(&mut self, value: OptionalQuantityLike) -> Result<(), JsValue> {
        self.leaves_quantity = resolve_optional_quantity(value.into())?;
        Ok(())
    }

    /// The last executed trade, or `undefined`.
    #[wasm_bindgen(getter, js_name = lastTrade)]
    pub fn last_trade(&self) -> Option<JsTrade> {
        self.last_trade.map(|inner| JsTrade { inner })
    }

    /// Sets the last executed trade (accepts a `Trade` object or a plain
    /// `TradeInit` literal).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is neither a `Trade` nor a valid
    /// literal.
    #[wasm_bindgen(setter, js_name = lastTrade)]
    pub fn set_last_trade(&mut self, value: TradeLike) -> Result<(), JsValue> {
        self.last_trade = resolve_optional_trade(value.into())?;
        Ok(())
    }

    /// Whether this report closes the order's report stream, or `undefined`.
    #[wasm_bindgen(getter, js_name = isFinal)]
    pub fn is_final(&self) -> Option<bool> {
        self.is_final
    }

    /// Sets the finality flag.
    #[wasm_bindgen(setter, js_name = isFinal)]
    pub fn set_is_final(&mut self, value: Option<bool>) {
        self.is_final = value;
    }

    /// Returns a deep copy of this fill-details group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsExecutionReportFillDetails {
        self.clone()
    }
}

impl JsExecutionReportFillDetails {
    /// Builds a fill-details group from a plain object literal. A present
    /// `lock` must be a `Lock` instance.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a present `lock` is not a `Lock`, or on an
    /// invalid `lastTrade`/`leavesQuantity`.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let lock_value = read_field(value, "lock")?;
        let lock = if lock_value.is_undefined() || lock_value.is_null() {
            None
        } else {
            Some(
                extract_cloned_wrapper::<JsLock>(&lock_value)?
                    .ok_or_else(|| invalid_field("fill.lock must be a Lock"))?,
            )
        };
        let mut fill = Self {
            last_trade: None,
            fee: None,
            leaves_quantity: None,
            lock: lock.map(|lock| JsLock::from_inner(lock.inner())),
            is_final: None,
        };
        fill.set_fee(read_field(value, "fee")?.unchecked_into())?;
        fill.set_last_trade(read_field(value, "lastTrade")?.unchecked_into())?;
        fill.set_leaves_quantity(read_field(value, "leavesQuantity")?.unchecked_into())?;
        fill.is_final = read_optional_bool(value, "isFinal")?;
        Ok(fill)
    }
}

/// Position-impact group: opening/closing effect and resulting side.
#[wasm_bindgen(js_name = ExecutionReportPositionImpact)]
#[derive(Clone, Copy, Default)]
pub struct JsExecutionReportPositionImpact {
    position_effect: Option<PositionEffect>,
    position_side: Option<PositionSide>,
}

#[wasm_bindgen(js_class = ExecutionReportPositionImpact)]
impl JsExecutionReportPositionImpact {
    /// Constructs an empty position-impact group.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsExecutionReportPositionImpact {
        Self::default()
    }

    /// The position effect wire string (`"OPEN"`/`"CLOSE"`), or `undefined`.
    #[wasm_bindgen(getter, js_name = positionEffect)]
    pub fn position_effect(&self) -> Option<String> {
        self.position_effect.map(|effect| effect.to_string())
    }

    /// Sets the position effect from its wire string.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is not `"OPEN"`/`"CLOSE"`.
    #[wasm_bindgen(setter, js_name = positionEffect)]
    pub fn set_position_effect(
        &mut self,
        value: OptionalPositionEffectLike,
    ) -> Result<(), JsValue> {
        self.position_effect = resolve_optional_position_effect(value.into())?;
        Ok(())
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

    /// Returns a fresh copy of this position-impact group.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsExecutionReportPositionImpact {
        *self
    }
}

impl JsExecutionReportPositionImpact {
    /// Builds a position-impact group from a plain object literal.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        let mut impact = Self::default();
        impact.set_position_effect(read_field(value, "positionEffect")?.unchecked_into())?;
        impact.set_position_side(read_field(value, "positionSide")?.unchecked_into())?;
        Ok(impact)
    }
}

/// Top-level execution report assembled from its four groups.
#[wasm_bindgen(js_name = ExecutionReport)]
#[derive(Clone, Default)]
pub struct JsExecutionReport {
    operation: Option<JsExecutionReportOperation>,
    financial_impact: Option<JsFinancialImpact>,
    fill: Option<JsExecutionReportFillDetails>,
    position_impact: Option<JsExecutionReportPositionImpact>,
}

#[wasm_bindgen(js_class = ExecutionReport)]
impl JsExecutionReport {
    /// Constructs an empty report; populate it through the group setters.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsExecutionReport {
        Self::default()
    }

    /// The operation group, or `undefined`.
    #[wasm_bindgen(getter, js_name = operation)]
    pub fn operation(&self) -> Option<JsExecutionReportOperation> {
        self.operation.clone()
    }

    /// Sets the operation group.
    ///
    /// Accepts an `ExecutionReportOperation` object or a plain
    /// `ExecutionReportOperationInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = operation)]
    pub fn set_operation(&mut self, value: ExecutionReportOperationLike) -> Result<(), JsValue> {
        self.operation = coerce_group(
            value.into(),
            JsExecutionReportOperation::from_object,
            "operation must be an ExecutionReportOperation or a plain object",
        )?;
        Ok(())
    }

    /// The financial-impact group, or `undefined`.
    #[wasm_bindgen(getter, js_name = financialImpact)]
    pub fn financial_impact(&self) -> Option<JsFinancialImpact> {
        self.financial_impact
    }

    /// Sets the financial-impact group.
    ///
    /// Accepts a `FinancialImpact` object or a plain `FinancialImpactInit`
    /// literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = financialImpact)]
    pub fn set_financial_impact(&mut self, value: FinancialImpactLike) -> Result<(), JsValue> {
        self.financial_impact = coerce_group(
            value.into(),
            JsFinancialImpact::from_object,
            "financialImpact must be a FinancialImpact or a plain object",
        )?;
        Ok(())
    }

    /// The fill-details group, or `undefined`.
    #[wasm_bindgen(getter, js_name = fill)]
    pub fn fill(&self) -> Option<JsExecutionReportFillDetails> {
        self.fill.clone()
    }

    /// Sets the fill-details group.
    ///
    /// Accepts an `ExecutionReportFillDetails` object or a plain
    /// `ExecutionReportFillDetailsInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a present literal field is invalid.
    #[wasm_bindgen(setter, js_name = fill)]
    pub fn set_fill(&mut self, value: ExecutionReportFillDetailsLike) -> Result<(), JsValue> {
        self.fill = coerce_group(
            value.into(),
            JsExecutionReportFillDetails::from_object,
            "fill must be an ExecutionReportFillDetails or a plain object",
        )?;
        Ok(())
    }

    /// The position-impact group, or `undefined`.
    #[wasm_bindgen(getter, js_name = positionImpact)]
    pub fn position_impact(&self) -> Option<JsExecutionReportPositionImpact> {
        self.position_impact
    }

    /// Sets the position-impact group.
    ///
    /// Accepts an `ExecutionReportPositionImpact` object or a plain
    /// `ExecutionReportPositionImpactInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a literal field is invalid.
    #[wasm_bindgen(setter, js_name = positionImpact)]
    pub fn set_position_impact(
        &mut self,
        value: ExecutionReportPositionImpactLike,
    ) -> Result<(), JsValue> {
        self.position_impact = coerce_group(
            value.into(),
            JsExecutionReportPositionImpact::from_object,
            "positionImpact must be an ExecutionReportPositionImpact or a plain object",
        )?;
        Ok(())
    }

    /// Returns a deep copy of this report, including its groups.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsExecutionReport {
        self.clone()
    }
}

/// Resolves an optional group argument: a typed wrapper or a plain object
/// literal assembled by `from_object`.
///
/// `undefined`/`null` maps to `None`; a wrapper instance is downcast and taken
/// as-is; a plain object literal is built through `from_object`.
///
/// # Errors
///
/// Throws `ParamError`/`AssetError` on an invalid literal field, or
/// `ParamError` (with `message`) when the value is neither a wrapper nor a
/// plain object.
fn coerce_group<Wrapper>(
    value: JsValue,
    from_object: impl Fn(&JsValue) -> Result<Wrapper, JsValue>,
    message: &str,
) -> Result<Option<Wrapper>, JsValue>
where
    Wrapper: TryFromJsValue,
{
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<Wrapper>(&value)? {
        return Ok(Some(wrapped));
    }
    if is_plain_object(&value) {
        return from_object(&value).map(Some);
    }
    Err(invalid_field(message))
}

impl JsExecutionReportOperation {
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
    pub(crate) fn account_id_inner(&self) -> Option<AccountId> {
        self.account_id
    }

    /// Returns the wrapped side.
    pub(crate) fn side_inner(&self) -> Option<Side> {
        self.side
    }
}

impl JsFinancialImpact {
    /// Returns the wrapped P&L.
    pub(crate) fn pnl_inner(&self) -> Option<Pnl> {
        self.pnl
    }

    /// Returns the wrapped fee.
    pub(crate) fn fee_inner(&self) -> Option<Fee> {
        self.fee
    }
}

impl JsExecutionReportFillDetails {
    /// Returns the wrapped last trade.
    pub(crate) fn last_trade_inner(&self) -> Option<Trade> {
        self.last_trade
    }

    /// Returns the wrapped fill fee.
    pub(crate) fn fee_inner(&self) -> Option<MonetaryAmount> {
        self.fee.clone()
    }

    /// Returns the wrapped leaves quantity.
    pub(crate) fn leaves_quantity_inner(&self) -> Option<Quantity> {
        self.leaves_quantity
    }

    /// Returns the wrapped lock payload.
    pub(crate) fn lock_inner(&self) -> Option<openpit::pretrade::PreTradeLock> {
        self.lock.as_ref().map(JsLock::inner)
    }

    /// Returns the finality flag.
    pub(crate) fn is_final_inner(&self) -> Option<bool> {
        self.is_final
    }
}

impl JsExecutionReportPositionImpact {
    /// Returns the wrapped position effect.
    pub(crate) fn position_effect_inner(&self) -> Option<PositionEffect> {
        self.position_effect
    }

    /// Returns the wrapped position side.
    pub(crate) fn position_side_inner(&self) -> Option<PositionSide> {
        self.position_side
    }
}

impl JsExecutionReport {
    /// Builds a report from a plain object literal `{ operation?,
    /// financialImpact?, fill?, positionImpact? }`.
    fn from_object(value: &JsValue) -> Result<Self, JsValue> {
        Ok(Self {
            operation: coerce_group(
                read_field(value, "operation")?,
                JsExecutionReportOperation::from_object,
                "operation must be an ExecutionReportOperation or a plain object",
            )?,
            financial_impact: coerce_group(
                read_field(value, "financialImpact")?,
                JsFinancialImpact::from_object,
                "financialImpact must be a FinancialImpact or a plain object",
            )?,
            fill: coerce_group(
                read_field(value, "fill")?,
                JsExecutionReportFillDetails::from_object,
                "fill must be an ExecutionReportFillDetails or a plain object",
            )?,
            position_impact: coerce_group(
                read_field(value, "positionImpact")?,
                JsExecutionReportPositionImpact::from_object,
                "positionImpact must be an ExecutionReportPositionImpact or a plain object",
            )?,
        })
    }

    /// Resolves an `ExecutionReport | ExecutionReportInit` argument into an
    /// owned report.
    ///
    /// A wrapper instance is cloned at the JS level before extraction, so the
    /// engine borrows a copy and the caller's `ExecutionReport` stays usable; a
    /// plain object literal is assembled. Both paths are non-consuming.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` on an invalid literal field, or
    /// `ParamError` when the value is neither an `ExecutionReport` nor a plain
    /// object.
    pub(crate) fn coerce(value: JsValue) -> Result<JsExecutionReport, JsValue> {
        if let Some(cloned) = clone_wrapper_value(&value)? {
            if let Ok(wrapped) = JsExecutionReport::try_from_js_value(cloned) {
                return Ok(wrapped);
            }
        }
        if is_plain_object(&value) {
            return JsExecutionReport::from_object(&value);
        }
        Err(invalid_field(
            "report must be an ExecutionReport or a plain object",
        ))
    }

    /// Converts this report into the engine-facing interop request.
    ///
    /// Absent groups become the interop `Absent` access variant; present groups
    /// copy field by field into `Populated`.
    pub(crate) fn to_interop(&self) -> openpit_interop::ExecutionReport {
        use openpit_interop::{
            ExecutionReportFillAccess, ExecutionReportOperationAccess,
            ExecutionReportPositionImpactAccess, FinancialImpactAccess,
            PopulatedExecutionReportFill, PopulatedExecutionReportOperation,
            PopulatedExecutionReportPositionImpact, PopulatedFinancialImpact,
        };

        let operation = match &self.operation {
            None => ExecutionReportOperationAccess::Absent,
            Some(op) => {
                ExecutionReportOperationAccess::Populated(PopulatedExecutionReportOperation {
                    instrument: op.instrument(),
                    account_id: op.account_id_inner(),
                    side: op.side_inner(),
                })
            }
        };

        let financial_impact = match &self.financial_impact {
            None => FinancialImpactAccess::Absent,
            Some(fi) => FinancialImpactAccess::Populated(PopulatedFinancialImpact {
                pnl: fi.pnl_inner(),
                fee: fi.fee_inner(),
            }),
        };

        let fill = match &self.fill {
            None => ExecutionReportFillAccess::Absent,
            Some(f) => {
                ExecutionReportFillAccess::Populated(Box::new(PopulatedExecutionReportFill {
                    last_trade: f.last_trade_inner(),
                    fee: f.fee_inner(),
                    leaves_quantity: f.leaves_quantity_inner(),
                    lock: f.lock_inner(),
                    is_final: f.is_final_inner(),
                }))
            }
        };

        let position_impact = match &self.position_impact {
            None => ExecutionReportPositionImpactAccess::Absent,
            Some(pi) => ExecutionReportPositionImpactAccess::Populated(
                PopulatedExecutionReportPositionImpact {
                    position_effect: pi.position_effect_inner(),
                    position_side: pi.position_side_inner(),
                },
            ),
        };

        openpit_interop::ExecutionReport {
            operation,
            financial_impact,
            fill,
            position_impact,
        }
    }
}
