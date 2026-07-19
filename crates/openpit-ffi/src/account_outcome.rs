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

use crate::define_optional;
use crate::last_error::{write_error, OpenPitOutError};
use crate::param::{
    OpenPitParamAccountId, OpenPitParamPnl, OpenPitParamPositionSize, OpenPitParamPriceOptional,
};
use crate::reject::{OpenPitPretradeAccountBlock, OpenPitPretradeAccountBlockList};
use crate::OpenPitStringView;
use openpit::param::{Asset, Price};
use openpit::{
    AccountAdjustmentOutcome, AccountOutcomeEntry, AccountPnlOutcome, OutcomeAmount, PnlHaltReason,
    PnlOutcome, PnlOutcomeAmount, PnlState, PolicyGroupId,
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

/// Raw reason code for a realized-PnL calculation halt.
///
/// This is a primitive rather than a Rust enum so callers can pass arbitrary
/// bytes without creating an invalid Rust enum discriminant at the FFI
/// boundary. Inbound values are validated before conversion to
/// `OpenPitPnlHaltReason` values.
pub type OpenPitPnlHaltReason = u8;

/// The realized-PnL amount is available.
pub const OPENPIT_PNL_HALT_REASON_NONE: OpenPitPnlHaltReason = 0;
/// A required FX quote was unavailable.
pub const OPENPIT_PNL_HALT_REASON_MISSING_FX: OpenPitPnlHaltReason = 1;
/// The current account currency was unavailable.
pub const OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY: OpenPitPnlHaltReason = 2;
/// The initial realized PnL needed to continue the ledger was unavailable.
pub const OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL: OpenPitPnlHaltReason = 3;
/// The position cost basis needed to calculate realized PnL was unavailable.
pub const OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS: OpenPitPnlHaltReason = 4;
/// Exact realized-PnL arithmetic overflowed.
pub const OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW: OpenPitPnlHaltReason = 5;

/// Realized-PnL result: either the amount or a halt reason.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitPnlOutcome {
    /// Reason why the PnL amount is unavailable.
    pub halt_reason: OpenPitPnlHaltReason,
    /// Optional computed PnL change and resulting absolute value. It is set
    /// only when `halt_reason` is `OPENPIT_PNL_HALT_REASON_NONE`.
    pub amount: OpenPitPnlOutcomeAmountOptional,
}

/// Raw discriminator for [`OpenPitPnlState`].
pub type OpenPitPnlStateKind = u8;

/// `OpenPitPnlState::value` contains the authoritative accumulated PnL.
pub const OPENPIT_PNL_STATE_VALUE: OpenPitPnlStateKind = 0;
/// `OpenPitPnlState::halt_reason` contains the reason calculation stopped.
pub const OPENPIT_PNL_STATE_HALTED: OpenPitPnlStateKind = 1;

/// Explicit realized-PnL accumulator state.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitPnlState {
    /// Selects the meaningful payload.
    pub kind: OpenPitPnlStateKind,
    /// Authoritative value when `kind` is `OPENPIT_PNL_STATE_VALUE`.
    pub value: OpenPitParamPnl,
    /// Halt reason when `kind` is `OPENPIT_PNL_STATE_HALTED`.
    pub halt_reason: OpenPitPnlHaltReason,
}

define_optional!(
    optional = OpenPitPnlOutcomeOptional,
    value = OpenPitPnlOutcome
);

/// Account-level realized-PnL result for one account.
///
/// When `halt_reason` is `OPENPIT_PNL_HALT_REASON_NONE`, `amount` is
/// authoritative. Otherwise
/// `halt_reason` explains why `amount` is not authoritative; do not interpret
/// it as zero or read any stored PnL value as current. Position accumulators
/// are independent. SpotFunds emits a halted account outcome only for the
/// operation that transitions the accumulator to halted; later operations
/// omit the unchanged halt.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitAccountPnlOutcome {
    /// Account whose PnL was considered.
    pub account_id: OpenPitParamAccountId,
    /// Policy-group tag of the policy that produced this outcome.
    pub policy_group_id: u16,
    /// Reason why the account-level PnL amount is unavailable.
    pub halt_reason: OpenPitPnlHaltReason,
    /// Optional computed PnL change and resulting absolute value. It is set
    /// only when `halt_reason` is `OPENPIT_PNL_HALT_REASON_NONE`.
    pub amount: OpenPitPnlOutcomeAmountOptional,
}

