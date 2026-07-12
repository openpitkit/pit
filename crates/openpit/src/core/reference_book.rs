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
// Please see https://openpit.dev and the OWNERS file for details.

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use super::{Instrument, InstrumentId};

/// Unit used to express a settlement delay.
///
/// `BusinessDays` is the zero value and the default. Calendar interpretation
/// belongs to settlement processing, which is intentionally outside this
/// reference-book seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SettlementUnit {
    /// Business days in the caller-supplied settlement calendar.
    #[default]
    BusinessDays = 0,
    /// Consecutive calendar days.
    CalendarDays = 1,
}

/// A delay between trade time and settlement of one leg.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SettlementLag {
    n: u64,
    unit: SettlementUnit,
}

impl SettlementLag {
    /// Creates a settlement delay.
    pub const fn new(n: u64, unit: SettlementUnit) -> Self {
        Self { n, unit }
    }

    /// Returns the number of elapsed `unit`s required for settlement.
    pub const fn n(self) -> u64 {
        self.n
    }

    /// Returns the unit used to measure the delay.
    pub const fn unit(self) -> SettlementUnit {
        self.unit
    }
}

/// Settlement delays for delivery and payment legs of an instrument.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SettlementScheme {
    delivery: SettlementLag,
    payment: SettlementLag,
}

impl SettlementScheme {
    /// Creates a scheme with independently configured delivery and payment
    /// lags.
    pub const fn new(delivery: SettlementLag, payment: SettlementLag) -> Self {
        Self { delivery, payment }
    }

    /// Creates a scheme where both legs settle after `n` business days.
    pub const fn uniform(n: u64) -> Self {
        let lag = SettlementLag::new(n, SettlementUnit::BusinessDays);
        Self::new(lag, lag)
    }

    /// Returns the delivery leg's settlement delay.
    pub const fn delivery(self) -> SettlementLag {
        self.delivery
    }

    /// Returns the payment leg's settlement delay.
    pub const fn payment(self) -> SettlementLag {
        self.payment
    }
}

/// Error returned when an instrument registration conflicts with an existing
/// reference-book entry.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReferenceBookRegistrationError {
    /// The supplied ID already names a different registered instrument.
    DuplicateId {
        /// The conflicting instrument ID.
        instrument_id: InstrumentId,
    },
    /// The supplied instrument is already registered under a different ID.
    DuplicateInstrument {
        /// The conflicting instrument.
        instrument: Instrument,
    },
}

impl Display for ReferenceBookRegistrationError {
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

impl std::error::Error for ReferenceBookRegistrationError {}

/// Error returned when an operation needs an instrument ID absent from a
/// [`ReferenceBook`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownReferenceBookInstrumentId {
    /// The instrument ID that was not found in the reference book.
    pub instrument_id: InstrumentId,
}

impl Display for UnknownReferenceBookInstrumentId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unknown reference-book instrument id: {}",
            self.instrument_id.0
        )
    }
}

impl std::error::Error for UnknownReferenceBookInstrumentId {}

struct ReferenceBookEntry {
    settlement_scheme: Option<SettlementScheme>,
}

/// Per-instrument reference data independent of market-data registration.
///
/// A reference book assigns or records stable [`InstrumentId`] values and
/// maintains typed attributes for each registered instrument. It does not
/// start market-data services or imply a registration there. A caller that
/// needs both subsystems can explicitly reuse an ID with
/// [`MarketDataService::register_with_id`].
///
/// [`MarketDataService::register_with_id`]: crate::marketdata::MarketDataService::register_with_id
#[derive(Default)]
pub struct ReferenceBook {
    by_id: HashMap<InstrumentId, ReferenceBookEntry>,
    by_instrument: HashMap<Instrument, InstrumentId>,
    next_auto_id: u64,
}

impl ReferenceBook {
    /// Creates an empty reference book.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an instrument under the next available automatically assigned
    /// ID.
    ///
    /// # Errors
    ///
    /// Returns [`ReferenceBookRegistrationError::DuplicateInstrument`] when the
    /// instrument is already registered.
    pub fn register(
        &mut self,
        instrument: Instrument,
    ) -> Result<InstrumentId, ReferenceBookRegistrationError> {
        if self.by_instrument.contains_key(&instrument) {
            return Err(ReferenceBookRegistrationError::DuplicateInstrument { instrument });
        }

        let instrument_id = self.next_auto_id();
        self.insert(instrument, instrument_id);
        Ok(instrument_id)
    }

    /// Registers an instrument under a caller-assigned ID.
    ///
    /// This permits a caller to share an explicit identity with a separate
    /// market-data registration without coupling the two registries.
    ///
    /// # Errors
    ///
    /// - Returns [`ReferenceBookRegistrationError::DuplicateId`] when `id` is
    ///   already registered.
    /// - Returns [`ReferenceBookRegistrationError::DuplicateInstrument`] when
    ///   `instrument` is already registered.
    pub fn register_with_id(
        &mut self,
        instrument: Instrument,
        instrument_id: InstrumentId,
    ) -> Result<InstrumentId, ReferenceBookRegistrationError> {
        if self.by_instrument.contains_key(&instrument) {
            return Err(ReferenceBookRegistrationError::DuplicateInstrument { instrument });
        }
        if self.by_id.contains_key(&instrument_id) {
            return Err(ReferenceBookRegistrationError::DuplicateId { instrument_id });
        }

        self.insert(instrument, instrument_id);
        Ok(instrument_id)
    }

    /// Resolves an instrument to its registered ID.
    pub fn resolve(&self, instrument: &Instrument) -> Option<InstrumentId> {
        self.by_instrument.get(instrument).copied()
    }

