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

/// Type-safe account-group identifier.
///
/// Optimized for speed and costs.
///
/// Every account belongs to the [default account group](DEFAULT_ACCOUNT_GROUP)
/// until it is explicitly assigned to another group. That default is a real,
/// addressable id but a reserved value that no constructor can produce, so any
/// id built here always names a non-default group.
///
/// Two constructors are provided; choose based on how your platform assigns
/// group IDs:
///
/// - [`AccountGroupId::from_u32`]: zero cost, zero collision risk. Prefer this
///   whenever group IDs are already numeric.
/// - [`AccountGroupId::from_str`]: convenience constructor that hashes a string
///   with FNV-1a 32-bit. Collisions are theoretically possible; see
///   [`AccountGroupId::from_str`] for the collision probability table.
///
/// WARNING:
/// Use exactly one constructor family per runtime state:
/// - either only [`AccountGroupId::from_u32`],
/// - or only [`AccountGroupId::from_str`].
///
/// Mixing both families can collapse distinct groups into one key when a
/// hashed string equals a direct numeric ID.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::AccountGroupId;
/// use std::collections::HashMap;
///
/// let numeric = AccountGroupId::from_u32(7)?;
/// let string  = AccountGroupId::from_str("desk-emea")?;
///
/// let mut map: HashMap<AccountGroupId, &str> = HashMap::new();
/// map.insert(numeric, "numeric group");
/// map.insert(string,  "string group");
///
/// assert_eq!(map[&AccountGroupId::from_u32(7)?], "numeric group");
/// assert_eq!(map[&AccountGroupId::from_str("desk-emea")?], "string group");
/// # Ok(())
/// # }
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AccountGroupId(u32);

/// The account group an account belongs to until it is assigned to another.
///
/// This is a real, addressable id - callers may use it to key per-group
/// settings, and a future cascading lookup can fall back from a specific
/// account to its group and finally to this default. It is also reserved: no
/// constructor can produce it ([`AccountGroupId::from_u32`] rejects it, and
/// [`AccountGroupId::from_str`] is guaranteed never to hash to it), and it
/// cannot be passed to [`Accounts::register_group`](crate::Accounts::register_group)
/// or [`Accounts::unregister_group`](crate::Accounts::unregister_group). The
/// only way to name it is this constant.
pub const DEFAULT_ACCOUNT_GROUP: AccountGroupId = AccountGroupId(0);

/// Errors returned by [`AccountGroupId`] constructors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccountGroupIdError {
    /// Account-group identifier string is empty.
    Empty,
    /// The requested identifier equals the reserved
    /// [`DEFAULT_ACCOUNT_GROUP`], which no constructor may produce.
    Reserved,
}

impl Display for AccountGroupIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("account group id string must not be empty"),
            Self::Reserved => {
                formatter.write_str("account group id must not equal the reserved default group")
            }
        }
    }
}

impl std::error::Error for AccountGroupIdError {}

impl AccountGroupId {
    /// The account group an account belongs to until assigned to another.
    ///
    /// Alias for the crate-level [`DEFAULT_ACCOUNT_GROUP`] constant.
    pub const DEFAULT: Self = DEFAULT_ACCOUNT_GROUP;

    /// Constructs an account-group identifier from a 32-bit integer.
    ///
    /// No hashing, no allocation, no collision risk.
    /// Prefer this constructor whenever group IDs are already numeric.
    ///
    /// WARNING:
    /// Do not mix IDs created with this function and IDs created with
    /// [`AccountGroupId::from_str`] in the same runtime state.
    ///
    /// # Errors
    ///
    /// Returns [`AccountGroupIdError::Reserved`] when `value` equals the
    /// reserved [`DEFAULT_ACCOUNT_GROUP`] (`0`).
    pub const fn from_u32(value: u32) -> Result<Self, AccountGroupIdError> {
        if value == DEFAULT_ACCOUNT_GROUP.0 {
            return Err(AccountGroupIdError::Reserved);
        }
        Ok(Self(value))
    }

    /// Constructs an account-group identifier by hashing a string with FNV-1a
    /// 32-bit.
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
    /// 32-bit hash space is approximately `n² / (2 × 2^32)`:
    ///
    /// | Groups | P(at least one collision) |
    /// | ------ | ------------------------- |
    /// | 100    | < 1.2 × 10⁻⁶              |
    /// | 1 000  | < 1.2 × 10⁻⁴              |
    /// | 10 000 | < 1.2 × 10⁻²              |
    ///
    /// If collision risk is unacceptable for your use case, maintain your own
    /// collision-free string→u32 mapping (e.g. a registry or a database
    /// sequence) and pass the resulting integer to [`AccountGroupId::from_u32`].
    ///
    /// WARNING:
    /// Do not mix IDs created with this function and IDs created with
    /// [`AccountGroupId::from_u32`] in the same runtime state.
    ///
    /// The reserved [`DEFAULT_ACCOUNT_GROUP`] value is never produced: a string
    /// whose hash collides with it is remapped to a fixed non-zero value (see
    /// [`from_str`](Self::from_str)'s source), so any non-empty input yields a
    /// non-default group.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openpit::param::AccountGroupId;
    /// use std::collections::HashMap;
    ///
    /// let id = AccountGroupId::from_str("desk-1")?;
    ///
    /// let mut map: HashMap<AccountGroupId, i32> = HashMap::new();
    /// map.insert(id, 42);
    ///
    /// assert_eq!(map[&AccountGroupId::from_str("desk-1")?], 42);
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: impl AsRef<str>) -> Result<Self, AccountGroupIdError> {
        let value = value.as_ref();
        if value.trim().is_empty() {
            return Err(AccountGroupIdError::Empty);
        }
        Ok(Self(non_default_hash(fnv1a_32(value))))
    }

    /// Returns the raw 32-bit integer value.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl Display for AccountGroupId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

