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

//! Built-in [`LockingPolicyFactory`](super::LockingPolicy) implementations.
//!
//! Each submodule provides one factory type.  The concrete policy types
//! produced by those factories are private to this crate; callers
//! parameterise their [`StorageBuilder`](super::StorageBuilder) with a
//! factory and never name the policy directly.

mod full_locking;
mod index_locking;
mod no_locking;

pub use full_locking::FullLocking;
pub use index_locking::IndexLocking;
pub use no_locking::NoLocking;
