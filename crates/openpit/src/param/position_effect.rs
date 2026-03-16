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

/// Position effect of a fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PositionEffect {
    /// Opens a new position or increases an existing one.
    Open,
    /// Reduces or closes an existing position.
    Close,
}

impl std::fmt::Display for PositionEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Open => "OPEN",
            Self::Close => "CLOSE",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::PositionEffect;

    #[test]
    fn display_uses_api_names() {
        assert_eq!(PositionEffect::Open.to_string(), "OPEN");
        assert_eq!(PositionEffect::Close.to_string(), "CLOSE");
    }
}
