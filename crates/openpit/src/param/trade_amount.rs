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

use crate::param::{Quantity, Volume};

/// Represents the amount of a trade.
///
/// The amount can be specified in instrument units or in settlement units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TradeAmount {
    /// Requested amount in instrument units.
    Quantity(Quantity),
    /// Requested amount in settlement notional units.
    Volume(Volume),
}

impl std::fmt::Display for TradeAmount {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quantity(qty) => write!(formatter, "{qty}"),
            Self::Volume(vol) => write!(formatter, "{vol}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::param::{Quantity, Volume};

    use super::TradeAmount;

    #[test]
    fn display_quantity_variant() {
        let amount = TradeAmount::Quantity(Quantity::from_str("10").expect("must be valid"));
        assert_eq!(amount.to_string(), "10");
    }

    #[test]
    fn display_volume_variant() {
        let amount = TradeAmount::Volume(Volume::from_str("500.5").expect("must be valid"));
        assert_eq!(amount.to_string(), "500.5");
    }
}
