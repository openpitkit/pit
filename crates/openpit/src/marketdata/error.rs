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

use crate::core::Instrument;

use super::instrument_id::InstrumentId;
use super::quote::Quote;

// ─── MarketDataError ──────────────────────────────────────────────────────────

/// Error returned by market-data service read operations.
///
/// A quote aged past its TTL is reported as
/// [`QuoteExpired`](Self::QuoteExpired) with the selected stale quote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarketDataError {
    /// The requested instrument was not registered with the service.
    UnknownInstrument,
    /// No usable quote is available (never pushed or cleared).
    QuoteUnavailable,
    /// The selected quote exists but has aged past its effective TTL.
    QuoteExpired(Quote),
}

impl Display for MarketDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownInstrument => f.write_str("unknown instrument"),
            Self::QuoteUnavailable => f.write_str("quote unavailable"),
            Self::QuoteExpired(_) => f.write_str("quote expired"),
        }
    }
}

impl std::error::Error for MarketDataError {}

/// Error returned when an operation references an
/// [`InstrumentId`] that is not registered with the service.
///
/// Returned by [`MarketDataService::push`], [`MarketDataService::push_patch`],
/// and the instrument-qualified TTL setters
/// ([`MarketDataService::set_instrument_ttl`] and the
/// `set_instrument_account_ttl` / `set_instrument_account_group_ttl` family).
///
/// [`MarketDataService::push`]:
///     super::service::MarketDataService::push
/// [`MarketDataService::push_patch`]:
///     super::service::MarketDataService::push_patch
/// [`MarketDataService::set_instrument_ttl`]:
///     super::service::MarketDataService::set_instrument_ttl
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownInstrumentId {
    /// The instrument id that was not found in the registry.
    pub instrument_id: InstrumentId,
}

impl Display for UnknownInstrumentId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown instrument id: {}", self.instrument_id.0)
    }
}

impl std::error::Error for UnknownInstrumentId {}

/// Error returned by
/// [`MarketDataService::register`](super::service::MarketDataService::register)
/// and
/// [`MarketDataService::register_with_ttl`](super::service::MarketDataService::register_with_ttl)
/// when the instrument is already registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlreadyRegistered {
    /// The instrument that was already registered.
    pub instrument: Instrument,
}

impl Display for AlreadyRegistered {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "instrument {:?} is already registered", self.instrument)
    }
}

impl std::error::Error for AlreadyRegistered {}

/// Error returned by
/// [`MarketDataService::register_with_id`](super::service::MarketDataService::register_with_id)
/// and
/// [`MarketDataService::register_with_id_and_ttl`](super::service::MarketDataService::register_with_id_and_ttl).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegistrationError {
    /// An instrument with the given ID is already registered.
    DuplicateId {
        /// The instrument ID that conflicted.
        instrument_id: InstrumentId,
    },
    /// The given instrument is already registered under a different ID.
    DuplicateInstrument {
        /// The instrument that conflicted.
        instrument: Instrument,
    },
}

impl Display for RegistrationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId { instrument_id } => {
                write!(f, "instrument id {} is already registered", instrument_id.0)
            }
            Self::DuplicateInstrument { instrument } => {
                write!(f, "instrument {:?} is already registered", instrument)
            }
        }
    }
}

impl std::error::Error for RegistrationError {}

/// Error returned by the targeted fan-out pushes
/// [`MarketDataService::push_for`](super::service::MarketDataService::push_for)
/// and
/// [`MarketDataService::push_for_patch`](super::service::MarketDataService::push_for_patch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PushForError {
    /// The target instrument id is not registered with the service.
    UnknownInstrument {
        /// The instrument id that was not found in the registry.
        instrument_id: InstrumentId,
    },
    /// Both the account list and the group list were empty.
    ///
    /// A targeted push with no targets is a caller bug, not a no-op: use
    /// [`MarketDataService::push`](super::service::MarketDataService::push) to
    /// write the default bucket instead.
    NoTarget,
}

impl Display for PushForError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownInstrument { instrument_id } => {
                write!(f, "unknown instrument id: {}", instrument_id.0)
            }
            Self::NoTarget => f.write_str("push_for requires at least one account or group target"),
        }
    }
}

impl std::error::Error for PushForError {}
