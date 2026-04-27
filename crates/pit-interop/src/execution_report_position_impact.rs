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

//! Runtime wrapper for the execution-report position-impact group.

use openpit::param::{PositionEffect, PositionSide};
use openpit::{
    HasExecutionReportPositionEffect, HasExecutionReportPositionSide, RequestFieldAccessError,
};

/// Populated execution-report position-impact group.
///
/// Both fields are optional within the group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedExecutionReportPositionImpact {
    /// Effect of this execution on the position lifecycle.
    pub position_effect: Option<PositionEffect>,
    /// Side of the resulting position.
    pub position_side: Option<PositionSide>,
}

impl HasExecutionReportPositionEffect for PopulatedExecutionReportPositionImpact {
    fn position_effect(&self) -> Result<Option<PositionEffect>, RequestFieldAccessError> {
        Ok(self.position_effect)
    }
}

impl HasExecutionReportPositionSide for PopulatedExecutionReportPositionImpact {
    fn position_side(&self) -> Result<Option<PositionSide>, RequestFieldAccessError> {
        Ok(self.position_side)
    }
}

/// Runtime access to an execution report's position-impact group.
///
/// Use [`ExecutionReportPositionImpactAccess::Populated`] when the group is
/// present, [`ExecutionReportPositionImpactAccess::Absent`] when it is not.
///
/// Both `position_effect` and `position_side` are optional fields; the absent
/// group also returns `Ok(None)` for both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionReportPositionImpactAccess {
    /// The position-impact group is present.
    Populated(PopulatedExecutionReportPositionImpact),
    /// The position-impact group is absent.
    Absent,
}

impl HasExecutionReportPositionEffect for ExecutionReportPositionImpactAccess {
    fn position_effect(&self) -> Result<Option<PositionEffect>, RequestFieldAccessError> {
        match self {
            Self::Populated(pi) => pi.position_effect(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasExecutionReportPositionSide for ExecutionReportPositionImpactAccess {
    fn position_side(&self) -> Result<Option<PositionSide>, RequestFieldAccessError> {
        match self {
            Self::Populated(pi) => pi.position_side(),
            Self::Absent => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{PositionEffect, PositionSide};

    #[test]
    fn populated_returns_values() {
        let access = ExecutionReportPositionImpactAccess::Populated(
            PopulatedExecutionReportPositionImpact {
                position_effect: Some(PositionEffect::Open),
                position_side: Some(PositionSide::Long),
            },
        );
        assert_eq!(
            access.position_effect().unwrap(),
            Some(PositionEffect::Open)
        );
        assert_eq!(access.position_side().unwrap(), Some(PositionSide::Long));
    }

    #[test]
    fn absent_returns_none_for_both() {
        let access = ExecutionReportPositionImpactAccess::Absent;
        assert_eq!(access.position_effect().unwrap(), None);
        assert_eq!(access.position_side().unwrap(), None);
    }
}
