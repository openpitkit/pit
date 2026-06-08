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

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use openpit::pretrade::PreTradeLock;
use openpit::PolicyGroupId;
use rust_decimal::Decimal;

use crate::bytes::OpenPitSharedBytes;
use crate::last_error::{write_error, OpenPitOutError};
use crate::param::{OpenPitParamDecimal, OpenPitParamPrice};

/// Opaque pre-trade lock handle.
pub struct OpenPitPretradePreTradeLock {
    inner: PreTradeLock,
}

impl OpenPitPretradePreTradeLock {
    pub(crate) fn from_inner(inner: PreTradeLock) -> *mut Self {
        Box::into_raw(Box::new(Self { inner }))
    }

    pub(crate) fn inner_clone(&self) -> PreTradeLock {
        self.inner.clone()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitPretradePreTradeLockPricesView {
    pub ptr: *const OpenPitParamPrice,
    pub len: usize,
}

impl OpenPitPretradePreTradeLockPricesView {
    const fn not_set() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    fn from_slice(values: &[OpenPitParamPrice]) -> Self {
        if values.is_empty() {
            return Self::not_set();
        }
        Self {
            ptr: values.as_ptr(),
            len: values.len(),
        }
    }
}

/// Caller-owned list of prices stored under a lock group.
pub struct OpenPitPretradePreTradeLockPrices {
    items: Vec<OpenPitParamPrice>,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitPretradePreTradeLockPricesStatus {
    Error = 0,
    Empty = 1,
    One = 2,
    List = 3,
}

/// A single `(policy_group_id, price)` record exchanged across the C boundary.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitPretradePreTradeLockEntry {
    pub policy_group_id: u16,
    pub price: OpenPitParamPrice,
}

fn export_price(price: openpit::param::Price) -> OpenPitParamPrice {
    OpenPitParamPrice(OpenPitParamDecimal::from_decimal(price.to_decimal()))
}

/// Validates an input entry array and returns the parsed `(PolicyGroupId, Price)`
/// records, writing the first validation error to `out_error` and returning
/// `false` on failure.
fn parse_entries(
    entries_ptr: *const OpenPitPretradePreTradeLockEntry,
    entries_len: usize,
    out_error: OpenPitOutError,
    out_parsed: &mut Vec<(PolicyGroupId, openpit::param::Price)>,
) -> bool {
    if entries_len != 0 && entries_ptr.is_null() {
        write_error(out_error, "entries pointer must be non-null");
        return false;
    }
    let entries = if entries_len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(entries_ptr, entries_len) }
    };
    out_parsed.reserve(entries.len());
    for entry in entries {
        match entry.price.to_param() {
            Ok(parsed) => out_parsed.push((PolicyGroupId::new(entry.policy_group_id), parsed)),
            Err(message) => {
                write_error(out_error, message.as_str());
                return false;
            }
        }
    }
    true
}

/// Allocates an empty lock.
///
/// Success:
/// - always returns a non-null caller-owned handle.
///
/// Cleanup:
/// - the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
#[no_mangle]
pub extern "C" fn openpit_create_pretrade_pre_trade_lock() -> *mut OpenPitPretradePreTradeLock {
    OpenPitPretradePreTradeLock::from_inner(PreTradeLock::new())
}

/// Releases a lock handle.
///
/// Contract:
/// - passing null is allowed;
/// - after this call the pointer is invalid;
/// - this function always succeeds.
#[no_mangle]
pub extern "C" fn openpit_destroy_pretrade_pre_trade_lock(
    handle: *mut OpenPitPretradePreTradeLock,
) {
    if handle.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(handle)) };
}

/// Returns a deep copy of `lock`.
///
/// Success:
/// - returns a non-null caller-owned handle independent of `lock`.
///
/// Error:
/// - returns null when `lock` is null.
///
/// Cleanup:
/// - the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_clone(
    lock: *const OpenPitPretradePreTradeLock,
) -> *mut OpenPitPretradePreTradeLock {
    if lock.is_null() {
        return std::ptr::null_mut();
    }
    let source = unsafe { &*lock };
    OpenPitPretradePreTradeLock::from_inner(source.inner_clone())
}

/// Total number of stored prices across all groups.
///
/// `lock` must be a valid non-null handle. Passing null aborts the process.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_len(
    lock: *const OpenPitPretradePreTradeLock,
) -> usize {
    assert!(!lock.is_null(), "lock pointer must be non-null");
    unsafe { &*lock }.inner.len()
}

