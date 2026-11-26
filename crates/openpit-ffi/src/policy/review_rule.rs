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

use openpit::param::{AccountGroupId, AccountId};
use openpit::pretrade::policies::{
    ReviewRuleAccountGroupRule, ReviewRuleAccountRule, ReviewRuleInstruments, ReviewRulePolicy,
    ReviewRulePolicyError, ReviewRuleSettings,
};

use super::{
    cstr_arg, try_slice_arg, write_error, FfiStorageFactory, OpenPitOutError, OpenPitStringView,
};
use crate::engine::{write_configure_error, OpenPitConfigureError};
use crate::instrument::{import_required_instrument, OpenPitInstrument};
use crate::write_error_format;

/// Global review rule. The verdict comes from the registered policy instance.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OpenPitPretradePoliciesReviewRuleGlobalRule {
    /// Exact instrument pairs, borrowed for the call; ignored for all
    /// instruments.
    pub instruments: *const OpenPitInstrument,
    /// Number of exact instruments; must be nonzero when `has_instruments` is
    /// true.
    pub instruments_len: usize,
    /// False covers all instruments; true selects a nonempty, distinct exact
    /// list.
    pub has_instruments: bool,
}

/// Review rule for one account group. Group keys must be distinct in a level.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OpenPitPretradePoliciesReviewRuleAccountGroupRule {
    /// Account group covered by this rule.
    pub account_group_id: crate::account_group_id::OpenPitParamAccountGroupId,
    /// Exact instrument pairs, borrowed for the call; ignored for all
    /// instruments.
    pub instruments: *const OpenPitInstrument,
    /// Number of exact instruments; must be nonzero when `has_instruments` is
    /// true.
    pub instruments_len: usize,
    /// False covers all instruments; true selects a nonempty, distinct exact
    /// list.
    pub has_instruments: bool,
}

/// Review rule for one account. Account keys must be distinct in a level.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OpenPitPretradePoliciesReviewRuleAccountRule {
    /// Account covered by this rule.
    pub account_id: crate::param::OpenPitParamAccountId,
    /// Exact instrument pairs, borrowed for the call; ignored for all
    /// instruments.
    pub instruments: *const OpenPitInstrument,
    /// Number of exact instruments; must be nonzero when `has_instruments` is
    /// true.
    pub instruments_len: usize,
    /// False covers all instruments; true selects a nonempty, distinct exact
    /// list.
    pub has_instruments: bool,
}

