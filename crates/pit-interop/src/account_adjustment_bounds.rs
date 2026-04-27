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

//! Runtime wrapper for the account-adjustment bounds group.

use openpit::param::PositionSize;
use openpit::{
    AccountAdjustmentBounds, HasAccountAdjustmentPendingLowerBound,
    HasAccountAdjustmentPendingUpperBound, HasAccountAdjustmentReservedLowerBound,
    HasAccountAdjustmentReservedUpperBound, HasAccountAdjustmentTotalLowerBound,
    HasAccountAdjustmentTotalUpperBound, RequestFieldAccessError,
};

/// Runtime access to an account adjustment's bounds group.
///
/// Use [`AccountAdjustmentBoundsAccess::Populated`] when the group is present,
/// [`AccountAdjustmentBoundsAccess::Absent`] when it is not.
///
/// When absent, all six traits return `Err`; within a populated group, each
/// individual bound is `Option<PositionSize>` and may be `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountAdjustmentBoundsAccess {
    /// The bounds group is present.
    Populated(AccountAdjustmentBounds),
    /// The bounds group is absent.
    Absent,
}

impl HasAccountAdjustmentTotalUpperBound for AccountAdjustmentBoundsAccess {
    fn total_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        match self {
            Self::Populated(b) => Ok(b.total_upper),
            Self::Absent => Err(RequestFieldAccessError::new("bounds.total_upper")),
        }
    }
}

impl HasAccountAdjustmentTotalLowerBound for AccountAdjustmentBoundsAccess {
    fn total_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        match self {
            Self::Populated(b) => Ok(b.total_lower),
            Self::Absent => Err(RequestFieldAccessError::new("bounds.total_lower")),
        }
    }
}

impl HasAccountAdjustmentReservedUpperBound for AccountAdjustmentBoundsAccess {
    fn reserved_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        match self {
            Self::Populated(b) => Ok(b.reserved_upper),
            Self::Absent => Err(RequestFieldAccessError::new("bounds.reserved_upper")),
        }
    }
}

impl HasAccountAdjustmentReservedLowerBound for AccountAdjustmentBoundsAccess {
    fn reserved_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        match self {
            Self::Populated(b) => Ok(b.reserved_lower),
            Self::Absent => Err(RequestFieldAccessError::new("bounds.reserved_lower")),
        }
    }
}

impl HasAccountAdjustmentPendingUpperBound for AccountAdjustmentBoundsAccess {
    fn pending_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        match self {
            Self::Populated(b) => Ok(b.pending_upper),
            Self::Absent => Err(RequestFieldAccessError::new("bounds.pending_upper")),
        }
    }
}

impl HasAccountAdjustmentPendingLowerBound for AccountAdjustmentBoundsAccess {
    fn pending_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        match self {
            Self::Populated(b) => Ok(b.pending_lower),
            Self::Absent => Err(RequestFieldAccessError::new("bounds.pending_lower")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::PositionSize;
    use openpit::AccountAdjustmentBounds;

    #[test]
    fn populated_returns_ok() {
        let access = AccountAdjustmentBoundsAccess::Populated(AccountAdjustmentBounds {
            total_upper: Some(PositionSize::from_str("100").expect("valid")),
            total_lower: Some(PositionSize::from_str("-10").expect("valid")),
            reserved_upper: Some(PositionSize::from_str("90").expect("valid")),
            reserved_lower: Some(PositionSize::from_str("-9").expect("valid")),
            pending_upper: Some(PositionSize::from_str("80").expect("valid")),
            pending_lower: Some(PositionSize::from_str("-8").expect("valid")),
        });
        assert!(access.total_upper().unwrap().is_some());
        assert!(access.total_lower().unwrap().is_some());
        assert!(access.reserved_upper().unwrap().is_some());
        assert!(access.reserved_lower().unwrap().is_some());
        assert!(access.pending_upper().unwrap().is_some());
        assert!(access.pending_lower().unwrap().is_some());
    }

    #[test]
    fn absent_returns_err_for_all() {
        let access = AccountAdjustmentBoundsAccess::Absent;
        assert!(access.total_upper().is_err());
        assert!(access.total_lower().is_err());
        assert!(access.reserved_upper().is_err());
        assert!(access.reserved_lower().is_err());
        assert!(access.pending_upper().is_err());
        assert!(access.pending_lower().is_err());
    }
}
