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

//! Account-adjustment outcome payload bindings.
//!
//! These describe the already-applied effect of an account adjustment:
//! `OutcomeAmount` is a `(delta, absolute)` pair, `AccountOutcomeEntry` carries
//! the per-asset amounts, and `AccountAdjustmentOutcome` ties an entry to its
//! policy group. They are constructible so a custom policy can return them and
//! readable so the engine can surface applied outcomes.

use openpit::param::{AccountId, Asset, Pnl, PositionSize, Price};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::domain::{
    extract_cloned_wrapper, parse_asset, parse_bounded_number, resolve_account_id, resolve_pnl,
    resolve_position_size, AccountIdLike, IntegerNumber, PnlLike, PositionSizeLike,
};
use crate::error::{make_error, ErrorKind};
use crate::param::ids::JsAccountId;
use crate::param::value_types::{JsPnl, JsPositionSize, JsPrice};

#[wasm_bindgen(typescript_custom_section)]
const PNL_HALT_REASON_TS: &'static str = r#"
/** Stable wire discriminants exposed by {@link PnlHaltReason.kind}. */
export type PnlHaltReasonKind =
  | "missing-fx"
  | "missing-account-currency"
  | "missing-initial-pnl"
  | "missing-cost-basis"
  | "arithmetic-overflow";
"#;

/// A `(delta, absolute)` position-size pair describing an applied change.
#[wasm_bindgen(js_name = OutcomeAmount)]
#[derive(Clone, Copy)]
pub struct JsOutcomeAmount {
    delta: PositionSize,
    absolute: PositionSize,
}

#[wasm_bindgen(js_class = OutcomeAmount)]
impl JsOutcomeAmount {
    /// Constructs an outcome amount from a delta and an absolute value.
    ///
    /// Each argument accepts a `PositionSize` value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `TypeError`, `RangeError`, or `ParamError` when either argument
    /// is not a valid position-size value.
    #[wasm_bindgen(constructor)]
    pub fn new(
        delta: PositionSizeLike,
        absolute: PositionSizeLike,
    ) -> Result<JsOutcomeAmount, JsValue> {
        Ok(Self {
            delta: resolve_position_size(delta.into())?,
            absolute: resolve_position_size(absolute.into())?,
        })
    }

    /// The signed change applied.
    #[wasm_bindgen(getter, js_name = delta)]
    pub fn delta(&self) -> JsPositionSize {
        JsPositionSize::from_inner(self.delta)
    }

    /// The resulting absolute value.
    #[wasm_bindgen(getter, js_name = absolute)]
    pub fn absolute(&self) -> JsPositionSize {
        JsPositionSize::from_inner(self.absolute)
    }

    /// Returns a fresh copy of this outcome amount.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOutcomeAmount {
        *self
    }
}

impl JsOutcomeAmount {
    /// Builds an outcome amount from the core type.
    pub(crate) fn from_core(value: &openpit::OutcomeAmount) -> Self {
        Self {
            delta: value.delta,
            absolute: value.absolute,
        }
    }

    /// Builds the core outcome amount from this wrapper.
    pub(crate) fn to_core(self) -> openpit::OutcomeAmount {
        openpit::OutcomeAmount {
            delta: self.delta,
            absolute: self.absolute,
        }
    }
}

/// A `(delta, absolute)` P&L pair describing an applied realized-P&L change.
///
/// The producing policy defines the denomination. SpotFunds position and
/// account P&L outcomes use the account currency.
#[wasm_bindgen(js_name = PnlOutcomeAmount)]
#[derive(Clone, Copy)]
pub struct JsPnlOutcomeAmount {
    delta: Pnl,
    absolute: Pnl,
}

