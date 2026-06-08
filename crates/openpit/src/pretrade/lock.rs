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

use core::slice;

use smallvec::SmallVec;

use crate::core::{PolicyGroupId, DEFAULT_POLICY_GROUP_ID};
use crate::param::Price;

/// Stable lock context captured during pre-trade reservation.
///
/// `PreTradeLock` is not just a copy of request input. It is the serialized
/// context of what the engine actually reserved and how that reservation must
/// later be reconciled.
///
/// A lock groups `(group_id, price)` records under their group identifier.
/// Pushing several prices for the same group accumulates them in insertion
/// order; the resulting list is what [`Self::prices_of`] returns.
///
/// The lock context must travel together with the order lifecycle. If a policy
/// relies on execution report fill details to reconcile the reservation, the
/// lock produced during pre-trade must be stored until the final execution
/// report for that order has been processed. Dropping it too early breaks the
/// engine's ability to correctly unlock the unused remainder or finalize the
/// reserved state using the same assumptions that were applied when the order
/// was accepted.
///
/// # Performance
///
/// In practice the overwhelming majority of orders carry only prices for
/// [`DEFAULT_POLICY_GROUP_ID`]. The internal storage keeps the default group in a
/// dedicated inline-capacity slot, so the hot path neither scans nor allocates.
/// Non-default groups are stored in a second inline-capacity list of sections,
/// each section grouping prices for one group identifier. Reads, writes, and
/// lookups for the default group are O(1); operations for non-default groups
/// scan the small sections list once.
///
/// The concrete representation is intentionally private. Construct values
/// through the provided constructors and inspect them through the returned
/// iterators. This keeps the type free to evolve its internal layout without
/// affecting cross-language bindings.
///
/// # Serialization
///
/// With the `serde` feature enabled the lock uses an extremely compact wire
/// format that emits nothing the receiver does not need:
///
/// - no field names, no map keys, no struct tags;
/// - the default group identifier is implicit (it is the first sublist);
/// - non-default groups are sent once with all their prices, never repeating
///   the group identifier.
///
/// The on-wire shape is a sequence of sublists. The first sublist is the list
/// of prices for [`DEFAULT_POLICY_GROUP_ID`] (it may be empty). Every following
/// sublist describes one non-default group: its first element is the `u16`
/// group identifier, the remaining elements are the prices stored for that
/// group.
///
/// Because the format only uses `serialize_seq`, the same compactness applies
/// to every self-describing serde format: JSON (the canonical FFI exchange
/// format), MessagePack (`rmp-serde`), CBOR (`ciborium`), BSON, YAML
/// (`serde_yaml`), TOML, RON, Apache Avro, and FlexBuffers. Rust-only
/// formats such as Bincode and postcard work too but remain outside the
/// cross-language compatibility contract. The format-specific overhead per
/// sublist is the array header only (one byte for short arrays in
/// MessagePack/CBOR, two characters `[` and `]` in JSON), so the wire size is
/// dominated by the [`Price`] encoding itself.
///
/// Same lock, three formats:
///
/// ```text
/// // a default-only lock with a single price 185
/// JSON         [["185"]]                         // 9 bytes
/// MessagePack  91 91 a3 31 38 35                 // 6 bytes (hex)
/// CBOR         81 81 63 31 38 35                 // 6 bytes (hex)
///
/// // a mixed lock: default 185, group 7 with two prices 200 and 201
/// JSON         [["185"],[7,"200","201"]]         // 25 bytes
/// MessagePack  92 91 a3 31 38 35 93 07 a3 32 30
///              30 a3 32 30 31                    // 16 bytes (hex)
/// CBOR         82 81 63 31 38 35 83 07 63 32 30
///              30 63 32 30 31                    // 16 bytes (hex)
///
/// // empty lock
/// JSON         []                                // 2 bytes
/// MessagePack  90                                // 1 byte
/// CBOR         80                                // 1 byte
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreTradeLock {
    default_prices: SmallVec<[Price; 1]>,
    other: SmallVec<[GroupSection; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GroupSection {
    group_id: PolicyGroupId,
    prices: SmallVec<[Price; 1]>,
}

impl PreTradeLock {
    /// Creates an empty lock context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of stored price entries.
    pub fn len(&self) -> usize {
        self.default_prices.len() + self.other.iter().map(|s| s.prices.len()).sum::<usize>()
    }

    /// Returns `true` when no price entries are stored.
    pub fn is_empty(&self) -> bool {
        self.default_prices.is_empty() && self.other.iter().all(|s| s.prices.is_empty())
    }

    /// Creates a lock context populated from the given `(group_id, price)`
    /// records.
    ///
    /// Records with the same `group_id` are merged in insertion order, just
    /// like repeated [`Self::push`] calls.
    pub fn from_entries<EntriesIter>(entries: EntriesIter) -> Self
    where
        EntriesIter: IntoIterator<Item = (PolicyGroupId, Price)>,
    {
        let mut lock = Self::default();
        lock.append(entries);
        lock
    }

    /// Stores `price` under `policy_group_id`.
    ///
    /// Prices already stored under the same `policy_group_id` are preserved; the new
    /// price is appended to the end of that group's list.
    pub fn push(&mut self, policy_group_id: PolicyGroupId, price: Price) {
        if policy_group_id == DEFAULT_POLICY_GROUP_ID {
            self.default_prices.push(price);
            return;
        }
        for section in self.other.iter_mut() {
            if section.group_id == policy_group_id {
                section.prices.push(price);
                return;
            }
        }
        let mut prices = SmallVec::<[Price; 1]>::new();
        prices.push(price);
        self.other.push(GroupSection {
            group_id: policy_group_id,
            prices,
        });
    }

    /// Stores every `price` under `policy_group_id`, preserving any prices already
    /// stored under the same group.
    ///
    /// Target capacity is computed up-front from the iterator's exact length,
    /// so the destination storage grows at most once regardless of how many
    /// prices are pushed.
    pub fn push_many<PricesIter>(&mut self, policy_group_id: PolicyGroupId, prices: PricesIter)
    where
        PricesIter: IntoIterator<Item = Price>,
        PricesIter::IntoIter: ExactSizeIterator,
    {
        let iter = prices.into_iter();
        let count = iter.len();
        if count == 0 {
            return;
        }
        if policy_group_id == DEFAULT_POLICY_GROUP_ID {
            self.default_prices.reserve(count);
            self.default_prices.extend(iter);
            return;
        }
        if let Some(section) = self
            .other
            .iter_mut()
            .find(|s| s.group_id == policy_group_id)
        {
            section.prices.reserve(count);
            section.prices.extend(iter);
            return;
        }
        let mut section_prices = SmallVec::<[Price; 1]>::with_capacity(count);
        section_prices.extend(iter);
        self.other.push(GroupSection {
            group_id: policy_group_id,
            prices: section_prices,
        });
    }

    /// Appends all entries from `other` into `self`.
    ///
    /// Prices for the default group are bulk-copied in one operation. For each
    /// non-default group in `other`, prices are bulk-copied into the matching
    /// existing section or into a new section if none exists yet.
    ///
    /// The hot path — a single-entry lock merging into an existing one — costs
    /// exactly one `extend_from_slice` call with no iteration or allocation.
    pub fn merge(&mut self, other: &Self) {
        self.default_prices.extend_from_slice(&other.default_prices);
        for section in &other.other {
            if let Some(existing) = self
                .other
                .iter_mut()
                .find(|s| s.group_id == section.group_id)
            {
                existing.prices.extend_from_slice(&section.prices);
            } else {
                self.other.push(section.clone());
            }
        }
    }

    /// Iterates over every `(group_id, price)` pair, default-group prices
    /// first, then each non-default group in insertion order.
    pub fn entries(&self) -> Entries<'_> {
        Entries {
            default_iter: self.default_prices.iter(),
            sections_iter: self.other.iter(),
            current: None,
        }
    }

    /// Iterates over every price stored under `policy_group_id`, in insertion order.
    ///
    /// Default-group lookups are O(1); non-default lookups scan the section
    /// list once. Both return cheap iterators that walk an inline-storage
    /// slice with no allocation.
    pub fn prices_of(&self, policy_group_id: PolicyGroupId) -> PricesByGroup<'_> {
        const EMPTY: &[Price] = &[];
        let slice: &[Price] = if policy_group_id == DEFAULT_POLICY_GROUP_ID {
            self.default_prices.as_slice()
        } else {
            self.other
                .iter()
                .find(|section| section.group_id == policy_group_id)
                .map_or(EMPTY, |section| section.prices.as_slice())
        };
        PricesByGroup { iter: slice.iter() }
    }

    fn append<EntriesIter>(&mut self, entries: EntriesIter)
    where
        EntriesIter: IntoIterator<Item = (PolicyGroupId, Price)>,
    {
        let iter = entries.into_iter();
        let (lower, _) = iter.size_hint();
        if lower > 0 {
            self.other.reserve(lower);
        }
        for (group_id, price) in iter {
            self.push(group_id, price);
        }
    }
}

