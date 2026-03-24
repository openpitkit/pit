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

use crate::param::PositionSize;

/// Signed balance/position adjustment payload; delta applies change, absolute sets target value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdjustmentAmount {
    /// Apply signed difference.
    Delta(PositionSize),
    /// Set resulting value.
    Absolute(PositionSize),
}

impl std::fmt::Display for AdjustmentAmount {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Delta(delta) => write!(formatter, "delta: {delta}"),
            Self::Absolute(sz) => write!(formatter, "sz: {sz}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AdjustmentAmount;
    use crate::param::PositionSize;

    #[test]
    fn delta_holds_signed_difference() {
        let value = PositionSize::from_str("-3.5").expect("must be valid");
        assert_eq!(
            AdjustmentAmount::Delta(value),
            AdjustmentAmount::Delta(value)
        );
    }

    #[test]
    fn absolute_holds_resulting_value() {
        let value = PositionSize::from_str("12").expect("must be valid");
        assert_eq!(
            AdjustmentAmount::Absolute(value),
            AdjustmentAmount::Absolute(value)
        );
    }

    #[test]
    fn display_delta_variant() {
        let value = PositionSize::from_str("-3.5").expect("must be valid");
        assert_eq!(AdjustmentAmount::Delta(value).to_string(), "delta: -3.5");
    }

    #[test]
    fn display_absolute_variant() {
        let value = PositionSize::from_str("12").expect("must be valid");
        assert_eq!(AdjustmentAmount::Absolute(value).to_string(), "sz: 12");
    }
}