#[wasm_bindgen(js_class = PnlOutcomeAmount)]
impl JsPnlOutcomeAmount {
    /// Constructs a P&L outcome amount from a delta and an absolute value.
    ///
    /// Each argument accepts a `Pnl` value object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `TypeError`, `RangeError`, or `ParamError` when either argument
    /// is not a valid P&L value.
    #[wasm_bindgen(constructor)]
    pub fn new(delta: PnlLike, absolute: PnlLike) -> Result<JsPnlOutcomeAmount, JsValue> {
        Ok(Self {
            delta: resolve_pnl(delta.into())?,
            absolute: resolve_pnl(absolute.into())?,
        })
    }

    /// The signed P&L change applied.
    #[wasm_bindgen(getter, js_name = delta)]
    pub fn delta(&self) -> JsPnl {
        JsPnl::from_inner(self.delta)
    }

    /// The resulting absolute P&L value.
    #[wasm_bindgen(getter, js_name = absolute)]
    pub fn absolute(&self) -> JsPnl {
        JsPnl::from_inner(self.absolute)
    }

    /// Returns a fresh copy of this P&L outcome amount.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPnlOutcomeAmount {
        *self
    }
}

impl JsPnlOutcomeAmount {
    /// Builds a P&L outcome amount from the core type.
    pub(crate) fn from_core(value: &openpit::PnlOutcomeAmount) -> Self {
        Self {
            delta: value.delta,
            absolute: value.absolute,
        }
    }

    /// Builds the core P&L outcome amount from this wrapper.
    pub(crate) fn to_core(self) -> openpit::PnlOutcomeAmount {
        openpit::PnlOutcomeAmount {
            delta: self.delta,
            absolute: self.absolute,
        }
    }
}

/// Reason why a realized-PnL value could not be calculated.
#[wasm_bindgen(js_name = PnlHaltReason)]
#[derive(Clone, Copy)]
pub struct JsPnlHaltReason {
    inner: openpit::PnlHaltReason,
}

#[wasm_bindgen(js_class = PnlHaltReason)]
impl JsPnlHaltReason {
    /// Builds a halt reason for a computation with no available required FX
    /// quote.
    ///
    /// SpotFunds accepts the latest known quote regardless of staleness. A
    /// stale quote alone does not halt P&L; this reason means no required quote
    /// was available and no authoritative P&L value could be calculated.
    #[wasm_bindgen(js_name = fromMissingFx)]
    pub fn new_missing_fx() -> JsPnlHaltReason {
        Self {
            inner: openpit::PnlHaltReason::MissingFx,
        }
    }

    /// Builds a halt reason for a computation missing the account currency.
    #[wasm_bindgen(js_name = fromMissingAccountCurrency)]
    pub fn new_missing_account_currency() -> JsPnlHaltReason {
        Self {
            inner: openpit::PnlHaltReason::MissingAccountCurrency,
        }
    }

    /// Builds a halt reason for a position without authoritative initial PnL.
    #[wasm_bindgen(js_name = fromMissingInitialPnl)]
    pub fn new_missing_initial_pnl() -> JsPnlHaltReason {
        Self {
            inner: openpit::PnlHaltReason::MissingInitialPnl,
        }
    }

    /// Builds a halt reason for a position without the required cost basis.
    #[wasm_bindgen(js_name = fromMissingCostBasis)]
    pub fn new_missing_cost_basis() -> JsPnlHaltReason {
        Self {
            inner: openpit::PnlHaltReason::MissingCostBasis,
        }
    }

    /// Builds a halt reason for arithmetic outside the affected accumulator's
    /// supported PnL range.
    #[wasm_bindgen(js_name = fromArithmeticOverflow)]
    pub fn new_arithmetic_overflow() -> JsPnlHaltReason {
        Self {
            inner: openpit::PnlHaltReason::ArithmeticOverflow,
        }
    }

