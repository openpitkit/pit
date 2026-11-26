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

//! Binding-facing access to an order's pre-trade review group.

use openpit::{HasAlreadyHeld, HasConfirmed, RequestFieldAccessError};

/// Pre-trade review parameters supplied by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedOrderReview {
    /// Whether the caller has already confirmed the order for pre-trade review.
    pub confirmed: bool,
    /// Whether the caller asserts that Spot Funds already holds this order's
    /// funds from an earlier committed review.
    pub already_held: bool,
}

impl HasConfirmed for PopulatedOrderReview {
    fn confirmed(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.confirmed)
    }
}

impl HasAlreadyHeld for PopulatedOrderReview {
    fn already_held(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.already_held)
    }
}

/// Runtime access to an order's pre-trade review group.
///
/// An absent review group means the order is unconfirmed and not held, so
/// [`HasConfirmed::confirmed`] and [`HasAlreadyHeld::already_held`] return
/// `Ok(false)`. An absent position group produces an access error from
/// [`openpit::HasReduceOnly::reduce_only`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderReviewAccess {
    /// The caller supplied a review group.
    Populated(PopulatedOrderReview),
    /// The caller supplied no review group.
    Absent,
}

impl HasConfirmed for OrderReviewAccess {
    fn confirmed(&self) -> Result<bool, RequestFieldAccessError> {
        match self {
            Self::Populated(review) => review.confirmed(),
            Self::Absent => Ok(false),
        }
    }
}

impl HasAlreadyHeld for OrderReviewAccess {
    fn already_held(&self) -> Result<bool, RequestFieldAccessError> {
        match self {
            Self::Populated(review) => review.already_held(),
            Self::Absent => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_review_is_unconfirmed_and_not_held() {
        assert_eq!(OrderReviewAccess::Absent.confirmed(), Ok(false));
        assert_eq!(OrderReviewAccess::Absent.already_held(), Ok(false));
    }

    #[test]
    fn populated_review_returns_its_confirmation() {
        for (confirmed, already_held) in [(true, false), (false, true)] {
            let populated = PopulatedOrderReview {
                confirmed,
                already_held,
            };
            assert_eq!(populated.confirmed(), Ok(confirmed));
            assert_eq!(populated.already_held(), Ok(already_held));
            let review = OrderReviewAccess::Populated(populated);
            assert_eq!(review.confirmed(), Ok(confirmed));
            assert_eq!(review.already_held(), Ok(already_held));
        }
    }
}
