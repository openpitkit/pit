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

/// Type-safe account identifier.
///
/// Optimized for speed and costs.
///
/// Two constructors are provided; choose based on how your venue assigns IDs:
///
/// - [`AccountId::from_u64`]: zero cost, zero collision risk. Prefer this
///   whenever the broker or venue assigns numeric account IDs.
/// - [`AccountId::from_str`]: convenience constructor that hashes a string
///   with FNV-1a 64-bit. Collisions are theoretically possible; see
///   [`AccountId::from_str`] for the collision probability table.
///
/// WARNING:
/// Use exactly one constructor family per runtime state:
/// - either only [`AccountId::from_u64`],
/// - or only [`AccountId::from_str`].
///
/// Mixing both families can collapse distinct accounts into one key when a
/// hashed string equals a direct numeric ID.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::AccountId;
/// use std::collections::HashMap;
///
/// let numeric = AccountId::from_u64(99224416);
/// let string  = AccountId::from_str("my-account")?;
///
/// let mut map: HashMap<AccountId, &str> = HashMap::new();
/// map.insert(numeric, "numeric account");
/// map.insert(string,  "string account");
///
/// assert_eq!(map[&AccountId::from_u64(99224416)], "numeric account");
/// assert_eq!(map[&AccountId::from_str("my-account")?], "string account");
/// # Ok(())
/// # }
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AccountId(u64);

/// Errors returned by [`AccountId`] constructors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccountIdError {
    /// Account identifier string is empty.
    Empty,
}

impl Display for AccountIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("account id string must not be empty"),
        }
    }
}

impl std::error::Error for AccountIdError {}

impl From<AccountIdError> for crate::param::Error {
    fn from(error: AccountIdError) -> Self {
        match error {
            AccountIdError::Empty => Self::AccountIdEmpty,
        }
    }
}

impl AccountId {
    /// Constructs an account identifier from a 64-bit integer.
    ///
    /// No hashing, no allocation, no collision risk.
    /// Prefer this constructor whenever the broker or venue assigns
    /// numeric account IDs.
    ///
    /// WARNING:
    /// Do not mix IDs created with this function and IDs created with
    /// [`AccountId::from_str`] in the same runtime state.
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Constructs an account identifier by hashing a string with FNV-1a 64-bit.
    ///
    /// Note: this method is intentionally *not* an implementation of
    /// [`std::str::FromStr`] — the caller must consciously choose `from_str`
    /// and read its collision warning. Implicit `From<&str>` / `From<String>`
    /// conversions are not provided for the same reason.
    ///
    /// See <http://www.isthe.com/chongo/tech/comp/fnv/> for the algorithm
    /// specification.
    ///
    /// Hash collisions are possible. By the birthday paradox, the probability
    /// of at least one collision among `n` distinct string identifiers in a
    /// 64-bit hash space is approximately `n² / (2 × 2^64)`:
    ///
    /// | Accounts  | P(at least one collision) |
    /// | --------- | ------------------------- |
    /// | 1 000     | < 3 × 10⁻¹⁴               |
    /// | 10 000    | < 3 × 10⁻¹²               |
    /// | 100 000   | < 3 × 10⁻¹⁰               |
    /// | 1 000 000 | < 3 × 10⁻⁸                |
    ///
    /// If collision risk is unacceptable for your use case, maintain your own
    /// collision-free string→u64 mapping (e.g. a registry or a database
    /// sequence) and pass the resulting integer to [`AccountId::from_u64`].
    ///
    /// WARNING:
    /// Do not mix IDs created with this function and IDs created with
    /// [`AccountId::from_u64`] in the same runtime state.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openpit::param::AccountId;
    /// use std::collections::HashMap;
    ///
    /// let id = AccountId::from_str("trading-account-1")?;
    ///
    /// let mut map: HashMap<AccountId, i32> = HashMap::new();
    /// map.insert(id, 42);
    ///
    /// assert_eq!(map[&AccountId::from_str("trading-account-1")?], 42);
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: impl AsRef<str>) -> Result<Self, AccountIdError> {
        let value = value.as_ref();
        if value.trim().is_empty() {
            return Err(AccountIdError::Empty);
        }
        Ok(Self(fnv1a_64(value)))
    }

    /// Returns the raw 64-bit integer value.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for AccountId {
    fn from(value: u64) -> Self {
        Self::from_u64(value)
    }
}

impl Display for AccountId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

fn fnv1a_64(s: &str) -> u64 {
    const OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;

    let mut hash = OFFSET_BASIS;
    for byte in s.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{AccountId, AccountIdError};

    #[test]
    fn from_u64_display_shows_integer() {
        assert_eq!(AccountId::from_u64(99224416).to_string(), "99224416");
        assert_eq!(AccountId::from_u64(42).to_string(), "42");
        assert_eq!(
            AccountId::from_u64(u64::MAX).to_string(),
            u64::MAX.to_string()
        );
    }

    #[test]
    fn from_u64_equality() {
        assert_eq!(AccountId::from_u64(7), AccountId::from_u64(7));
        assert_ne!(AccountId::from_u64(7), AccountId::from_u64(8));
    }

    #[test]
    fn from_str_same_string_equal() {
        assert_eq!(
            AccountId::from_str("account-a"),
            AccountId::from_str("account-a")
        );
    }

    #[test]
    fn from_str_different_strings_not_equal() {
        assert_ne!(
            AccountId::from_str("account-a"),
            AccountId::from_str("account-b")
        );
    }

    // from_u64 and from_str of the same numeric string are NOT required to be
    // equal: from_u64 stores the integer directly while from_str hashes the
    // UTF-8 bytes of the decimal representation.
    #[test]
    fn from_u64_and_from_str_of_same_numeric_string_differ() {
        assert_ne!(
            AccountId::from_u64(42),
            AccountId::from_str("42").expect("account id must be valid")
        );
    }

    #[test]
    fn hashmap_lookup_with_from_u64() {
        let mut map: HashMap<AccountId, &str> = HashMap::new();
        map.insert(AccountId::from_u64(100), "alpha");

        assert_eq!(map[&AccountId::from_u64(100)], "alpha");
    }

    #[test]
    fn hashmap_lookup_with_from_str() {
        let mut map: HashMap<AccountId, &str> = HashMap::new();
        map.insert(
            AccountId::from_str("beta").expect("account id must be valid"),
            "beta-value",
        );

        assert_eq!(
            map[&AccountId::from_str("beta").expect("account id must be valid")],
            "beta-value"
        );
    }

    #[test]
    fn from_str_rejects_empty_or_whitespace() {
        assert_eq!(AccountId::from_str(""), Err(AccountIdError::Empty));
        assert_eq!(AccountId::from_str("   "), Err(AccountIdError::Empty));
    }

    #[test]
    fn account_id_error_display_is_stable() {
        assert_eq!(
            AccountIdError::Empty.to_string(),
            "account id string must not be empty"
        );
    }

    #[test]
    fn as_u64_returns_inner_value() {
        assert_eq!(AccountId::from_u64(99).as_u64(), 99);
        assert_eq!(AccountId::from_u64(99224416).as_u64(), 99224416);
        assert_eq!(AccountId::from_u64(u64::MAX).as_u64(), u64::MAX);
    }

    #[test]
    fn from_u64_trait_delegates_to_constructor() {
        let via_trait: AccountId = AccountId::from(42u64);
        let via_constructor = AccountId::from_u64(42);
        assert_eq!(via_trait, via_constructor);
    }
}
