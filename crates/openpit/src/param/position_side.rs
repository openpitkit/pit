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

/// Represents an open position direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PositionSide {
    /// Long position direction.
    Long,

    /// Short position direction.
    Short,
}

impl PositionSide {
    /// Returns `true` when the side is [`PositionSide::Long`].
    #[inline]
    pub fn is_long(self) -> bool {
        match self {
            Self::Long => true,
            Self::Short => false,
        }
    }

    /// Returns `true` when the side is [`PositionSide::Short`].
    #[inline]
    pub fn is_short(self) -> bool {
        match self {
            Self::Long => false,
            Self::Short => true,
        }
    }

    /// Returns the opposite position side.
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::PositionSide;
    ///
    /// assert_eq!(PositionSide::Long.opposite(), PositionSide::Short);
    /// assert_eq!(PositionSide::Short.opposite(), PositionSide::Long);
    /// ```
    #[inline]
    pub fn opposite(self) -> Self {
        match self {
            Self::Long => Self::Short,
            Self::Short => Self::Long,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PositionSide;

    #[test]
    fn side_predicates_work() {
        assert!(PositionSide::Long.is_long());
        assert!(!PositionSide::Long.is_short());
        assert!(PositionSide::Short.is_short());
        assert!(!PositionSide::Short.is_long());
    }

    #[test]
    fn opposite_returns_other_side() {
        assert_eq!(PositionSide::Long.opposite(), PositionSide::Short);
        assert_eq!(PositionSide::Short.opposite(), PositionSide::Long);
    }
}