impl FromIterator<(PolicyGroupId, Price)> for PreTradeLock {
    fn from_iter<EntriesIter>(iter: EntriesIter) -> Self
    where
        EntriesIter: IntoIterator<Item = (PolicyGroupId, Price)>,
    {
        Self::from_entries(iter)
    }
}

impl Extend<(PolicyGroupId, Price)> for PreTradeLock {
    fn extend<EntriesIter>(&mut self, iter: EntriesIter)
    where
        EntriesIter: IntoIterator<Item = (PolicyGroupId, Price)>,
    {
        self.append(iter);
    }
}

/// Iterator over every `(group_id, price)` pair in a [`PreTradeLock`].
pub struct Entries<'a> {
    default_iter: slice::Iter<'a, Price>,
    sections_iter: slice::Iter<'a, GroupSection>,
    current: Option<(PolicyGroupId, slice::Iter<'a, Price>)>,
}

impl Iterator for Entries<'_> {
    type Item = (PolicyGroupId, Price);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(price) = self.default_iter.next() {
            return Some((DEFAULT_POLICY_GROUP_ID, *price));
        }
        loop {
            if let Some((group_id, iter)) = self.current.as_mut() {
                if let Some(price) = iter.next() {
                    return Some((*group_id, *price));
                }
                self.current = None;
            }
            let section = self.sections_iter.next()?;
            self.current = Some((section.group_id, section.prices.iter()));
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let mut remaining = self.default_iter.len();
        if let Some((_, iter)) = self.current.as_ref() {
            remaining += iter.len();
        }
        for section in self.sections_iter.clone() {
            remaining += section.prices.len();
        }
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Entries<'_> {}

/// Iterator over every price stored for a given group.
pub struct PricesByGroup<'a> {
    iter: slice::Iter<'a, Price>,
}

