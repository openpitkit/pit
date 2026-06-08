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

use crate::define_optional;
use crate::last_error::{write_error, OpenPitOutError};
use crate::OpenPitStringView;
use openpit::param::AccountGroupId;

/// Stable account-group identifier type for FFI payloads.
///
/// WARNING:
/// Use exactly one account-group-id source model per runtime:
/// - either purely numeric IDs
///   (`openpit_create_param_account_group_id_from_uint32`),
/// - or purely string-derived IDs
///   (`openpit_create_param_account_group_id_from_string`).
///
/// Do not mix both models in the same runtime state. A hashed string value can
/// coincide with a direct numeric ID, collapsing two distinct groups into one
/// key.
pub type OpenPitParamAccountGroupId = u32;

define_optional!(
    optional = OpenPitParamAccountGroupIdOptional,
    value = OpenPitParamAccountGroupId
);

/// The reserved default account-group identifier. Every account belongs to this
/// group until it is registered into another one, so no constructor may produce
/// it. Mirrors `openpit::param::DEFAULT_ACCOUNT_GROUP`.
pub const OPENPIT_DEFAULT_ACCOUNT_GROUP: OpenPitParamAccountGroupId = 0;

const _: () =
    assert!(OPENPIT_DEFAULT_ACCOUNT_GROUP == openpit::param::DEFAULT_ACCOUNT_GROUP.as_u32());

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Constructs an account-group identifier from a 32-bit integer.
///
/// This is a direct numeric mapping with no collision risk.
///
/// The value `0` is reserved for the default account group
/// (`OPENPIT_DEFAULT_ACCOUNT_GROUP`) and is rejected: every account already
/// belongs to that group implicitly, so no external input may name it.
///
/// WARNING:
/// Do not mix IDs produced by this function with IDs produced by
/// `openpit_create_param_account_group_id_from_string` in the same runtime
/// state.
///
/// Contract:
/// - returns `true` and writes a stable account-group identifier to `out`
///   on success;
/// - returns `false` on the reserved value (`0`) and optionally writes an
///   error message to `out_error`.
///
/// # Safety
///
/// `out` must be either null or a valid writable pointer.
pub unsafe extern "C" fn openpit_create_param_account_group_id_from_uint32(
    value: u32,
    out: *mut OpenPitParamAccountGroupId,
    out_error: OpenPitOutError,
) -> bool {
    match AccountGroupId::from_u32(value) {
        Ok(id) => {
            if !out.is_null() {
                unsafe { *out = id.as_u32() };
            }
            true
        }
        Err(e) => {
            write_error(out_error, &e.to_string());
            false
        }
    }
}

#[no_mangle]
/// Constructs an account-group identifier from a UTF-8 byte sequence using
/// FNV-1a 32-bit hashing.
///
/// The bytes are read only for the duration of the call. No trailing zero byte
/// is required.
///
/// Collision note:
/// - different group strings can map to the same identifier;
/// - for `n` distinct group strings the probability of at least one collision
///   is approximately `n^2 / (2 * 2^32)`.
/// - if collision risk is unacceptable, keep your own collision-free
///   string-to-integer mapping and use
///   `openpit_create_param_account_group_id_from_uint32`.
///
/// WARNING:
/// Do not mix IDs produced by this function with IDs produced by
/// `openpit_create_param_account_group_id_from_uint32` in the same runtime
/// state.
///
/// Contract:
/// - returns `true` and writes a stable account-group identifier to `out`
///   on success;
/// - returns `false` on invalid input (empty string) and optionally writes
///   an error message to `out_error`.
///
/// # Safety
///
/// `value.ptr` must be non-null and point to at least `value.len` readable
/// UTF-8 bytes when `value.len > 0`.
pub unsafe extern "C" fn openpit_create_param_account_group_id_from_string(
    value: OpenPitStringView,
    out: *mut OpenPitParamAccountGroupId,
    out_error: OpenPitOutError,
) -> bool {
    let bytes: &[u8] = if value.ptr.is_null() || value.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(value.ptr, value.len) }
    };
    let utf8 = String::from_utf8_lossy(bytes);
    let s = utf8.as_ref();
    match AccountGroupId::from_str(s) {
        Ok(id) => {
            if !out.is_null() {
                unsafe { *out = id.as_u32() };
            }
            true
        }
        Err(e) => {
            write_error(out_error, &e.to_string());
            false
        }
    }
}

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn view(s: &str) -> OpenPitStringView {
        OpenPitStringView::from_utf8(s)
    }

    #[test]
    fn from_uint32_is_stable_passthrough() {
        let mut out: u32 = 0;
        let ok = unsafe {
            openpit_create_param_account_group_id_from_uint32(42, &mut out, std::ptr::null_mut())
        };
        assert!(ok);
        assert_eq!(out, 42);
        let ok = unsafe {
            openpit_create_param_account_group_id_from_uint32(
                u32::MAX,
                &mut out,
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert_eq!(out, u32::MAX);
    }

    #[test]
    fn from_uint32_rejects_reserved_default() {
        let mut out: u32 = 7;
        let ok = unsafe {
            openpit_create_param_account_group_id_from_uint32(0, &mut out, std::ptr::null_mut())
        };
        assert!(!ok);
        assert_eq!(out, 7);
    }

    #[test]
    fn from_string_happy_path() {
        let mut out: u32 = 0;
        let ok = unsafe {
            openpit_create_param_account_group_id_from_string(
                view("desk-1"),
                &mut out,
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert_ne!(out, 0);
    }

    #[test]
    fn from_string_same_string_stable() {
        let mut a: u32 = 0;
        let mut b: u32 = 0;
        unsafe {
            openpit_create_param_account_group_id_from_string(
                view("group-a"),
                &mut a,
                std::ptr::null_mut(),
            );
            openpit_create_param_account_group_id_from_string(
                view("group-a"),
                &mut b,
                std::ptr::null_mut(),
            );
        }
        assert_eq!(a, b);
    }

    #[test]
    fn from_string_rejects_empty() {
        let mut out: u32 = 0;
        let ok = unsafe {
            openpit_create_param_account_group_id_from_string(
                view(""),
                &mut out,
                std::ptr::null_mut(),
            )
        };
        assert!(!ok);
    }

    #[test]
    fn from_string_rejects_whitespace() {
        let mut out: u32 = 0;
        let ok = unsafe {
            openpit_create_param_account_group_id_from_string(
                view("   "),
                &mut out,
                std::ptr::null_mut(),
            )
        };
        assert!(!ok);
    }
}
