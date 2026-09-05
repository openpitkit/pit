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

// Regression fixtures for financial-types-rust.yml. Run by `just check-semgrep`
// through `semgrep --test`; deliberate violations sit outside the relevant
// rules' `paths.include`, which keeps them out of the repository scan. There is
// no scan-level exclusion: a new rule with no `paths` block will see this file.

use rust_decimal::Decimal;

struct Price;

enum PnlHaltReason {}

// Pins the source-helper exemption.
// ok: raw-decimal-in-public-api
fn trade_price_with_factor(price: Price, factor: Decimal) -> Result<Price, PnlHaltReason> {
    let _ = factor;
    Ok(price)
}

// The scanner drops the visibility modifier, so a private signature is
// reported too. Pinned as-is so a future fix to the rule shows up here.
// ruleid: raw-decimal-in-public-api
fn private_price_with_factor(_price: Price, factor: Decimal) -> Decimal {
    factor
}

// ruleid: raw-decimal-in-public-api
pub fn exported_trade_price_with_factor(_price: Price, factor: Decimal) -> Decimal {
    factor
}

// The signature shapes below are the ones a text regex loses: a generic clause
// containing `>`, and a parameter list containing `)`. The structural pattern
// keeps them in scope.

// ruleid: raw-decimal-in-public-api
pub fn exported_generic_factor<T: Into<Decimal>>(factor: Decimal) -> u64 {
    let _ = factor;
    0
}

// ruleid: raw-decimal-in-public-api
pub fn exported_callback_parameter(map: fn(u8) -> u8) -> Decimal {
    let _ = map;
    Decimal::ZERO
}

fn length_count(entries: &[u8]) -> u64 {
    // ruleid: numeric-cast-in-policy-calculation
    entries.len() as u64
}

// ruleid: raw-decimal-in-public-api
fn financial_value_cast(value: Decimal) -> u64 {
    // ruleid: numeric-cast-in-policy-calculation
    value.mantissa() as u64
}