    /// Stable wire discriminant suitable for logs, persistence, and switches.
    #[wasm_bindgen(
        getter,
        js_name = kind,
        unchecked_return_type = "PnlHaltReasonKind"
    )]
    pub fn kind(&self) -> String {
        match self.inner {
            openpit::PnlHaltReason::MissingFx => "missing-fx",
            openpit::PnlHaltReason::MissingAccountCurrency => "missing-account-currency",
            openpit::PnlHaltReason::MissingInitialPnl => "missing-initial-pnl",
            openpit::PnlHaltReason::MissingCostBasis => "missing-cost-basis",
            openpit::PnlHaltReason::ArithmeticOverflow => "arithmetic-overflow",
        }
        .to_owned()
    }

    /// Returns `true` when no required FX quote was available.
    ///
    /// SpotFunds accepts stale quotes, so staleness alone does not set this
    /// reason.
    #[wasm_bindgen(getter, js_name = isMissingFx)]
    pub fn is_missing_fx(&self) -> bool {
        matches!(self.inner, openpit::PnlHaltReason::MissingFx)
    }

    /// Returns `true` when the account currency was unavailable.
    #[wasm_bindgen(getter, js_name = isMissingAccountCurrency)]
    pub fn is_missing_account_currency(&self) -> bool {
        matches!(self.inner, openpit::PnlHaltReason::MissingAccountCurrency)
    }

    /// Returns `true` when no authoritative initial PnL was available.
    #[wasm_bindgen(getter, js_name = isMissingInitialPnl)]
    pub fn is_missing_initial_pnl(&self) -> bool {
        matches!(self.inner, openpit::PnlHaltReason::MissingInitialPnl)
    }

    /// Returns `true` when the required position cost basis was unavailable.
    #[wasm_bindgen(getter, js_name = isMissingCostBasis)]
    pub fn is_missing_cost_basis(&self) -> bool {
        matches!(self.inner, openpit::PnlHaltReason::MissingCostBasis)
    }

    /// Returns `true` when PnL arithmetic overflowed.
    #[wasm_bindgen(getter, js_name = isArithmeticOverflow)]
    pub fn is_arithmetic_overflow(&self) -> bool {
        matches!(self.inner, openpit::PnlHaltReason::ArithmeticOverflow)
    }

    /// Returns a deep copy of this PnL halt reason.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPnlHaltReason {
        *self
    }
}

impl JsPnlHaltReason {
    /// Builds the core PnL halt reason from this wrapper.
    pub(crate) fn to_core(self) -> openpit::PnlHaltReason {
        self.inner
    }
}

/// Resolves an explicit PnL accumulator state from a value or halt reason.
///
/// Numeric inputs follow the ordinary `Pnl | DecimalInput` conversion path;
/// a `PnlHaltReason` selects the halted state. Nullish values are not states.
pub(crate) fn resolve_pnl_state(value: JsValue) -> Result<openpit::PnlState, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Err(make_error(
            ErrorKind::Type,
            "state must be a Pnl, DecimalInput, or PnlHaltReason",
            None,
        ));
    }
    if let Some(reason) = extract_cloned_wrapper::<JsPnlHaltReason>(&value)? {
        return Ok(openpit::PnlState::Halted(reason.to_core()));
    }
    resolve_pnl(value).map(openpit::PnlState::Value)
}

/// Converts a normalized PnL accumulator state back to its public JS union.
pub(crate) fn pnl_state_to_js(state: openpit::PnlState) -> JsValue {
    match state {
        openpit::PnlState::Value(pnl) => JsValue::from(JsPnl::from_inner(pnl)),
        openpit::PnlState::Halted(reason) => JsValue::from(JsPnlHaltReason { inner: reason }),
    }
}

/// Realized-PnL result: either the amount or a halt reason.
///
/// Exactly one of `pnl` and `haltReason` is present.
#[wasm_bindgen(js_name = PnlOutcome)]
#[derive(Clone)]
pub struct JsPnlOutcome {
    pnl: Option<JsPnlOutcomeAmount>,
    halt_reason: Option<JsPnlHaltReason>,
}

