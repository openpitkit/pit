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

// spot_loadtest measures the C++ FFI submit->decision latency for the openpit
// pre-trade engine running a spot-limit funds policy at high offered rates.
//
// Run with:
//
//   ./build/spot_loadtest \
//     --config examples/cpp/spot_loadtest/configs/baseline.ini
//
// See README.md for the full build and run recipe.

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/driver/driver.hpp"
#include "spot_loadtest/driver/live.hpp"
#include "spot_loadtest/env/env.hpp"
#include "spot_loadtest/generator/generator.hpp"
#include "spot_loadtest/progress/progress.hpp"
#include "spot_loadtest/reporter/reporter.hpp"

#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <iostream>
#include <memory>
#include <string>

namespace {

namespace sl = spot_loadtest;

// Refuses to continue when the linked core was built with debug settings,
// because FFI latency numbers from such a build are meaningless. Returns false
// (with a fatal message printed) unless `allow` overrides it.
[[nodiscard]] bool RunDebugCoreGuard(const sl::env::CoreBuildProfile &profile,
                                     bool allow) {
  if (!profile.IsDebug()) {
    return true;
  }
  std::cerr
      << "error: the loaded native core appears to be a debug build (profile="
      << profile.profile << ", opt_level=" << profile.optLevel
      << ", debug_assertions=" << (profile.debugAssertions ? "true" : "false")
      << ").\n"
      << "FFI latency numbers from a debug core are meaningless; build the "
         "core "
         "in release mode first:\n\n"
      << "  cargo build --release\n\n"
      << "Pass --allow-debug-core to override (development only).\n";
  if (!allow) {
    return false;
  }
  std::cerr << "\n*** WARNING: running with a debug core "
               "(--allow-debug-core). ***\n"
            << "*** Latency numbers are NOT meaningful. ***\n\n";
  return true;
}

// Walks up from the running executable to find the repo root (the directory
// containing the pit Cargo workspace). Falls back to the current working
// directory when it cannot be determined. The example binary lives at
// <repo>/examples/cpp/spot_loadtest/, so the repo root is four levels up.
[[nodiscard]] std::string RepoRootFromExe(const char *argv0) {
  namespace fs = std::filesystem;
  std::error_code ec;
  fs::path exe = fs::weakly_canonical(fs::path(argv0), ec);
  if (ec) {
    return fs::current_path().string();
  }
  fs::path root = exe;
  for (int i = 0; i < 5; ++i) { // file + 4 dirs up.
    root = root.parent_path();
  }
  if (root.empty()) {
    return fs::current_path().string();
  }
  return root.string();
}

} // namespace

int main(int argc, char **argv) {
  std::string configPath;
  bool allowDebugCore = false;
  bool showProgress = true;
  bool validateConfig = false;

  for (int i = 1; i < argc; ++i) {
    const std::string arg = argv[i];
    if (arg == "--config") {
      if (i + 1 >= argc) {
        std::cerr << "error: --config requires a path\n";
        return 1;
      }
      configPath = argv[++i];
    } else if (arg.rfind("--config=", 0) == 0) {
      configPath = arg.substr(std::string("--config=").size());
    } else if (arg == "--validate-config") {
      validateConfig = true;
    } else if (arg == "--allow-debug-core") {
      allowDebugCore = true;
    } else if (arg == "--progress=false" || arg == "--no-progress") {
      showProgress = false;
    } else if (arg == "--progress" || arg == "--progress=true") {
      showProgress = true;
    } else {
      std::cerr << "error: unknown option: " << arg << "\n";
      return 1;
    }
  }

  if (configPath.empty()) {
    std::cerr << "error: --config is required\n"
              << "usage: spot_loadtest --config <path/to/config.ini> "
                 "[--validate-config] [--allow-debug-core] "
                 "[--progress=false]\n";
    return 1;
  }

  std::error_code ec;
  const std::string absConfig =
      std::filesystem::absolute(configPath, ec).string();
  if (ec) {
    std::cerr << "error: resolve config path \"" << configPath << "\"\n";
    return 1;
  }

  sl::config::Config cfg;
  try {
    cfg = sl::config::Load(absConfig);
  } catch (const std::exception &e) {
    std::cerr << "error: " << e.what() << "\n";
    return 1;
  }

  if (validateConfig) {
    std::cerr << "config valid: " << absConfig << "\n";
    return 0;
  }

  const std::string repoRoot = RepoRootFromExe(argv[0]);
  const sl::env::Env e = sl::env::Capture(repoRoot);

  if (!RunDebugCoreGuard(e.core, allowDebugCore)) {
    return 1;
  }

  // Build the deterministic event stream (CPU-only; no FFI).
  std::cerr << "Generating event stream...\n";
  std::unique_ptr<sl::generator::Stream> stream;
  try {
    stream = sl::generator::Generate(cfg);
  } catch (const std::exception &ex) {
    std::cerr << "error: generator: " << ex.what() << "\n";
    return 1;
  }
  std::cerr << "Stream ready: " << stream->stats.orderChecks
            << " order-checks, " << stream->stats.settlements
            << " settlements, " << stream->stats.fundings << " fundings\n";

  sl::driver::Config driverCfg = sl::driver::FromAppConfig(cfg);
  sl::driver::LiveSource liveSrc;
  driverCfg.live = &liveSrc;

  std::unique_ptr<sl::progress::Reporter> prog;
  if (showProgress) {
    const std::uint64_t totalDecided =
        stream->stats.orderChecks + stream->stats.settlements;
    prog = std::make_unique<sl::progress::Reporter>(
        std::cerr, liveSrc, totalDecided, sl::progress::kDefaultInterval);
    prog->Start(std::chrono::steady_clock::now());
  }

  // Run the driver: open-loop submission + collection + oracle checks.
  std::string invalidReason;
  sl::driver::RunResult result;
  std::string hardError;
  try {
    result = sl::driver::RunCollecting(*stream, driverCfg, invalidReason);
  } catch (const std::exception &ex) {
    hardError = ex.what();
  }

  if (prog) {
    prog->Stop();
  }

  if (!hardError.empty()) {
    std::cerr << "error: driver: " << hardError << "\n";
    return 1;
  }

  if (!invalidReason.empty()) {
    sl::reporter::WriteInvalid(std::cout, e, cfg, configPath, result.snapshot,
                               result.stats.submitterThreads, stream->stats);
    std::cerr << "\nerror: run invalid — " << invalidReason
              << "; latency numbers suppressed\n";
    return 1;
  }

  // Write the full report to stdout. Nothing else writes to stdout.
  sl::reporter::Write(std::cout, e, cfg, configPath, result.snapshot,
                      result.stats.submitterThreads, stream->stats);
  return 0;
}