    /// Sets the typed settlement scheme for a registered instrument.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownReferenceBookInstrumentId`] when `instrument_id` is not
    /// registered.
    pub fn set_settlement_scheme(
        &mut self,
        instrument_id: InstrumentId,
        settlement_scheme: SettlementScheme,
    ) -> Result<(), UnknownReferenceBookInstrumentId> {
        let entry = self
            .by_id
            .get_mut(&instrument_id)
            .ok_or(UnknownReferenceBookInstrumentId { instrument_id })?;
        entry.settlement_scheme = Some(settlement_scheme);
        Ok(())
    }

    /// Clears the settlement scheme for a registered instrument.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownReferenceBookInstrumentId`] when `instrument_id` is not
    /// registered.
    pub fn clear_settlement_scheme(
        &mut self,
        instrument_id: InstrumentId,
    ) -> Result<(), UnknownReferenceBookInstrumentId> {
        let entry = self
            .by_id
            .get_mut(&instrument_id)
            .ok_or(UnknownReferenceBookInstrumentId { instrument_id })?;
        entry.settlement_scheme = None;
        Ok(())
    }

    /// Returns the settlement scheme assigned to a registered instrument.
    ///
    /// A successful `None` result means the instrument has no settlement
    /// configuration yet; this is T+0 behavior until T2 consumes the setting.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownReferenceBookInstrumentId`] when `instrument_id` is not
    /// registered.
    pub fn settlement_scheme(
        &self,
        instrument_id: InstrumentId,
    ) -> Result<Option<SettlementScheme>, UnknownReferenceBookInstrumentId> {
        self.by_id
            .get(&instrument_id)
            .map(|entry| entry.settlement_scheme)
            .ok_or(UnknownReferenceBookInstrumentId { instrument_id })
    }

    fn next_auto_id(&mut self) -> InstrumentId {
        loop {
            let candidate = InstrumentId(self.next_auto_id);
            self.next_auto_id += 1;
            if !self.by_id.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    fn insert(&mut self, instrument: Instrument, instrument_id: InstrumentId) {
        self.by_instrument.insert(instrument, instrument_id);
        self.by_id.insert(
            instrument_id,
            ReferenceBookEntry {
                settlement_scheme: None,
            },
        );
        if self.next_auto_id == instrument_id.0 {
            self.next_auto_id += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::param::Asset;

    use super::{
        Instrument, InstrumentId, ReferenceBook, ReferenceBookRegistrationError, SettlementLag,
        SettlementScheme, SettlementUnit, UnknownReferenceBookInstrumentId,
    };

    fn instrument() -> Instrument {
        Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        )
    }

    #[test]
    fn settlement_defaults_to_zero_business_days() {
        assert_eq!(SettlementUnit::default(), SettlementUnit::BusinessDays);
        assert_eq!(SettlementLag::default().n(), 0);
        assert_eq!(
            SettlementLag::default().unit(),
            SettlementUnit::BusinessDays
        );
        assert_eq!(SettlementScheme::default(), SettlementScheme::uniform(0));
    }

    #[test]
    fn uniform_scheme_uses_business_days_for_both_legs() {
        let scheme = SettlementScheme::uniform(2);

        assert_eq!(
            scheme.delivery(),
            SettlementLag::new(2, SettlementUnit::BusinessDays)
        );
        assert_eq!(
            scheme.payment(),
            SettlementLag::new(2, SettlementUnit::BusinessDays)
        );
    }

    #[test]
    fn resolves_instruments_and_preserves_separate_legs() {
        let mut book = ReferenceBook::new();
        let instrument = instrument();
        let instrument_id = InstrumentId::new(42);
        let scheme = SettlementScheme::new(
            SettlementLag::new(2, SettlementUnit::BusinessDays),
            SettlementLag::new(1, SettlementUnit::CalendarDays),
        );

        assert_eq!(
            book.register_with_id(instrument.clone(), instrument_id),
            Ok(instrument_id)
        );
        assert_eq!(book.resolve(&instrument), Some(instrument_id));
        assert_eq!(book.settlement_scheme(instrument_id), Ok(None));

        assert_eq!(book.set_settlement_scheme(instrument_id, scheme), Ok(()));
        assert_eq!(book.settlement_scheme(instrument_id), Ok(Some(scheme)));
    }

    #[test]
    fn rejects_duplicate_ids_and_instruments() {
        let mut book = ReferenceBook::new();
        let aapl = instrument();
        let msft = Instrument::new(
            Asset::new("MSFT").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        );
        let id = InstrumentId::new(42);

        assert_eq!(book.register_with_id(aapl.clone(), id), Ok(id));
        assert_eq!(
            book.register_with_id(msft, id),
            Err(ReferenceBookRegistrationError::DuplicateId { instrument_id: id })
        );
        assert_eq!(
            book.register_with_id(aapl.clone(), InstrumentId::new(43)),
            Err(ReferenceBookRegistrationError::DuplicateInstrument { instrument: aapl })
        );
    }

    #[test]
    fn reports_unknown_ids_for_settlement_configuration() {
        let mut book = ReferenceBook::new();
        let id = InstrumentId::new(99);
        let error = UnknownReferenceBookInstrumentId { instrument_id: id };

        assert_eq!(book.settlement_scheme(id), Err(error));
        assert_eq!(
            book.set_settlement_scheme(id, SettlementScheme::uniform(1)),
            Err(error)
        );
        assert_eq!(book.clear_settlement_scheme(id), Err(error));
    }

    #[test]
    fn marketdata_instrument_id_is_the_core_type() {
        let core_id = InstrumentId::new(7);
        let marketdata_id: crate::marketdata::InstrumentId = core_id;

        assert_eq!(marketdata_id, core_id);
    }
}
