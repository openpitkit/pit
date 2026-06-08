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

//! Pull-based market-data service for supplying live quotes to pre-trade
//! policies and periodic portfolio revaluators.
//!
//! Quotes are stored per instrument in three conceptual buckets — per-account,
//! per-account-group, and the default ("everyone-else") bucket, which is the
//! bucket of [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
//!
//! The service is built with [`MarketDataBuilder`] (with a required
//! [`QuoteTtl`] default). Instruments are registered via
//! [`MarketDataService::register`] (or the `_with_ttl` / `_with_id` /
//! `_with_id_and_ttl` variants); all registration calls are **strict** and
//! return an error if the instrument or id is already registered.
//!
//! Quotes are published on the hot path via:
//! - [`MarketDataService::push`] / [`MarketDataService::push_patch`] — by id
//!   into the default bucket; the id must have been registered beforehand.
//! - [`MarketDataService::push_for`] / [`MarketDataService::push_for_patch`] —
//!   by id, fanned out to specific accounts and groups.
//! - [`MarketDataService::push_by_instrument`] /
//!   [`MarketDataService::push_by_instrument_patch`] — by instrument name into
//!   the default bucket; auto-registers a named slot on first sight.
//!
//! TTL is settable along the instrument, account, and group axes (plus the
//! instrument × account and instrument × group cells and a global default) via
//! the `set_*_ttl` setters; a multi-axis cascade resolves the effective
//! lifetime per read. See [`MarketDataService`] for the tier order.
//!
//! Consumers poll via [`MarketDataService::get`] /
//! [`MarketDataService::get_or_err`], passing the reading account, a
//! [`AccountInfo`], and a [`QuoteResolution`]; quotes older than their
//! effective TTL surface as "unavailable".

pub(crate) mod builder;
pub(crate) mod error;
pub(crate) mod instrument_id;
pub(crate) mod internals;
pub(crate) mod lock;
pub(crate) mod quote;
pub(crate) mod resolution;
pub(crate) mod service;
pub(crate) mod ttl;

pub use builder::{sealed, MarketDataBuilder, MarketDataSync};
pub use error::{
    AlreadyRegistered, MarketDataError, PushForError, RegistrationError, UnknownInstrumentId,
};
pub use instrument_id::InstrumentId;
pub use lock::{
    LocalTtlGate, MarketDataLock, NoopLock, NoopReadGuard, NoopWriteGuard, RuntimeLock,
    RuntimeReadGuard, RuntimeWriteGuard, ServiceTtlGate,
};
pub use quote::{Quote, QuoteTtl};
pub use resolution::{AccountInfo, QuoteResolution};
pub use service::MarketDataService;
