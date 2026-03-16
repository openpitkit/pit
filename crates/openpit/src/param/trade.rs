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

use crate::param::{Price, Quantity};

/// Represents trade.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trade {
    /// Actual trade execution price.
    pub price: Price,
    /// Executed trade quantity.
    pub quantity: Quantity,
}

impl std::fmt::Display for Trade {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} @ {}", self.quantity, self.price)
    }
}

#[cfg(test)]
mod tests {
    use crate::param::{Price, Quantity};

    use super::Trade;

    #[test]
    fn display_formats_quantity_at_price() {
        let trade = Trade {
            price: Price::from_str("185.5").expect("must be valid"),
            quantity: Quantity::from_str("10").expect("must be valid"),
        };
        assert_eq!(trade.to_string(), "10 @ 185.5");
    }
}
