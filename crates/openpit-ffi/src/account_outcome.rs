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

use crate::define_optional;
use crate::last_error::{write_error, OpenPitOutError};
use crate::param::{OpenPitParamPnl, OpenPitParamPositionSize, OpenPitParamPriceOptional};
use crate::OpenPitStringView;
use openpit::param::{Asset, Price};
use openpit::{
    AccountAdjustmentOutcome, AccountOutcomeEntry, OutcomeAmount, PnlOutcomeAmount, PolicyGroupId,
};

/// A delta/absolute pair for one position field.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitOutcomeAmount {
    /// Signed change applied by this operation relative to the field value at
    /// operation start. Authoritative for position bookkeeping.
    pub delta: OpenPitParamPositionSize,
    /// Field value at the moment the policy returned, before deferred commit.
    /// Treat as a convenience hint only; prefer `delta`.
    pub absolute: OpenPitParamPositionSize,
}

define_optional!(
    optional = OpenPitOutcomeAmountOptional,
    value = OpenPitOutcomeAmount
);

/// An account-currency delta/absolute pair for realized PnL.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitPnlOutcomeAmount {
    /// Signed account-currency PnL change applied by this operation.
    pub delta: OpenPitParamPnl,
    /// Cumulative account-currency realized PnL after this operation.
    pub absolute: OpenPitParamPnl,
}

define_optional!(
    optional = OpenPitPnlOutcomeAmountOptional,
    value = OpenPitPnlOutcomeAmount
);

/// Raw outcome data produced by a policy for one asset.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitAccountOutcomeEntry {
    /// Asset this outcome refers to.
    pub asset: OpenPitStringView,
    /// Settled balance/position outcome.
    pub balance: OpenPitOutcomeAmountOptional,
    /// Held (reserved) amount outcome.
    pub held: OpenPitOutcomeAmountOptional,
    /// Incoming (pending inflow) amount outcome.
    pub incoming: OpenPitOutcomeAmountOptional,
    /// Account-currency realized PnL outcome (delta = this op, absolute =
    /// cumulative). Unset means realized PnL was not tracked or not emitted;
    /// missing account currency or FX stops tracking without reject/block.
    pub realized_pnl: OpenPitPnlOutcomeAmountOptional,
    /// Current account-currency average entry price (absolute). Unset means
    /// average entry price was not tracked or not emitted; missing account
    /// currency or FX stops tracking without reject/block.
    pub average_entry_price: OpenPitParamPriceOptional,
}

/// Account position outcome with the group tag of the business entity that
/// produced it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitAccountAdjustmentOutcome {
    /// Policy-group tag of the policy that produced this outcome.
    pub policy_group_id: u16,
    /// Account adjustment outcome entry.
    pub entry: OpenPitAccountOutcomeEntry,
}

impl OpenPitAccountAdjustmentOutcome {
    pub(crate) fn from_outcome(inner: &AccountAdjustmentOutcome) -> Self {
        Self {
            policy_group_id: inner.policy_group_id.value(),
            entry: export_outcome_entry(&inner.entry),
        }
    }
}

fn export_outcome_amount(value: &OutcomeAmount) -> OpenPitOutcomeAmount {
    OpenPitOutcomeAmount {
        delta: OpenPitParamPositionSize(value.delta.to_decimal().into()),
        absolute: OpenPitParamPositionSize(value.absolute.to_decimal().into()),
    }
}

fn export_outcome_amount_optional(value: Option<&OutcomeAmount>) -> OpenPitOutcomeAmountOptional {
    match value {
        Some(amount) => OpenPitOutcomeAmountOptional {
            value: export_outcome_amount(amount),
            is_set: true,
        },
        None => OpenPitOutcomeAmountOptional::default(),
    }
}

fn export_pnl_outcome_amount(value: &PnlOutcomeAmount) -> OpenPitPnlOutcomeAmount {
    OpenPitPnlOutcomeAmount {
        delta: OpenPitParamPnl(value.delta.to_decimal().into()),
        absolute: OpenPitParamPnl(value.absolute.to_decimal().into()),
    }
}

fn export_pnl_outcome_amount_optional(
    value: Option<&PnlOutcomeAmount>,
) -> OpenPitPnlOutcomeAmountOptional {
    match value {
        Some(amount) => OpenPitPnlOutcomeAmountOptional {
            value: export_pnl_outcome_amount(amount),
            is_set: true,
        },
        None => OpenPitPnlOutcomeAmountOptional::default(),
    }
}

