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

use std::fmt::{Display, Formatter};

use crate::param::PositionSize;

/// Error returned by arithmetic-only adjustment operations.
///
/// Used by force-write operations that do not enforce non-negative
/// invariants and can only fail on decimal overflow. See
/// [`super::Holdings::apply_adjustment`],
/// [`super::Holdings::apply_fill_outflow`] and
/// [`super::Holdings::apply_fill_inflow`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdjustmentOverflowError {
    /// The arithmetic operation overflowed the underlying decimal range.
    ArithmeticOverflow,
}

impl Display for AdjustmentOverflowError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArithmeticOverflow => write!(formatter, "arithmetic overflow"),
        }
    }
}

impl std::error::Error for AdjustmentOverflowError {}

/// Error returned by [`super::Holdings::try_hold`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HoldError {
    /// `amount` exceeded current `available`.
    InsufficientAvailable {
        /// Current available amount.
        available: PositionSize,
        /// Requested hold amount.
        requested: PositionSize,
    },
    /// Underlying decimal arithmetic overflowed.
    ArithmeticOverflow,
}

impl Display for HoldError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientAvailable {
                available,
                requested,
            } => write!(
                formatter,
                "insufficient available amount: available {available}, \
                 requested {requested}"
            ),
            Self::ArithmeticOverflow => write!(formatter, "arithmetic overflow"),
        }
    }
}

impl std::error::Error for HoldError {}
