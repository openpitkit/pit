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

/// Represents the side of a trade or order.
///
/// This enum is `Copy` and intended to behave like a small value type.
/// It does not encode numeric meaning implicitly — use methods like
/// [`Side::sign`] if a signed representation is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Buy side (long direction).
    Buy,

    /// Sell side (short direction).
    Sell,
}

impl std::fmt::Display for Side {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        })
    }
}

impl Side {
    /// Returns `true` if the side is [`Side::Buy`].
    #[inline]
    pub fn is_buy(self) -> bool {
        match self {
            Side::Buy => true,
            Side::Sell => false,
        }
    }

    /// Returns `true` if the side is [`Side::Sell`].
    #[inline]
    pub fn is_sell(self) -> bool {
        match self {
            Side::Buy => false,
            Side::Sell => true,
        }
    }

    /// Returns the opposite trading side.
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Side;
    ///
    /// assert_eq!(Side::Buy.opposite(), Side::Sell);
    /// assert_eq!(Side::Sell.opposite(), Side::Buy);
    /// ```
    #[inline]
    pub fn opposite(self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }

    /// Returns a signed representation of the side.
    ///
    /// `+1` for [`Side::Buy`], `-1` for [`Side::Sell`].
    ///
    /// Intended for financial calculations (e.g., signed quantity or exposure).
    #[inline]
    pub fn sign(self) -> i8 {
        match self {
            Side::Buy => 1,
            Side::Sell => -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Side;

    #[test]
    fn side_predicates_work() {
        assert!(Side::Buy.is_buy());
        assert!(!Side::Buy.is_sell());
        assert!(Side::Sell.is_sell());
        assert!(!Side::Sell.is_buy());
    }

    #[test]
    fn opposite_returns_other_side() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
    }

    #[test]
    fn sign_matches_direction() {
        assert_eq!(Side::Buy.sign(), 1);
        assert_eq!(Side::Sell.sign(), -1);
    }

    #[test]
    fn display_uses_api_names() {
        assert_eq!(Side::Buy.to_string(), "BUY");
        assert_eq!(Side::Sell.to_string(), "SELL");
    }
}
