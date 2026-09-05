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

// Regression fixtures for fallbacks-rust.yml. Run by `just check-semgrep`
// through `semgrep --test`; deliberate violations sit outside the relevant
// rules' `paths.include`, which keeps them out of the repository scan. There is
// no scan-level exclusion: a new rule with no `paths` block will see this file.

fn same_level_catch_all(kind: u8) -> u8 {
    // ruleid: unapproved-catch-all
    match kind {
        0 => 0,
        _ => 1,
    }
}

fn nested_catch_all(kind: u8, nested_value: u8) -> u8 {
    // ok: unapproved-catch-all
    match kind {
        invalid_kind => match nested_value {
            0 => invalid_kind,
            _ => 1,
        },
    }
}

fn unwrap_or_is_a_fallback(value: Option<u8>) -> u8 {
    // ruleid: unapproved-fallback
    value.unwrap_or(0)
}

fn unwrap_or_default_is_a_fallback(value: Option<u8>) -> u8 {
    // ruleid: unapproved-fallback
    value.unwrap_or_default()
}

fn unwrap_or_else_is_a_fallback(value: Option<u8>) -> u8 {
    // ruleid: unapproved-fallback
    value.unwrap_or_else(|| 0)
}

// The two cases below carry no `ok:` annotation on purpose: the rule requires
// the marker on the line directly above the match, which is the same line an
// annotation would occupy. They are covered negatively - `semgrep --test`
// fails on an unannotated finding, so a regression that stops honouring the
// marker fails the run.

fn approved_unwrap_or(value: Option<u8>) -> u8 {
    // fallback(approved): substitutes the documented zero default
    value.unwrap_or(0)
}

fn approved_unwrap_or_default(value: Option<u8>) -> u8 {
    // fallback(approved): substitutes the documented zero default
    value.unwrap_or_default()
}
