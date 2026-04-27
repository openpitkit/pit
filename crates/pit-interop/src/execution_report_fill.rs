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

//! Runtime wrapper for the execution-report fill group.

use openpit::param::{Quantity, Trade};
use openpit::pretrade::PreTradeLock;
use openpit::{
    HasExecutionReportIsTerminal, HasExecutionReportLastTrade, HasLeavesQuantity, HasLock,
    RequestFieldAccessError,
};

/// Populated execution-report fill group.
///
/// The boolean `is_terminal` is always set when the group is present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedExecutionReportFill {
    /// Actual execution trade, or `None` if not yet filled.
    pub last_trade: Option<Trade>,
    /// Remaining order quantity after this fill.
    pub leaves_quantity: Quantity,
    /// Order lock payload.
    pub lock: PreTradeLock,
    /// Whether this report closes the order's report stream.
    pub is_terminal: bool,
}

impl HasExecutionReportLastTrade for PopulatedExecutionReportFill {
    fn last_trade(&self) -> Result<Option<Trade>, RequestFieldAccessError> {
        Ok(self.last_trade)
    }
}

impl HasExecutionReportIsTerminal for PopulatedExecutionReportFill {
    fn is_terminal(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.is_terminal)
    }
}

impl HasLeavesQuantity for PopulatedExecutionReportFill {
    fn leaves_quantity(&self) -> Result<Quantity, RequestFieldAccessError> {
        Ok(self.leaves_quantity)
    }
}

impl HasLock for PopulatedExecutionReportFill {
    fn lock(&self) -> Result<PreTradeLock, RequestFieldAccessError> {
        Ok(self.lock)
    }
}

/// Runtime access to an execution report's fill group.
///
/// Use [`ExecutionReportFillAccess::Populated`] when the group is present,
/// [`ExecutionReportFillAccess::Absent`] when it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionReportFillAccess {
    /// The fill group is present.
    Populated(PopulatedExecutionReportFill),
    /// The fill group is absent.
    Absent,
}

impl HasExecutionReportLastTrade for ExecutionReportFillAccess {
    fn last_trade(&self) -> Result<Option<Trade>, RequestFieldAccessError> {
        match self {
            Self::Populated(f) => f.last_trade(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasExecutionReportIsTerminal for ExecutionReportFillAccess {
    fn is_terminal(&self) -> Result<bool, RequestFieldAccessError> {
        match self {
            Self::Populated(f) => f.is_terminal(),
            Self::Absent => Err(RequestFieldAccessError::new("fill.is_terminal")),
        }
    }
}

impl HasLeavesQuantity for ExecutionReportFillAccess {
    fn leaves_quantity(&self) -> Result<Quantity, RequestFieldAccessError> {
        match self {
            Self::Populated(f) => f.leaves_quantity(),
            Self::Absent => Err(RequestFieldAccessError::new("fill.leaves_quantity")),
        }
    }
}

impl HasLock for ExecutionReportFillAccess {
    fn lock(&self) -> Result<PreTradeLock, RequestFieldAccessError> {
        match self {
            Self::Populated(f) => f.lock(),
            Self::Absent => Err(RequestFieldAccessError::new("fill.lock")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn populated_returns_values() {
        let access = ExecutionReportFillAccess::Populated(PopulatedExecutionReportFill {
            last_trade: None,
            leaves_quantity: Quantity::ZERO,
            lock: PreTradeLock::new(None),
            is_terminal: true,
        });
        assert_eq!(access.last_trade().unwrap(), None);
        assert_eq!(access.leaves_quantity().unwrap(), Quantity::ZERO);
        assert_eq!(access.lock().unwrap(), PreTradeLock::new(None));
        assert!(access.is_terminal().unwrap());
    }

    #[test]
    fn absent_last_trade_returns_none() {
        assert_eq!(
            ExecutionReportFillAccess::Absent.last_trade().unwrap(),
            None
        );
    }

    #[test]
    fn absent_is_terminal_returns_err() {
        assert!(ExecutionReportFillAccess::Absent.is_terminal().is_err());
    }

    #[test]
    fn absent_leaves_quantity_returns_err() {
        assert!(ExecutionReportFillAccess::Absent.leaves_quantity().is_err());
    }

    #[test]
    fn absent_lock_returns_err() {
        assert!(ExecutionReportFillAccess::Absent.lock().is_err());
    }
}