impl OpenPitAccountPnlOutcome {
    fn from_outcome(inner: &AccountPnlOutcome) -> Self {
        let pnl = export_pnl_outcome(&inner.result);
        Self {
            account_id: inner.account_id.as_u64(),
            policy_group_id: inner.policy_group_id.value(),
            halt_reason: pnl.halt_reason,
            amount: pnl.amount,
        }
    }

    pub(crate) fn to_outcome(self) -> Result<AccountPnlOutcome, String> {
        Ok(AccountPnlOutcome {
            result: import_pnl_outcome(&OpenPitPnlOutcome {
                halt_reason: self.halt_reason,
                amount: self.amount,
            })?,
            account_id: openpit::param::AccountId::from_u64(self.account_id),
            policy_group_id: PolicyGroupId::new(self.policy_group_id),
        })
    }
}

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
    /// Optional position realized-PnL result in the account currency.
    /// It is set to either an amount or the halt reason from the operation that
    /// first failed. Later operations omit it until an asset-scoped balance
    /// adjustment force-sets a new realized PnL. Position and account PnL halt
    /// independently; this field never drives the account kill switch.
    pub realized_pnl: OpenPitPnlOutcomeOptional,
    /// Current account-currency average entry price (absolute) for the
    /// `(account, asset)` holdings slot. The underlying asset identifies one
    /// slot even when it is traded against multiple quote currencies. Unset
    /// means the average was not tracked or not emitted.
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

pub(crate) fn export_pnl_halt_reason(value: PnlHaltReason) -> OpenPitPnlHaltReason {
    match value {
        PnlHaltReason::MissingFx => OPENPIT_PNL_HALT_REASON_MISSING_FX,
        PnlHaltReason::MissingAccountCurrency => OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY,
        PnlHaltReason::MissingInitialPnl => OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL,
        PnlHaltReason::MissingCostBasis => OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS,
        PnlHaltReason::ArithmeticOverflow => OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW,
    }
}

pub(crate) fn export_pnl_state(value: PnlState) -> OpenPitPnlState {
    match value {
        PnlState::Value(value) => OpenPitPnlState {
            kind: OPENPIT_PNL_STATE_VALUE,
            value: OpenPitParamPnl(value.to_decimal().into()),
            halt_reason: OPENPIT_PNL_HALT_REASON_NONE,
        },
        PnlState::Halted(reason) => OpenPitPnlState {
            kind: OPENPIT_PNL_STATE_HALTED,
            value: OpenPitParamPnl::default(),
            halt_reason: export_pnl_halt_reason(reason),
        },
    }
}

fn export_pnl_outcome(value: &PnlOutcome) -> OpenPitPnlOutcome {
    match value {
        Ok(amount) => OpenPitPnlOutcome {
            halt_reason: OPENPIT_PNL_HALT_REASON_NONE,
            amount: OpenPitPnlOutcomeAmountOptional {
                value: export_pnl_outcome_amount(amount),
                is_set: true,
            },
        },
        Err(reason) => OpenPitPnlOutcome {
            halt_reason: export_pnl_halt_reason(*reason),
            amount: OpenPitPnlOutcomeAmountOptional::default(),
        },
    }
}