/// Returns `true` when the lock carries no price records.
///
/// `lock` must be a valid non-null handle. Passing null aborts the process.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_is_empty(
    lock: *const OpenPitPretradePreTradeLock,
) -> bool {
    assert!(!lock.is_null(), "lock pointer must be non-null");
    unsafe { &*lock }.inner.is_empty()
}

/// Appends `price` under `policy_group_id`.
///
/// Success:
/// - returns `true`; the lock now carries one extra record for
///   `policy_group_id`.
///
/// Error:
/// - returns `false` when `lock` is null or when `price` fails domain
///   validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_push(
    lock: *mut OpenPitPretradePreTradeLock,
    policy_group_id: u16,
    price: OpenPitParamPrice,
    out_error: OpenPitOutError,
) -> bool {
    if lock.is_null() {
        write_error(out_error, "lock pointer must be non-null");
        return false;
    }
    let parsed = match price.to_param() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    unsafe { &mut *lock }
        .inner
        .push(PolicyGroupId::new(policy_group_id), parsed);
    true
}

/// Appends every `(policy_group_id, price)` record from `entries` into `lock`.
///
/// `entries_ptr`/`entries_len` describe an array of
/// `OpenPitPretradePreTradeLockEntry`. A zero length is allowed and leaves the
/// lock unchanged regardless of `entries_ptr`.
///
/// Success:
/// - returns `true`; every record has been appended in input order.
///
/// Error:
/// - returns `false` when `lock` is null, when `entries_ptr` is null while
///   `entries_len` is non-zero, or when any price fails domain validation; on
///   the first invalid price no record is appended;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_push_many(
    lock: *mut OpenPitPretradePreTradeLock,
    entries_ptr: *const OpenPitPretradePreTradeLockEntry,
    entries_len: usize,
    out_error: OpenPitOutError,
) -> bool {
    if lock.is_null() {
        write_error(out_error, "lock pointer must be non-null");
        return false;
    }
    let mut parsed = Vec::new();
    if !parse_entries(entries_ptr, entries_len, out_error, &mut parsed) {
        return false;
    }
    unsafe { &mut *lock }.inner.extend(parsed);
    true
}

/// Builds a new lock populated from the given `(policy_group_id, price)` records.
///
/// `entries_ptr`/`entries_len` describe an array of
/// `OpenPitPretradePreTradeLockEntry`. A zero length is allowed and yields an
/// empty lock regardless of `entries_ptr`.
///
/// Success:
/// - returns a non-null caller-owned lock handle.
///
/// Error:
/// - returns null when `entries_ptr` is null while `entries_len` is non-zero or
///   when any price fails domain validation;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
#[no_mangle]
pub extern "C" fn openpit_create_pretrade_pre_trade_lock_from_entries(
    entries_ptr: *const OpenPitPretradePreTradeLockEntry,
    entries_len: usize,
    out_error: OpenPitOutError,
) -> *mut OpenPitPretradePreTradeLock {
    let mut parsed = Vec::new();
    if !parse_entries(entries_ptr, entries_len, out_error, &mut parsed) {
        return std::ptr::null_mut();
    }
    OpenPitPretradePreTradeLock::from_inner(PreTradeLock::from_entries(parsed))
}

/// Appends every record from `src` into `dst`, leaving `src` unchanged.
///
/// Success:
/// - returns `true`; `dst` now also carries every record from `src`.
///
/// Error:
/// - returns `false` when `dst` or `src` is null;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_merge(
    dst: *mut OpenPitPretradePreTradeLock,
    src: *const OpenPitPretradePreTradeLock,
    out_error: OpenPitOutError,
) -> bool {
    if dst.is_null() {
        write_error(out_error, "destination lock pointer must be non-null");
        return false;
    }
    if src.is_null() {
        write_error(out_error, "source lock pointer must be non-null");
        return false;
    }
    let source = unsafe { &*src };
    unsafe { &mut *dst }.inner.merge(&source.inner);
    true
}

/// Releases a caller-owned lock price list.
///
/// Contract:
/// - `handle` must be a valid non-null pointer;
/// - this function always succeeds.
#[no_mangle]
pub extern "C" fn openpit_destroy_pretrade_pre_trade_lock_prices(
    handle: *mut OpenPitPretradePreTradeLockPrices,
) {
    assert!(!handle.is_null(), "lock prices pointer must be non-null");
    unsafe { drop(Box::from_raw(handle)) };
}