impl Iterator for PricesByGroup<'_> {
    type Item = Price;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().copied()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl ExactSizeIterator for PricesByGroup<'_> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use core::fmt;

    use serde::de::{DeserializeSeed, Error as DeError, SeqAccess, Visitor};
    use serde::ser::SerializeSeq;
    use serde::{Deserializer, Serializer};

    use super::{GroupSection, PreTradeLock};
    use crate::core::{PolicyGroupId, DEFAULT_POLICY_GROUP_ID};
    use crate::param::Price;

    struct SectionView<'a>(&'a GroupSection);

    impl serde::Serialize for SectionView<'_> {
        fn serialize<Target>(&self, target: Target) -> Result<Target::Ok, Target::Error>
        where
            Target: Serializer,
        {
            let mut seq = target.serialize_seq(Some(1 + self.0.prices.len()))?;
            seq.serialize_element(&self.0.group_id.value())?;
            for price in &self.0.prices {
                seq.serialize_element(price)?;
            }
            seq.end()
        }
    }

    impl serde::Serialize for PreTradeLock {
        fn serialize<Target>(&self, target: Target) -> Result<Target::Ok, Target::Error>
        where
            Target: Serializer,
        {
            if self.is_empty() {
                return target.serialize_seq(Some(0))?.end();
            }
            let mut seq = target.serialize_seq(Some(1 + self.other.len()))?;
            seq.serialize_element(self.default_prices.as_slice())?;
            for section in &self.other {
                seq.serialize_element(&SectionView(section))?;
            }
            seq.end()
        }
    }

    struct DefaultPricesSeed<'a>(&'a mut PreTradeLock);

    impl<'de> DeserializeSeed<'de> for DefaultPricesSeed<'_> {
        type Value = ();

        fn deserialize<Source>(self, source: Source) -> Result<Self::Value, Source::Error>
        where
            Source: Deserializer<'de>,
        {
            source.deserialize_seq(DefaultPricesVisitor(self.0))
        }
    }

    struct DefaultPricesVisitor<'a>(&'a mut PreTradeLock);

    impl<'de> Visitor<'de> for DefaultPricesVisitor<'_> {
        type Value = ();

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a list of prices for the default group")
        }

        fn visit_seq<SeqEntries>(
            self,
            mut entries: SeqEntries,
        ) -> Result<Self::Value, SeqEntries::Error>
        where
            SeqEntries: SeqAccess<'de>,
        {
            if let Some(hint) = entries.size_hint() {
                self.0.default_prices.reserve(hint);
            }
            while let Some(price) = entries.next_element::<Price>()? {
                self.0.push(DEFAULT_POLICY_GROUP_ID, price);
            }
            Ok(())
        }
    }

    struct GroupSectionSeed<'a>(&'a mut PreTradeLock);

    impl<'de> DeserializeSeed<'de> for GroupSectionSeed<'_> {
        type Value = ();

        fn deserialize<Source>(self, source: Source) -> Result<Self::Value, Source::Error>
        where
            Source: Deserializer<'de>,
        {
            source.deserialize_seq(GroupSectionVisitor(self.0))
        }
    }

    struct GroupSectionVisitor<'a>(&'a mut PreTradeLock);

    impl<'de> Visitor<'de> for GroupSectionVisitor<'_> {
        type Value = ();

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a non-default group section [group_id, prices...]")
        }

        fn visit_seq<SeqEntries>(
            self,
            mut entries: SeqEntries,
        ) -> Result<Self::Value, SeqEntries::Error>
        where
            SeqEntries: SeqAccess<'de>,
        {
            let raw_group_id: u16 = entries.next_element()?.ok_or_else(|| {
                DeError::invalid_length(0, &"non-default group section must start with a group_id")
            })?;
            let group_id = PolicyGroupId::new(raw_group_id);
            if group_id == DEFAULT_POLICY_GROUP_ID {
                return Err(DeError::custom(
                    "default group must be encoded as the first sublist, not as a tagged section",
                ));
            }
            while let Some(price) = entries.next_element::<Price>()? {
                self.0.push(group_id, price);
            }
            Ok(())
        }
    }

    struct LockVisitor;

    impl<'de> Visitor<'de> for LockVisitor {
        type Value = PreTradeLock;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(
                "a sequence whose first element is the default-group price list \
                 and whose remaining elements are [group_id, prices...] sections",
            )
        }

        fn visit_seq<SeqEntries>(
            self,
            mut entries: SeqEntries,
        ) -> Result<Self::Value, SeqEntries::Error>
        where
            SeqEntries: SeqAccess<'de>,
        {
            let mut lock = PreTradeLock::new();
            if entries
                .next_element_seed(DefaultPricesSeed(&mut lock))?
                .is_none()
            {
                return Ok(lock);
            }
            while entries
                .next_element_seed(GroupSectionSeed(&mut lock))?
                .is_some()
            {}
            Ok(lock)
        }
    }

    impl<'de> serde::Deserialize<'de> for PreTradeLock {
        fn deserialize<Source>(source: Source) -> Result<Self, Source::Error>
        where
            Source: Deserializer<'de>,
        {
            source.deserialize_seq(LockVisitor)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PreTradeLock;
    use crate::core::{PolicyGroupId, DEFAULT_POLICY_GROUP_ID};
    use crate::param::Price;

    fn price(value: &str) -> Price {
        Price::from_str(value).expect("price must be valid")
    }

    #[test]
    fn new_is_empty() {
        let lock = PreTradeLock::new();

        assert!(lock.is_empty());
        assert_eq!(lock.len(), 0);
        assert_eq!(lock.entries().count(), 0);
        assert!(lock.prices_of(DEFAULT_POLICY_GROUP_ID).next().is_none());
    }

    #[test]
    fn from_entries_populates_default_group() {
        let lock = PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price("185"))]);

        assert_eq!(lock.len(), 1);
        let by_default: Vec<_> = lock.prices_of(DEFAULT_POLICY_GROUP_ID).collect();
        assert_eq!(by_default, vec![price("185")]);
    }

    #[test]
    fn push_merges_prices_per_group() {
        let gid = PolicyGroupId::new(7);
        let mut lock = PreTradeLock::new();

        lock.push(DEFAULT_POLICY_GROUP_ID, price("100"));
        lock.push(gid, price("200"));
        lock.push(DEFAULT_POLICY_GROUP_ID, price("101"));
        lock.push(gid, price("201"));

        let defaults: Vec<_> = lock.prices_of(DEFAULT_POLICY_GROUP_ID).collect();
        assert_eq!(defaults, vec![price("100"), price("101")]);

        let others: Vec<_> = lock.prices_of(gid).collect();
        assert_eq!(others, vec![price("200"), price("201")]);
    }

    #[test]
    fn prices_of_unknown_group_is_empty() {
        let mut lock = PreTradeLock::new();
        lock.push(PolicyGroupId::new(1), price("10"));

        assert!(lock.prices_of(PolicyGroupId::new(99)).next().is_none());
    }

    #[test]
    fn entries_iterates_default_first_then_each_group() {
        let gid_a = PolicyGroupId::new(3);
        let gid_b = PolicyGroupId::new(5);
        let mut lock = PreTradeLock::new();

        lock.push(gid_a, price("300"));
        lock.push(DEFAULT_POLICY_GROUP_ID, price("100"));
        lock.push(gid_b, price("500"));
        lock.push(gid_a, price("301"));

        let collected: Vec<_> = lock.entries().collect();
        assert_eq!(
            collected,
            vec![
                (DEFAULT_POLICY_GROUP_ID, price("100")),
                (gid_a, price("300")),
                (gid_a, price("301")),
                (gid_b, price("500")),
            ]
        );
    }

    #[test]
    fn from_iterator_and_extend_traits_work() {
        let gid = PolicyGroupId::new(2);
        let source = [
            (DEFAULT_POLICY_GROUP_ID, price("10")),
            (gid, price("20")),
            (DEFAULT_POLICY_GROUP_ID, price("11")),
            (gid, price("21")),
        ];

        let collected: PreTradeLock = source.iter().copied().collect();
        assert_eq!(collected.len(), 4);

        let mut extended = PreTradeLock::new();
        extended.extend(source.iter().copied());
        assert_eq!(extended, collected);
    }

    #[test]
    fn entries_size_hint_matches_total_length() {
        let mut lock = PreTradeLock::new();
        lock.push(DEFAULT_POLICY_GROUP_ID, price("1"));
        lock.push(PolicyGroupId::new(1), price("2"));
        lock.push(PolicyGroupId::new(2), price("3"));
        lock.push(PolicyGroupId::new(1), price("4"));

        let iter = lock.entries();
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.len(), 4);
    }

    #[test]
    fn merge_appends_entries() {
        let gid = PolicyGroupId::new(7);
        let mut base = PreTradeLock::from_entries([
            (DEFAULT_POLICY_GROUP_ID, price("100")),
            (gid, price("200")),
        ]);
        let extra = PreTradeLock::from_entries([
            (DEFAULT_POLICY_GROUP_ID, price("101")),
            (gid, price("201")),
        ]);

        base.merge(&extra);

        assert_eq!(base.len(), 4);
        let defaults: Vec<_> = base.prices_of(DEFAULT_POLICY_GROUP_ID).collect();
        assert_eq!(defaults, vec![price("100"), price("101")]);
        let others: Vec<_> = base.prices_of(gid).collect();
        assert_eq!(others, vec![price("200"), price("201")]);
    }

    #[test]
    fn merge_single_entry_hot_path() {
        let mut base = PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price("100"))]);
        let single = PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price("101"))]);

        base.merge(&single);

        assert_eq!(base.len(), 2);
        let defaults: Vec<_> = base.prices_of(DEFAULT_POLICY_GROUP_ID).collect();
        assert_eq!(defaults, vec![price("100"), price("101")]);
    }

    #[test]
    fn merge_empty_into_non_empty() {
        let mut base = PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price("1"))]);
        base.merge(&PreTradeLock::new());
        assert_eq!(base.len(), 1);
    }

    #[test]
    fn merge_non_empty_into_empty() {
        let mut base = PreTradeLock::new();
        let other = PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price("42"))]);
        base.merge(&other);
        assert_eq!(base.len(), 1);
        assert_eq!(
            base.prices_of(DEFAULT_POLICY_GROUP_ID).next(),
            Some(price("42"))
        );
    }

    #[test]
    fn lock_clone_preserves_state() {
        let mut lock = PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price("185"))]);
        lock.push(PolicyGroupId::new(4), price("400"));
        lock.push(PolicyGroupId::new(4), price("401"));

        let cloned = lock.clone();

        assert_eq!(cloned, lock);
        assert_eq!(cloned.len(), 3);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn lock_implements_serde_traits() {
        fn assert_serde<Subject: serde::Serialize + serde::de::DeserializeOwned>() {}
        assert_serde::<PreTradeLock>();
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_default_only() {
        let mut original = PreTradeLock::new();
        original.push(DEFAULT_POLICY_GROUP_ID, price("100"));
        original.push(DEFAULT_POLICY_GROUP_ID, price("200"));

        let json = serde_json::to_string(&original).expect("serialize must succeed");
        let restored: PreTradeLock = serde_json::from_str(&json).expect("deserialize must succeed");

        assert_eq!(restored.len(), 2, "len must be preserved after round-trip");
        assert!(!restored.is_empty(), "must not be empty after round-trip");

        let defaults: Vec<_> = restored.prices_of(DEFAULT_POLICY_GROUP_ID).collect();
        assert_eq!(defaults, vec![price("100"), price("200")]);

        let json2 = serde_json::to_string(&restored).expect("re-serialize must succeed");
        assert_eq!(json, json2, "repeated serialize must be idempotent");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_mixed() {
        let gid = PolicyGroupId::new(3);
        let mut original = PreTradeLock::new();
        original.push(DEFAULT_POLICY_GROUP_ID, price("100"));
        original.push(DEFAULT_POLICY_GROUP_ID, price("101"));
        original.push(gid, price("300"));
        original.push(gid, price("301"));

        let json = serde_json::to_string(&original).expect("serialize must succeed");
        let restored: PreTradeLock = serde_json::from_str(&json).expect("deserialize must succeed");

        assert_eq!(restored.len(), 4);
        assert!(!restored.is_empty());

        let defaults: Vec<_> = restored.prices_of(DEFAULT_POLICY_GROUP_ID).collect();
        assert_eq!(defaults, vec![price("100"), price("101")]);

        let others: Vec<_> = restored.prices_of(gid).collect();
        assert_eq!(others, vec![price("300"), price("301")]);

        let json2 = serde_json::to_string(&restored).expect("re-serialize must succeed");
        assert_eq!(json, json2, "repeated serialize must be idempotent");
    }
}
