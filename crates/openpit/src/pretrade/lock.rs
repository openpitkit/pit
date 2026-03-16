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

use crate::param::Price;

/// Stable lock context captured during pre-trade reservation.
///
/// `Lock` is not just a copy of request input. It is the serialized context of
/// what the engine actually reserved and how that reservation must later be
/// reconciled.
///
/// Policies may need to persist data that is known at reservation time but will
/// also be required later, when execution reports arrive and the engine must
/// release, consume, or re-price the remaining reserved state correctly.
/// Typical examples include the reservation price, translated quantities, or
/// any other policy-specific values that describe how funds were locked.
///
/// The lock context must travel together with the order lifecycle. If a policy
/// relies on execution report fill details to reconcile the reservation, the
/// lock produced during pre-trade must be stored until the final execution
/// report for that order has been processed. Dropping it too early breaks the
/// engine's ability to correctly unlock the unused remainder or finalize the
/// reserved state using the same assumptions that were applied when the order
/// was accepted.
///
/// A common case is price-sensitive reservation. The engine may reserve funds
/// using a worst execution price known during pre-trade. Later, when partial
/// fills and the final terminal report arrive, that same price must still be
/// available to compute how much reserved amount remains to be released.
/// Without the stored lock context, post-trade reconciliation would need to
/// guess, which is not a valid contract for deterministic risk handling.
///
/// In other words, `Lock` is the continuation token of reservation logic. It
/// captures the policy context that must survive from pre-trade acceptance to
/// the last execution report relevant for that reservation.
///
/// This crate exposes Serde compatibility only when the `serde` feature is
/// enabled. Applications embedding `openpit` choose the concrete transport
/// format themselves.
///
/// JSON is the canonical format for external bindings. Binary transports such
/// as MessagePack and CBOR are selected by the consuming application. Bincode
/// remains Rust-only and is outside the cross-language compatibility contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[non_exhaustive]
pub struct Lock {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    price: Option<Price>,
}

impl Lock {
    /// Creates a new lock context captured during reservation.
    pub fn new(price: Option<Price>) -> Self {
        Self { price }
    }

    /// Returns the reservation price stored in the lock context.
    ///
    /// This value is typically used later during execution-report processing to
    /// reconcile reserved versus actually used amounts.
    pub fn price(&self) -> Option<Price> {
        self.price
    }
}

#[cfg(test)]
mod tests {
    use super::Lock;
    use crate::param::Price;

    #[test]
    fn new_stores_none_price() {
        let lock = Lock::new(None);

        assert_eq!(lock.price(), None);
    }

    #[test]
    fn new_stores_some_price() {
        let price = Price::from_str("185").expect("price must be valid");

        let lock = Lock::new(Some(price));

        assert_eq!(lock.price(), Some(price));
    }

    #[test]
    fn lock_is_copy() {
        let lock = Lock::new(Some(Price::from_str("185").expect("price must be valid")));
        let copied = lock;

        assert_eq!(copied, lock);
    }
}
