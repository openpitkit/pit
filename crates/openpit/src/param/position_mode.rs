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

/// Position accounting mode for derivatives-like position adjustments.
///
/// This mode controls how resulting exposure is interpreted by the host
/// integration after an adjustment:
///
/// - netting mode tracks one aggregated position per instrument;
/// - hedged mode tracks independent long/short legs for the same instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PositionMode {
    /// One net position per instrument.
    ///
    /// Positive/negative resulting size is interpreted as directional exposure
    /// in a single aggregate bucket.
    Netting,
    /// Independent long and short legs per instrument.
    ///
    /// The resulting exposure is interpreted as two separate hedge-mode legs
    /// instead of one aggregated net position.
    Hedged,
}

impl std::fmt::Display for PositionMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Netting => "netting",
            Self::Hedged => "hedged",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::PositionMode;

    #[test]
    fn supports_netting_mode() {
        assert_eq!(PositionMode::Netting, PositionMode::Netting);
    }

    #[test]
    fn supports_hedged_mode() {
        assert_eq!(PositionMode::Hedged, PositionMode::Hedged);
    }

    #[test]
    fn display_netting_mode() {
        assert_eq!(PositionMode::Netting.to_string(), "netting");
    }

    #[test]
    fn display_hedged_mode() {
        assert_eq!(PositionMode::Hedged.to_string(), "hedged");
    }
}
