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

/// Type of fill event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FillType {
    /// Normal trade execution.
    Trade,
    /// Forced liquidation by the venue.
    Liquidation,
    /// Auto-deleveraging event.
    AutoDeleverage,
    /// Settlement at expiry or delivery.
    Settlement,
    /// Funding payment.
    Funding,
}

impl std::fmt::Display for FillType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Trade => "TRADE",
            Self::Liquidation => "LIQUIDATION",
            Self::AutoDeleverage => "AUTO_DELEVERAGE",
            Self::Settlement => "SETTLEMENT",
            Self::Funding => "FUNDING",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::FillType;

    #[test]
    fn display_uses_api_names() {
        assert_eq!(FillType::Trade.to_string(), "TRADE");
        assert_eq!(FillType::Liquidation.to_string(), "LIQUIDATION");
        assert_eq!(FillType::AutoDeleverage.to_string(), "AUTO_DELEVERAGE");
        assert_eq!(FillType::Settlement.to_string(), "SETTLEMENT");
        assert_eq!(FillType::Funding.to_string(), "FUNDING");
    }
}