#[wasm_bindgen(js_class = PnlOutcome)]
impl JsPnlOutcome {
    /// Constructs a realized-PnL outcome.
    ///
    /// Pass either a computed `pnl` or a `haltReason` created by a
    /// `PnlHaltReason` factory.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` unless exactly one of `pnl` and `haltReason` is
    /// present.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_optional_param_type = "PnlOutcomeAmount | null")] pnl: Option<
            JsValue,
        >,
        #[wasm_bindgen(unchecked_optional_param_type = "PnlHaltReason | null")] halt_reason: Option<
            JsValue,
        >,
    ) -> Result<JsPnlOutcome, JsValue> {
        Self::from_parts(
            pnl,
            halt_reason,
            "PnL outcome requires exactly one of pnl or haltReason",
        )
    }

    /// The computed PnL, or `undefined` when a halt reason is present.
    #[wasm_bindgen(getter, js_name = pnl)]
    pub fn pnl(&self) -> Option<JsPnlOutcomeAmount> {
        self.pnl
    }

    /// The reason why PnL could not be calculated, or `undefined` when it is
    /// available.
    #[wasm_bindgen(getter, js_name = haltReason)]
    pub fn halt_reason(&self) -> Option<JsPnlHaltReason> {
        self.halt_reason
    }

    /// Whether this outcome contains authoritative PnL.
    #[wasm_bindgen(getter, js_name = ok)]
    pub fn ok(&self) -> bool {
        self.pnl.is_some()
    }

    /// Whether this accumulator halted during the operation.
    #[wasm_bindgen(getter, js_name = isHalted)]
    pub fn is_halted(&self) -> bool {
        self.halt_reason.is_some()
    }

    /// Returns a deep copy of this PnL outcome.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPnlOutcome {
        self.clone()
    }
}

impl JsPnlOutcome {
    fn from_parts(
        pnl: Option<JsValue>,
        halt_reason: Option<JsValue>,
        invalid_message: &str,
    ) -> Result<Self, JsValue> {
        let pnl = optional_cloned_wrapper(pnl, "pnl")?;
        let halt_reason = optional_cloned_wrapper(halt_reason, "haltReason")?;
        if pnl.is_some() == halt_reason.is_some() {
            return Err(make_error(ErrorKind::Type, invalid_message, None));
        }
        Ok(Self { pnl, halt_reason })
    }

    /// Builds a PnL outcome from the core type.
    pub(crate) fn from_core(value: &openpit::PnlOutcome) -> Self {
        match value {
            Ok(amount) => Self {
                pnl: Some(JsPnlOutcomeAmount::from_core(amount)),
                halt_reason: None,
            },
            Err(reason) => Self {
                pnl: None,
                halt_reason: Some(JsPnlHaltReason { inner: *reason }),
            },
        }
    }

    /// Builds the core PnL outcome from this wrapper.
    pub(crate) fn to_core(&self) -> Result<openpit::PnlOutcome, JsValue> {
        match (&self.pnl, &self.halt_reason) {
            (Some(amount), None) => Ok(Ok(amount.to_core())),
            (None, Some(reason)) => Ok(Err(reason.to_core())),
            _ => Err(make_error(
                ErrorKind::Type,
                "PnL outcome requires exactly one of pnl or haltReason",
                None,
            )),
        }
    }
}

/// Policy-tagged account-level realized-PnL outcome.
///
/// SpotFunds denominates both the delta and absolute value in the account
/// currency.
/// SpotFunds emits a halted outcome only for the report that transitions the
/// account accumulator to halted. Later reports omit the unchanged halt until
/// its account PnL is explicitly force-set. Re-arming a position does not
/// re-arm account PnL, and re-arming account PnL does not re-arm any position.
///
/// Exactly one of `pnl` and `haltReason` is present.
#[wasm_bindgen(js_name = AccountPnlOutcome)]
#[derive(Clone)]
pub struct JsAccountPnlOutcome {
    policy_group_id: u16,
    account_id: AccountId,
    outcome: JsPnlOutcome,
}

