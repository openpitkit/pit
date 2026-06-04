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

//! Small constructors for [`Reject`] / [`AccountBlock`] values used by
//! [`SpotFundsPolicy`](super::SpotFundsPolicy).

use crate::param::{AccountId, Asset, PositionSize};
use crate::pretrade::policy::{field_access_error_account_adjustment_reject, PolicyName};
use crate::pretrade::{AccountBlock, Reject, RejectCode, RejectScope, Rejects};

pub(super) fn insufficient_funds_reject(
    policy: &str,
    asset: &Asset,
    account_id: AccountId,
    available: PositionSize,
    requested: PositionSize,
) -> Reject {
    Reject::new(
        policy,
        RejectScope::Order,
        RejectCode::InsufficientFunds,
        "spot funds insufficient",
        format!(
            "account {account_id}, asset {asset}: available {available}, requested {requested}"
        ),
    )
}

pub(super) fn adj_field<P: PolicyName + ?Sized, T>(
    policy: &P,
    name: &str,
    result: Result<T, crate::RequestFieldAccessError>,
) -> Result<T, Rejects> {
    result.map_err(|e| {
        Rejects::from(field_access_error_account_adjustment_reject(
            policy, name, &e,
        ))
    })
}

pub(super) fn order_value_calculation_failed_reject(
    policy: &str,
    details: impl Into<String>,
) -> Reject {
    Reject::new(
        policy,
        RejectScope::Order,
        RejectCode::OrderValueCalculationFailed,
        "order value calculation failed",
        details.into(),
    )
}

pub(super) fn arithmetic_overflow_reject(
    policy: &str,
    scope: RejectScope,
    details: impl Into<String>,
) -> Reject {
    Reject::new(
        policy,
        scope,
        RejectCode::ArithmeticOverflow,
        "arithmetic overflow",
        details.into(),
    )
}

pub(super) fn arithmetic_overflow_account_block(
    policy: &str,
    details: impl Into<String>,
) -> AccountBlock {
    AccountBlock::new(
        policy,
        RejectCode::ArithmeticOverflow,
        "arithmetic overflow",
        details.into(),
    )
}

pub(super) fn account_adjustment_bounds_exceeded_reject(
    policy: &str,
    account_id: AccountId,
    asset: &Asset,
    field: &str,
    actual: PositionSize,
    lower: Option<PositionSize>,
    upper: Option<PositionSize>,
) -> Reject {
    Reject::new(
        policy,
        RejectScope::Account,
        RejectCode::AccountAdjustmentBoundsExceeded,
        "account adjustment bounds exceeded",
        format!(
            "account {account_id}, asset {asset}: {field} {actual}, \
             lower {lower:?}, upper {upper:?}"
        ),
    )
}