unsafe fn parse_instruments(
    instruments: *const OpenPitInstrument,
    len: usize,
    has_instruments: bool,
    location: std::fmt::Arguments<'_>,
) -> Result<ReviewRuleInstruments, String> {
    if !has_instruments {
        return Ok(ReviewRuleInstruments::All);
    }
    let entries = unsafe {
        try_slice_arg(
            instruments,
            len,
            "review_rule instruments",
            std::ptr::null_mut(),
        )
    }
    .ok_or_else(|| format!("{location} instruments is null"))?;
    let instruments = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            import_required_instrument(entry)
                .map_err(|err| format!("{location} instruments[{index}] {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ReviewRuleInstruments::Exact(instruments))
}

struct ParsedRules {
    global: Option<ReviewRuleInstruments>,
    groups: Vec<ReviewRuleAccountGroupRule>,
    accounts: Vec<ReviewRuleAccountRule>,
}

unsafe fn parse_rules(
    global: *const OpenPitPretradePoliciesReviewRuleGlobalRule,
    groups: *const OpenPitPretradePoliciesReviewRuleAccountGroupRule,
    groups_len: usize,
    accounts: *const OpenPitPretradePoliciesReviewRuleAccountRule,
    accounts_len: usize,
) -> Result<ParsedRules, String> {
    let global = if global.is_null() {
        None
    } else {
        let rule = unsafe { &*global };
        Some(unsafe {
            parse_instruments(
                rule.instruments,
                rule.instruments_len,
                rule.has_instruments,
                format_args!("global_rule"),
            )
        }?)
    };
    let group_entries = unsafe {
        try_slice_arg(
            groups,
            groups_len,
            "review_rule account groups",
            std::ptr::null_mut(),
        )
    }
    .ok_or_else(|| "review_rule account groups is null".to_owned())?;
    let mut groups = Vec::with_capacity(group_entries.len());
    for (index, rule) in group_entries.iter().enumerate() {
        groups.push(ReviewRuleAccountGroupRule {
            account_group_id: AccountGroupId::from_u32(rule.account_group_id)
                .map_err(|err| format!("account_group_rules[{index}] {err}"))?,
            instruments: unsafe {
                parse_instruments(
                    rule.instruments,
                    rule.instruments_len,
                    rule.has_instruments,
                    format_args!("account_group_rules[{index}]"),
                )
            }?,
        });
    }
    let account_entries = unsafe {
        try_slice_arg(
            accounts,
            accounts_len,
            "review_rule accounts",
            std::ptr::null_mut(),
        )
    }
    .ok_or_else(|| "review_rule accounts is null".to_owned())?;
    let mut accounts = Vec::with_capacity(account_entries.len());
    for (index, rule) in account_entries.iter().enumerate() {
        accounts.push(ReviewRuleAccountRule {
            account_id: AccountId::from_u64(rule.account_id),
            instruments: unsafe {
                parse_instruments(
                    rule.instruments,
                    rule.instruments_len,
                    rule.has_instruments,
                    format_args!("account_rules[{index}]"),
                )
            }?,
        });
    }
    Ok(ParsedRules {
        global,
        groups,
        accounts,
    })
}

#[no_mangle]
/// Adds the built-in review-rule policy to the engine builder.
///
/// Contract:
/// - A null or already-consumed `builder` is a handled error.
/// - `policy_group_id` assigns the policy group; pass 0 for the default group.
/// - `instance` supplies the registration name and mode.
///   Register a policy that keeps its own state (Spot Funds, the rate limit,
///   the P&L bounds kill switch, a custom policy that holds funds or counts) in
///   `OPENPIT_PRETRADE_RULE_MODE_REJECT` mode; \ref OpenPitPolicyInstance::mode
///   says what a review mode discards.
/// - Null `global` means no global rule. Empty array levels contain no rules.
///   An empty policy admits orders and may be configured later.
/// - `has_instruments == false` covers all instruments, ignoring pointer and
///   length. Otherwise the list must be nonempty, contain complete instrument
///   pairs, and have no duplicates. Group and account keys must be distinct.
///
/// # Safety
///
/// - A non-null `builder` must point to a live, aligned, initialized builder.
/// - Non-null `global` and `instance` must point to aligned, initialized,
///   readable values for the call; instance name bytes must be readable.
/// - Arrays with nonzero lengths must be non-null, aligned, initialized and
///   readable for their lengths. This includes each exact instrument array.
/// - All supplied bool fields must contain valid bool values. Each non-null
///   instrument string view must point to its length of readable bytes.
/// - `out_error` may be null; otherwise it must point to aligned, writable
///   storage for an `OpenPitSharedString` handle.
///
/// Success:
/// - Returns true; the builder retains the policy with owned rules.
///
/// Error:
/// - Returns false on invalid rules, parsing, instance or builder errors;
///   nothing is registered. If `out_error` is non-null, writes a caller-owned
///   error handle; release it with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_review_rule_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    policy_group_id: u16,
    global: *const OpenPitPretradePoliciesReviewRuleGlobalRule,
    account_group_rules: *const OpenPitPretradePoliciesReviewRuleAccountGroupRule,
    account_group_rules_len: usize,
    account_rules: *const OpenPitPretradePoliciesReviewRuleAccountRule,
    account_rules_len: usize,
    instance: *const crate::engine::OpenPitPolicyInstance,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    let rules = unsafe {
        parse_rules(
            global,
            account_group_rules,
            account_group_rules_len,
            account_rules,
            account_rules_len,
        )
    };
    let ParsedRules {
        global,
        groups,
        accounts,
    } = match rules {
        Ok(rules) => rules,
        Err(err) => {
            write_error(out_error, &err);
            return false;
        }
    };
    let settings = match ReviewRuleSettings::new(global, groups, accounts) {
        Ok(settings) => settings,
        Err(err) => {
            write_error_format!(out_error, "review_rule_policy creation failed: {}", err);
            return false;
        }
    };
    let policy = ReviewRulePolicy::<FfiStorageFactory>::new(settings)
        .with_policy_group_id(openpit::PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(unsafe { &mut *builder }, policy, instance)
    {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Updates the named review-rule policy, replacing each supplied level.
///
/// Contract:
/// - A null `engine` is a handled error. `name` must be UTF-8 and selects the
///   registration name (the default is "ReviewRulePolicy").
/// - False `has_*` keeps that level, ignoring its pointer and length.
/// - True `has_global` with null `global` clears the global rule. A supplied
///   array level replaces that whole level; length zero clears it.
/// - Instrument filters and distinct keys follow the registration contract.
///   Parsing and validation are atomic across all supplied levels.
///
/// # Safety
///
/// - A non-null `engine` must point to a live, aligned, initialized engine.
/// - All bool arguments and supplied bool fields must hold valid bool values.
/// - Non-null `name.ptr` must point to `name.len` readable bytes for the call.
/// - When its `has_*` is true, non-null `global` must point to one readable,
///   aligned, initialized rule. Array levels of nonzero length must point to
///   that many readable, aligned, initialized rules. Each exact instrument
///   array and its string views must remain readable for the call.
/// - `out_error` may be null; otherwise it must point to aligned, writable
///   storage for an `OpenPitConfigureError` pointer.
///
/// Success:
/// - Returns true; the new rules apply to subsequent orders.
///
/// Error:
/// - Returns false without changing settings. Parsing or rule errors return
///   Validation; unknown names and settings type mismatches retain their kind.
/// - If `out_error` is non-null, writes a caller-owned error; release it with
///   `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_review_rule(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    global: *const OpenPitPretradePoliciesReviewRuleGlobalRule,
    has_global: bool,
    account_group_rules: *const OpenPitPretradePoliciesReviewRuleAccountGroupRule,
    account_group_rules_len: usize,
    has_account_group_rules: bool,
    account_rules: *const OpenPitPretradePoliciesReviewRuleAccountRule,
    account_rules_len: usize,
    has_account_rules: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    let result = (|| {
        if engine.is_null() {
            return Err(OpenPitConfigureError::validation(
                "engine is null".to_owned(),
            ));
        }
        let name = unsafe { cstr_arg(name) }.ok_or_else(|| {
            OpenPitConfigureError::validation("policy name is null or invalid UTF-8".to_owned())
        })?;
        let ParsedRules {
            global,
            groups,
            accounts,
        } = unsafe {
            parse_rules(
                if has_global { global } else { std::ptr::null() },
                if has_account_group_rules {
                    account_group_rules
                } else {
                    std::ptr::null()
                },
                if has_account_group_rules {
                    account_group_rules_len
                } else {
                    0
                },
                if has_account_rules {
                    account_rules
                } else {
                    std::ptr::null()
                },
                if has_account_rules {
                    account_rules_len
                } else {
                    0
                },
            )
        }
        .map_err(OpenPitConfigureError::validation)?;
        unsafe { &*engine }
            .configurator()
            .review_rule(&name, |settings| -> Result<(), ReviewRulePolicyError> {
                if has_global {
                    settings.set_global(global.clone())?;
                }
                if has_account_group_rules {
                    settings.set_account_group_rules(groups.iter().cloned())?;
                }
                if has_account_rules {
                    settings.set_account_rules(accounts.iter().cloned())?;
                }
                Ok(())
            })
            .map_err(OpenPitConfigureError::new)
    })();
    match result {
        Ok(()) => true,
        Err(err) => {
            write_configure_error(out_error, err);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::*;
    use crate::order::{OpenPitOrder, OpenPitOrderOperation, OpenPitOrderOperationOptional};
    use crate::param::*;
    use crate::reject::*;
    use std::ptr::{null, null_mut};

    fn instrument(underlying: &'static str) -> OpenPitInstrument {
        OpenPitInstrument {
            underlying_asset: OpenPitStringView::from_utf8(underlying),
            settlement_asset: OpenPitStringView::from_utf8("USD"),
        }
    }

    fn global(
        instruments: Option<&[OpenPitInstrument]>,
    ) -> OpenPitPretradePoliciesReviewRuleGlobalRule {
        OpenPitPretradePoliciesReviewRuleGlobalRule {
            instruments: instruments.map_or(null(), |list| list.as_ptr()),
            instruments_len: instruments.map_or(0, |list| list.len()),
            has_instruments: instruments.is_some(),
        }
    }

    fn account(id: u64) -> OpenPitPretradePoliciesReviewRuleAccountRule {
        OpenPitPretradePoliciesReviewRuleAccountRule {
            account_id: id,
            instruments: null(),
            instruments_len: 0,
            has_instruments: false,
        }
    }

    fn group() -> OpenPitPretradePoliciesReviewRuleAccountGroupRule {
        OpenPitPretradePoliciesReviewRuleAccountGroupRule {
            account_group_id: 7,
            instruments: null(),
            instruments_len: 0,
            has_instruments: false,
        }
    }

    fn order(id: u64, underlying: &'static str) -> OpenPitOrder {
        OpenPitOrder {
            operation: OpenPitOrderOperationOptional {
                is_set: true,
                value: OpenPitOrderOperation {
                    instrument: instrument(underlying),
                    account_id: OpenPitParamAccountIdOptional {
                        value: id,
                        is_set: true,
                    },
                    trade_amount: OpenPitParamTradeAmount {
                        value: OpenPitParamDecimal {
                            mantissa_lo: 1,
                            mantissa_hi: 0,
                            scale: 0,
                        },
                        kind: OPENPIT_PARAM_TRADE_AMOUNT_KIND_QUANTITY,
                    },
                    side: OPENPIT_PARAM_SIDE_BUY,
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
            user_data: null_mut(),
            review: Default::default(),
        }
    }

    fn build(
        global: Option<&OpenPitPretradePoliciesReviewRuleGlobalRule>,
        groups: &[OpenPitPretradePoliciesReviewRuleAccountGroupRule],
        accounts: &[OpenPitPretradePoliciesReviewRuleAccountRule],
        mode: u8,
    ) -> *mut OpenPitEngine {
        let instance = OpenPitPolicyInstance {
            name: OpenPitStringView::from_utf8("rules"),
            mode,
        };
        build_with_instance(global, groups, accounts, &instance)
    }

    fn build_with_instance(
        global: Option<&OpenPitPretradePoliciesReviewRuleGlobalRule>,
        groups: &[OpenPitPretradePoliciesReviewRuleAccountGroupRule],
        accounts: &[OpenPitPretradePoliciesReviewRuleAccountRule],
        instance: *const OpenPitPolicyInstance,
    ) -> *mut OpenPitEngine {
        let builder = openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, null_mut());
        assert!(unsafe {
            openpit_engine_builder_add_builtin_review_rule_policy(
                builder,
                0,
                global.map_or(null(), |rule| rule),
                groups.as_ptr(),
                groups.len(),
                accounts.as_ptr(),
                accounts.len(),
                instance,
                null_mut(),
            )
        });
        let engine = openpit_engine_builder_build(builder, null_mut(), null_mut());
        openpit_destroy_engine_builder(builder);
        assert!(!engine.is_null());
        let members = [1001];
        assert!(openpit_engine_register_account_group(
            engine,
            members.as_ptr(),
            members.len(),
            7,
            null_mut(),
            null_mut(),
        ));
        engine
    }

    fn check(engine: *mut OpenPitEngine, id: u64, underlying: &'static str, mode: Option<u8>) {
        let mut reservation = null_mut();
        let mut rejects = null_mut();
        let mut error = null_mut();
        let status = openpit_engine_execute_pre_trade(
            engine,
            &order(id, underlying),
            &mut reservation,
            &mut rejects,
            &mut error,
        );
        assert!(error.is_null());
        assert_eq!(
            status,
            if mode == Some(1) {
                OpenPitPretradeStatus::Rejected
            } else {
                OpenPitPretradeStatus::Passed
            }
        );
        let records = if mode == Some(1) {
            assert!(reservation.is_null());
            rejects
        } else {
            assert!(rejects.is_null());
            openpit_pretrade_pre_trade_reservation_get_reviews(reservation)
        };
        assert_eq!(
            openpit_pretrade_reject_list_len(records),
            usize::from(mode.is_some())
        );
        if let Some(mode) = mode {
            let mut record = std::mem::MaybeUninit::uninit();
            assert!(openpit_pretrade_reject_list_get(
                records,
                0,
                record.as_mut_ptr()
            ));
            let record = unsafe { record.assume_init() };
            assert_eq!(record.mode, mode);
            assert_eq!(
                record.code,
                OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION
            );
            assert_eq!(record.scope, OPENPIT_PRETRADE_REJECT_SCOPE_ORDER);
            assert_eq!(
                unsafe { std::slice::from_raw_parts(record.policy.ptr, record.policy.len) },
                b"ReviewRulePolicy"
            );
        }
        openpit_destroy_pretrade_reject_list(records);
        openpit_destroy_pretrade_pre_trade_reservation(reservation);
    }

    #[test]
    fn review_rule_account_instance_verdicts() {
        for mode in [1, 2] {
            let engine = build(None, &[], &[account(1001)], mode);
            check(engine, 1001, "AAPL", Some(mode));
            check(engine, 1001, "MSFT", Some(mode));
            check(engine, 1002, "AAPL", None);
            openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn review_rule_instance_registration_through_c_abi() {
        for (instance, name, mode) in [
            (None, "ReviewRulePolicy", 1),
            (
                Some(OpenPitPolicyInstance::default()),
                "ReviewRulePolicy",
                1,
            ),
            (
                Some(OpenPitPolicyInstance {
                    name: OpenPitStringView::from_utf8(""),
                    mode: 0,
                }),
                "ReviewRulePolicy",
                1,
            ),
            (
                Some(OpenPitPolicyInstance {
                    name: OpenPitStringView::from_utf8("rules"),
                    mode: 2,
                }),
                "rules",
                2,
            ),
            (
                Some(OpenPitPolicyInstance {
                    name: OpenPitStringView::from_utf8("rules"),
                    mode: 3,
                }),
                "rules",
                3,
            ),
        ] {
            let engine = build_with_instance(
                Some(&global(None)),
                &[],
                &[],
                instance.as_ref().map_or(null(), |instance| instance),
            );
            let mut error = null_mut();
            assert!(unsafe {
                openpit_engine_configure_review_rule(
                    engine,
                    OpenPitStringView::from_utf8(name),
                    null(),
                    false,
                    null(),
                    0,
                    false,
                    null(),
                    0,
                    false,
                    &mut error,
                )
            });
            assert!(error.is_null());
            check(engine, 1001, "AAPL", Some(mode));
            openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn review_rule_invalid_instance_registers_nothing_through_c_abi() {
        for instance in [
            OpenPitPolicyInstance {
                name: OpenPitStringView::default(),
                mode: 4,
            },
            OpenPitPolicyInstance {
                name: OpenPitStringView {
                    ptr: b"\xff".as_ptr(),
                    len: 1,
                },
                mode: 1,
            },
        ] {
            assert_refused_registration(&global(None), &[], &[], &instance, None);
        }
    }

    #[test]
    fn review_rule_group_and_global_exact() {
        let engine = build(None, &[group()], &[], 1);
        check(engine, 1001, "AAPL", Some(1));
        check(engine, 1002, "AAPL", None);
        openpit_destroy_engine(engine);
        let instruments = [instrument("AAPL")];
        let engine = build(Some(&global(Some(&instruments))), &[], &[], 1);
        check(engine, 1001, "AAPL", Some(1));
        check(engine, 1002, "AAPL", Some(1));
        check(engine, 1001, "MSFT", None);
        openpit_destroy_engine(engine);
        let engine = build(Some(&global(None)), &[], &[], 1);
        check(engine, 1002, "MSFT", Some(1));
        openpit_destroy_engine(engine);
    }

    #[test]
    fn review_rule_invalid_registration_registers_nothing() {
        let absent = OpenPitInstrument {
            underlying_asset: OpenPitStringView::default(),
            settlement_asset: OpenPitStringView::default(),
        };
        let half = OpenPitInstrument {
            underlying_asset: OpenPitStringView::from_utf8("AAPL"),
            ..absent
        };
        let duplicate = [instrument("AAPL"), instrument("AAPL")];
        let absent_in_list = [absent, instrument("AAPL")];
        let filters: [&[OpenPitInstrument]; 4] = [&[], &absent_in_list, &[half], &duplicate];
        for level in 0..3 {
            for instruments in filters {
                let filter = global(Some(instruments));
                let groups = [OpenPitPretradePoliciesReviewRuleAccountGroupRule {
                    instruments: filter.instruments,
                    instruments_len: filter.instruments_len,
                    has_instruments: true,
                    ..group()
                }];
                let accounts = [OpenPitPretradePoliciesReviewRuleAccountRule {
                    instruments: filter.instruments,
                    instruments_len: filter.instruments_len,
                    has_instruments: true,
                    ..account(1001)
                }];
                assert_invalid_registration(
                    if level == 0 { &filter } else { null() },
                    if level == 1 { &groups } else { &[] },
                    if level == 2 { &accounts } else { &[] },
                );
            }
        }
        assert_invalid_registration(null(), &[group(), group()], &[]);
        assert_invalid_registration(null(), &[], &[account(1001), account(1001)]);
    }

    fn assert_invalid_registration(
        global: *const OpenPitPretradePoliciesReviewRuleGlobalRule,
        groups: &[OpenPitPretradePoliciesReviewRuleAccountGroupRule],
        accounts: &[OpenPitPretradePoliciesReviewRuleAccountRule],
    ) {
        assert_refused_registration(global, groups, accounts, null(), None);
    }

    fn assert_refused_registration(
        global: *const OpenPitPretradePoliciesReviewRuleGlobalRule,
        groups: &[OpenPitPretradePoliciesReviewRuleAccountGroupRule],
        accounts: &[OpenPitPretradePoliciesReviewRuleAccountRule],
        instance: *const OpenPitPolicyInstance,
        expected: Option<&str>,
    ) {
        let builder = openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, null_mut());
        let mut error = null_mut();
        assert!(!unsafe {
            openpit_engine_builder_add_builtin_review_rule_policy(
                builder,
                0,
                global,
                groups.as_ptr(),
                groups.len(),
                accounts.as_ptr(),
                accounts.len(),
                instance,
                &mut error,
            )
        });
        assert!(!error.is_null());
        assert!(crate::string::openpit_shared_string_view(error).len > 0);
        if let Some(expected) = expected {
            let view = crate::string::openpit_shared_string_view(error);
            let message = unsafe { cstr_arg(view) }.expect("registration error must be UTF-8");
            assert!(
                message.contains(expected),
                "registration error must include {expected:?}, got {message:?}"
            );
        }
        crate::string::openpit_destroy_shared_string(error);
        assert!(unsafe {
            super::super::order_size_limit::openpit_engine_builder_add_builtin_order_size_limit_policy(
                builder, 0, null(), null(), 0, null(), 0, null(), null_mut(),
            )
        });
        let engine = openpit_engine_builder_build(builder, null_mut(), null_mut());
        openpit_destroy_engine_builder(builder);
        assert!(
            !engine.is_null(),
            "invalid registration must not consume the builder"
        );
        check(engine, 1001, "AAPL", None);
        let mut error = null_mut();
        assert!(!unsafe {
            openpit_engine_configure_review_rule(
                engine,
                OpenPitStringView::from_utf8("ReviewRulePolicy"),
                null(),
                false,
                null(),
                0,
                false,
                null(),
                0,
                false,
                &mut error,
            )
        });
        assert_eq!(
            openpit_configure_error_get_kind(error),
            OpenPitConfigureErrorKind::Unknown
        );
        openpit_destroy_configure_error(error);
        openpit_destroy_engine(engine);
    }

    fn assert_parse_error(
        global: *const OpenPitPretradePoliciesReviewRuleGlobalRule,
        groups: &[OpenPitPretradePoliciesReviewRuleAccountGroupRule],
        accounts: &[OpenPitPretradePoliciesReviewRuleAccountRule],
        expected: &str,
    ) {
        assert_refused_registration(global, groups, accounts, null(), Some(expected));

        let engine = build(None, &[], &[account(1001)], 1);
        let mut error = null_mut();
        assert!(!unsafe {
            openpit_engine_configure_review_rule(
                engine,
                OpenPitStringView::from_utf8("rules"),
                global,
                true,
                groups.as_ptr(),
                groups.len(),
                true,
                accounts.as_ptr(),
                accounts.len(),
                true,
                &mut error,
            )
        });
        assert_eq!(
            openpit_configure_error_get_kind(error),
            OpenPitConfigureErrorKind::Validation
        );
        let view = openpit_configure_error_get_message(error);
        let message = unsafe { cstr_arg(view) }.expect("configure error must be UTF-8");
        assert!(
            message.contains(expected),
            "configure error must include {expected:?}, got {message:?}"
        );
        openpit_destroy_configure_error(error);
        check(engine, 1001, "AAPL", Some(1));
        check(engine, 1002, "AAPL", None);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn review_rule_global_parse_error_names_exact_instrument() {
        let instruments = [instrument("AAPL"), OpenPitInstrument::default()];
        assert_parse_error(
            &global(Some(&instruments)),
            &[],
            &[],
            "global_rule instruments[1] instrument is not set",
        );
    }

    #[test]
    fn review_rule_group_parse_error_names_exact_instrument() {
        let instruments = [
            instrument("AAPL"),
            OpenPitInstrument {
                settlement_asset: OpenPitStringView::default(),
                ..instrument("MSFT")
            },
        ];
        let groups = [
            group(),
            OpenPitPretradePoliciesReviewRuleAccountGroupRule {
                account_group_id: 8,
                instruments: instruments.as_ptr(),
                instruments_len: instruments.len(),
                has_instruments: true,
            },
        ];
        assert_parse_error(
            null(),
            &groups,
            &[],
            "account_group_rules[1] instruments[1] instrument must provide both",
        );
    }

    #[test]
    fn review_rule_account_parse_error_names_exact_instrument() {
        let instruments = [instrument("AAPL"), OpenPitInstrument::default()];
        let accounts = [
            account(1001),
            OpenPitPretradePoliciesReviewRuleAccountRule {
                instruments: instruments.as_ptr(),
                instruments_len: instruments.len(),
                has_instruments: true,
                ..account(1002)
            },
        ];
        assert_parse_error(
            null(),
            &[],
            &accounts,
            "account_rules[1] instruments[1] instrument is not set",
        );
    }

    #[test]
    fn review_rule_null_exact_list_names_its_rule() {
        let accounts = [OpenPitPretradePoliciesReviewRuleAccountRule {
            instruments_len: 1,
            has_instruments: true,
            ..account(1001)
        }];
        assert_parse_error(
            null(),
            &[],
            &accounts,
            "account_rules[0] instruments is null",
        );
    }

    #[test]
    fn review_rule_rejects_reserved_group_id() {
        let groups = [
            group(),
            OpenPitPretradePoliciesReviewRuleAccountGroupRule {
                account_group_id: 0,
                ..group()
            },
        ];
        assert_parse_error(&global(None), &groups, &[], "account_group_rules[1]");
    }

    #[test]
    fn review_rule_registration_ignores_instruments_when_flag_is_false() {
        let instruments = [instrument("AAPL")];
        let all = OpenPitPretradePoliciesReviewRuleGlobalRule {
            has_instruments: false,
            ..global(Some(&instruments))
        };
        let engine = build(Some(&all), &[], &[], 1);
        check(engine, 1001, "AAPL", Some(1));
        check(engine, 1001, "MSFT", Some(1));
        openpit_destroy_engine(engine);
    }

    #[test]
    fn review_rule_configure_ignores_instruments_when_flag_is_false() {
        let instruments = [instrument("AAPL")];
        let all = OpenPitPretradePoliciesReviewRuleGlobalRule {
            has_instruments: false,
            ..global(Some(&instruments))
        };
        let engine = build(None, &[], &[], 1);
        check(engine, 1001, "MSFT", None);
        let mut error = null_mut();
        assert!(unsafe {
            openpit_engine_configure_review_rule(
                engine,
                OpenPitStringView::from_utf8("rules"),
                &all,
                true,
                null(),
                0,
                false,
                null(),
                0,
                false,
                &mut error,
            )
        });
        assert!(error.is_null());
        check(engine, 1001, "AAPL", Some(1));
        check(engine, 1001, "MSFT", Some(1));
        openpit_destroy_engine(engine);
    }

    #[test]
    fn review_rule_configure_lifecycle_is_atomic() {
        let engine = build(None, &[], &[], 1);
        check(engine, 1001, "AAPL", None);
        let all = global(None);
        let groups = [group()];
        let accounts = [account(1001)];
        for level in 0..3 {
            let mut error = null_mut();
            assert!(unsafe {
                openpit_engine_configure_review_rule(
                    engine,
                    OpenPitStringView::from_utf8("rules"),
                    &all,
                    level == 0,
                    groups.as_ptr(),
                    groups.len(),
                    level == 1,
                    accounts.as_ptr(),
                    accounts.len(),
                    level == 2,
                    &mut error,
                )
            });
            assert!(error.is_null());
            check(engine, 1001, "AAPL", Some(1));
            assert!(unsafe {
                openpit_engine_configure_review_rule(
                    engine,
                    OpenPitStringView::from_utf8("rules"),
                    null(),
                    false,
                    null(),
                    9,
                    false,
                    null(),
                    9,
                    false,
                    &mut error,
                )
            });
            check(engine, 1001, "MSFT", Some(1));
            let empty = global(Some(&[]));
            assert!(!unsafe {
                openpit_engine_configure_review_rule(
                    engine,
                    OpenPitStringView::from_utf8("rules"),
                    &empty,
                    true,
                    null(),
                    0,
                    true,
                    null(),
                    0,
                    true,
                    &mut error,
                )
            });
            assert_eq!(
                openpit_configure_error_get_kind(error),
                OpenPitConfigureErrorKind::Validation
            );
            openpit_destroy_configure_error(error);
            error = null_mut();
            check(engine, 1001, "AAPL", Some(1));
            let exact = [instrument("MSFT")];
            let exact_global = global(Some(&exact));
            let exact_groups = [OpenPitPretradePoliciesReviewRuleAccountGroupRule {
                instruments: exact.as_ptr(),
                instruments_len: exact.len(),
                has_instruments: true,
                ..group()
            }];
            let exact_accounts = [OpenPitPretradePoliciesReviewRuleAccountRule {
                instruments: exact.as_ptr(),
                instruments_len: exact.len(),
                has_instruments: true,
                ..account(1001)
            }];
            assert!(unsafe {
                openpit_engine_configure_review_rule(
                    engine,
                    OpenPitStringView::from_utf8("rules"),
                    &exact_global,
                    level == 0,
                    exact_groups.as_ptr(),
                    exact_groups.len(),
                    level == 1,
                    exact_accounts.as_ptr(),
                    exact_accounts.len(),
                    level == 2,
                    &mut error,
                )
            });
            check(engine, 1001, "AAPL", None);
            check(engine, 1001, "MSFT", Some(1));
            assert!(unsafe {
                openpit_engine_configure_review_rule(
                    engine,
                    OpenPitStringView::from_utf8("rules"),
                    null(),
                    true,
                    null(),
                    0,
                    true,
                    null(),
                    0,
                    true,
                    &mut error,
                )
            });
            check(engine, 1001, "AAPL", None);
        }
        // A later level fails after a valid global change: neither is
        // published.
        let duplicates = [account(1001), account(1001)];
        let mut error = null_mut();
        assert!(!unsafe {
            openpit_engine_configure_review_rule(
                engine,
                OpenPitStringView::from_utf8("rules"),
                &all,
                true,
                null(),
                0,
                false,
                duplicates.as_ptr(),
                duplicates.len(),
                true,
                &mut error,
            )
        });
        assert_eq!(
            openpit_configure_error_get_kind(error),
            OpenPitConfigureErrorKind::Validation
        );
        openpit_destroy_configure_error(error);
        check(engine, 1001, "AAPL", None);
        check(engine, 1002, "MSFT", None);
        // A parse failure also preserves an existing account rule.
        let mut error = null_mut();
        assert!(unsafe {
            openpit_engine_configure_review_rule(
                engine,
                OpenPitStringView::from_utf8("rules"),
                null(),
                false,
                null(),
                0,
                false,
                accounts.as_ptr(),
                accounts.len(),
                true,
                &mut error,
            )
        });
        let half = [OpenPitInstrument {
            underlying_asset: OpenPitStringView::from_utf8("AAPL"),
            settlement_asset: OpenPitStringView::default(),
        }];
        assert!(!unsafe {
            openpit_engine_configure_review_rule(
                engine,
                OpenPitStringView::from_utf8("rules"),
                &global(Some(&half)),
                true,
                null(),
                0,
                false,
                null(),
                0,
                true,
                &mut error,
            )
        });
        assert_eq!(
            openpit_configure_error_get_kind(error),
            OpenPitConfigureErrorKind::Validation
        );
        openpit_destroy_configure_error(error);
        check(engine, 1001, "AAPL", Some(1));
        check(engine, 1002, "AAPL", None);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn review_rule_configure_reports_name_type_and_pointer_errors() {
        let builder = openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, null_mut());
        assert!(unsafe {
            super::super::order_size_limit::openpit_engine_builder_add_builtin_order_size_limit_policy(
            builder, 0, null(), null(), 0, null(), 0, null(), null_mut(),
        )
        });
        let engine = openpit_engine_builder_build(builder, null_mut(), null_mut());
        openpit_destroy_engine_builder(builder);
        let invalid_utf8 = [0xff];
        for (target, name, kind) in [
            (
                engine,
                OpenPitStringView::from_utf8("missing"),
                OpenPitConfigureErrorKind::Unknown,
            ),
            (
                engine,
                OpenPitStringView::from_utf8("OrderSizeLimitPolicy"),
                OpenPitConfigureErrorKind::TypeMismatch,
            ),
            (
                engine,
                OpenPitStringView::default(),
                OpenPitConfigureErrorKind::Validation,
            ),
            (
                engine,
                OpenPitStringView {
                    ptr: invalid_utf8.as_ptr(),
                    len: 1,
                },
                OpenPitConfigureErrorKind::Validation,
            ),
            (
                null_mut(),
                OpenPitStringView::from_utf8("rules"),
                OpenPitConfigureErrorKind::Validation,
            ),
        ] {
            let mut error = null_mut();
            assert!(!unsafe {
                openpit_engine_configure_review_rule(
                    target,
                    name,
                    null(),
                    false,
                    null(),
                    0,
                    false,
                    null(),
                    0,
                    false,
                    &mut error,
                )
            });
            assert!(!error.is_null());
            assert_eq!(openpit_configure_error_get_kind(error), kind);
            openpit_destroy_configure_error(error);
        }
        openpit_destroy_engine(engine);
    }
}
