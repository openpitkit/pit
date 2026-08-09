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

#pragma once

#include <string>
#include <string_view>

// Self-contained SHA-256 lets the report echo the config hash without an
// external dependency. It runs once per run, off the hot path, on the config
// file bytes.

namespace spot_loadtest {

// Returns the lowercase hex-encoded SHA-256 digest of `data`.
[[nodiscard]] std::string Sha256Hex(std::string_view data);

} // namespace spot_loadtest