/// Borrows a read-only view of a lock price list.
///
/// `handle` must be a valid non-null pointer; violating this triggers a panic.
///
/// Returns an unset view (`ptr == null`, `len == 0`) when the list is empty.
/// The view remains valid only while `handle` is alive.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_prices_view(
    handle: *const OpenPitPretradePreTradeLockPrices,
) -> OpenPitPretradePreTradeLockPricesView {
    assert!(!handle.is_null(), "lock prices pointer must be non-null");
    OpenPitPretradePreTradeLockPricesView::from_slice(unsafe { &*handle }.items.as_slice())
}

/// Returns the prices stored under `policy_group_id`.
///
/// Single-price case:
/// - when the group holds exactly one price, it is written directly to
///   `out_price`.
///
/// Status:
/// - `Error`: `lock`, `out_price`, or `out_prices` is null; `out_error`
///   receives an error handle when provided.
/// - `Empty`: the call succeeded and the group has no prices; `out_prices` is
///   set to null.
/// - `One`: the call succeeded and `out_price` contains the only stored price;
///   `out_prices` is set to null.
/// - `List`: the call succeeded and `out_prices` contains a caller-owned list.
///
/// Cleanup:
/// - when status is `List`, the caller MUST release `*out_prices` with
///   `openpit_destroy_pretrade_pre_trade_lock_prices` exactly once.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_prices_of(
    lock: *const OpenPitPretradePreTradeLock,
    policy_group_id: u16,
    out_price: *mut OpenPitParamPrice,
    out_prices: *mut *mut OpenPitPretradePreTradeLockPrices,
    out_error: OpenPitOutError,
) -> OpenPitPretradePreTradeLockPricesStatus {
    if lock.is_null() {
        write_error(out_error, "lock pointer must be non-null");
        return OpenPitPretradePreTradeLockPricesStatus::Error;
    }
    if out_price.is_null() {
        write_error(out_error, "out_price pointer must be non-null");
        return OpenPitPretradePreTradeLockPricesStatus::Error;
    }
    if out_prices.is_null() {
        write_error(out_error, "out_prices pointer must be non-null");
        return OpenPitPretradePreTradeLockPricesStatus::Error;
    }
    unsafe {
        *out_price = OpenPitParamPrice::default();
        *out_prices = std::ptr::null_mut();
    }
    let lock_ref = unsafe { &*lock };
    let mut prices = lock_ref
        .inner
        .prices_of(PolicyGroupId::new(policy_group_id));
    let Some(first) = prices.next() else {
        return OpenPitPretradePreTradeLockPricesStatus::Empty;
    };
    if prices.len() == 0 {
        unsafe { *out_price = export_price(first) };
        return OpenPitPretradePreTradeLockPricesStatus::One;
    }

    let mut items = Vec::with_capacity(prices.len() + 1);
    items.push(export_price(first));
    items.extend(prices.map(export_price));
    unsafe {
        *out_prices = Box::into_raw(Box::new(OpenPitPretradePreTradeLockPrices { items }));
    }
    OpenPitPretradePreTradeLockPricesStatus::List
}

/// Read-only view over a caller-owned lock entry snapshot.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitPretradePreTradeLockEntriesView {
    pub ptr: *const OpenPitPretradePreTradeLockEntry,
    pub len: usize,
}

impl OpenPitPretradePreTradeLockEntriesView {
    const fn not_set() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    fn from_slice(values: &[OpenPitPretradePreTradeLockEntry]) -> Self {
        if values.is_empty() {
            return Self::not_set();
        }
        Self {
            ptr: values.as_ptr(),
            len: values.len(),
        }
    }
}

/// Caller-owned snapshot of every `(policy_group_id, price)` record in a lock.
pub struct OpenPitPretradePreTradeLockEntries {
    items: Vec<OpenPitPretradePreTradeLockEntry>,
}

/// Returns a caller-owned snapshot of every `(policy_group_id, price)` record
/// stored in `lock`, in iteration order (default-group records first, then each
/// non-default group in insertion order).
///
/// `lock` must be a valid non-null handle. Passing null aborts the process.
///
/// Cleanup:
/// - the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock_entries` exactly once.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_entries(
    lock: *const OpenPitPretradePreTradeLock,
) -> *mut OpenPitPretradePreTradeLockEntries {
    assert!(!lock.is_null(), "lock pointer must be non-null");
    let lock_ref = unsafe { &*lock };
    let items: Vec<OpenPitPretradePreTradeLockEntry> = lock_ref
        .inner
        .entries()
        .map(|(group_id, price)| OpenPitPretradePreTradeLockEntry {
            policy_group_id: group_id.value(),
            price: export_price(price),
        })
        .collect();
    Box::into_raw(Box::new(OpenPitPretradePreTradeLockEntries { items }))
}

