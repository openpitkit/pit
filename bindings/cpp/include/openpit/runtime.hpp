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

#include "openpit/string.hpp"

#include <openpit.h>

#include <string>

namespace openpit {

// Returns the OpenPit runtime version string. Never fails.
[[nodiscard]] inline std::string GetVersion() {
  const OpenPitStringView version = openpit_get_runtime_version();
  if (version.ptr == nullptr) {
    return {};
  }
  return {reinterpret_cast<const char*>(version.ptr), version.len};
}

// Returns the build-profile descriptor of the linked runtime: a stable
// `key=value;`-delimited string (keys include `version`, `profile`,
// `debug_assertions`). Never fails.
[[nodiscard]] inline std::string GetBuildProfile() {
  const OpenPitStringView profile = openpit_get_runtime_build_profile();
  if (profile.ptr == nullptr) {
    return {};
  }
  return {reinterpret_cast<const char*>(profile.ptr), profile.len};
}

}  // namespace openpit