fn export_price_optional(value: Option<Price>) -> OpenPitParamPriceOptional {
    match value {
        Some(price) => OpenPitParamPriceOptional {
            value: crate::param::OpenPitParamPrice(price.to_decimal().into()),
            is_set: true,
        },
        None => OpenPitParamPriceOptional::default(),
    }
}

fn export_outcome_entry(value: &AccountOutcomeEntry) -> OpenPitAccountOutcomeEntry {
    OpenPitAccountOutcomeEntry {
        asset: OpenPitStringView::from_utf8(value.asset.as_ref()),
        balance: export_outcome_amount_optional(value.balance.as_ref()),
        held: export_outcome_amount_optional(value.held.as_ref()),
        incoming: export_outcome_amount_optional(value.incoming.as_ref()),
        realized_pnl: export_pnl_outcome_amount_optional(value.realized_pnl.as_ref()),
        average_entry_price: export_price_optional(value.average_entry_price),
    }
}

fn import_outcome_amount(value: &OpenPitOutcomeAmount) -> Result<OutcomeAmount, String> {
    Ok(OutcomeAmount {
        delta: value.delta.to_param()?,
        absolute: value.absolute.to_param()?,
    })
}

fn import_outcome_amount_optional(
    value: &OpenPitOutcomeAmountOptional,
) -> Result<Option<OutcomeAmount>, String> {
    if !value.is_set {
        return Ok(None);
    }
    Ok(Some(import_outcome_amount(&value.value)?))
}

fn import_pnl_outcome_amount(value: &OpenPitPnlOutcomeAmount) -> Result<PnlOutcomeAmount, String> {
    Ok(PnlOutcomeAmount {
        delta: value.delta.to_param()?,
        absolute: value.absolute.to_param()?,
    })
}

fn import_pnl_outcome_amount_optional(
    value: &OpenPitPnlOutcomeAmountOptional,
) -> Result<Option<PnlOutcomeAmount>, String> {
    if !value.is_set {
        return Ok(None);
    }
    Ok(Some(import_pnl_outcome_amount(&value.value)?))
}

impl OpenPitAccountOutcomeEntry {
    /// Imports this view into an owned [`AccountOutcomeEntry`].
    ///
    /// Counterpart of `export_outcome_entry`: parses the asset code with
    /// `Asset::new` and each optional outcome amount via
    /// `OpenPitParamPositionSize::to_param`.
    pub(crate) fn to_entry(self) -> Result<AccountOutcomeEntry, String> {
        let asset_code = if self.asset.ptr.is_null() {
            String::new()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(self.asset.ptr, self.asset.len) };
            std::str::from_utf8(bytes)
                .map_err(|_| "outcome entry asset is not valid string".to_owned())?
                .to_owned()
        };
        let asset = Asset::new(asset_code).map_err(|e| e.to_string())?;
        Ok(AccountOutcomeEntry {
            asset,
            balance: import_outcome_amount_optional(&self.balance)?,
            held: import_outcome_amount_optional(&self.held)?,
            incoming: import_outcome_amount_optional(&self.incoming)?,
            realized_pnl: import_pnl_outcome_amount_optional(&self.realized_pnl)?,
            average_entry_price: if self.average_entry_price.is_set {
                Some(self.average_entry_price.value.to_param()?)
            } else {
                None
            },
        })
    }
}

/// Caller-owned list of account-adjustment outcomes.
pub struct OpenPitAccountAdjustmentOutcomeList {
    pub(crate) items: Vec<AccountAdjustmentOutcome>,
}

pub(crate) fn outcomes_to_list_owned(
    values: Vec<AccountAdjustmentOutcome>,
) -> OpenPitAccountAdjustmentOutcomeList {
    OpenPitAccountAdjustmentOutcomeList { items: values }
}

#[no_mangle]
/// Releases a caller-owned account-adjustment outcome list.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
///
/// # Safety
///
/// `outcomes` must be either null or a pointer returned by this library.
/// The list must be destroyed at most once.
pub unsafe extern "C" fn openpit_destroy_account_adjustment_outcome_list(
    outcomes: *mut OpenPitAccountAdjustmentOutcomeList,
) {
    if outcomes.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(outcomes)) };
}

#[no_mangle]
/// Returns the number of outcomes in the list.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
///
/// # Safety
///
/// `list` must be a valid non-null pointer returned by this library and must
/// remain alive for the duration of this call.
pub unsafe extern "C" fn openpit_account_adjustment_outcome_list_len(
    list: *const OpenPitAccountAdjustmentOutcomeList,
) -> usize {
    assert!(!list.is_null(), "outcome list pointer is null");
    let list = unsafe { &*list };
    list.items.len()
}