/// Releases a caller-owned lock entry snapshot.
///
/// Contract:
/// - `handle` must be a valid non-null pointer;
/// - this function always succeeds.
#[no_mangle]
pub extern "C" fn openpit_destroy_pretrade_pre_trade_lock_entries(
    handle: *mut OpenPitPretradePreTradeLockEntries,
) {
    assert!(!handle.is_null(), "lock entries pointer must be non-null");
    unsafe { drop(Box::from_raw(handle)) };
}

/// Borrows a read-only view of a lock entry snapshot.
///
/// `handle` must be a valid non-null pointer; violating this triggers a panic.
///
/// Returns an unset view (`ptr == null`, `len == 0`) when the snapshot is
/// empty. The view remains valid only while `handle` is alive.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_entries_view(
    handle: *const OpenPitPretradePreTradeLockEntries,
) -> OpenPitPretradePreTradeLockEntriesView {
    assert!(!handle.is_null(), "lock entries pointer must be non-null");
    OpenPitPretradePreTradeLockEntriesView::from_slice(unsafe { &*handle }.items.as_slice())
}

/// Serializes the lock as MessagePack.
///
/// Success:
/// - returns a non-null caller-owned `OpenPitSharedBytes` carrying the
///   MessagePack payload.
///
/// Error:
/// - returns null when `lock` is null or when the encoder fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_shared_bytes` exactly once.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_to_msgpack(
    lock: *const OpenPitPretradePreTradeLock,
    out_error: OpenPitOutError,
) -> *mut OpenPitSharedBytes {
    if lock.is_null() {
        write_error(out_error, "lock pointer must be non-null");
        return std::ptr::null_mut();
    }
    let lock_ref = unsafe { &*lock };
    match rmp_serde::to_vec(&lock_ref.inner) {
        Ok(bytes) => OpenPitSharedBytes::new_handle(bytes),
        Err(error) => {
            write_error(
                out_error,
                format!("lock msgpack encode failed: {error}").as_str(),
            );
            std::ptr::null_mut()
        }
    }
}

/// Builds a new lock from a MessagePack payload.
///
/// Success:
/// - returns a non-null caller-owned lock handle.
///
/// Error:
/// - returns null when `data_ptr` is null or when the payload cannot be
///   decoded;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
#[no_mangle]
pub extern "C" fn openpit_create_pretrade_pre_trade_lock_from_msgpack(
    data_ptr: *const u8,
    data_len: usize,
    out_error: OpenPitOutError,
) -> *mut OpenPitPretradePreTradeLock {
    if data_ptr.is_null() {
        write_error(out_error, "msgpack input pointer must be non-null");
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    match rmp_serde::from_slice::<PreTradeLock>(slice) {
        Ok(lock) => OpenPitPretradePreTradeLock::from_inner(lock),
        Err(error) => {
            write_error(
                out_error,
                format!("lock msgpack decode failed: {error}").as_str(),
            );
            std::ptr::null_mut()
        }
    }
}

/// Serializes the lock as compact JSON.
///
/// Success:
/// - returns a non-null caller-owned `OpenPitSharedString` carrying the JSON
///   payload.
///
/// Error:
/// - returns null when `lock` is null or when the encoder fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_shared_string` exactly once.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_to_json(
    lock: *const OpenPitPretradePreTradeLock,
    out_error: OpenPitOutError,
) -> *mut crate::string::OpenPitSharedString {
    if lock.is_null() {
        write_error(out_error, "lock pointer must be non-null");
        return std::ptr::null_mut();
    }
    let lock_ref = unsafe { &*lock };
    match serde_json::to_string(&lock_ref.inner) {
        Ok(text) => crate::string::OpenPitSharedString::new_handle(text.as_str()),
        Err(error) => {
            write_error(
                out_error,
                format!("lock json encode failed: {error}").as_str(),
            );
            std::ptr::null_mut()
        }
    }
}

