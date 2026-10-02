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

//! Worst-case position limit for spot-funds reservations.
//!
//! The owned position uses `available + held` as recorded, including a negative
//! `held` residual. Long projections add only open positive `incoming`; short
//! projections subtract only open positive `held`.

use crate::param::{AccountId, Asset, PositionSize, Quantity};
use crate::pretrade::holdings::Holdings;
use crate::pretrade::{Reject, RejectCode, RejectScope, Rejects};

use super::market_data::SpotFundsSettings;
use super::pre_trade::ReservationStep;
use super::rejects::arithmetic_overflow_reject;
use super::SPOT_FUNDS_POLICY_NAME;

#[derive(Clone, Copy)]
pub(super) enum Check {
    FirstStepTransition,
    Position {
        limit: Quantity,
        earlier_held: PositionSize,
        earlier_incoming: PositionSize,
    },
}

#[derive(Clone, Copy)]
enum ExactArithmeticError {
    Overflow,
    Inexact,
}

pub(super) fn plan(
    settings: &SpotFundsSettings,
    account_id: AccountId,
    steps: &[ReservationStep; 2],
) -> [Option<Check>; 2] {
    if steps[0].asset != steps[1].asset {
        return [
            check_for(settings, account_id, &steps[0], None),
            check_for(settings, account_id, &steps[1], None),
        ];
    }

    let first_touches = touches(&steps[0]);
    let second_touches = touches(&steps[1]);
    // A same-asset order touching its slot in both steps gets its verdict on
    // the second step against the pre-order slot rebuilt by undoing the first
    // step.
    // That rebuild is valid only if the first step's own write was exact, so a
    // non-zero net makes the first step prove exactness and fail closed
    // otherwise.
    match (first_touches, second_touches) {
        (false, false) => [None, None],
        (true, false) => [check_for(settings, account_id, &steps[0], None), None],
        (false, true) => [None, check_for(settings, account_id, &steps[1], None)],
        (true, true) => {
            let final_check = check_for(settings, account_id, &steps[1], Some(&steps[0]));
            let first_check = final_check.and_then(|_| {
                match aggregate_net(
                    steps[1].held,
                    steps[1].incoming,
                    steps[0].held,
                    steps[0].incoming,
                ) {
                    Ok(net) if net.is_zero() => None,
                    Ok(_) | Err(_) => Some(Check::FirstStepTransition),
                }
            });
            [first_check, final_check]
        }
    }
}

pub(super) fn verify(
    check: Option<Check>,
    asset: &Asset,
    current: Holdings,
    held: PositionSize,
    incoming: PositionSize,
) -> Result<(), Rejects> {
    let Some(check) = check else {
        return Ok(());
    };

    let Check::Position {
        limit,
        earlier_held,
        earlier_incoming,
    } = check
    else {
        return verify_first_step_transition(asset, current, held, incoming);
    };

    let net = aggregate_net(held, incoming, earlier_held, earlier_incoming)
        .map_err(|cause| arithmetic_reject(asset, "net", cause))?;
    if net.is_zero() {
        return Ok(());
    }

    let available = exact_add(current.available(), earlier_held)
        .map_err(|cause| arithmetic_reject(asset, "pre-order reconstruction", cause))?;
    let held_before_order = exact_sub(current.held(), earlier_held)
        .map_err(|cause| arithmetic_reject(asset, "pre-order reconstruction", cause))?;
    let incoming_before_order = exact_sub(current.incoming(), earlier_incoming)
        .map_err(|cause| arithmetic_reject(asset, "pre-order reconstruction", cause))?;
    let owned = exact_add(available, held_before_order)
        .map_err(|cause| arithmetic_reject(asset, "pre-order reconstruction", cause))?;
    let limit = limit.to_position_size();

    let (side, projected, exceeded) = if net > PositionSize::ZERO {
        let open_incoming = incoming_before_order.max(PositionSize::ZERO);
        let projected = exact_add(owned, open_incoming)
            .and_then(|position| exact_add(position, net))
            .map_err(|cause| arithmetic_reject(asset, "projection", cause))?;
        ("long", projected, projected > limit)
    } else {
        let open_held = held_before_order.max(PositionSize::ZERO);
        let projected = exact_sub(owned, open_held)
            .and_then(|position| exact_add(position, net))
            .map_err(|cause| arithmetic_reject(asset, "projection", cause))?;
        ("short", projected, projected < -limit)
    };

    if exceeded {
        return Err(Reject::new(
            SPOT_FUNDS_POLICY_NAME,
            RejectScope::Order,
            RejectCode::PositionLimitExceeded,
            "position limit exceeded",
            format!(
                "asset {asset}, side {side}, projected worst-case position {projected}, limit {limit}"
            ),
        )
        .into());
    }
    Ok(())
}

