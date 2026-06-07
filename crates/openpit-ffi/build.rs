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

//! Captures the build profile of the FFI crate at compile time so that the
//! C-ABI accessor `openpit_get_runtime_build_profile` can report it to FFI
//! consumers. A consumer can then refuse a debug-built core whose latency
//! numbers would be meaningless.

use std::env;

/// Placeholder emitted when a best-effort value cannot be determined, so the
/// reported string never silently omits a field.
const UNKNOWN: &str = "unknown";

fn main() {
    // Required values, sourced directly from Cargo's build-script environment.
    let profile = required_env("PROFILE");
    let opt_level = required_env("OPT_LEVEL");
    let target = required_env("TARGET");

    // Best-effort values, parsed from the encoded rustflags. They become the
    // literal `unknown` when absent rather than being dropped from the output.
    let encoded_rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let rustflags: Vec<&str> = if encoded_rustflags.is_empty() {
        Vec::new()
    } else {
        encoded_rustflags.split('\u{1f}').collect()
    };
    let target_cpu = parse_target_cpu(&rustflags).unwrap_or_else(|| UNKNOWN.to_owned());
    let lto = parse_lto(&rustflags).unwrap_or_else(|| UNKNOWN.to_owned());

    println!("cargo:rustc-env=OPENPIT_BUILD_PROFILE={profile}");
    println!("cargo:rustc-env=OPENPIT_BUILD_OPT_LEVEL={opt_level}");
    println!("cargo:rustc-env=OPENPIT_BUILD_TARGET={target}");
    println!("cargo:rustc-env=OPENPIT_BUILD_TARGET_CPU={target_cpu}");
    println!("cargo:rustc-env=OPENPIT_BUILD_LTO={lto}");

    // Keep the embedded values correct when the profile or the rustflags change.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PROFILE");
    println!("cargo:rerun-if-env-changed=OPT_LEVEL");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
}

/// Reads a build-script environment variable that Cargo always sets. A missing
/// value indicates a broken build environment, so failing loudly is correct.
fn required_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|err| panic!("build script env {key} is unavailable: {err}"))
}

/// Extracts the selected target CPU from rustflags, handling both the
/// `-Ctarget-cpu=...` and the split `-C target-cpu=...` spellings.
fn parse_target_cpu(rustflags: &[&str]) -> Option<String> {
    parse_codegen_value(rustflags, "target-cpu")
}

/// Extracts the LTO codegen setting from rustflags, if it was passed there.
/// LTO is more commonly a Cargo profile setting, so this is best-effort.
fn parse_lto(rustflags: &[&str]) -> Option<String> {
    parse_codegen_value(rustflags, "lto")
}

/// Finds the value of a `-C <name>=<value>` codegen option in the flag list.
/// Supports the joined form `-Cname=value`, the split form `-C name=value`, and
/// a bare `name=value` flag.
fn parse_codegen_value(rustflags: &[&str], name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let mut expect_value = false;
    for &flag in rustflags {
        if expect_value {
            expect_value = false;
            if let Some(value) = flag.strip_prefix(&prefix) {
                return Some(value.to_owned());
            }
        }
        if flag == "-C" {
            expect_value = true;
            continue;
        }
        if let Some(rest) = flag.strip_prefix("-C") {
            if let Some(value) = rest.strip_prefix(&prefix) {
                return Some(value.to_owned());
            }
        }
    }
    None
}
