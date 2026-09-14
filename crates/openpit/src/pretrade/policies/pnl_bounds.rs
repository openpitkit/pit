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

use std::collections::HashMap;
use std::hash::Hash;

use crate::param::{Asset, Pnl};
use crate::pretrade::policy::PolicyName;
use crate::pretrade::{AccountBlock, Reject, RejectCode, RejectScope};

pub(super) fn has_configured_bound(lower_bound: &Option<Pnl>, upper_bound: &Option<Pnl>) -> bool {
    lower_bound.is_some() || upper_bound.is_some()
}

pub(super) fn breached_sides(
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
    realized: Pnl,
) -> Vec<&'static str> {
    let mut sides = Vec::new();
    if let Some(lb) = lower_bound {
        if realized < lb {
            sides.push("lower");
        }
    }
    if let Some(ub) = upper_bound {
        if realized > ub {
            sides.push("upper");
        }
    }
    sides
}

pub(super) fn outside_bounds(
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
    realized: Pnl,
) -> bool {
    !breached_sides(lower_bound, upper_bound, realized).is_empty()
}

/// Formats the breached value and only the bounds that were configured.
pub(super) fn barrier_breach_details(
    breached_sides: &[&'static str],
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
    realized: Pnl,
    asset_label: &'static str,
    asset: &Asset,
) -> String {
    let desc = breached_sides.join(" and ");
    let mut details = format!("{desc} bound breached: realized pnl {realized}");
    if let Some(lower) = lower_bound {
        details.push_str(&format!(", lower bound {lower}"));
    }
    if let Some(upper) = upper_bound {
        details.push_str(&format!(", upper bound {upper}"));
    }
    details.push_str(&format!(", {asset_label} {asset}"));
    details
}

#[allow(clippy::too_many_arguments)]
pub(super) fn barrier_breach_reject(
    policy_name: &'static str,
    reason: &'static str,
    breached_sides: &[&'static str],
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
    realized: Pnl,
    asset_label: &'static str,
    asset: &Asset,
) -> Reject {
    Reject::new(
        policy_name,
        RejectScope::Account,
        RejectCode::PnlKillSwitchTriggered,
        reason,
        barrier_breach_details(
            breached_sides,
            lower_bound,
            upper_bound,
            realized,
            asset_label,
            asset,
        ),
    )
}

pub(super) fn pnl_breach_account_block(
    policy_name: &'static str,
    details: impl Into<String>,
) -> AccountBlock {
    AccountBlock::new(
        policy_name,
        RejectCode::PnlKillSwitchTriggered,
        "pnl kill switch triggered",
        details.into(),
    )
}

/// Blocks the account when its account-tier barrier currency mismatches the
/// effective account currency.
///
/// The account-tier barrier still controls the account, so the mismatch fails
/// closed as a configuration fault. Group and global mismatches are skipped
/// before this constructor is called.
pub(super) fn pnl_barrier_currency_mismatch_account_block(
    policy_name: &'static str,
    details: impl Into<String>,
) -> AccountBlock {
    AccountBlock::new(
        policy_name,
        RejectCode::PnlKillSwitchTriggered,
        "pnl barrier currency mismatch",
        details.into(),
    )
}

pub(super) fn pnl_calculation_failed_block<Policy: PolicyName + ?Sized>(
    policy: &Policy,
    reason: &'static str,
    details: String,
) -> AccountBlock {
    AccountBlock::new(
        policy.policy_name(),
        RejectCode::OrderValueCalculationFailed,
        reason,
        details,
    )
}

pub(super) fn set_or_clear<Key, Value>(
    map: &mut HashMap<Key, Value>,
    key: Key,
    value: Option<Value>,
) where
    Key: Eq + Hash,
{
    if let Some(value) = value {
        map.insert(key, value);
    } else {
        map.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breach_details_preserve_decimal_values_and_configured_bounds() {
        let asset = Asset::new("USD").expect("valid asset");
        let lower = Pnl::from_str("-1.25").expect("valid lower bound");
        let upper = Pnl::from_str("2.75").expect("valid upper bound");
        for (side, low, high, realized, expected) in [
            ("lower", Some(lower), None, "-1.25001", "lower bound breached: realized pnl -1.25001, lower bound -1.25, currency USD"),
            ("upper", None, Some(upper), "2.75001", "upper bound breached: realized pnl 2.75001, upper bound 2.75, currency USD"),
            ("lower", Some(lower), Some(upper), "-1.25001", "lower bound breached: realized pnl -1.25001, lower bound -1.25, upper bound 2.75, currency USD"),
        ] {
            assert_eq!(
                barrier_breach_details(
                    &[side], low, high,
                    Pnl::from_str(realized).expect("valid realized pnl"),
                    "currency", &asset,
                ),
                expected,
            );
        }
    }

    // The account id must never surface in the pnl-bounds reject free text:
    // those strings flow into logs and to managers who could otherwise use the
    // id to reach data they are not authorized to see. The reject constructor
    // takes no account id; this guards the format string against a regression
    // that re-introduces one.
    #[test]
    fn account_id_is_not_leaked_into_barrier_breach_reject() {
        let asset = Asset::new("USD").expect("asset literal must be valid");
        let reject = barrier_breach_reject(
            "pnl-bounds",
            "pnl kill switch triggered",
            &["lower"],
            Some(Pnl::from_str("-100").expect("pnl literal must be valid")),
            Some(Pnl::from_str("50").expect("pnl literal must be valid")),
            Pnl::from_str("-101").expect("pnl literal must be valid"),
            "settlement asset",
            &asset,
        );

        assert!(
            !reject.reason.contains("account"),
            "reason: {}",
            reject.reason
        );
        assert!(
            !reject.details.contains("account"),
            "details: {}",
            reject.details
        );
        // The financial operands stay in the details.
        assert!(reject.details.contains("realized pnl -101"));
        assert!(reject.details.contains("settlement asset USD"));
    }
}
