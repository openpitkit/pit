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

use openpit::param::{Asset, Pnl, PositionSize, Price};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::domain::{
    extract_cloned_wrapper, parse_asset, parse_bounded_number, resolve_pnl, resolve_position_size,
    IntegerNumber, PnlLike, PositionSizeLike,
};
use crate::error::{make_error, ErrorKind};
use crate::param::value_types::{JsPnl, JsPositionSize, JsPrice};

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
    /// Throws `ParamError` on an invalid value.
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
    /// Throws `ParamError` on an invalid value.
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

/// Per-asset adjustment outcome across the balance, held, and incoming buckets.
#[wasm_bindgen(js_name = AccountOutcomeEntry)]
#[derive(Clone)]
pub struct JsAccountOutcomeEntry {
    asset: Asset,
    balance: Option<JsOutcomeAmount>,
    held: Option<JsOutcomeAmount>,
    incoming: Option<JsOutcomeAmount>,
    realized_pnl: Option<JsPnlOutcomeAmount>,
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
    /// Throws `AssetError` when `asset` is empty.
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
        #[wasm_bindgen(unchecked_optional_param_type = "PnlOutcomeAmount | null")]
        realized_pnl: Option<JsValue>,
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

    /// The realized-P&L outcome, or `undefined`.
    #[wasm_bindgen(getter, js_name = realizedPnl)]
    pub fn realized_pnl(&self) -> Option<JsPnlOutcomeAmount> {
        self.realized_pnl
    }

    /// The resulting average entry price, or `undefined`.
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
            realized_pnl: value
                .realized_pnl
                .as_ref()
                .map(JsPnlOutcomeAmount::from_core),
            average_entry_price: value.average_entry_price,
        }
    }

    /// Builds the core outcome entry from this wrapper.
    pub(crate) fn to_core(&self) -> openpit::AccountOutcomeEntry {
        openpit::AccountOutcomeEntry {
            asset: self.asset.clone(),
            balance: self.balance.map(JsOutcomeAmount::to_core),
            held: self.held.map(JsOutcomeAmount::to_core),
            incoming: self.incoming.map(JsOutcomeAmount::to_core),
            realized_pnl: self.realized_pnl.map(JsPnlOutcomeAmount::to_core),
            average_entry_price: self.average_entry_price,
        }
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
    /// Throws `ParamError` (code `"Other"`) when `policyGroupId` is outside
    /// `0..=65535`.
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
    pub(crate) fn to_core(&self) -> openpit::AccountAdjustmentOutcome {
        openpit::AccountAdjustmentOutcome {
            policy_group_id: openpit::PolicyGroupId::new(self.policy_group_id),
            entry: self.entry.to_core(),
        }
    }
}