fn check_for(
    settings: &SpotFundsSettings,
    account_id: AccountId,
    step: &ReservationStep,
    earlier: Option<&ReservationStep>,
) -> Option<Check> {
    settings
        .position_limit_for(account_id, &step.asset)
        .map(|limit| Check::Position {
            limit,
            earlier_held: earlier.map_or(PositionSize::ZERO, |step| step.held),
            earlier_incoming: earlier.map_or(PositionSize::ZERO, |step| step.incoming),
        })
}

fn touches(step: &ReservationStep) -> bool {
    !step.held.is_zero() || !step.incoming.is_zero()
}

fn verify_first_step_transition(
    asset: &Asset,
    current: Holdings,
    held: PositionSize,
    incoming: PositionSize,
) -> Result<(), Rejects> {
    exact_sub(current.available(), held)
        .and_then(|_| exact_add(current.held(), held))
        .and_then(|_| exact_add(current.incoming(), incoming))
        .map(|_| ())
        .map_err(|cause| arithmetic_reject(asset, "first-step transition", cause))
}

fn arithmetic_reject(asset: &Asset, stage: &str, error: ExactArithmeticError) -> Rejects {
    let cause = match error {
        ExactArithmeticError::Overflow => "overflow",
        ExactArithmeticError::Inexact => "inexact",
    };
    Rejects::from(arithmetic_overflow_reject(
        SPOT_FUNDS_POLICY_NAME,
        RejectScope::Order,
        format!(
            "position-limit arithmetic failure: asset {asset}, stage {stage}, \
             cause {cause}",
        ),
    ))
}

fn step_net(
    held: PositionSize,
    incoming: PositionSize,
) -> Result<PositionSize, ExactArithmeticError> {
    exact_sub(incoming, held)
}

fn aggregate_net(
    held: PositionSize,
    incoming: PositionSize,
    earlier_held: PositionSize,
    earlier_incoming: PositionSize,
) -> Result<PositionSize, ExactArithmeticError> {
    step_net(held, incoming).and_then(|net| {
        step_net(earlier_held, earlier_incoming).and_then(|earlier| exact_add(earlier, net))
    })
}

fn exact_add(
    left: PositionSize,
    right: PositionSize,
) -> Result<PositionSize, ExactArithmeticError> {
    let result = left
        .checked_add(right)
        .map_err(|_| ExactArithmeticError::Overflow)?;
    // Each round trip catches precision lost from one operand order.
    if result.checked_sub(left).ok() == Some(right) && result.checked_sub(right).ok() == Some(left)
    {
        Ok(result)
    } else {
        Err(ExactArithmeticError::Inexact)
    }
}

fn exact_sub(
    left: PositionSize,
    right: PositionSize,
) -> Result<PositionSize, ExactArithmeticError> {
    let result = left
        .checked_sub(right)
        .map_err(|_| ExactArithmeticError::Overflow)?;
    // Each round trip catches precision lost from one operand order.
    if result.checked_add(right).ok() == Some(left) && left.checked_sub(result).ok() == Some(right)
    {
        Ok(result)
    } else {
        Err(ExactArithmeticError::Inexact)
    }
}