#[no_mangle]
/// Copies a non-owning outcome view at `index` into `out_outcome`.
///
/// The copied view borrows string memory from `list`.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - `out_outcome` must be a valid non-null pointer;
/// - returns `true` when a value exists and was copied;
/// - returns `false` when `index` is out of bounds and does not write
///   `out_outcome`;
/// - the copied view remains valid while `list` is alive and unchanged;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
///
/// # Safety
///
/// `list` and `out_outcome` must be valid non-null pointers returned by or
/// provided to this library and must remain alive for the duration of this
/// call.
pub unsafe extern "C" fn openpit_account_adjustment_outcome_list_get(
    list: *const OpenPitAccountAdjustmentOutcomeList,
    index: usize,
    out_outcome: *mut OpenPitAccountAdjustmentOutcome,
) -> bool {
    assert!(!list.is_null(), "outcome list pointer is null");
    assert!(!out_outcome.is_null(), "outcome output pointer is null");
    let list = unsafe { &*list };
    let Some(outcome) = list.items.get(index) else {
        return false;
    };
    unsafe { *out_outcome = OpenPitAccountAdjustmentOutcome::from_outcome(outcome) };
    true
}

//--------------------------------------------------------------------------------------------------
// Callback-scoped result collectors filled by custom pre-trade policy callbacks.
//
// Each type is a callback-scoped, non-owning out-parameter the engine creates,
// passes to the callback, and drains after the callback returns. The callback
// must not store or use the pointer after it returns. Push helpers validate the
// pushed payload (asset parse, decimal parse) and report failures through
// `out_error`, mirroring the `openpit_pretrade_pre_trade_lock_push` idiom.
//--------------------------------------------------------------------------------------------------

/// Callback-scoped collector for the per-policy main-stage pre-trade result.
///
/// Holds the two result channels a policy may produce during the main-stage
/// check: lock prices and account adjustments. Neither channel carries a
/// `policy_group_id`; the engine assigns the policy's group when assembling the
/// final result.
pub struct OpenPitPretradePreTradeResult {
    pub(crate) lock_prices: Vec<Price>,
    pub(crate) account_adjustments: Vec<AccountOutcomeEntry>,
}

#[no_mangle]
/// Appends one lock price to the main-stage pre-trade result.
///
/// # Safety
///
/// If `result` is non-null it must be a valid, properly aligned pointer to an
/// `OpenPitPretradePreTradeResult` that is exclusively accessible for the
/// duration of this call.
///
/// Contract:
/// - `result` must be a valid non-null callback-scoped pointer;
/// - `price` is validated with the same domain rules as
///   `openpit_create_param_price`;
/// - no `policy_group_id` is accepted: the engine assigns the policy's group.
///
/// Success:
/// - returns `true`; the result now carries one extra lock price.
///
/// Error:
/// - returns `false` when `result` is null or `price` fails domain validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_pretrade_pre_trade_result_push_lock_price(
    result: *mut OpenPitPretradePreTradeResult,
    price: crate::param::OpenPitParamPrice,
    out_error: OpenPitOutError,
) -> bool {
    if result.is_null() {
        write_error(out_error, "pre-trade result pointer is null");
        return false;
    }
    let parsed = match price.to_param() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    unsafe { &mut *result }.lock_prices.push(parsed);
    true
}

#[no_mangle]
/// Appends one account-adjustment outcome to the main-stage pre-trade result.
///
/// # Safety
///
/// If `result` is non-null it must be a valid, properly aligned pointer to an
/// `OpenPitPretradePreTradeResult` that is exclusively accessible for the
/// duration of this call.
///
/// Contract:
/// - `result` must be a valid non-null callback-scoped pointer;
/// - `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
/// - no `policy_group_id` is accepted: the engine assigns the policy's group.
///
/// Success:
/// - returns `true`; the result now carries one extra account-adjustment entry.
///
/// Error:
/// - returns `false` when `result` is null or `entry` fails validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_pretrade_pre_trade_result_push_account_adjustment(
    result: *mut OpenPitPretradePreTradeResult,
    entry: OpenPitAccountOutcomeEntry,
    out_error: OpenPitOutError,
) -> bool {
    if result.is_null() {
        write_error(out_error, "pre-trade result pointer is null");
        return false;
    }
    let parsed = match entry.to_entry() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    unsafe { &mut *result }.account_adjustments.push(parsed);
    true
}

