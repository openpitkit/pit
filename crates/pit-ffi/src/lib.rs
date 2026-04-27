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

pub mod account_adjustment;
pub(crate) mod engine;
pub mod execution_report;
pub mod instrument;
pub mod last_error;
#[macro_use]
mod macros;
pub mod order;
pub mod param;
pub(crate) mod policy;
pub(crate) mod reject;
pub mod string;

pub use account_adjustment::{AccountAdjustment, AccountAdjustmentOperation};
pub use execution_report::{ExecutionReport, FillDetailsData, PositionImpactData};
pub use order::Order;
pub use policy::{PitAccountAdjustmentPolicy, PitAccountAdjustmentPolicyApplyFn};

use string::PitStringView;

#[no_mangle]
/// Returns the Pit runtime version string.
///
/// This function never fails.
///
/// The returned view is read-only, never null, and remains valid for the
/// entire process lifetime. The caller must not release it.
pub extern "C" fn pit_get_runtime_version() -> PitStringView {
    PitStringView::from_utf8(env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::pit_get_runtime_version;

    #[test]
    fn runtime_version_is_non_empty_string_view() {
        let view = pit_get_runtime_version();
        assert!(!view.ptr.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        let version = std::str::from_utf8(bytes)
            .expect("runtime version must be valid utf-8")
            .to_owned();
        assert!(!version.is_empty());
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }
}
