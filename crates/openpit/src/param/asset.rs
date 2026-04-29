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

use smol_str::SmolStr;
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

/// Asset or currency identifier such as `USD`, `SPX`, `AAPL`, or `BTC`.
///
/// Lightweight wrapper providing type safety for asset codes.
/// Uses [`SmolStr`](https://docs.rs/smol_str) for efficient inline storage of short strings.
///
/// Empty and whitespace-only values are rejected by [`Asset::new`], while the
/// full symbol semantics remain caller-defined.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::Asset;
///
/// let usd = Asset::new("USD")?;
/// assert_eq!(&*usd, "USD");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Asset(SmolStr);

/// Errors returned by [`Asset`] constructors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetError {
    /// Asset identifier is empty.
    Empty,
}

impl Display for AssetError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("asset must not be empty"),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<AssetError> for crate::param::Error {
    fn from(error: AssetError) -> Self {
        match error {
            AssetError::Empty => Self::AssetEmpty,
        }
    }
}

impl Asset {
    /// Creates a validated asset identifier.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::Empty`] when `value` is empty or contains only
    /// whitespace.
    pub fn new(value: impl AsRef<str>) -> Result<Self, AssetError> {
        let value = value.as_ref();
        if value.trim().is_empty() {
            return Err(AssetError::Empty);
        }

        Ok(Self(SmolStr::new(value)))
    }
}

impl TryFrom<&str> for Asset {
    type Error = AssetError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for Asset {
    type Error = AssetError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl AsRef<str> for Asset {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Deref for Asset {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl Display for Asset {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use super::{Asset, AssetError};

    #[test]
    fn new_and_deref_work() {
        let from_str = Asset::new("SPX").expect("asset code must be valid");
        let from_string = Asset::new(String::from("SPX")).expect("asset code must be valid");

        assert_eq!(&*from_str, "SPX");
        assert_eq!(from_str.as_ref(), "SPX");
        assert_eq!(from_str, from_string);
    }

    #[test]
    fn new_rejects_empty_and_whitespace() {
        assert_eq!(Asset::new(""), Err(AssetError::Empty));
        assert_eq!(Asset::new("   "), Err(AssetError::Empty));
    }

    #[test]
    fn display_outputs_inner_value() {
        let asset = Asset::new("SPX").expect("asset code must be valid");
        assert_eq!(format!("{asset}"), "SPX");
    }

    #[test]
    fn try_from_validates_asset_code() {
        let from_str = Asset::try_from("USD").expect("asset code must be valid");
        let from_string = Asset::try_from(String::from("EUR")).expect("asset code must be valid");

        assert_eq!(from_str.as_ref(), "USD");
        assert_eq!(from_string.as_ref(), "EUR");
        assert_eq!(Asset::try_from(""), Err(AssetError::Empty));
        assert_eq!(Asset::try_from(String::from(" ")), Err(AssetError::Empty));
    }

    #[test]
    fn asset_error_display_is_stable() {
        assert_eq!(AssetError::Empty.to_string(), "asset must not be empty");
    }
}
