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

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Non-owning byte slice view.
///
/// Lifetime contract:
/// - `ptr` points to `len` readable bytes;
/// - the memory is valid while the original object is alive;
/// - the caller must not free or mutate memory behind `ptr`;
/// - if the caller needs to retain the bytes beyond that announced lifetime,
///   the caller must copy them.
pub struct OpenPitBytesView {
    pub ptr: *const u8,
    pub len: usize,
}

impl OpenPitBytesView {
    pub const fn not_set() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    pub fn from_slice(value: &[u8]) -> Self {
        Self {
            ptr: value.as_ptr(),
            len: value.len(),
        }
    }
}

/// Owning shared-bytes handle.
///
/// Use this type when an FFI function needs to hand a binary payload to the
/// caller whose lifetime must extend beyond the single FFI call.
///
/// Ownership contract:
/// - every non-null `*mut OpenPitSharedBytes` returned through FFI is owned by
///   the caller;
/// - the caller MUST release it with `openpit_destroy_shared_bytes` when no
///   longer needed; failing to do so leaks the underlying allocation.
pub struct OpenPitSharedBytes {
    inner: Vec<u8>,
}

impl OpenPitSharedBytes {
    pub fn new_handle(bytes: Vec<u8>) -> *mut Self {
        Box::into_raw(Box::new(Self { inner: bytes }))
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.inner
    }
}

#[no_mangle]
/// Releases a `OpenPitSharedBytes` handle.
///
/// Null input is a no-op.
pub extern "C" fn openpit_destroy_shared_bytes(handle: *mut OpenPitSharedBytes) {
    if handle.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(handle)) };
}

#[no_mangle]
/// Borrows a read-only view of the bytes stored in the handle.
///
/// Returns an unset view (`ptr == null`, `len == 0`) when `handle` is null.
pub extern "C" fn openpit_shared_bytes_view(handle: *const OpenPitSharedBytes) -> OpenPitBytesView {
    if handle.is_null() {
        return OpenPitBytesView::not_set();
    }
    OpenPitBytesView::from_slice(unsafe { &*handle }.as_slice())
}

#[cfg(test)]
mod tests {
    use super::{
        openpit_destroy_shared_bytes, openpit_shared_bytes_view, OpenPitBytesView,
        OpenPitSharedBytes,
    };

    #[test]
    fn view_not_set_is_null() {
        let view = OpenPitBytesView::not_set();
        assert!(view.ptr.is_null());
        assert_eq!(view.len, 0);
    }

    #[test]
    fn new_handle_roundtrips_bytes() {
        let handle = OpenPitSharedBytes::new_handle(vec![1, 2, 3, 255]);
        assert!(!handle.is_null());
        let view = openpit_shared_bytes_view(handle);
        let slice = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        assert_eq!(slice, &[1, 2, 3, 255]);
        openpit_destroy_shared_bytes(handle);
    }

    #[test]
    fn null_inputs_are_safe() {
        let view = openpit_shared_bytes_view(std::ptr::null());
        assert!(view.ptr.is_null());
        assert_eq!(view.len, 0);
        openpit_destroy_shared_bytes(std::ptr::null_mut());
    }
}
