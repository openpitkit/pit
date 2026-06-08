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

use openpit::param::AccountId;
use openpit::pretrade::policies::{
    PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy,
};

use crate::param::{OpenPitParamPnl, OpenPitParamPnlOptional};

use super::*;

/// One broker barrier definition for
/// `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`.
///
/// What it describes:
/// - A settlement asset and its lower/upper P&L bounds applied as a broker
///   barrier across all accounts.
///
/// Contract:
/// - `settlement_asset` must point to a valid string for the duration of the
///   call.
/// - The array passed to the add function may contain multiple entries.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesPnlBoundsBarrier {
    /// Settlement asset whose accumulated P&L is being monitored.
    pub settlement_asset: OpenPitStringView,
    /// Optional lower bound for accumulated P&L.
    pub lower_bound: OpenPitParamPnlOptional,
    /// Optional upper bound for accumulated P&L.
    pub upper_bound: OpenPitParamPnlOptional,
}

/// Per-(account, settlement-asset) P&L bounds barrier with an initial P&L seed.
///
/// What it describes:
/// - Refines P&L bounds for a specific account and settlement asset.
/// - `initial_pnl` is pre-loaded into storage at construction; accumulation
///   starts from this value.
/// - Both the broker barrier (if any) and this account+asset barrier are
///   evaluated on every check; the order passes only if neither is breached.
///
/// Passed to `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy` in
/// the `account` array.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesPnlBoundsAccountBarrier {
    /// Account this barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Settlement asset whose accumulated P&L is being monitored.
    pub settlement_asset: OpenPitStringView,
    /// Optional lower bound for accumulated P&L for this account+asset pair.
    pub lower_bound: OpenPitParamPnlOptional,
    /// Optional upper bound for accumulated P&L for this account+asset pair.
    pub upper_bound: OpenPitParamPnlOptional,
    /// Starting accumulated P&L pre-loaded into storage at construction.
    pub initial_pnl: OpenPitParamPnl,
}

