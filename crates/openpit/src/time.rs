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

//! Internal monotonic clock abstraction.
//!
//! Hot paths read a monotonic [`Instant`] for windowing and freshness
//! checks. On native targets this is [`std::time::Instant`]. On
//! `wasm32-unknown-unknown` the std backend panics ("time not implemented
//! on this platform"); under the workspace `panic = "abort"` profile that
//! panic is a fatal, unrecoverable trap. The `wasm-clock` feature swaps in
//! [`web_time::Instant`], which is API-compatible and reads the browser /
//! WASI monotonic clock instead. Native behavior is byte-for-byte
//! unchanged; the alias resolves to `std` everywhere except the wasm build
//! with `wasm-clock` enabled.

#[cfg(not(all(target_arch = "wasm32", feature = "wasm-clock")))]
pub(crate) use std::time::Instant;

#[cfg(all(target_arch = "wasm32", feature = "wasm-clock"))]
pub(crate) use web_time::Instant;
