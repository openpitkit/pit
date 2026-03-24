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

//! Traits for binding-level types to report which Optional groups are populated.

/// Implemented by binding-level order types so that guarded policies can
/// check at runtime which Optional groups are present before dispatching
/// to the inner policy.
pub trait OrderGroupAccess {
    /// Returns `true` when the order carries the operation group
    /// (instrument, side, trade_amount, price).
    fn has_operation(&self) -> bool;
}

/// Implemented by binding-level execution-report types so that guarded
/// policies can check at runtime which Optional groups are present.
pub trait ExecutionReportGroupAccess {
    /// Returns `true` when the report carries the operation group
    /// (instrument, side).
    fn has_operation(&self) -> bool;
    /// Returns `true` when the report carries the financial-impact group
    /// (pnl, fee).
    fn has_financial_impact(&self) -> bool;
}

/// Implemented by binding-level account-adjustment types so that guarded
/// account-adjustment policies can check at runtime which Optional groups are
/// present.
pub trait AccountAdjustmentGroupAccess {
    /// Returns `true` when the adjustment carries the operation group.
    fn has_operation(&self) -> bool;
    /// Returns `true` when the adjustment carries the amount group.
    fn has_amount(&self) -> bool;
    /// Returns `true` when the adjustment carries the bounds group.
    fn has_bounds(&self) -> bool;
}

impl<T> OrderGroupAccess for T
where
    T: std::ops::Deref,
    T::Target: OrderGroupAccess,
{
    fn has_operation(&self) -> bool {
        self.deref().has_operation()
    }
}

impl<T> ExecutionReportGroupAccess for T
where
    T: std::ops::Deref,
    T::Target: ExecutionReportGroupAccess,
{
    fn has_operation(&self) -> bool {
        self.deref().has_operation()
    }
    fn has_financial_impact(&self) -> bool {
        self.deref().has_financial_impact()
    }
}

impl<T> AccountAdjustmentGroupAccess for T
where
    T: std::ops::Deref,
    T::Target: AccountAdjustmentGroupAccess,
{
    fn has_operation(&self) -> bool {
        self.deref().has_operation()
    }
    fn has_amount(&self) -> bool {
        self.deref().has_amount()
    }
    fn has_bounds(&self) -> bool {
        self.deref().has_bounds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeOrder {
        has_op: bool,
    }

    impl OrderGroupAccess for FakeOrder {
        fn has_operation(&self) -> bool {
            self.has_op
        }
    }

    struct FakeReport {
        has_op: bool,
        has_fi: bool,
    }

    impl ExecutionReportGroupAccess for FakeReport {
        fn has_operation(&self) -> bool {
            self.has_op
        }
        fn has_financial_impact(&self) -> bool {
            self.has_fi
        }
    }

    struct FakeAdjustment {
        has_op: bool,
        has_amount: bool,
        has_bounds: bool,
    }

    impl AccountAdjustmentGroupAccess for FakeAdjustment {
        fn has_operation(&self) -> bool {
            self.has_op
        }
        fn has_amount(&self) -> bool {
            self.has_amount
        }
        fn has_bounds(&self) -> bool {
            self.has_bounds
        }
    }

    #[test]
    fn order_group_access_via_box() {
        let present: Box<FakeOrder> = Box::new(FakeOrder { has_op: true });
        let absent: Box<FakeOrder> = Box::new(FakeOrder { has_op: false });
        assert!(present.has_operation());
        assert!(!absent.has_operation());
    }

    #[test]
    fn execution_report_group_access_via_box() {
        let full: Box<FakeReport> = Box::new(FakeReport {
            has_op: true,
            has_fi: true,
        });
        let no_op: Box<FakeReport> = Box::new(FakeReport {
            has_op: false,
            has_fi: true,
        });
        let no_fi: Box<FakeReport> = Box::new(FakeReport {
            has_op: true,
            has_fi: false,
        });
        assert!(full.has_operation());
        assert!(full.has_financial_impact());
        assert!(!no_op.has_operation());
        assert!(no_op.has_financial_impact());
        assert!(no_fi.has_operation());
        assert!(!no_fi.has_financial_impact());
    }

    #[test]
    fn account_adjustment_group_access_via_box() {
        let full: Box<FakeAdjustment> = Box::new(FakeAdjustment {
            has_op: true,
            has_amount: true,
            has_bounds: true,
        });
        let missing_amount: Box<FakeAdjustment> = Box::new(FakeAdjustment {
            has_op: true,
            has_amount: false,
            has_bounds: true,
        });

        assert!(full.has_operation());
        assert!(full.has_amount());
        assert!(full.has_bounds());
        assert!(missing_amount.has_operation());
        assert!(!missing_amount.has_amount());
        assert!(missing_amount.has_bounds());
    }
}