#[wasm_bindgen(js_class = AccountPnlOutcome)]
impl JsAccountPnlOutcome {
    /// Constructs a policy-tagged account-level PnL outcome.
    ///
    /// `policyGroupId` accepts an integer in `0..=65535`. `accountId` accepts
    /// an `AccountId` object or a `number | bigint | string`.
    /// Pass either a computed `pnl` or a `haltReason` created by a
    /// `PnlHaltReason` factory.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` unless exactly one of `pnl` and `haltReason` is
    /// present, `TypeError`/`RangeError` when `policyGroupId` is invalid, or
    /// `AccountIdError` when the account is invalid.
    #[wasm_bindgen(constructor)]
    pub fn new(
        policy_group_id: IntegerNumber,
        account_id: AccountIdLike,
        #[wasm_bindgen(unchecked_optional_param_type = "PnlOutcomeAmount | null")] pnl: Option<
            JsValue,
        >,
        #[wasm_bindgen(unchecked_optional_param_type = "PnlHaltReason | null")] halt_reason: Option<
            JsValue,
        >,
    ) -> Result<JsAccountPnlOutcome, JsValue> {
        let outcome = JsPnlOutcome::from_parts(
            pnl,
            halt_reason,
            "account PnL outcome requires exactly one of pnl or haltReason",
        )?;
        Ok(Self {
            policy_group_id: parse_bounded_number(
                policy_group_id.into(),
                u64::from(u16::MAX),
                "policyGroupId",
            )? as u16,
            account_id: resolve_account_id(account_id.into())?,
            outcome,
        })
    }

    /// The policy-group tag of the policy that produced this outcome.
    #[wasm_bindgen(getter, js_name = policyGroupId)]
    pub fn policy_group_id(&self) -> u16 {
        self.policy_group_id
    }

    /// The account that owns the realized-PnL ledger.
    #[wasm_bindgen(getter, js_name = accountId)]
    pub fn account_id(&self) -> JsAccountId {
        JsAccountId::from_inner(self.account_id)
    }

    /// The computed PnL, or `undefined` when a halt reason is present.
    #[wasm_bindgen(getter, js_name = pnl)]
    pub fn pnl(&self) -> Option<JsPnlOutcomeAmount> {
        self.outcome.pnl()
    }

    /// The reason why PnL could not be calculated, or `undefined` when it is
    /// available.
    #[wasm_bindgen(getter, js_name = haltReason)]
    pub fn halt_reason(&self) -> Option<JsPnlHaltReason> {
        self.outcome.halt_reason()
    }

    /// Whether this outcome contains authoritative PnL.
    #[wasm_bindgen(getter, js_name = ok)]
    pub fn ok(&self) -> bool {
        self.outcome.ok()
    }

    /// Whether this account accumulator halted during the operation.
    #[wasm_bindgen(getter, js_name = isHalted)]
    pub fn is_halted(&self) -> bool {
        self.outcome.is_halted()
    }

    /// Returns a deep copy of this account-level PnL outcome.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountPnlOutcome {
        self.clone()
    }
}

impl JsAccountPnlOutcome {
    /// Builds an account-level PnL outcome from the core type.
    pub(crate) fn from_core(value: &openpit::AccountPnlOutcome) -> Self {
        Self {
            policy_group_id: value.policy_group_id.value(),
            account_id: value.account_id,
            outcome: JsPnlOutcome::from_core(&value.result),
        }
    }

    /// Builds the core account-level PnL outcome from this wrapper.
    pub(crate) fn to_core(&self) -> Result<openpit::AccountPnlOutcome, JsValue> {
        let result = self.outcome.to_core()?;
        Ok(openpit::AccountPnlOutcome {
            result,
            account_id: self.account_id,
            policy_group_id: openpit::PolicyGroupId::new(self.policy_group_id),
        })
    }
}