#[no_mangle]
/// Adds the built-in P&L bounds kill-switch policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy_group_id` assigns the policy to a policy group (pass `0` for default).
/// - At least one barrier must be provided: `broker_len > 0` or
///   `account_len > 0`.
/// - When a length is greater than zero the corresponding pointer must point
///   to that many readable entries.
/// - Each `settlement_asset` string view inside an array entry must be valid
///   for the duration of the call.
///
/// Success:
/// - returns `true`; the builder retains the policy.
///
/// Error:
/// - returns `false` when the builder is null or already consumed, when no
///   barrier is configured, or when argument parsing fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    policy_group_id: u16,
    broker: *const OpenPitPretradePoliciesPnlBoundsBarrier,
    broker_len: usize,
    account: *const OpenPitPretradePoliciesPnlBoundsAccountBarrier,
    account_len: usize,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    let broker_slice = match unsafe {
        try_slice_arg(
            broker,
            broker_len,
            "pnl_bounds_killswitch_policy broker",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };
    let mut barriers = Vec::with_capacity(broker_slice.len());
    for (index, param) in broker_slice.iter().enumerate() {
        let settlement = match parse_settlement_asset_or_error(
            param.settlement_asset,
            "broker",
            index,
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        let lower_bound = match parse_optional_pnl_or_error(
            param.lower_bound,
            "broker",
            index,
            "lower_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let upper_bound = match parse_optional_pnl_or_error(
            param.upper_bound,
            "broker",
            index,
            "upper_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        barriers.push(PnlBoundsBrokerBarrier {
            settlement_asset: settlement,
            lower_bound,
            upper_bound,
        });
    }

    let account_slice = match unsafe {
        try_slice_arg(
            account,
            account_len,
            "pnl_bounds_killswitch_policy account",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };
    let mut account_barriers = Vec::with_capacity(account_slice.len());
    for (index, param) in account_slice.iter().enumerate() {
        let account_id = AccountId::from_u64(param.account_id);
        let settlement = match parse_settlement_asset_or_error(
            param.settlement_asset,
            "account",
            index,
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        let lower_bound = match parse_optional_pnl_or_error(
            param.lower_bound,
            "account",
            index,
            "lower_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let upper_bound = match parse_optional_pnl_or_error(
            param.upper_bound,
            "account",
            index,
            "upper_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let initial_pnl = match param.initial_pnl.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "account[{index}] initial_pnl is invalid: {}", e);
                return false;
            }
        };
        account_barriers.push(PnlBoundsAccountAssetBarrier {
            barrier: PnlBoundsBrokerBarrier {
                settlement_asset: settlement,
                lower_bound,
                upper_bound,
            },
            account_id,
            initial_pnl,
        });
    }

    let builder_ref = unsafe { &mut *builder };
    let storage = match policy_storage(builder_ref) {
        Some(storage) => storage,
        None => {
            write_error(out_error, "engine builder is no longer available");
            return false;
        }
    };
    let policy = match PnlBoundsKillSwitchPolicy::new(barriers, account_barriers, storage) {
        Ok(v) => v,
        Err(e) => {
            write_error_format!(
                out_error,
                "pnl_bounds_killswitch_policy creation failed: {}",
                e
            );
            return false;
        }
    };
    let policy = policy.with_policy_group_id(openpit::PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::order::OpenPitOrder;
    use crate::param::{OpenPitParamDecimal, OpenPitParamPnl, OpenPitParamQuantity};

    fn cstr_to_string(handle: *mut crate::string::OpenPitSharedString) -> String {
        if handle.is_null() {
            return String::new();
        }
        let view = crate::string::openpit_shared_string_view(handle);
        let result = if view.ptr.is_null() {
            String::new()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
            std::str::from_utf8(bytes).expect("utf8").to_string()
        };
        crate::string::openpit_destroy_shared_string(handle);
        result
    }

    fn pnl_param(mantissa: i128, scale: i32) -> OpenPitParamPnl {
        OpenPitParamPnl(OpenPitParamDecimal {
            mantissa_lo: mantissa as i64,
            mantissa_hi: (mantissa >> 64) as i64,
            scale,
        })
    }

    fn pnl_optional(value: Option<OpenPitParamPnl>) -> OpenPitParamPnlOptional {
        match value {
            Some(v) => OpenPitParamPnlOptional {
                is_set: true,
                value: v,
            },
            None => OpenPitParamPnlOptional::default(),
        }
    }

    fn quantity_param(mantissa: i128, scale: i32) -> OpenPitParamQuantity {
        OpenPitParamQuantity(OpenPitParamDecimal {
            mantissa_lo: mantissa as i64,
            mantissa_hi: (mantissa >> 64) as i64,
            scale,
        })
    }

    fn build_engine_with_builtin_start_policy(
        add_fn: impl FnOnce(*mut crate::engine::OpenPitEngineBuilder) -> bool,
    ) -> *mut crate::engine::OpenPitEngine {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        assert!(add_fn(builder), "failed to add policy");
        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn valid_pit_order() -> OpenPitOrder {
        use crate::instrument::OpenPitInstrument;
        use crate::order::{OpenPitOrderOperation, OpenPitOrderOperationOptional};
        use crate::param::{
            OpenPitParamAccountIdOptional, OpenPitParamPrice, OpenPitParamPriceOptional,
            OpenPitParamSide, OpenPitParamTradeAmount, OpenPitParamTradeAmountKind,
        };
        OpenPitOrder {
            operation: OpenPitOrderOperationOptional {
                is_set: true,
                value: OpenPitOrderOperation {
                    instrument: OpenPitInstrument {
                        underlying_asset: OpenPitStringView::from_utf8("SPX"),
                        settlement_asset: OpenPitStringView::from_utf8("USD"),
                    },
                    trade_amount: OpenPitParamTradeAmount {
                        value: quantity_param(1, 0).0,
                        kind: OpenPitParamTradeAmountKind::Quantity,
                    },
                    account_id: OpenPitParamAccountIdOptional {
                        value: 7,
                        is_set: true,
                    },
                    side: OpenPitParamSide::Buy,
                    price: OpenPitParamPriceOptional {
                        is_set: true,
                        value: OpenPitParamPrice(OpenPitParamDecimal {
                            mantissa_lo: 100,
                            mantissa_hi: 0,
                            scale: 0,
                        }),
                    },
                },
            },
            position: Default::default(),
            margin: Default::default(),
            user_data: std::ptr::null_mut(),
        }
    }

    fn run_start_pre_trade_passes(engine: *mut crate::engine::OpenPitEngine) {
        let order = valid_pit_order();
        let mut request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(
            status,
            crate::engine::OpenPitPretradeStatus::Passed,
            "start_pre_trade should pass"
        );
        crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
    }

    #[test]
    fn add_builtin_pnl_bounds_killswitch_policy_happy_path() {
        let usd = OpenPitStringView::from_utf8("USD");
        let broker = [OpenPitPretradePoliciesPnlBoundsBarrier {
            settlement_asset: usd,
            lower_bound: pnl_optional(Some(pnl_param(-10000, 0))),
            upper_bound: pnl_optional(None),
        }];
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
                0,
                broker.as_ptr(),
                broker.len(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_pnl_bounds_killswitch_policy_empty_config_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                &mut out_error,
            )
        };
        assert!(!ok);
        let message = cstr_to_string(out_error);
        assert!(
            message.contains("pnl_bounds_killswitch_policy creation failed")
                && message.contains("must be configured"),
            "expected SDK no-barrier error wrapped by FFI, got: {message}"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_builtin_pnl_bounds_killswitch_null_broker_with_positive_len_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
                0,
                std::ptr::null(),
                1,
                std::ptr::null(),
                0,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert_eq!(
            cstr_to_string(out_error),
            "pnl_bounds_killswitch_policy broker is null"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_builtin_pnl_bounds_killswitch_null_account_with_positive_len_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                1,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert_eq!(
            cstr_to_string(out_error),
            "pnl_bounds_killswitch_policy account is null"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }
}
