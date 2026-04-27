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

//! Runtime wrapper for the account-adjustment amount group.

use openpit::param::AdjustmentAmount;
use openpit::{
    AccountAdjustmentAmount, HasAccountAdjustmentPending, HasAccountAdjustmentReserved,
    HasAccountAdjustmentTotal, RequestFieldAccessError,
};

/// Runtime access to an account adjustment's amount group.
///
/// Use [`AccountAdjustmentAmountAccess::Populated`] when the group is present,
/// [`AccountAdjustmentAmountAccess::Absent`] when it is not.
///
/// When absent, all three traits return `Err`; within a populated group, each
/// individual amount is `Option<AdjustmentAmount>` and may be `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountAdjustmentAmountAccess {
    /// The amount group is present.
    Populated(AccountAdjustmentAmount),
    /// The amount group is absent.
    Absent,
}

impl HasAccountAdjustmentTotal for AccountAdjustmentAmountAccess {
    fn total(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        match self {
            Self::Populated(a) => Ok(a.total),
            Self::Absent => Err(RequestFieldAccessError::new("amount.total")),
        }
    }
}

impl HasAccountAdjustmentReserved for AccountAdjustmentAmountAccess {
    fn reserved(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        match self {
            Self::Populated(a) => Ok(a.reserved),
            Self::Absent => Err(RequestFieldAccessError::new("amount.reserved")),
        }
    }
}

impl HasAccountAdjustmentPending for AccountAdjustmentAmountAccess {
    fn pending(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        match self {
            Self::Populated(a) => Ok(a.pending),
            Self::Absent => Err(RequestFieldAccessError::new("amount.pending")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{AdjustmentAmount, PositionSize};
    use openpit::AccountAdjustmentAmount;

    #[test]
    fn populated_returns_ok_with_some_values() {
        let access = AccountAdjustmentAmountAccess::Populated(AccountAdjustmentAmount {
            total: Some(AdjustmentAmount::Absolute(
                PositionSize::from_str("10").expect("valid"),
            )),
            reserved: None,
            pending: None,
        });
        assert!(access.total().unwrap().is_some());
        assert!(access.reserved().unwrap().is_none());
        assert!(access.pending().unwrap().is_none());
    }

    #[test]
    fn absent_returns_err() {
        let access = AccountAdjustmentAmountAccess::Absent;
        assert!(access.total().is_err());
        assert!(access.reserved().is_err());
        assert!(access.pending().is_err());
    }
}
