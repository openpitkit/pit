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

#include "spot_loadtest/env/env.hpp"

#include "openpit/runtime.hpp"

#include <array>
#include <cctype>
#include <cstdio>
#include <memory>
#include <sstream>
#include <string>
#include <thread>

#if defined(__APPLE__) || defined(__linux__)
#include <sys/utsname.h>
#include <unistd.h>
#endif
#if defined(__APPLE__)
#include <sys/sysctl.h>
#include <sys/types.h>
#endif

namespace spot_loadtest::env {
namespace {

constexpr const char *kUnknown = "unknown";
constexpr double kBytesPerGiB = 1024.0 * 1024.0 * 1024.0;

[[nodiscard]] FILE *OpenCommandPipe(const char *cmd) {
#if defined(_WIN32)
  return _popen(cmd, "r");
#else
  return popen(cmd, "r");
#endif
}

int CloseCommandPipe(FILE *pipe) {
#if defined(_WIN32)
  return _pclose(pipe);
#else
  return pclose(pipe);
#endif
}

[[nodiscard]] std::string Trim(const std::string &s) {
  std::size_t a = 0;
  std::size_t b = s.size();
  while (a < b && std::isspace(static_cast<unsigned char>(s[a]))) {
    ++a;
  }
  while (b > a && std::isspace(static_cast<unsigned char>(s[b - 1]))) {
    --b;
  }
  return s.substr(a, b - a);
}

// Runs `cmd` and returns its trimmed stdout, or empty on failure. Used only for
// diagnostic git commands.
[[nodiscard]] std::string RunCommand(const std::string &cmd) {
  std::array<char, 256> buffer{};
  std::string result;
  std::unique_ptr<FILE, int (*)(FILE *)> pipe(OpenCommandPipe(cmd.c_str()),
                                              CloseCommandPipe);
  if (!pipe) {
    return {};
  }
  while (std::fgets(buffer.data(), static_cast<int>(buffer.size()),
                    pipe.get()) != nullptr) {
    result += buffer.data();
  }
  return Trim(result);
}

[[nodiscard]] Host CaptureHost() {
  Host h;
  const unsigned hw = std::thread::hardware_concurrency();
  h.cores = hw == 0 ? 0 : static_cast<int>(hw);

#if defined(__APPLE__)
  {
    std::array<char, 256> model{};
    std::size_t size = model.size();
    if (sysctlbyname("machdep.cpu.brand_string", model.data(), &size, nullptr,
                     0) == 0) {
      h.cpuModel = Trim(std::string(model.data()));
    }
    std::int64_t mem = 0;
    size = sizeof(mem);
    if (sysctlbyname("hw.memsize", &mem, &size, nullptr, 0) == 0 && mem > 0) {
      char buf[32];
      std::snprintf(buf, sizeof(buf), "%.1f GiB",
                    static_cast<double>(mem) / kBytesPerGiB);
      h.ram = buf;
    }
  }
#elif defined(__linux__)
  {
    const std::string model =
        RunCommand("awk -F: '/model name/{print $2; exit}' /proc/cpuinfo");
    if (!model.empty()) {
      h.cpuModel = Trim(model);
    }
    const std::string memKb =
        RunCommand("awk '/MemTotal/{print $2; exit}' /proc/meminfo");
    if (!memKb.empty()) {
      try {
        const double bytes = std::stod(memKb) * 1024.0;
        char buf[32];
        std::snprintf(buf, sizeof(buf), "%.1f GiB", bytes / kBytesPerGiB);
        h.ram = buf;
      } catch (...) {
      }
    }
  }
#endif

#if defined(__APPLE__) || defined(__linux__)
  {
    struct utsname uts{};
    if (uname(&uts) == 0) {
      h.os = std::string(uts.sysname) + " " + std::string(uts.release);
      h.kernel = std::string(uts.sysname) + " " + std::string(uts.release);
    }
  }
#endif
  return h;
}

[[nodiscard]] Toolchain CaptureToolchain() {
  Toolchain t;
#if defined(__clang__)
  t.compiler = std::string("Clang ") + __clang_version__;
#elif defined(__GNUC__)
  t.compiler = "GCC " + std::to_string(__GNUC__) + "." +
               std::to_string(__GNUC_MINOR__) + "." +
               std::to_string(__GNUC_PATCHLEVEL__);
#elif defined(_MSC_VER)
  t.compiler = "MSVC " + std::to_string(_MSC_VER);
#else
  t.compiler = kUnknown;
#endif
#if __cplusplus >= 202002L
  t.cppStd = "C++20";
#elif __cplusplus >= 201703L
  t.cppStd = "C++17";
#else
  t.cppStd = "C++";
#endif
#if defined(__aarch64__) || defined(_M_ARM64)
  t.targetArch = "arm64";
#elif defined(__x86_64__) || defined(_M_X64)
  t.targetArch = "amd64";
#else
  t.targetArch = kUnknown;
#endif
  return t;
}

[[nodiscard]] PitRepo CapturePitRepo(const std::string &repoRoot) {
  PitRepo p;
  const std::string base = "git -C '" + repoRoot + "' ";
  const std::string commit = RunCommand(base + "rev-parse HEAD 2>/dev/null");
  if (!commit.empty()) {
    p.commit = commit;
  }
  // `git status --porcelain` is empty for a clean tree; the command succeeds on
  // a real repo. We detect availability by checking rev-parse already worked.
  if (!commit.empty()) {
    const std::string status =
        RunCommand(base + "status --porcelain 2>/dev/null");
    p.dirty = !status.empty();
    p.dirtyKnown = true;
  }
  return p;
}

[[nodiscard]] CoreBuildProfile CaptureCore() {
  CoreBuildProfile p;
  p.raw = ::openpit::GetBuildProfile();
  p.version = ::openpit::GetVersion();

  std::stringstream ss(p.raw);
  std::string pair;
  while (std::getline(ss, pair, ';')) {
    pair = Trim(pair);
    if (pair.empty()) {
      continue;
    }
    const std::size_t eq = pair.find('=');
    if (eq == std::string::npos) {
      continue;
    }
    const std::string k = Trim(pair.substr(0, eq));
    const std::string v = Trim(pair.substr(eq + 1));
    if (k == "profile") {
      p.profile = v;
    } else if (k == "opt_level") {
      p.optLevel = v;
    } else if (k == "debug_assertions") {
      p.debugAssertions = (v == "true");
    } else if (k == "target") {
      p.target = v;
    } else if (k == "target_cpu") {
      p.targetCpu = v;
    } else if (k == "lto") {
      p.lto = v;
    }
    // "version" is already captured via GetVersion(); skip.
  }
  return p;
}

} // namespace

std::string PitRepo::DirtyStatus() const {
  if (!dirtyKnown) {
    return kUnknown;
  }
  return dirty ? "dirty" : "clean";
}

bool CoreBuildProfile::IsDebug() const {
  return debugAssertions || optLevel == "0" || profile == "debug";
}

Env Capture(const std::string &repoRoot) {
  Env e;
  e.host = CaptureHost();
  e.toolchain = CaptureToolchain();
  e.pit = CapturePitRepo(repoRoot);
  e.core = CaptureCore();
  return e;
}

} // namespace spot_loadtest::env