//--------------------------------------------------------------------------------------------------

/// Callback-scoped collector for post-trade account-adjustment outcomes.
///
/// Holds the group-tagged account-adjustment outcomes a policy produces after
/// an execution report. Each push carries the producing policy's `policy_group_id`.
pub struct OpenPitPostTradeAdjustmentList {
    pub(crate) items: Vec<AccountAdjustmentOutcome>,
}

#[no_mangle]
/// Appends one group-tagged account-adjustment outcome to the post-trade list.
///
/// # Safety
///
/// If `list` is non-null it must be a valid, properly aligned pointer to an
/// `OpenPitPostTradeAdjustmentList` that is exclusively accessible for the
/// duration of this call.
///
/// Contract:
/// - `list` must be a valid non-null callback-scoped pointer;
/// - `policy_group_id` tags the produced outcome;
/// - `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`.
///
/// Success:
/// - returns `true`; the list now carries one extra outcome.
///
/// Error:
/// - returns `false` when `list` is null or `entry` fails validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_pretrade_post_trade_adjustment_list_push(
    list: *mut OpenPitPostTradeAdjustmentList,
    policy_group_id: u16,
    entry: OpenPitAccountOutcomeEntry,
    out_error: OpenPitOutError,
) -> bool {
    if list.is_null() {
        write_error(out_error, "post-trade adjustment list pointer is null");
        return false;
    }
    let parsed = match entry.to_entry() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    unsafe { &mut *list }.items.push(AccountAdjustmentOutcome {
        policy_group_id: PolicyGroupId::new(policy_group_id),
        entry: parsed,
    });
    true
}

//--------------------------------------------------------------------------------------------------

/// Callback-scoped collector for account-adjustment outcome entries.
///
/// Holds the account-adjustment outcome entries a policy produces. No
/// `policy_group_id` is carried; the engine assigns the policy's group.
pub struct OpenPitAccountOutcomeEntryList {
    pub(crate) items: Vec<AccountOutcomeEntry>,
}

#[no_mangle]
/// Appends one account-outcome entry to the account-adjustment outcome list.
///
/// # Safety
///
/// If `list` is non-null it must be a valid, properly aligned pointer to an
/// `OpenPitAccountOutcomeEntryList` that is exclusively accessible for the
/// duration of this call.
///
/// Contract:
/// - `list` must be a valid non-null callback-scoped pointer;
/// - `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
/// - no `policy_group_id` is accepted: the engine assigns the policy's group.
///
/// Success:
/// - returns `true`; the list now carries one extra entry.
///
/// Error:
/// - returns `false` when `list` is null or `entry` fails validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_account_outcome_entry_list_push(
    list: *mut OpenPitAccountOutcomeEntryList,
    entry: OpenPitAccountOutcomeEntry,
    out_error: OpenPitOutError,
) -> bool {
    if list.is_null() {
        write_error(out_error, "account outcome entry list pointer is null");
        return false;
    }
    let parsed = match entry.to_entry() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    unsafe { &mut *list }.items.push(parsed);
    true
}

//--------------------------------------------------------------------------------------------------
// Null-tolerant destroy functions for the callback-scoped collectors.
//
// The engine owns these collectors over the callback lifetime and drains them
// internally, so callers normally never destroy one. The destroy functions are
// provided for symmetry and tooling; passing null is allowed and has no effect.
//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Releases a main-stage pre-trade result collector. Passing null is allowed.
///
/// # Safety
///
/// `result` must be either null or a pointer returned by this library, and must
/// be destroyed at most once.
pub unsafe extern "C" fn openpit_destroy_pretrade_pre_trade_result(
    result: *mut OpenPitPretradePreTradeResult,
) {
    if result.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(result)) };
}

#[no_mangle]
/// Releases a post-trade adjustment list collector. Passing null is allowed.
///
/// # Safety
///
/// `list` must be either null or a pointer returned by this library, and must
/// be destroyed at most once.
pub unsafe extern "C" fn openpit_destroy_post_trade_adjustment_list(
    list: *mut OpenPitPostTradeAdjustmentList,
) {
    if list.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(list)) };
}

#[no_mangle]
/// Releases an account-outcome entry list collector. Passing null is allowed.
///
/// # Safety
///
/// `list` must be either null or a pointer returned by this library, and must
/// be destroyed at most once.
pub unsafe extern "C" fn openpit_destroy_account_outcome_entry_list(
    list: *mut OpenPitAccountOutcomeEntryList,
) {
    if list.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(list)) };
}
