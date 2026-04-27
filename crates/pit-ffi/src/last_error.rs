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

use crate::string::PitSharedString;

/// Error out-pointer used by fallible FFI calls.
pub type PitOutError = *mut *mut PitSharedString;

/// Writes a caller-owned shared-string error handle into `out_error`.
///
/// Passing null is allowed and means the caller does not want the message.
pub fn write_error(out_error: PitOutError, msg: &str) {
    if out_error.is_null() {
        return;
    }
    unsafe {
        *out_error = PitSharedString::new_handle(msg);
    }
}

#[macro_export]
macro_rules! write_error_format {
    ($out_error:expr, $fmt:expr, $($arg:tt)*) => {
        $crate::last_error::write_error($out_error, &format!($fmt, $($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use crate::string::{pit_destroy_shared_string, pit_shared_string_view};
    use crate::PitStringView;

    use super::write_error;

    fn view_to_string(view: PitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes)
            .expect("error value must be valid utf-8")
            .to_owned()
    }

    #[test]
    fn write_error_stores_shared_string_when_out_pointer_is_present() {
        let mut out_error = std::ptr::null_mut();

        write_error(&mut out_error, "transport failure");

        assert!(!out_error.is_null());
        assert_eq!(
            view_to_string(pit_shared_string_view(out_error)),
            "transport failure"
        );
        pit_destroy_shared_string(out_error);
    }

    #[test]
    fn write_error_accepts_null_out_pointer() {
        write_error(std::ptr::null_mut(), "transport failure");
    }
}
