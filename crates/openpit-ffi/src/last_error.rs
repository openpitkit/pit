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

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::string::{openpit_destroy_shared_string, OpenPitSharedString};

/// Error out-pointer used by fallible FFI calls.
pub type OpenPitOutError = *mut *mut OpenPitSharedString;

/// Parameter error code transported through FFI.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OpenPitParamErrorCode {
    /// Error code is not specified.
    #[default]
    Unspecified = openpit::param::ErrorCode::Unspecified as u32,
    /// Value must be non-negative.
    Negative = openpit::param::ErrorCode::Negative as u32,
    /// Division by zero.
    DivisionByZero = openpit::param::ErrorCode::DivisionByZero as u32,
    /// Arithmetic overflow.
    Overflow = openpit::param::ErrorCode::Overflow as u32,
    /// Arithmetic underflow.
    Underflow = openpit::param::ErrorCode::Underflow as u32,
    /// Invalid float value.
    InvalidFloat = openpit::param::ErrorCode::InvalidFloat as u32,
    /// Invalid textual format.
    InvalidFormat = openpit::param::ErrorCode::InvalidFormat as u32,
    /// Invalid price value.
    InvalidPrice = openpit::param::ErrorCode::InvalidPrice as u32,
    /// Invalid leverage value.
    InvalidLeverage = openpit::param::ErrorCode::InvalidLeverage as u32,
    /// Asset identifier is empty.
    AssetEmpty = openpit::param::ErrorCode::AssetEmpty as u32,
    /// Account identifier string is empty.
    AccountIdEmpty = openpit::param::ErrorCode::AccountIdEmpty as u32,
    /// Catch-all code for unknown cases.
    Other = openpit::param::ErrorCode::Other as u32,
}

/// Caller-owned parameter error container.
#[repr(C)]
#[derive(Debug)]
pub struct OpenPitParamError {
    /// Stable machine-readable error code.
    pub code: OpenPitParamErrorCode,
    /// Human-readable message allocated as shared string.
    pub message: *mut OpenPitSharedString,
}

/// Parameter error out-pointer used by fallible param FFI calls.
pub type OpenPitOutParamError = *mut *mut OpenPitParamError;

/// Writes a caller-owned shared-string error handle into `out_error`.
///
/// Passing null is allowed and means the caller does not want the message.
pub fn write_error(out_error: OpenPitOutError, msg: &str) {
    if out_error.is_null() {
        return;
    }
    unsafe {
        *out_error = OpenPitSharedString::new_handle(msg);
    }
}

fn write_param_error(out_error: OpenPitOutParamError, code: OpenPitParamErrorCode, msg: &str) {
    if out_error.is_null() {
        return;
    }
    let handle = Box::new(OpenPitParamError {
        code,
        message: OpenPitSharedString::new_handle(msg),
    });
    unsafe {
        *out_error = Box::into_raw(handle);
    }
}

/// Writes a caller-owned parameter error with unspecified error code.
pub fn write_param_error_unspecified(out_error: OpenPitOutParamError, msg: &str) {
    write_param_error(out_error, OpenPitParamErrorCode::Unspecified, msg);
}

/// Converts core parameter error into a coded FFI parameter error payload.
pub fn consume_param_error_with_code(out_error: OpenPitOutParamError, code: openpit::param::Error) {
    let error_code = match code.code() {
        openpit::param::ErrorCode::Negative => OpenPitParamErrorCode::Negative,
        openpit::param::ErrorCode::DivisionByZero => OpenPitParamErrorCode::DivisionByZero,
        openpit::param::ErrorCode::Overflow => OpenPitParamErrorCode::Overflow,
        openpit::param::ErrorCode::Underflow => OpenPitParamErrorCode::Underflow,
        openpit::param::ErrorCode::InvalidFloat => OpenPitParamErrorCode::InvalidFloat,
        openpit::param::ErrorCode::InvalidFormat => OpenPitParamErrorCode::InvalidFormat,
        openpit::param::ErrorCode::InvalidPrice => OpenPitParamErrorCode::InvalidPrice,
        openpit::param::ErrorCode::InvalidLeverage => OpenPitParamErrorCode::InvalidLeverage,
        openpit::param::ErrorCode::AssetEmpty => OpenPitParamErrorCode::AssetEmpty,
        openpit::param::ErrorCode::AccountIdEmpty => OpenPitParamErrorCode::AccountIdEmpty,
        openpit::param::ErrorCode::Other => OpenPitParamErrorCode::Other,
        _ => OpenPitParamErrorCode::Other,
    };
    write_param_error(out_error, error_code, &code.to_string());
}

/// Releases a caller-owned parameter error container.
///
/// # Safety
///
/// `handle` must be either null or a pointer returned by this library through
/// `OpenPitOutParamError`. The handle must be destroyed at most once.
#[no_mangle]
pub unsafe extern "C" fn openpit_destroy_param_error(handle: *mut OpenPitParamError) {
    if handle.is_null() {
        return;
    }
    let error = unsafe { Box::from_raw(handle) };
    openpit_destroy_shared_string(error.message);
}

#[macro_export]
macro_rules! write_error_format {
    ($out_error:expr, $fmt:expr, $($arg:tt)*) => {
        $crate::last_error::write_error($out_error, &format!($fmt, $($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use crate::string::{openpit_destroy_shared_string, openpit_shared_string_view};
    use crate::OpenPitStringView;

    use super::write_error;

    fn view_to_string(view: OpenPitStringView) -> String {
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
            view_to_string(openpit_shared_string_view(out_error)),
            "transport failure"
        );
        openpit_destroy_shared_string(out_error);
    }

    #[test]
    fn write_error_accepts_null_out_pointer() {
        write_error(std::ptr::null_mut(), "transport failure");
    }
}
