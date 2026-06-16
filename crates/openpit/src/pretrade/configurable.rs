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

/// Internal contract for a built-in policy with runtime-updatable settings.
///
/// The engine stores a clone of the cell selected by `Configuration`. Both the
/// registry and the running policy therefore observe the same value without
/// adding synchronization to the order-check hot path.
pub(crate) trait ConfigurablePolicy<Configuration>
where
    Configuration: crate::storage::LockingPolicyFactory,
{
    /// The policy's runtime-updatable settings type.
    type Settings: Clone + 'static;

    /// Returns a clone of the policy's settings cell.
    fn settings_cell(&self) -> Configuration::Config<Self::Settings>;
}