fn fnv1a_32(s: &str) -> u32 {
    const OFFSET_BASIS: u32 = 2_166_136_261;
    const PRIME: u32 = 16_777_619;

    let mut hash = OFFSET_BASIS;
    for byte in s.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Remaps the reserved [`DEFAULT_ACCOUNT_GROUP`] value off `0` so a hashed
/// string never names the default group. Any string hashing to `0` is folded
/// to `1`; every other value passes through unchanged. This keeps `from_str`
/// total over non-empty inputs at the cost of an additional (already
/// astronomically unlikely) collision onto group `1`.
const fn non_default_hash(hash: u32) -> u32 {
    if hash == DEFAULT_ACCOUNT_GROUP.0 {
        1
    } else {
        hash
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{non_default_hash, AccountGroupId, AccountGroupIdError, DEFAULT_ACCOUNT_GROUP};

    fn group(value: u32) -> AccountGroupId {
        AccountGroupId::from_u32(value).expect("account group id must be valid")
    }

    #[test]
    fn from_u32_display_shows_integer() {
        assert_eq!(group(7).to_string(), "7");
        assert_eq!(group(42).to_string(), "42");
        assert_eq!(group(u32::MAX).to_string(), u32::MAX.to_string());
    }

    #[test]
    fn from_u32_equality() {
        assert_eq!(group(7), group(7));
        assert_ne!(group(7), group(8));
    }

    #[test]
    fn from_u32_rejects_reserved_default() {
        assert_eq!(
            AccountGroupId::from_u32(0),
            Err(AccountGroupIdError::Reserved)
        );
    }

    #[test]
    fn default_account_group_value_is_zero() {
        assert_eq!(DEFAULT_ACCOUNT_GROUP.as_u32(), 0);
        assert_eq!(AccountGroupId::DEFAULT, DEFAULT_ACCOUNT_GROUP);
    }

    #[test]
    fn from_str_same_string_equal() {
        assert_eq!(
            AccountGroupId::from_str("group-a"),
            AccountGroupId::from_str("group-a")
        );
    }

    #[test]
    fn from_str_different_strings_not_equal() {
        assert_ne!(
            AccountGroupId::from_str("group-a"),
            AccountGroupId::from_str("group-b")
        );
    }

    // from_u32 and from_str of the same numeric string are NOT required to be
    // equal: from_u32 stores the integer directly while from_str hashes the
    // UTF-8 bytes of the decimal representation.
    #[test]
    fn from_u32_and_from_str_of_same_numeric_string_differ() {
        assert_ne!(
            group(42),
            AccountGroupId::from_str("42").expect("account group id must be valid")
        );
    }

    #[test]
    fn hashmap_lookup_with_from_u32() {
        let mut map: HashMap<AccountGroupId, &str> = HashMap::new();
        map.insert(group(100), "alpha");

        assert_eq!(map[&group(100)], "alpha");
    }

    #[test]
    fn hashmap_lookup_with_from_str() {
        let mut map: HashMap<AccountGroupId, &str> = HashMap::new();
        map.insert(
            AccountGroupId::from_str("beta").expect("account group id must be valid"),
            "beta-value",
        );

        assert_eq!(
            map[&AccountGroupId::from_str("beta").expect("account group id must be valid")],
            "beta-value"
        );
    }

    #[test]
    fn from_str_rejects_empty_or_whitespace() {
        assert_eq!(
            AccountGroupId::from_str(""),
            Err(AccountGroupIdError::Empty)
        );
        assert_eq!(
            AccountGroupId::from_str("   "),
            Err(AccountGroupIdError::Empty)
        );
    }

    // The reservation must never surface as an error from from_str: every
    // non-empty input maps to some non-default group.
    #[test]
    fn from_str_never_yields_reserved_default() {
        for raw in ["", "a", "desk-emea", "0", "42", " spaced "] {
            if let Ok(id) = AccountGroupId::from_str(raw) {
                assert_ne!(id, DEFAULT_ACCOUNT_GROUP);
            }
        }
    }

    // Directly exercise the remap branch: a hash colliding with the reserved
    // value is folded to a fixed non-zero group, everything else passes through.
    #[test]
    fn non_default_hash_remaps_reserved_value_only() {
        assert_eq!(non_default_hash(0), 1);
        assert_eq!(non_default_hash(1), 1);
        assert_eq!(non_default_hash(42), 42);
        assert_eq!(non_default_hash(u32::MAX), u32::MAX);
    }

    #[test]
    fn account_group_id_error_display_is_stable() {
        assert_eq!(
            AccountGroupIdError::Empty.to_string(),
            "account group id string must not be empty"
        );
        assert_eq!(
            AccountGroupIdError::Reserved.to_string(),
            "account group id must not equal the reserved default group"
        );
    }

    #[test]
    fn as_u32_returns_inner_value() {
        assert_eq!(group(99).as_u32(), 99);
        assert_eq!(group(7).as_u32(), 7);
        assert_eq!(group(u32::MAX).as_u32(), u32::MAX);
    }

    #[test]
    fn from_str_is_deterministic() {
        let first = AccountGroupId::from_str("deterministic")
            .expect("account group id must be valid")
            .as_u32();
        let second = AccountGroupId::from_str("deterministic")
            .expect("account group id must be valid")
            .as_u32();
        assert_eq!(first, second);
    }
}
