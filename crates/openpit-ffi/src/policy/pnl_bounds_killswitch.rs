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

#![allow(clippy::missing_safety_doc, clippy::not_unsafe_ptr_arg_deref)]

use openpit::param::{AccountId, Pnl};
use openpit::pretrade::policies::pnl_bounds_killswitch::PnlBoundsAccountAssetBarrierUpdate;
use openpit::pretrade::policies::{
    PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy,
    PnlBoundsKillSwitchPolicyError, PnlBoundsKillSwitchSettings,
};

use crate::engine::{write_configure_error, OpenPitConfigureError};
use crate::param::{OpenPitParamPnl, OpenPitParamPnlOptional};

use super::*;

/// Parses an optional P&L bound for a configure function, mapping any failure
/// to an [`OpenPitConfigureError`].
fn parse_configure_optional_pnl(
    bound: OpenPitParamPnlOptional,
    label: &str,
    index: usize,
    field: &str,
) -> Result<Option<Pnl>, OpenPitConfigureError> {
    if !bound.is_set {
        return Ok(None);
    }
    bound.value.to_param().map(Some).map_err(|e| {
        OpenPitConfigureError::validation(format!("{label}[{index}] {field} is invalid: {e}"))
    })
}

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

/// Runtime replacement for a per-(account, settlement-asset) P&L barrier.
///
/// Passed to `openpit_engine_configure_pnl_bounds_killswitch`. It intentionally
/// has no `initial_pnl`: runtime replacement preserves and evaluates the live
/// accumulated P&L.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate {
    /// Account this replacement barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Settlement asset whose live accumulated P&L is monitored.
    pub settlement_asset: OpenPitStringView,
    /// Optional replacement lower bound.
    pub lower_bound: OpenPitParamPnlOptional,
    /// Optional replacement upper bound.
    pub upper_bound: OpenPitParamPnlOptional,
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
        let settlement = match parse_asset_or_error(
            param.settlement_asset,
            "broker",
            index,
            "settlement_asset",
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
        let settlement = match parse_asset_or_error(
            param.settlement_asset,
            "account",
            index,
            "settlement_asset",
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

    let settings = match PnlBoundsKillSwitchSettings::new(barriers, account_barriers) {
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

    let builder_ref = unsafe { &mut *builder };
    let storage = match policy_storage(builder_ref) {
        Some(storage) => storage,
        None => {
            write_error(out_error, "engine builder is no longer available");
            return false;
        }
    };
    let policy = PnlBoundsKillSwitchPolicy::new(settings, storage)
        .with_policy_group_id(openpit::PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Retunes the built-in P&L bounds kill-switch policy registered under `name`.
///
/// This is a partial update (PATCH) at the axis level: each axis is replaced
/// wholesale only when its `has_*` flag is `true`, mirroring the
/// replace-shaped settings setters. Runtime account barriers use a dedicated
/// update DTO with no `initial_pnl`; accumulated P&L is preserved.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; it is interpreted as UTF-8. A built-in
///   policy added via
///   `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`
///   registers under its fixed name `"PnlBoundsKillSwitchPolicy"`, so pass
///   that string here.
/// - When `has_broker` is `true`, the broker axis is replaced by the
///   `broker_len` entries at `broker` (a length of zero clears it, subject to
///   the policy's "at least one barrier" rule).
/// - When `has_account` is `true`, the account+asset axis is replaced by the
///   `account_len` entries at `account`.
/// - Each `settlement_asset` view must be valid for the duration of the call.
/// - A `has_*` flag set to `false` leaves that axis untouched.
///
/// Success:
/// - returns `true`; the new barriers apply from the next check onward.
///
/// Error:
/// - returns `false`; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
/// - a null `engine` returns `false` and, when `out_error` is non-null, writes
///   a caller-owned `OpenPitConfigureError` (`Validation`) that must be released
///   with `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_pnl_bounds_killswitch(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    broker: *const OpenPitPretradePoliciesPnlBoundsBarrier,
    broker_len: usize,
    has_broker: bool,
    account: *const OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate,
    account_len: usize,
    has_account: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    if engine.is_null() {
        write_configure_error(
            out_error,
            OpenPitConfigureError::validation("engine is null".to_owned()),
        );
        return false;
    }
    let name = match unsafe { cstr_arg(name) } {
        Some(name) => name,
        None => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(
                    "policy name is null or invalid UTF-8".to_owned(),
                ),
            );
            return false;
        }
    };

    let broker_barriers: Vec<PnlBoundsBrokerBarrier> = if has_broker {
        let slice = match unsafe {
            try_slice_arg(
                broker,
                broker_len,
                "pnl_bounds broker",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation("pnl_bounds broker is null".to_owned()),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let settlement = match parse_configure_asset(
                entry.settlement_asset,
                "broker",
                index,
                "settlement_asset",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            let lower_bound = match parse_configure_optional_pnl(
                entry.lower_bound,
                "broker",
                index,
                "lower_bound",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            let upper_bound = match parse_configure_optional_pnl(
                entry.upper_bound,
                "broker",
                index,
                "upper_bound",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(PnlBoundsBrokerBarrier {
                settlement_asset: settlement,
                lower_bound,
                upper_bound,
            });
        }
        out
    } else {
        Vec::new()
    };

    let account_barriers: Vec<PnlBoundsAccountAssetBarrierUpdate> = if has_account {
        let slice = match unsafe {
            try_slice_arg(
                account,
                account_len,
                "pnl_bounds account",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation("pnl_bounds account is null".to_owned()),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let settlement = match parse_configure_asset(
                entry.settlement_asset,
                "account",
                index,
                "settlement_asset",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            let lower_bound = match parse_configure_optional_pnl(
                entry.lower_bound,
                "account",
                index,
                "lower_bound",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            let upper_bound = match parse_configure_optional_pnl(
                entry.upper_bound,
                "account",
                index,
                "upper_bound",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(PnlBoundsAccountAssetBarrierUpdate {
                barrier: PnlBoundsBrokerBarrier {
                    settlement_asset: settlement,
                    lower_bound,
                    upper_bound,
                },
                account_id: AccountId::from_u64(entry.account_id),
            });
        }
        out
    } else {
        Vec::new()
    };

    let result = unsafe { &*engine }.configurator().pnl_bounds_killswitch(
        &name,
        |settings| -> Result<(), PnlBoundsKillSwitchPolicyError> {
            if has_broker {
                settings.set_broker_barriers(broker_barriers.iter().cloned())?;
            }
            if has_account {
                settings.set_account_barriers(account_barriers.iter().cloned())?;
            }
            Ok(())
        },
    );
    match result {
        Ok(()) => true,
        Err(err) => {
            write_configure_error(out_error, OpenPitConfigureError::new(err));
            false
        }
    }
}

#[no_mangle]
/// Force-sets the live accumulated P&L for a `(account_id, settlement_asset)`
/// entry of the P&L bounds kill-switch policy registered under `name`.
///
/// This is an absolute assignment, deliberately distinct from
/// `openpit_engine_configure_pnl_bounds_killswitch`: that function retunes the
/// bounds and never touches accumulated P&L, whereas this overwrites the live
/// accumulator. The entry is created if it does not exist yet. The new value is
/// evaluated against the live bounds from the next check onward.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; it is interpreted as UTF-8. A built-in policy
///   added via
///   `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`
///   registers under its fixed name `"PnlBoundsKillSwitchPolicy"`, so pass
///   that string here.
/// - `settlement_asset` must be valid for the duration of the call.
/// - `pnl` is the absolute value the entry is set to.
///
/// Success:
/// - returns `true`; the new accumulated P&L applies from the next check
///   onward.
///
/// Error:
/// - returns `false`; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
/// - a null `engine` returns `false` and, when `out_error` is non-null, writes
///   a caller-owned `OpenPitConfigureError` (`Validation`) that must be released
///   with `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    account_id: OpenPitParamAccountId,
    settlement_asset: OpenPitStringView,
    pnl: OpenPitParamPnl,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    if engine.is_null() {
        write_configure_error(
            out_error,
            OpenPitConfigureError::validation("engine is null".to_owned()),
        );
        return false;
    }
    let name = match unsafe { cstr_arg(name) } {
        Some(name) => name,
        None => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(
                    "policy name is null or invalid UTF-8".to_owned(),
                ),
            );
            return false;
        }
    };
    let settlement = match parse_configure_asset(
        settlement_asset,
        "pnl_bounds settlement",
        0,
        "settlement_asset",
    ) {
        Ok(v) => v,
        Err(e) => {
            write_configure_error(out_error, e);
            return false;
        }
    };
    let pnl = match pnl.to_param() {
        Ok(v) => v,
        Err(e) => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(format!("pnl is invalid: {e}")),
            );
            return false;
        }
    };

    let result = unsafe { &*engine }.configurator().set_account_pnl(
        &name,
        AccountId::from_u64(account_id),
        settlement,
        pnl,
    );
    match result {
        Ok(()) => true,
        Err(err) => {
            write_configure_error(out_error, OpenPitConfigureError::new(err));
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

    #[test]
    fn configure_pnl_bounds_killswitch_rejects_null_and_invalid_utf8_names() {
        let broker = [OpenPitPretradePoliciesPnlBoundsBarrier {
            settlement_asset: OpenPitStringView::from_utf8("USD"),
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
        let invalid_utf8 = [0xff];
        let invalid_name = OpenPitStringView {
            ptr: invalid_utf8.as_ptr(),
            len: invalid_utf8.len(),
        };

        for name in [OpenPitStringView::default(), invalid_name] {
            let mut out_error = std::ptr::null_mut();
            let ok = unsafe {
                openpit_engine_configure_pnl_bounds_killswitch(
                    engine,
                    name,
                    std::ptr::null(),
                    0,
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    &mut out_error,
                )
            };
            assert!(!ok);
            assert!(!out_error.is_null());
            assert_eq!(
                crate::engine::openpit_configure_error_get_kind(out_error),
                crate::engine::OpenPitConfigureErrorKind::Validation
            );
            crate::engine::openpit_destroy_configure_error(out_error);
        }

        crate::engine::openpit_destroy_engine(engine);
    }

    // Force-setting the accumulated P&L past a configured bound makes the next
    // start-pre-trade reject the order, proving the override reaches the live
    // ledger the hot path reads.
    #[test]
    fn set_account_pnl_overrides_accumulated_pnl_and_blocks_next_order() {
        // Lower bound -100 USD across all accounts; account 7 (the order's
        // account) has no history, so it passes before the override.
        let broker = [OpenPitPretradePoliciesPnlBoundsBarrier {
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            lower_bound: pnl_optional(Some(pnl_param(-100, 0))),
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

        // Force account 7's USD P&L to -150, breaching the lower bound -100.
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(
                engine,
                OpenPitStringView::from_utf8("PnlBoundsKillSwitchPolicy"),
                7,
                OpenPitStringView::from_utf8("USD"),
                pnl_param(-150, 0),
                &mut out_error,
            )
        };
        assert!(ok, "set_account_pnl must succeed");
        assert!(out_error.is_null(), "success leaves out_error untouched");

        // The next order on account 7 now breaches the bound and is rejected.
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
            crate::engine::OpenPitPretradeStatus::Rejected,
            "order must be rejected after the P&L override"
        );
        crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
        crate::reject::openpit_pretrade_destroy_reject_list(out_rejects);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn set_account_pnl_null_engine_reports_validation_error() {
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(
                std::ptr::null_mut(),
                OpenPitStringView::from_utf8("PnlBoundsKillSwitchPolicy"),
                7,
                OpenPitStringView::from_utf8("USD"),
                pnl_param(0, 0),
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation
        );
        crate::engine::openpit_destroy_configure_error(out_error);
    }
}