/// Builds a new lock from a JSON payload produced by
/// `openpit_pretrade_pre_trade_lock_to_json` (or any compatible serializer).
///
/// `text_ptr`/`text_len` describe a UTF-8 byte sequence.
///
/// Success:
/// - returns a non-null caller-owned lock handle.
///
/// Error:
/// - returns null when `text_ptr` is null or when the payload cannot be
///   decoded (invalid UTF-8 or invalid lock JSON);
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
#[no_mangle]
pub extern "C" fn openpit_create_pretrade_pre_trade_lock_from_json(
    text_ptr: *const u8,
    text_len: usize,
    out_error: OpenPitOutError,
) -> *mut OpenPitPretradePreTradeLock {
    if text_ptr.is_null() {
        write_error(out_error, "json input pointer must be non-null");
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(text_ptr, text_len) };
    let text = match std::str::from_utf8(slice) {
        Ok(text) => text,
        Err(error) => {
            write_error(
                out_error,
                format!("json input is not valid UTF-8: {error}").as_str(),
            );
            return std::ptr::null_mut();
        }
    };
    match serde_json::from_str::<PreTradeLock>(text) {
        Ok(lock) => OpenPitPretradePreTradeLock::from_inner(lock),
        Err(error) => {
            write_error(
                out_error,
                format!("lock json decode failed: {error}").as_str(),
            );
            std::ptr::null_mut()
        }
    }
}

/// Serializes the lock as CBOR.
///
/// Success:
/// - returns a non-null caller-owned `OpenPitSharedBytes` carrying the CBOR
///   payload.
///
/// Error:
/// - returns null when `lock` is null or when the encoder fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_shared_bytes` exactly once.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_to_cbor(
    lock: *const OpenPitPretradePreTradeLock,
    out_error: OpenPitOutError,
) -> *mut OpenPitSharedBytes {
    if lock.is_null() {
        write_error(out_error, "lock pointer must be non-null");
        return std::ptr::null_mut();
    }
    let lock_ref = unsafe { &*lock };
    let mut buffer = Vec::new();
    match ciborium::ser::into_writer(&lock_ref.inner, &mut buffer) {
        Ok(()) => OpenPitSharedBytes::new_handle(buffer),
        Err(error) => {
            write_error(
                out_error,
                format!("lock cbor encode failed: {error}").as_str(),
            );
            std::ptr::null_mut()
        }
    }
}

