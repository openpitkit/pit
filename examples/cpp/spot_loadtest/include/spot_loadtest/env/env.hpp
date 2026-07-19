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

// Captures the host, runtime, pit repository, and core build profile for the
// load-test environment block.
//
// Mirror of: examples/go/spot_loadtest/internal/env/env.go
//
// Every field that cannot be read on the current platform becomes "unknown"
// rather than hard-failing; the debug-core guard is the only operation that can
// refuse to proceed.

namespace spot_loadtest::env {

// Summarizes the physical machine.
struct Host {
  std::string cpuModel = "unknown";
  int cores = 0;
  std::string ram = "unknown";    // e.g. "32.0 GiB"
  std::string os = "unknown";     // e.g. "darwin 26.5"
  std::string kernel = "unknown"; // e.g. "Darwin 24.5.0"
};

// Summarizes the C++ toolchain (the analogue of the Go runtime block).
struct Toolchain {
  std::string compiler; // e.g. "Clang 17.0.0"
  std::string cppStd;   // e.g. "C++17"
  std::string targetArch;
};

// Summarizes the pit monorepo revision. The working-tree status is TRI-STATE:
// clean | dirty | unknown — an unauditable build is never reported as
// "clean".
struct PitRepo {
  std::string commit = "unknown";
  bool dirty = false;
  bool dirtyKnown = false;

  // Renders the tri-state working-tree status: "clean", "dirty", or "unknown".
  [[nodiscard]] std::string DirtyStatus() const;
};

// The parsed build profile of the linked native runtime.
struct CoreBuildProfile {
  std::string version;
  std::string profile = "unknown";  // "release" or "debug"
  std::string optLevel = "unknown"; // "0", "1", "2", "3", "s", "z"
  bool debugAssertions = false;
  std::string target = "unknown";
  std::string targetCpu = "unknown";
  std::string lto = "unknown";
  std::string raw; // the unparsed key=value; string for the report.

  // True when the core was built without optimizations or with debug
  // assertions — conditions that make latency numbers meaningless.
  [[nodiscard]] bool IsDebug() const;
};

// The complete environment snapshot captured before the run starts.
struct Env {
  Host host;
  Toolchain toolchain;
  PitRepo pit;
  CoreBuildProfile core;
};

// Collects all environment fields. Never returns a hard error for individual
// fields; only "unknown" field values.
[[nodiscard]] Env Capture(const std::string &repoRoot);

} // namespace spot_loadtest::env
