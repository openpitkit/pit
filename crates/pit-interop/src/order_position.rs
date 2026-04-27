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

//! Runtime wrapper for the order position group.

use openpit::param::PositionSide;
use openpit::{HasClosePosition, HasOrderPositionSide, HasReduceOnly, RequestFieldAccessError};

/// Populated order-position group.
///
/// Boolean fields are always set when the group is present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedOrderPosition {
    /// Position side for hedged-mode orders.
    pub position_side: Option<PositionSide>,
    /// Whether the order can only reduce an existing position.
    pub reduce_only: bool,
    /// Whether the order must close the entire position.
    pub close_position: bool,
}

impl HasOrderPositionSide for PopulatedOrderPosition {
    fn position_side(&self) -> Result<Option<PositionSide>, RequestFieldAccessError> {
        Ok(self.position_side)
    }
}

impl HasReduceOnly for PopulatedOrderPosition {
    fn reduce_only(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.reduce_only)
    }
}

impl HasClosePosition for PopulatedOrderPosition {
    fn close_position(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.close_position)
    }
}

/// Runtime access to an order's position group.
///
/// Use [`OrderPositionAccess::Populated`] when the group is present,
/// [`OrderPositionAccess::Absent`] when it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderPositionAccess {
    /// The position group is present.
    Populated(PopulatedOrderPosition),
    /// The position group is absent.
    Absent,
}

impl HasOrderPositionSide for OrderPositionAccess {
    fn position_side(&self) -> Result<Option<PositionSide>, RequestFieldAccessError> {
        match self {
            Self::Populated(p) => p.position_side(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasReduceOnly for OrderPositionAccess {
    fn reduce_only(&self) -> Result<bool, RequestFieldAccessError> {
        match self {
            Self::Populated(p) => p.reduce_only(),
            Self::Absent => Err(RequestFieldAccessError::new("position.reduce_only")),
        }
    }
}

impl HasClosePosition for OrderPositionAccess {
    fn close_position(&self) -> Result<bool, RequestFieldAccessError> {
        match self {
            Self::Populated(p) => p.close_position(),
            Self::Absent => Err(RequestFieldAccessError::new("position.close_position")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn populated_returns_values() {
        let access = OrderPositionAccess::Populated(PopulatedOrderPosition {
            position_side: Some(PositionSide::Long),
            reduce_only: true,
            close_position: false,
        });
        assert_eq!(access.position_side().unwrap(), Some(PositionSide::Long));
        assert!(access.reduce_only().unwrap());
        assert!(!access.close_position().unwrap());
    }

    #[test]
    fn absent_optional_returns_none() {
        let access = OrderPositionAccess::Absent;
        assert_eq!(access.position_side().unwrap(), None);
    }

    #[test]
    fn absent_required_returns_err() {
        let access = OrderPositionAccess::Absent;
        assert!(access.reduce_only().is_err());
        assert!(access.close_position().is_err());
    }
}