/// Per-asset adjustment outcome across the balance, held, and incoming buckets.
#[wasm_bindgen(js_name = AccountOutcomeEntry)]
#[derive(Clone)]
pub struct JsAccountOutcomeEntry {
    asset: Asset,
    balance: Option<JsOutcomeAmount>,
    held: Option<JsOutcomeAmount>,
    incoming: Option<JsOutcomeAmount>,
    realized_pnl: Option<JsPnlOutcome>,
    average_entry_price: Option<Price>,
}

#[wasm_bindgen(js_class = AccountOutcomeEntry)]
impl JsAccountOutcomeEntry {
    /// Constructs an outcome entry for `asset`.
    ///
    /// The bucket amounts default to absent.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `asset` is empty or `TypeError` when an
    /// optional outcome is not the expected wrapper type.
    #[wasm_bindgen(constructor)]
    pub fn new(
        asset: String,
        #[wasm_bindgen(unchecked_optional_param_type = "OutcomeAmount | null")] balance: Option<
            JsValue,
        >,
        #[wasm_bindgen(unchecked_optional_param_type = "OutcomeAmount | null")] held: Option<
            JsValue,
        >,
        #[wasm_bindgen(unchecked_optional_param_type = "OutcomeAmount | null")] incoming: Option<
            JsValue,
        >,
        #[wasm_bindgen(unchecked_optional_param_type = "PnlOutcome | null")] realized_pnl: Option<
            JsValue,
        >,
        #[wasm_bindgen(unchecked_optional_param_type = "Price | null")] average_entry_price: Option<
            JsValue,
        >,
    ) -> Result<JsAccountOutcomeEntry, JsValue> {
        Ok(Self {
            asset: parse_asset(&asset)?,
            balance: optional_cloned_wrapper(balance, "balance")?,
            held: optional_cloned_wrapper(held, "held")?,
            incoming: optional_cloned_wrapper(incoming, "incoming")?,
            realized_pnl: optional_cloned_wrapper(realized_pnl, "realizedPnl")?,
            average_entry_price: optional_cloned_wrapper::<JsPrice>(
                average_entry_price,
                "averageEntryPrice",
            )?
            .map(|price| price.inner()),
        })
    }

    /// The asset this entry applies to.
    #[wasm_bindgen(getter, js_name = asset)]
    pub fn asset(&self) -> String {
        self.asset.to_string()
    }

    /// The balance-bucket outcome, or `undefined`.
    #[wasm_bindgen(getter, js_name = balance)]
    pub fn balance(&self) -> Option<JsOutcomeAmount> {
        self.balance
    }

    /// The held-bucket outcome, or `undefined`.
    #[wasm_bindgen(getter, js_name = held)]
    pub fn held(&self) -> Option<JsOutcomeAmount> {
        self.held
    }

    /// The incoming-bucket outcome, or `undefined`.
    #[wasm_bindgen(getter, js_name = incoming)]
    pub fn incoming(&self) -> Option<JsOutcomeAmount> {
        self.incoming
    }

    /// The account-currency realized-P&L outcome, or `undefined`.
    ///
    /// The operation that first cannot calculate PnL contains a halt reason;
    /// later operations omit the field until an adjustment force-sets PnL.
    #[wasm_bindgen(getter, js_name = realizedPnl)]
    pub fn realized_pnl(&self) -> Option<JsPnlOutcome> {
        self.realized_pnl.clone()
    }

    /// The resulting account-currency average entry price, or `undefined`.
    #[wasm_bindgen(getter, js_name = averageEntryPrice)]
    pub fn average_entry_price(&self) -> Option<JsPrice> {
        self.average_entry_price.map(JsPrice::from_inner)
    }

    /// Returns a deep copy of this outcome entry.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountOutcomeEntry {
        self.clone()
    }
}

