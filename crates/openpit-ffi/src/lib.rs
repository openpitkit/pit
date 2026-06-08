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

pub mod account_adjustment;
pub mod account_control;
pub mod account_group_id;
pub mod account_outcome;
pub mod bytes;
pub(crate) mod engine;
pub mod execution_report;
pub mod group_id;
pub mod instrument;
pub mod last_error;
#[macro_use]
mod macros;
pub mod marketdata;
pub mod order;
pub mod param;
pub(crate) mod policy;
pub mod pre_trade_lock;
pub(crate) mod reject;
pub mod string;

pub use account_adjustment::AccountAdjustment;
pub use execution_report::ExecutionReport;
pub use order::Order;
pub use policy::{
    OpenPitPretradePreTradePolicy, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn,
    OpenPitPretradePreTradePolicyApplyExecutionReportFn,
    OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyFreeUserDataFn,
    OpenPitPretradePreTradePolicyPerformPreTradeCheckFn,
};

use string::OpenPitStringView;

#[no_mangle]
/// Returns the OpenPit runtime version string.
///
/// This function never fails.
///
/// The returned view is read-only, never null, and remains valid for the
/// entire process lifetime. The caller must not release it.
pub extern "C" fn openpit_get_runtime_version() -> OpenPitStringView {
    OpenPitStringView::from_utf8(env!("CARGO_PKG_VERSION"))
}

/// Build profile descriptor for a `debug_assertions`-enabled build.
///
/// The values are captured at compile time by `build.rs`; `debug_assertions`
/// is hard-coded per variant so the flag is reported truthfully rather than
/// read from an environment variable.
const RUNTIME_BUILD_PROFILE_DEBUG: &str = concat!(
    "version=",
    env!("CARGO_PKG_VERSION"),
    ";profile=",
    env!("OPENPIT_BUILD_PROFILE"),
    ";opt_level=",
    env!("OPENPIT_BUILD_OPT_LEVEL"),
    ";debug_assertions=true",
    ";target=",
    env!("OPENPIT_BUILD_TARGET"),
    ";target_cpu=",
    env!("OPENPIT_BUILD_TARGET_CPU"),
    ";lto=",
    env!("OPENPIT_BUILD_LTO"),
);

/// Build profile descriptor for a build with `debug_assertions` disabled.
const RUNTIME_BUILD_PROFILE_RELEASE: &str = concat!(
    "version=",
    env!("CARGO_PKG_VERSION"),
    ";profile=",
    env!("OPENPIT_BUILD_PROFILE"),
    ";opt_level=",
    env!("OPENPIT_BUILD_OPT_LEVEL"),
    ";debug_assertions=false",
    ";target=",
    env!("OPENPIT_BUILD_TARGET"),
    ";target_cpu=",
    env!("OPENPIT_BUILD_TARGET_CPU"),
    ";lto=",
    env!("OPENPIT_BUILD_LTO"),
);

#[no_mangle]
/// Returns the build profile of the linked OpenPit runtime.
///
/// This function never fails.
///
/// The value is a stable, machine-parseable `key=value;`-delimited string
/// (keys `version`, `profile`, `opt_level`, `debug_assertions`, `target`,
/// `target_cpu`, `lto`). It lets a consumer reliably distinguish a debug core
/// from a release core, for example to refuse latency-sensitive work on a
/// debug build. The `target_cpu` and `lto` fields report the literal `unknown`
/// when they cannot be determined at build time.
///
/// The returned view is read-only, never null, and remains valid for the
/// entire process lifetime. The caller must not release it.
pub extern "C" fn openpit_get_runtime_build_profile() -> OpenPitStringView {
    let profile = if cfg!(debug_assertions) {
        RUNTIME_BUILD_PROFILE_DEBUG
    } else {
        RUNTIME_BUILD_PROFILE_RELEASE
    };
    OpenPitStringView::from_utf8(profile)
}

#[cfg(test)]
mod tests {
    use super::{openpit_get_runtime_build_profile, openpit_get_runtime_version};

    #[test]
    fn runtime_version_is_non_empty_string_view() {
        let view = openpit_get_runtime_version();
        assert!(!view.ptr.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        let version = std::str::from_utf8(bytes)
            .expect("runtime version must be valid utf-8")
            .to_owned();
        assert!(!version.is_empty());
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn runtime_build_profile_is_non_empty_string_view() {
        let view = openpit_get_runtime_build_profile();
        assert!(!view.ptr.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        let profile = std::str::from_utf8(bytes)
            .expect("runtime build profile must be valid utf-8")
            .to_owned();
        assert!(!profile.is_empty());
        assert!(profile.contains("profile="));
        assert!(profile.contains("debug_assertions="));
    }
}
