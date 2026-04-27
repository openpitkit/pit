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

//! Runtime wrapper for the execution-report financial-impact group.

use openpit::param::{Fee, Pnl};
use openpit::{HasFee, HasPnl, RequestFieldAccessError};

/// Populated financial-impact group with individually-optional fields.
///
/// Each field is stored as [`Option`]. A `Some` value returns `Ok`; a `None`
/// required field returns `Err(RequestFieldAccessError)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedFinancialImpact {
    /// Realized P&L contributed by this execution report.
    pub pnl: Option<Pnl>,
    /// Fee or rebate associated with this execution report.
    pub fee: Option<Fee>,
}

impl HasPnl for PopulatedFinancialImpact {
    fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
        self.pnl
            .ok_or_else(|| RequestFieldAccessError::new("financial_impact.pnl"))
    }
}

impl HasFee for PopulatedFinancialImpact {
    fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
        self.fee
            .ok_or_else(|| RequestFieldAccessError::new("financial_impact.fee"))
    }
}

/// Runtime access to an execution report's financial-impact group.
///
/// Use [`FinancialImpactAccess::Populated`] when the group is present,
/// [`FinancialImpactAccess::Absent`] when it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FinancialImpactAccess {
    /// The financial-impact group is present.
    Populated(PopulatedFinancialImpact),
    /// The financial-impact group is absent.
    Absent,
}

impl HasPnl for FinancialImpactAccess {
    fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
        match self {
            Self::Populated(fi) => fi.pnl(),
            Self::Absent => Err(RequestFieldAccessError::new("financial_impact.pnl")),
        }
    }
}

impl HasFee for FinancialImpactAccess {
    fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
        match self {
            Self::Populated(fi) => fi.fee(),
            Self::Absent => Err(RequestFieldAccessError::new("financial_impact.fee")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{Fee, Pnl};

    #[test]
    fn populated_some_returns_ok() {
        let access = FinancialImpactAccess::Populated(PopulatedFinancialImpact {
            pnl: Some(Pnl::from_str("100").expect("valid")),
            fee: Some(Fee::from_str("1").expect("valid")),
        });
        assert!(access.pnl().is_ok());
        assert!(access.fee().is_ok());
    }

    #[test]
    fn populated_none_returns_err() {
        let access = FinancialImpactAccess::Populated(PopulatedFinancialImpact {
            pnl: None,
            fee: None,
        });
        assert!(access.pnl().is_err());
        assert!(access.fee().is_err());
    }

    #[test]
    fn absent_returns_err() {
        let access = FinancialImpactAccess::Absent;
        assert!(access.pnl().is_err());
        assert!(access.fee().is_err());
    }
}