/// Builds a new lock from a CBOR payload.
///
/// Success:
/// - returns a non-null caller-owned lock handle.
///
/// Error:
/// - returns null when `data_ptr` is null or when the payload cannot be
///   decoded;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
#[no_mangle]
pub extern "C" fn openpit_create_pretrade_pre_trade_lock_from_cbor(
    data_ptr: *const u8,
    data_len: usize,
    out_error: OpenPitOutError,
) -> *mut OpenPitPretradePreTradeLock {
    if data_ptr.is_null() {
        write_error(out_error, "cbor input pointer must be non-null");
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    match ciborium::de::from_reader::<PreTradeLock, _>(slice) {
        Ok(lock) => OpenPitPretradePreTradeLock::from_inner(lock),
        Err(error) => {
            write_error(
                out_error,
                format!("lock cbor decode failed: {error}").as_str(),
            );
            std::ptr::null_mut()
        }
    }
}

/// Serializes the lock using the in-process binary-stable raw layout.
///
/// `lock` must be a valid non-null handle; violating this triggers a panic.
///
/// Success:
/// - always returns a non-null caller-owned `OpenPitSharedBytes` carrying the
///   raw payload.
///
/// Cleanup:
/// - the caller MUST release the returned handle with
///   `openpit_destroy_shared_bytes` exactly once.
#[no_mangle]
pub extern "C" fn openpit_pretrade_pre_trade_lock_to_raw(
    lock: *const OpenPitPretradePreTradeLock,
) -> *mut OpenPitSharedBytes {
    assert!(!lock.is_null(), "lock pointer must be non-null");
    let lock_ref = unsafe { &*lock };
    OpenPitSharedBytes::new_handle(encode_raw(&lock_ref.inner))
}

/// Builds a new lock from a raw payload produced by
/// `openpit_pretrade_pre_trade_lock_to_raw`.
///
/// Success:
/// - returns a non-null caller-owned lock handle.
///
/// Error:
/// - returns null when `data_ptr` is null or when the payload cannot be
///   decoded;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - on success the caller MUST release the returned handle with
///   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
#[no_mangle]
pub extern "C" fn openpit_create_pretrade_pre_trade_lock_from_raw(
    data_ptr: *const u8,
    data_len: usize,
    out_error: OpenPitOutError,
) -> *mut OpenPitPretradePreTradeLock {
    if data_ptr.is_null() {
        write_error(out_error, "raw input pointer must be non-null");
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    match decode_raw(slice) {
        Ok(lock) => OpenPitPretradePreTradeLock::from_inner(lock),
        Err(error) => {
            write_error(out_error, error.as_str());
            std::ptr::null_mut()
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Raw encoder/decoder (in-process binary-stable layout)

const SERIALIZED_DECIMAL_SIZE: usize = 16;

fn encode_raw(lock: &PreTradeLock) -> Vec<u8> {
    let default_prices: Vec<openpit::param::Price> =
        lock.prices_of(openpit::DEFAULT_POLICY_GROUP_ID).collect();

    let mut groups: Vec<(PolicyGroupId, Vec<openpit::param::Price>)> = Vec::new();
    for (group_id, price) in lock.entries() {
        if group_id == openpit::DEFAULT_POLICY_GROUP_ID {
            continue;
        }
        if let Some(position) = groups.iter().position(|(g, _)| *g == group_id) {
            groups[position].1.push(price);
        } else {
            groups.push((group_id, vec![price]));
        }
    }

    let groups_payload_size: usize = groups
        .iter()
        .map(|(_, prices)| 2 + 4 + prices.len() * SERIALIZED_DECIMAL_SIZE)
        .sum();
    let total_size = 4 + default_prices.len() * SERIALIZED_DECIMAL_SIZE + 4 + groups_payload_size;

    let mut buffer = Vec::with_capacity(total_size);
    buffer.extend_from_slice(&(default_prices.len() as u32).to_le_bytes());
    for price in &default_prices {
        buffer.extend_from_slice(&price.to_decimal().serialize());
    }
    buffer.extend_from_slice(&(groups.len() as u32).to_le_bytes());
    for (group_id, prices) in &groups {
        buffer.extend_from_slice(&group_id.value().to_le_bytes());
        buffer.extend_from_slice(&(prices.len() as u32).to_le_bytes());
        for price in prices {
            buffer.extend_from_slice(&price.to_decimal().serialize());
        }
    }
    buffer
}

fn decode_raw(bytes: &[u8]) -> Result<PreTradeLock, String> {
    let mut cursor = RawCursor::new(bytes);
    let mut lock = PreTradeLock::new();

    let default_count = cursor.read_u32()?;
    for _ in 0..default_count {
        let price = cursor.read_price()?;
        lock.push(openpit::DEFAULT_POLICY_GROUP_ID, price);
    }

    let group_count = cursor.read_u32()?;
    for _ in 0..group_count {
        let group_id = PolicyGroupId::new(cursor.read_u16()?);
        let prices_count = cursor.read_u32()?;
        for _ in 0..prices_count {
            let price = cursor.read_price()?;
            lock.push(group_id, price);
        }
    }

    if !cursor.is_empty() {
        return Err(format!(
            "raw lock decode: {} trailing bytes",
            cursor.remaining()
        ));
    }
    Ok(lock)
}

struct RawCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> RawCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        let slice = self.take(2)?;
        Ok(u16::from_le_bytes([slice[0], slice[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let slice = self.take(4)?;
        Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn read_price(&mut self) -> Result<openpit::param::Price, String> {
        let slice = self.take(SERIALIZED_DECIMAL_SIZE)?;
        let mut buf = [0u8; SERIALIZED_DECIMAL_SIZE];
        buf.copy_from_slice(slice);
        let decimal = Decimal::deserialize(buf);
        Ok(openpit::param::Price::new(decimal))
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        if self.pos + len > self.bytes.len() {
            return Err(format!(
                "raw lock decode: unexpected end of input (need {} bytes at offset {}, total {})",
                len,
                self.pos,
                self.bytes.len()
            ));
        }
        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn is_empty(&self) -> bool {
        self.pos == self.bytes.len()
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytes::{openpit_destroy_shared_bytes, openpit_shared_bytes_view};
    use crate::string::{openpit_destroy_shared_string, OpenPitSharedString};

    fn price_of(value: &str) -> OpenPitParamPrice {
        let parsed = openpit::param::Price::from_str(value).expect("price must be valid");
        OpenPitParamPrice(OpenPitParamDecimal::from_decimal(parsed.to_decimal()))
    }

    #[test]
    fn create_destroy_roundtrip() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        assert!(!lock.is_null());
        assert_eq!(openpit_pretrade_pre_trade_lock_len(lock), 0);
        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    fn push_and_clone_preserve_state() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        assert!(openpit_pretrade_pre_trade_lock_push(
            lock,
            0,
            price_of("185"),
            &mut err
        ));
        assert!(openpit_pretrade_pre_trade_lock_push(
            lock,
            7,
            price_of("200"),
            &mut err
        ));
        assert!(openpit_pretrade_pre_trade_lock_push(
            lock,
            7,
            price_of("201"),
            &mut err
        ));
        assert!(err.is_null());
        assert_eq!(openpit_pretrade_pre_trade_lock_len(lock), 3);

        let cloned = openpit_pretrade_pre_trade_lock_clone(lock);
        assert!(!cloned.is_null());
        assert_eq!(openpit_pretrade_pre_trade_lock_len(cloned), 3);

        openpit_destroy_pretrade_pre_trade_lock(cloned);
        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    fn prices_of_returns_explicit_status() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let first = price_of("200");
        let second = price_of("201");
        openpit_pretrade_pre_trade_lock_push(lock, 7, first, &mut err);
        openpit_pretrade_pre_trade_lock_push(lock, 7, second, &mut err);

        let mut out_price = OpenPitParamPrice::default();
        let mut out_prices: *mut OpenPitPretradePreTradeLockPrices = std::ptr::null_mut();
        let status = openpit_pretrade_pre_trade_lock_prices_of(
            lock,
            7,
            &mut out_price,
            &mut out_prices,
            &mut err,
        );
        assert_eq!(status, OpenPitPretradePreTradeLockPricesStatus::List);
        assert!(out_price == OpenPitParamPrice::default());
        assert!(!out_prices.is_null());
        let view = openpit_pretrade_pre_trade_lock_prices_view(out_prices);
        assert!(!view.ptr.is_null());
        assert_eq!(view.len, 2);
        let values = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        assert_eq!(values, &[first, second]);
        openpit_destroy_pretrade_pre_trade_lock_prices(out_prices);

        let missing = openpit_pretrade_pre_trade_lock_prices_of(
            lock,
            99,
            &mut out_price,
            &mut out_prices,
            &mut err,
        );
        assert_eq!(missing, OpenPitPretradePreTradeLockPricesStatus::Empty);
        assert!(out_prices.is_null());
        assert!(err.is_null());

        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    fn prices_of_returns_single_price_without_allocated_list() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let price = price_of("185");
        openpit_pretrade_pre_trade_lock_push(lock, 0, price, &mut err);

        let mut out_price = OpenPitParamPrice::default();
        let mut out_prices: *mut OpenPitPretradePreTradeLockPrices = std::ptr::null_mut();
        let status = openpit_pretrade_pre_trade_lock_prices_of(
            lock,
            0,
            &mut out_price,
            &mut out_prices,
            &mut err,
        );
        assert_eq!(status, OpenPitPretradePreTradeLockPricesStatus::One);
        assert_eq!(out_price, price);
        assert!(out_prices.is_null());
        assert!(err.is_null());

        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    fn raw_round_trip_via_ffi() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        openpit_pretrade_pre_trade_lock_push(lock, 0, price_of("185"), &mut err);
        openpit_pretrade_pre_trade_lock_push(lock, 7, price_of("200"), &mut err);
        openpit_pretrade_pre_trade_lock_push(lock, 7, price_of("201"), &mut err);

        let raw_handle = openpit_pretrade_pre_trade_lock_to_raw(lock);
        assert!(!raw_handle.is_null());
        let view = openpit_shared_bytes_view(raw_handle);
        assert!(view.len > 0);

        let restored =
            openpit_create_pretrade_pre_trade_lock_from_raw(view.ptr, view.len, &mut err);
        assert!(!restored.is_null());
        assert_eq!(openpit_pretrade_pre_trade_lock_len(restored), 3);

        openpit_destroy_pretrade_pre_trade_lock(restored);
        openpit_destroy_shared_bytes(raw_handle);
        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    fn raw_empty_lock_decodes_to_empty() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        let raw_handle = openpit_pretrade_pre_trade_lock_to_raw(lock);
        let view = openpit_shared_bytes_view(raw_handle);

        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let restored =
            openpit_create_pretrade_pre_trade_lock_from_raw(view.ptr, view.len, &mut err);
        assert!(!restored.is_null());
        assert_eq!(openpit_pretrade_pre_trade_lock_len(restored), 0);

        openpit_destroy_pretrade_pre_trade_lock(restored);
        openpit_destroy_shared_bytes(raw_handle);
        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    fn msgpack_round_trip_via_ffi() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        openpit_pretrade_pre_trade_lock_push(lock, 0, price_of("185"), &mut err);

        let bytes_handle = openpit_pretrade_pre_trade_lock_to_msgpack(lock, &mut err);
        assert!(!bytes_handle.is_null());
        let view = openpit_shared_bytes_view(bytes_handle);
        assert!(view.len > 0);

        let restored =
            openpit_create_pretrade_pre_trade_lock_from_msgpack(view.ptr, view.len, &mut err);
        assert!(!restored.is_null());
        assert_eq!(openpit_pretrade_pre_trade_lock_len(restored), 1);

        openpit_destroy_pretrade_pre_trade_lock(restored);
        openpit_destroy_shared_bytes(bytes_handle);
        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    fn raw_to_msgpack_cross_format_round_trip() {
        let lock = openpit_create_pretrade_pre_trade_lock();
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        openpit_pretrade_pre_trade_lock_push(lock, 0, price_of("185"), &mut err);
        openpit_pretrade_pre_trade_lock_push(lock, 4, price_of("400"), &mut err);

        let msgpack_handle = openpit_pretrade_pre_trade_lock_to_msgpack(lock, &mut err);
        let msgpack_view = openpit_shared_bytes_view(msgpack_handle);
        let restored = openpit_create_pretrade_pre_trade_lock_from_msgpack(
            msgpack_view.ptr,
            msgpack_view.len,
            &mut err,
        );
        assert!(!restored.is_null());

        let raw_handle = openpit_pretrade_pre_trade_lock_to_raw(restored);
        let raw_view = openpit_shared_bytes_view(raw_handle);
        let final_lock =
            openpit_create_pretrade_pre_trade_lock_from_raw(raw_view.ptr, raw_view.len, &mut err);
        assert!(!final_lock.is_null());
        assert_eq!(openpit_pretrade_pre_trade_lock_len(final_lock), 2);

        openpit_destroy_pretrade_pre_trade_lock(final_lock);
        openpit_destroy_shared_bytes(raw_handle);
        openpit_destroy_pretrade_pre_trade_lock(restored);
        openpit_destroy_shared_bytes(msgpack_handle);
        openpit_destroy_pretrade_pre_trade_lock(lock);
    }

    #[test]
    #[allow(clippy::manual_c_str_literals)]
    fn from_raw_rejects_truncated_input() {
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let result = openpit_create_pretrade_pre_trade_lock_from_raw(
            b"\x05\x00\x00\x00".as_ptr(),
            4,
            &mut err,
        );
        assert!(result.is_null());
        assert!(!err.is_null());
        openpit_destroy_shared_string(err);
    }

    #[test]
    fn clone_of_null_is_null() {
        assert!(openpit_pretrade_pre_trade_lock_clone(std::ptr::null()).is_null());
    }

    #[test]
    fn destroy_null_is_noop() {
        openpit_destroy_pretrade_pre_trade_lock(std::ptr::null_mut());
    }

    #[test]
    fn push_returns_error_on_null_lock() {
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let ok = openpit_pretrade_pre_trade_lock_push(
            std::ptr::null_mut(),
            0,
            OpenPitParamPrice::default(),
            &mut err,
        );
        assert!(!ok);
        assert!(!err.is_null());
        openpit_destroy_shared_string(err);
    }

    #[test]
    fn to_msgpack_returns_error_on_null_lock() {
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let result = openpit_pretrade_pre_trade_lock_to_msgpack(std::ptr::null(), &mut err);
        assert!(result.is_null());
        assert!(!err.is_null());
        openpit_destroy_shared_string(err);
    }

    #[test]
    fn from_raw_returns_error_on_null_input() {
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let result = openpit_create_pretrade_pre_trade_lock_from_raw(std::ptr::null(), 0, &mut err);
        assert!(result.is_null());
        assert!(!err.is_null());
        openpit_destroy_shared_string(err);
    }

    #[test]
    fn from_msgpack_returns_error_on_null_input() {
        let mut err: *mut OpenPitSharedString = std::ptr::null_mut();
        let result =
            openpit_create_pretrade_pre_trade_lock_from_msgpack(std::ptr::null(), 0, &mut err);
        assert!(result.is_null());
        assert!(!err.is_null());
        openpit_destroy_shared_string(err);
    }
}