/// Resolves an optional exported-class constructor argument without moving the
/// caller-owned wrapper.
fn optional_cloned_wrapper<T>(value: Option<JsValue>, field: &str) -> Result<Option<T>, JsValue>
where
    T: TryFromJsValue,
{
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    extract_cloned_wrapper(&value)?.map(Some).ok_or_else(|| {
        make_error(
            ErrorKind::Type,
            &format!("{field} must be the expected wrapper type"),
            None,
        )
    })
}

impl JsAccountOutcomeEntry {
    /// Builds an outcome entry from the core type.
    pub(crate) fn from_core(value: &openpit::AccountOutcomeEntry) -> Self {
        Self {
            asset: value.asset.clone(),
            balance: value.balance.as_ref().map(JsOutcomeAmount::from_core),
            held: value.held.as_ref().map(JsOutcomeAmount::from_core),
            incoming: value.incoming.as_ref().map(JsOutcomeAmount::from_core),
            realized_pnl: value.realized_pnl.as_ref().map(JsPnlOutcome::from_core),
            average_entry_price: value.average_entry_price,
        }
    }

    /// Builds the core outcome entry from this wrapper.
    pub(crate) fn to_core(&self) -> Result<openpit::AccountOutcomeEntry, JsValue> {
        Ok(openpit::AccountOutcomeEntry {
            asset: self.asset.clone(),
            balance: self.balance.map(JsOutcomeAmount::to_core),
            held: self.held.map(JsOutcomeAmount::to_core),
            incoming: self.incoming.map(JsOutcomeAmount::to_core),
            realized_pnl: self
                .realized_pnl
                .as_ref()
                .map(JsPnlOutcome::to_core)
                .transpose()?,
            average_entry_price: self.average_entry_price,
        })
    }
}

/// An adjustment outcome tied to its policy group.
#[wasm_bindgen(js_name = AccountAdjustmentOutcome)]
#[derive(Clone)]
pub struct JsAccountAdjustmentOutcome {
    policy_group_id: u16,
    entry: JsAccountOutcomeEntry,
}

#[wasm_bindgen(js_class = AccountAdjustmentOutcome)]
impl JsAccountAdjustmentOutcome {
    /// Constructs an adjustment outcome from a policy group id and an entry.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` when `policyGroupId` is not a number or `RangeError`
    /// when it is outside `0..=65535`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        policy_group_id: IntegerNumber,
        entry: &JsAccountOutcomeEntry,
    ) -> Result<JsAccountAdjustmentOutcome, JsValue> {
        let policy_group_id =
            parse_bounded_number(policy_group_id.into(), u64::from(u16::MAX), "policyGroupId")?
                as u16;
        Ok(Self {
            policy_group_id,
            entry: entry.clone(),
        })
    }

    /// The policy group identifier.
    #[wasm_bindgen(getter, js_name = policyGroupId)]
    pub fn policy_group_id(&self) -> u16 {
        self.policy_group_id
    }

    /// The per-asset outcome entry.
    #[wasm_bindgen(getter, js_name = entry)]
    pub fn entry(&self) -> JsAccountOutcomeEntry {
        self.entry.clone()
    }

    /// Returns a deep copy of this adjustment outcome.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountAdjustmentOutcome {
        self.clone()
    }
}

impl JsAccountAdjustmentOutcome {
    /// Builds an adjustment outcome from the core type.
    pub(crate) fn from_core(value: &openpit::AccountAdjustmentOutcome) -> Self {
        Self {
            policy_group_id: value.policy_group_id.value(),
            entry: JsAccountOutcomeEntry::from_core(&value.entry),
        }
    }

    /// Builds the core adjustment outcome from this wrapper.
    pub(crate) fn to_core(&self) -> Result<openpit::AccountAdjustmentOutcome, JsValue> {
        Ok(openpit::AccountAdjustmentOutcome {
            policy_group_id: openpit::PolicyGroupId::new(self.policy_group_id),
            entry: self.entry.to_core()?,
        })
    }
}
