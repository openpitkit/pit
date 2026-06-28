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

use crate::param::{AccountId, Asset, Pnl};
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
    account_id: AccountId,
) -> Reject {
    let desc = breached_sides.join(" and ");
    Reject::new(
        policy_name,
        RejectScope::Account,
        RejectCode::PnlKillSwitchTriggered,
        reason,
        format!(
            "{desc} bound breached: realized pnl {realized}, \
             lower_bound {lower_bound:?}, upper_bound {upper_bound:?}, \
             {asset_label} {asset}, account {account_id}"
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
