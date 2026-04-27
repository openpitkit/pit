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

use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Non-owning UTF-8 string view.
///
/// This type never owns memory. It borrows bytes from another object.
///
/// Lifetime contract:
/// - `ptr` points to `len` readable bytes;
/// - the memory is valid while the original object is alive and the source
///   string has not been modified;
/// - the caller must not free or mutate memory behind `ptr`.
/// - if the caller needs to retain the string beyond that announced lifetime,
///   the caller must copy the bytes.
pub struct PitStringView {
    /// Pointer to the first UTF-8 byte.
    pub ptr: *const u8,
    /// Number of bytes at `ptr`.
    pub len: usize,
}

impl PitStringView {
    pub const fn not_set() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    pub fn from_utf8(value: &str) -> Self {
        Self {
            ptr: value.as_ptr(),
            len: value.len(),
        }
    }
}

/// Owning shared-string handle.
///
/// Use this type when an FFI function needs to hand a string to the caller
/// whose lifetime must extend beyond the single FFI call and whose storage
/// must not depend on thread-local state remaining intact on the reader side.
///
/// Ownership contract:
/// - every non-null `*mut PitSharedString` returned through FFI is owned by
///   the caller;
/// - the caller MUST release it with `pit_destroy_shared_string` when no
///   longer needed; failing to do so leaks the underlying allocation;
/// - the handle internally holds a reference-counted copy of the string, so
///   multiple live handles pointing at the same original value are safe and
///   independent; destroying one handle does not affect the others.
///
/// Read contract:
/// - read the bytes with `pit_shared_string_view`;
/// - the returned `PitStringView` is valid while this specific handle is
///   alive and must not outlive the call to `pit_destroy_shared_string`
///   for this handle.
///
/// Threading contract:
/// - the handle itself is safe to move to and read from any thread once the
///   caller has received it; no thread-local state is consulted when reading
///   or destroying.
pub struct PitSharedString {
    inner: Arc<String>,
}

impl PitSharedString {
    /// Builds a new handle that holds a fresh shared copy of `value`.
    ///
    /// Returns a heap-allocated pointer the caller owns.
    pub fn new_handle(value: &str) -> *mut Self {
        Self::from_arc(Arc::new(value.to_owned()))
    }

    /// Builds a new handle around an existing `Arc<String>` without copying
    /// the underlying bytes. Each handle bumps the refcount independently.
    pub fn from_arc(inner: Arc<String>) -> *mut Self {
        Box::into_raw(Box::new(Self { inner }))
    }

    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }
}

#[no_mangle]
/// Releases a `PitSharedString` handle.
///
/// Null input is a no-op.
///
/// After this call, the handle and any `PitStringView` previously obtained
/// from it are invalid and must not be used.
pub extern "C" fn pit_destroy_shared_string(handle: *mut PitSharedString) {
    if handle.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(handle)) };
}

#[no_mangle]
/// Borrows a read-only view of the bytes stored in the handle.
///
/// Returns an unset view (`ptr == null`, `len == 0`) when `handle` is null.
///
/// The returned view is valid only while `handle` remains alive. The caller
/// must copy the bytes if they must outlive the handle.
pub extern "C" fn pit_shared_string_view(handle: *const PitSharedString) -> PitStringView {
    if handle.is_null() {
        return PitStringView::not_set();
    }
    PitStringView::from_utf8(unsafe { &*handle }.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        pit_destroy_shared_string, pit_shared_string_view, PitSharedString, PitStringView,
    };

    #[test]
    fn pit_string_view_helpers_are_stable() {
        let not_set = PitStringView::not_set();
        assert!(not_set.ptr.is_null());
        assert_eq!(not_set.len, 0);

        let value = "openpit";
        let view = PitStringView::from_utf8(value);
        assert!(!view.ptr.is_null());
        assert_eq!(view.len, value.len());
    }

    fn view_to_string(view: crate::PitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes).expect("utf8").to_owned()
    }

    #[test]
    fn new_handle_roundtrips_bytes() {
        let handle = PitSharedString::new_handle("hello");
        assert!(!handle.is_null());
        assert_eq!(view_to_string(pit_shared_string_view(handle)), "hello");
        pit_destroy_shared_string(handle);
    }

    #[test]
    fn null_inputs_are_safe() {
        let view = pit_shared_string_view(std::ptr::null());
        assert!(view.ptr.is_null());
        assert_eq!(view.len, 0);

        pit_destroy_shared_string(std::ptr::null_mut());
    }

    #[test]
    fn independent_handles_can_be_dropped_in_any_order() {
        let shared = std::sync::Arc::new("shared".to_owned());
        let a = PitSharedString::from_arc(shared.clone());
        let b = PitSharedString::from_arc(shared);
        assert_eq!(view_to_string(pit_shared_string_view(a)), "shared");
        pit_destroy_shared_string(a);
        assert_eq!(view_to_string(pit_shared_string_view(b)), "shared");
        pit_destroy_shared_string(b);
    }
}
