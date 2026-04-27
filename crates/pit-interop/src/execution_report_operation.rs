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

//! Runtime wrapper for the execution-report operation group.

use openpit::param::{AccountId, Side};
use openpit::{HasAccountId, HasInstrument, HasSide, Instrument, RequestFieldAccessError};

/// Populated execution-report operation group with individually-optional fields.
///
/// Each field is stored as [`Option`]. A `Some` value returns `Ok`; a `None`
/// required field returns `Err(RequestFieldAccessError)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedExecutionReportOperation {
    /// Trading instrument (`underlying + settlement` asset pair).
    pub instrument: Option<Instrument>,
    /// Account identifier for the report.
    pub account_id: Option<AccountId>,
    /// Execution side.
    pub side: Option<Side>,
}

impl HasInstrument for PopulatedExecutionReportOperation {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.instrument
            .as_ref()
            .ok_or_else(|| RequestFieldAccessError::new("operation.instrument"))
    }
}

impl HasAccountId for PopulatedExecutionReportOperation {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.account_id
            .ok_or_else(|| RequestFieldAccessError::new("operation.account_id"))
    }
}

impl HasSide for PopulatedExecutionReportOperation {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        self.side
            .ok_or_else(|| RequestFieldAccessError::new("operation.side"))
    }
}

/// Runtime access to an execution report's operation group.
///
/// Use [`ExecutionReportOperationAccess::Populated`] when the group is present,
/// [`ExecutionReportOperationAccess::Absent`] when it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionReportOperationAccess {
    /// The operation group is present.
    Populated(PopulatedExecutionReportOperation),
    /// The operation group is absent.
    Absent,
}

impl HasInstrument for ExecutionReportOperationAccess {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.instrument(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.instrument")),
        }
    }
}

impl HasAccountId for ExecutionReportOperationAccess {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.account_id(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.account_id")),
        }
    }
}

impl HasSide for ExecutionReportOperationAccess {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.side(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.side")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{AccountId, Asset, Side};
    use openpit::Instrument;

    fn instrument() -> Instrument {
        Instrument::new(
            Asset::new("AAPL").expect("valid"),
            Asset::new("USD").expect("valid"),
        )
    }

    #[test]
    fn populated_all_some_returns_ok() {
        let access = ExecutionReportOperationAccess::Populated(PopulatedExecutionReportOperation {
            instrument: Some(instrument()),
            account_id: Some(AccountId::from_u64(1)),
            side: Some(Side::Sell),
        });
        assert!(access.instrument().is_ok());
        assert!(access.account_id().is_ok());
        assert_eq!(access.side().unwrap(), Side::Sell);
    }

    #[test]
    fn populated_none_fields_return_err() {
        let access = ExecutionReportOperationAccess::Populated(PopulatedExecutionReportOperation {
            instrument: None,
            account_id: None,
            side: None,
        });
        assert!(access.instrument().is_err());
        assert!(access.account_id().is_err());
        assert!(access.side().is_err());
    }

    #[test]
    fn absent_returns_err_for_all() {
        let access = ExecutionReportOperationAccess::Absent;
        assert!(access.instrument().is_err());
        assert!(access.account_id().is_err());
        assert!(access.side().is_err());
    }
}