fn export_pnl_outcome_optional(value: Option<&PnlOutcome>) -> OpenPitPnlOutcomeOptional {
    match value {
        Some(outcome) => OpenPitPnlOutcomeOptional {
            value: export_pnl_outcome(outcome),
            is_set: true,
        },
        None => OpenPitPnlOutcomeOptional::default(),
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
        realized_pnl: export_pnl_outcome_optional(value.realized_pnl.as_ref()),
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

fn import_pnl_halt_reason(value: OpenPitPnlHaltReason) -> Result<PnlHaltReason, String> {
    match value {
        OPENPIT_PNL_HALT_REASON_MISSING_FX => Ok(PnlHaltReason::MissingFx),
        OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY => {
            Ok(PnlHaltReason::MissingAccountCurrency)
        }
        OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL => Ok(PnlHaltReason::MissingInitialPnl),
        OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS => Ok(PnlHaltReason::MissingCostBasis),
        OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW => Ok(PnlHaltReason::ArithmeticOverflow),
        raw => Err(format!("invalid PnL halt reason code {raw}")),
    }
}

fn import_pnl_outcome(value: &OpenPitPnlOutcome) -> Result<PnlOutcome, String> {
    match value.halt_reason {
        OPENPIT_PNL_HALT_REASON_NONE if value.amount.is_set => {
            Ok(Ok(import_pnl_outcome_amount(&value.amount.value)?))
        }
        OPENPIT_PNL_HALT_REASON_NONE => Err("pnl outcome amount is required".to_owned()),
        raw if value.amount.is_set => Err(format!(
            "halted pnl outcome with reason {raw} must not contain an amount"
        )),
        raw => import_pnl_halt_reason(raw).map(Err),
    }
}

pub(crate) fn import_pnl_state(value: OpenPitPnlState) -> Result<PnlState, String> {
    match value.kind {
        OPENPIT_PNL_STATE_VALUE if value.halt_reason == OPENPIT_PNL_HALT_REASON_NONE => {
            Ok(PnlState::Value(value.value.to_param()?))
        }
        OPENPIT_PNL_STATE_VALUE => {
            Err("value PnL state must use OPENPIT_PNL_HALT_REASON_NONE".to_owned())
        }
        OPENPIT_PNL_STATE_HALTED if value.halt_reason == OPENPIT_PNL_HALT_REASON_NONE => {
            Err("halted PnL state requires a halt reason".to_owned())
        }
        OPENPIT_PNL_STATE_HALTED => import_pnl_halt_reason(value.halt_reason).map(PnlState::Halted),
        raw => Err(format!("invalid PnL state kind {raw}")),
    }
}

fn import_pnl_outcome_optional(
    value: &OpenPitPnlOutcomeOptional,
) -> Result<Option<PnlOutcome>, String> {
    if !value.is_set {
        return Ok(None);
    }
    Ok(Some(import_pnl_outcome(&value.value)?))
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
            realized_pnl: import_pnl_outcome_optional(&self.realized_pnl)?,
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
/// `list` must be returned by this library. `out_outcome` must be valid and
/// writable for the duration of this call.
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

/// Borrowed list of account-level PnL outcomes owned by a post-trade result.
pub struct OpenPitAccountPnlOutcomeList {
    pub(crate) items: Vec<AccountPnlOutcome>,
}

pub(crate) fn account_pnls_to_list(values: Vec<AccountPnlOutcome>) -> OpenPitAccountPnlOutcomeList {
    OpenPitAccountPnlOutcomeList { items: values }
}

#[no_mangle]
/// Returns the number of account-level PnL outcomes in the list.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
///
/// # Safety
///
/// `list` must be borrowed from a live `OpenPitPostTradeResult` and
/// remain alive for the duration of this call.
pub unsafe extern "C" fn openpit_account_pnl_outcome_list_len(
    list: *const OpenPitAccountPnlOutcomeList,
) -> usize {
    assert!(!list.is_null(), "account PnL outcome list pointer is null");
    let list = unsafe { &*list };
    list.items.len()
}

#[no_mangle]
/// Copies the self-contained account-level PnL outcome at `index` into
/// `out_outcome`.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - `out_outcome` must be a valid non-null pointer;
/// - returns `true` when a value exists and was copied;
/// - returns `false` when `index` is out of bounds and does not write
///   `out_outcome`;
/// - the copied value remains valid independently of the owning result;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
///
/// # Safety
///
/// `list` must be borrowed from a live `OpenPitPostTradeResult`.
/// `out_outcome` must be valid and writable for the duration of this call.
pub unsafe extern "C" fn openpit_account_pnl_outcome_list_get(
    list: *const OpenPitAccountPnlOutcomeList,
    index: usize,
    out_outcome: *mut OpenPitAccountPnlOutcome,
) -> bool {
    assert!(!list.is_null(), "account PnL outcome list pointer is null");
    assert!(
        !out_outcome.is_null(),
        "account PnL outcome output pointer is null"
    );
    let list = unsafe { &*list };
    let Some(outcome) = list.items.get(index) else {
        return false;
    };
    unsafe { *out_outcome = OpenPitAccountPnlOutcome::from_outcome(outcome) };
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

/// Callback-scoped collector for post-trade account-level PnL outcomes.
///
/// Holds the group-tagged account-level PnL outcomes a policy produces after
/// an execution report.
pub struct OpenPitPostTradeAccountPnlList {
    pub(crate) items: Vec<AccountPnlOutcome>,
}

#[no_mangle]
/// Appends one group-tagged account-level PnL outcome to the post-trade list.
///
/// # Safety
///
/// If `list` is non-null it must be a valid, properly aligned pointer to an
/// `OpenPitPostTradeAccountPnlList` that is exclusively accessible for the
/// duration of this call.
///
/// Contract:
/// - `list` must be a valid non-null callback-scoped pointer;
/// - `outcome` carries the producing policy's `policy_group_id` and is fully
///   validated before it is appended.
///
/// Success:
/// - returns `true`; the list now carries one extra outcome.
///
/// Error:
/// - returns `false` when `list` is null or `outcome` fails validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_pretrade_post_trade_account_pnl_list_push(
    list: *mut OpenPitPostTradeAccountPnlList,
    outcome: OpenPitAccountPnlOutcome,
    out_error: OpenPitOutError,
) -> bool {
    if list.is_null() {
        write_error(out_error, "post-trade account PnL list pointer is null");
        return false;
    }
    let parsed = match outcome.to_outcome() {
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

/// Stored account-adjustment outcome entries within a callback result.
///
/// Holds the account-adjustment outcome entries a policy produces. No
/// `policy_group_id` is carried; the engine assigns the policy's group.
pub(crate) struct OpenPitAccountOutcomeEntryList {
    pub(crate) items: Vec<AccountOutcomeEntry>,
}

/// Callback-scoped collector for one account-adjustment policy result.
///
/// Holds the outcome entries and account blocks produced by the callback. The
/// engine keeps both channels only when the callback accepts the adjustment.
pub struct OpenPitPretradeAccountAdjustmentResult {
    pub(crate) account_outcomes: OpenPitAccountOutcomeEntryList,
    pub(crate) account_blocks: OpenPitPretradeAccountBlockList,
}

impl Default for OpenPitPretradeAccountAdjustmentResult {
    fn default() -> Self {
        Self {
            account_outcomes: OpenPitAccountOutcomeEntryList { items: Vec::new() },
            account_blocks: OpenPitPretradeAccountBlockList { items: Vec::new() },
        }
    }
}

#[no_mangle]
/// Appends one account-outcome entry to an account-adjustment policy result.
///
/// # Safety
///
/// If `result` is non-null it must be a valid, properly aligned pointer to an
/// `OpenPitPretradeAccountAdjustmentResult` that is exclusively accessible for
/// the duration of this call.
///
/// Contract:
/// - `result` must be a valid non-null callback-scoped pointer;
/// - `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
/// - no `policy_group_id` is accepted: the engine assigns the policy's group.
///
/// Success:
/// - returns `true`; the result now carries one extra outcome entry.
///
/// Error:
/// - returns `false` when `result` is null or `entry` fails validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_pretrade_account_adjustment_result_push_account_outcome(
    result: *mut OpenPitPretradeAccountAdjustmentResult,
    entry: OpenPitAccountOutcomeEntry,
    out_error: OpenPitOutError,
) -> bool {
    if result.is_null() {
        write_error(out_error, "account-adjustment result pointer is null");
        return false;
    }
    let parsed = match entry.to_entry() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    unsafe { &mut *result }.account_outcomes.items.push(parsed);
    true
}

#[no_mangle]
/// Appends one account block to an account-adjustment policy result.
///
/// # Safety
///
/// If `result` is non-null it must be a valid, properly aligned pointer to an
/// `OpenPitPretradeAccountAdjustmentResult` that is exclusively accessible for
/// the duration of this call.
///
/// Contract:
/// - `result` must be a valid non-null callback-scoped pointer;
/// - string views in `block` are copied before this function returns;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub unsafe extern "C" fn openpit_pretrade_account_adjustment_result_push_account_block(
    result: *mut OpenPitPretradeAccountAdjustmentResult,
    block: OpenPitPretradeAccountBlock,
) {
    assert!(
        !result.is_null(),
        "account-adjustment result pointer is null"
    );
    unsafe { &mut *result }
        .account_blocks
        .items
        .push(block.to_block());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pnl_halt_reason_mapping_accepts_every_defined_code() {
        let cases = [
            (OPENPIT_PNL_HALT_REASON_MISSING_FX, PnlHaltReason::MissingFx),
            (
                OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY,
                PnlHaltReason::MissingAccountCurrency,
            ),
            (
                OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL,
                PnlHaltReason::MissingInitialPnl,
            ),
            (
                OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS,
                PnlHaltReason::MissingCostBasis,
            ),
            (
                OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW,
                PnlHaltReason::ArithmeticOverflow,
            ),
        ];

        for (raw, expected) in cases {
            let imported = import_pnl_outcome(&OpenPitPnlOutcome {
                halt_reason: raw,
                amount: OpenPitPnlOutcomeAmountOptional::default(),
            });
            assert_eq!(imported, Ok(Err(expected)));
        }
    }

    #[test]
    fn pnl_halt_reason_mapping_rejects_invalid_code() {
        let imported = import_pnl_outcome(&OpenPitPnlOutcome {
            halt_reason: u8::MAX,
            amount: OpenPitPnlOutcomeAmountOptional::default(),
        });
        assert_eq!(imported, Err("invalid PnL halt reason code 255".to_owned()));
    }

    #[test]
    fn pnl_state_value_round_trips_through_export_and_import() {
        let value = PnlState::Value(openpit::param::Pnl::from_str("12.5").expect("valid"));

        let exported = export_pnl_state(value);
        assert_eq!(exported.kind, OPENPIT_PNL_STATE_VALUE);
        assert_eq!(exported.halt_reason, OPENPIT_PNL_HALT_REASON_NONE);
        assert_eq!(import_pnl_state(exported), Ok(value));
    }

    #[test]
    fn pnl_state_halt_reason_mapping_uses_the_same_validated_decoder() {
        let imported = import_pnl_state(OpenPitPnlState {
            kind: OPENPIT_PNL_STATE_HALTED,
            halt_reason: OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS,
            ..OpenPitPnlState::default()
        });
        assert_eq!(
            imported,
            Ok(PnlState::Halted(PnlHaltReason::MissingCostBasis))
        );

        let invalid = import_pnl_state(OpenPitPnlState {
            kind: OPENPIT_PNL_STATE_HALTED,
            halt_reason: u8::MAX,
            ..OpenPitPnlState::default()
        });
        assert_eq!(invalid, Err("invalid PnL halt reason code 255".to_owned()));
    }

    #[test]
    fn post_trade_account_pnl_collector_accepts_valid_outcome() {
        let mut list = OpenPitPostTradeAccountPnlList { items: Vec::new() };

        assert!(unsafe {
            openpit_pretrade_post_trade_account_pnl_list_push(
                &mut list,
                OpenPitAccountPnlOutcome {
                    account_id: 42,
                    policy_group_id: 7,
                    halt_reason: OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY,
                    ..OpenPitAccountPnlOutcome::default()
                },
                std::ptr::null_mut(),
            )
        });

        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].account_id.as_u64(), 42);
        assert_eq!(list.items[0].policy_group_id, PolicyGroupId::new(7));
        assert_eq!(
            list.items[0].result,
            Err(PnlHaltReason::MissingAccountCurrency)
        );
    }

    #[test]
    fn post_trade_account_pnl_collector_reports_null_handle() {
        let mut error = std::ptr::null_mut();

        assert!(!unsafe {
            openpit_pretrade_post_trade_account_pnl_list_push(
                std::ptr::null_mut(),
                OpenPitAccountPnlOutcome::default(),
                &mut error,
            )
        });
        assert!(!error.is_null());
        crate::string::openpit_destroy_shared_string(error);
    }

    #[test]
    fn account_adjustment_result_collects_outcomes_and_blocks() {
        let mut result = OpenPitPretradeAccountAdjustmentResult::default();

        assert!(unsafe {
            openpit_pretrade_account_adjustment_result_push_account_outcome(
                &mut result,
                OpenPitAccountOutcomeEntry {
                    asset: OpenPitStringView::from_utf8("USD"),
                    ..OpenPitAccountOutcomeEntry::default()
                },
                std::ptr::null_mut(),
            )
        });
        unsafe {
            openpit_pretrade_account_adjustment_result_push_account_block(
                &mut result,
                OpenPitPretradeAccountBlock {
                    policy: OpenPitStringView::from_utf8("custom.policy"),
                    reason: OpenPitStringView::from_utf8("blocked"),
                    details: OpenPitStringView::from_utf8("test"),
                    user_data: std::ptr::null_mut(),
                    code: crate::reject::OPENPIT_PRETRADE_REJECT_CODE_OTHER,
                },
            );
        }

        assert_eq!(result.account_outcomes.items.len(), 1);
        assert_eq!(result.account_blocks.items.len(), 1);
    }
}
