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

#include "platform.hpp"

#include "openpit/runtime.hpp"

#include <array>
#include <cctype>
#include <cstdint>
#include <cstdio>
#include <fstream>
#include <iostream>
#include <memory>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

namespace spot_table {
namespace {

// kBytesPerKiB is consumed only by the Linux gatherer (KiB -> bytes).
[[maybe_unused]] constexpr std::uint64_t kBytesPerKiB = 1024;
constexpr double kBytesPerGiB = 1024.0 * 1024.0 * 1024.0;

[[nodiscard]] FILE *OpenCommandPipe(const char *command) {
#if defined(_WIN32)
  return _popen(command, "r");
#else
  return popen(command, "r");
#endif
}

int CloseCommandPipe(FILE *pipe) {
#if defined(_WIN32)
  return _pclose(pipe);
#else
  return pclose(pipe);
#endif
}

[[nodiscard]] std::string TrimSpace(const std::string &s) {
  const auto is_space = [](unsigned char c) { return std::isspace(c) != 0; };
  std::size_t begin = 0;
  std::size_t end = s.size();
  while (begin < end && is_space(static_cast<unsigned char>(s[begin]))) {
    ++begin;
  }
  while (end > begin && is_space(static_cast<unsigned char>(s[end - 1]))) {
    --end;
  }
  return s.substr(begin, end - begin);
}

[[nodiscard]] bool HasPrefix(const std::string &s, const std::string &prefix) {
  return s.size() >= prefix.size() && s.compare(0, prefix.size(), prefix) == 0;
}

// Runs a fixed diagnostic command and returns trimmed stdout. Returns "" when
// the pipe cannot be opened or the command yields no non-whitespace stdout, as
// with an unavailable or failing diagnostic tool.
[[nodiscard]] std::string RunOut(const std::string &command) {
  std::array<char, 256> buffer{};
  std::string output;
  // NOLINTNEXTLINE: fixed diagnostic commands, no user input.
  std::unique_ptr<FILE, int (*)(FILE *)> pipe(OpenCommandPipe(command.c_str()),
                                              CloseCommandPipe);
  if (!pipe) {
    return "";
  }
  while (std::fgets(buffer.data(), static_cast<int>(buffer.size()),
                    pipe.get()) != nullptr) {
    output += buffer.data();
  }
  return TrimSpace(output);
}

// ReadLines / ReadFirstLine read the Linux /proc and /sys files; unused on
// other hosts.
[[maybe_unused]] [[nodiscard]] std::vector<std::string>
ReadLines(const std::string &path) {
  std::vector<std::string> lines;
  std::ifstream file(path);
  if (!file.is_open()) {
    return lines;
  }
  std::string line;
  while (std::getline(file, line)) {
    if (!line.empty() && line.back() == '\r') {
      line.pop_back();
    }
    lines.push_back(line);
  }
  return lines;
}

[[maybe_unused]] [[nodiscard]] std::string
ReadFirstLine(const std::string &path) {
  const std::vector<std::string> lines = ReadLines(path);
  if (lines.empty()) {
    return "";
  }
  return TrimSpace(lines.front());
}

[[nodiscard]] std::string FormatGiB(std::uint64_t bytes) {
  std::array<char, 32> buf{};
  std::snprintf(buf.data(), buf.size(), "%.1f GiB",
                static_cast<double>(bytes) / kBytesPerGiB);
  return std::string(buf.data());
}

// Reports the C++ compiler and version used to build this binary.
[[nodiscard]] std::string CompilerToolchain() {
#if defined(__clang__)
  return "clang " + std::string(__clang_version__);
#elif defined(__GNUC__)
  return "gcc " + std::to_string(__GNUC__) + "." +
         std::to_string(__GNUC_MINOR__) + "." +
         std::to_string(__GNUC_PATCHLEVEL__);
#elif defined(_MSC_VER)
  return "msvc " + std::to_string(_MSC_VER);
#else
  return "unknown";
#endif
}

[[nodiscard]] std::string HostArch() {
#if defined(__APPLE__)
  const std::string os = "darwin";
#elif defined(__linux__)
  const std::string os = "linux";
#elif defined(_WIN32)
  const std::string os = "windows";
#else
  const std::string os = "unknown";
#endif
#if defined(__aarch64__) || defined(_M_ARM64)
  const std::string arch = "arm64";
#elif defined(__x86_64__) || defined(_M_X64)
  const std::string arch = "amd64";
#else
  const std::string arch = "unknown";
#endif
  return os + "/" + arch;
}

// Compile OS-specific gatherers only on their corresponding hosts.
#if defined(__APPLE__)

// Reads the root volume's transport from `diskutil info /`.
[[nodiscard]] std::string DarwinDiskInterface() {
  std::istringstream stream(RunOut("diskutil info /"));
  std::string line;
  while (std::getline(stream, line)) {
    const std::string trimmed = TrimSpace(line);
    if (HasPrefix(trimmed, "Protocol:")) {
      return TrimSpace(trimmed.substr(std::string("Protocol:").size()));
    }
  }
  return "";
}

void GatherDarwin(PlatformInfo &p) {
  if (const std::string v = RunOut("sysctl -n hw.model"); !v.empty()) {
    p.hardware = v;
  }
  if (const std::string v = RunOut("sysctl -n machdep.cpu.brand_string");
      !v.empty()) {
    p.cpu = v;
  }
  if (const std::string v = RunOut("sysctl -n hw.memsize"); !v.empty()) {
    try {
      const std::uint64_t n = std::stoull(v);
      p.memory = FormatGiB(n);
    } catch (...) {
      // leave "unknown"
    }
  }
  const std::string name = RunOut("sw_vers -productName");
  const std::string version = RunOut("sw_vers -productVersion");
  const std::string combined = TrimSpace(name + " " + version);
  if (!combined.empty()) {
    p.os = combined;
  }
  if (const std::string v = DarwinDiskInterface(); !v.empty()) {
    p.disk = v;
  }
}

#elif defined(__linux__)

[[nodiscard]] std::string LinuxOSPretty() {
  for (const std::string &line : ReadLines("/etc/os-release")) {
    if (HasPrefix(line, "PRETTY_NAME=")) {
      std::string rest = line.substr(std::string("PRETTY_NAME=").size());
      if (rest.size() >= 2 && rest.front() == '"' && rest.back() == '"') {
        rest = rest.substr(1, rest.size() - 2);
      }
      return rest;
    }
  }
  return "";
}

[[nodiscard]] std::string LinuxCPUModel() {
  for (const std::string &line : ReadLines("/proc/cpuinfo")) {
    const std::string::size_type colon = line.find(':');
    if (colon == std::string::npos) {
      continue;
    }
    const std::string key = TrimSpace(line.substr(0, colon));
    if (key == "model name" || key == "Model" || key == "Hardware") {
      return TrimSpace(line.substr(colon + 1));
    }
  }
  return "";
}

[[nodiscard]] std::string LinuxMemTotal() {
  for (const std::string &line : ReadLines("/proc/meminfo")) {
    if (!HasPrefix(line, "MemTotal:")) {
      continue;
    }
    std::istringstream stream(line.substr(std::string("MemTotal:").size()));
    std::string kb;
    if (!(stream >> kb)) { // "<kB> kB"
      return "";
    }
    try {
      const std::uint64_t value = std::stoull(kb);
      return FormatGiB(value * kBytesPerKiB);
    } catch (...) {
      return "";
    }
  }
  return "";
}

// Reports the transport (nvme/sata/usb/...) of the disk backing the root
// filesystem.
[[nodiscard]] std::string LinuxDiskInterface() {
  const std::string src = RunOut("findmnt -no SOURCE /");
  if (src.empty()) {
    return "";
  }
  std::istringstream stream(RunOut("lsblk -no TRAN " + src));
  std::string line;
  while (std::getline(stream, line)) {
    std::string trimmed = TrimSpace(line);
    if (!trimmed.empty()) {
      return trimmed;
    }
  }
  return "";
}

void GatherLinux(PlatformInfo &p) {
  if (const std::string v = LinuxOSPretty(); !v.empty()) {
    p.os = v;
  }
  if (const std::string v = LinuxCPUModel(); !v.empty()) {
    p.cpu = v;
  }
  if (const std::string v = LinuxMemTotal(); !v.empty()) {
    p.memory = v;
  }
  if (const std::string v = ReadFirstLine("/sys/class/dmi/id/product_name");
      !v.empty()) {
    p.hardware = v;
  }
  if (const std::string v = LinuxDiskInterface(); !v.empty()) {
    p.disk = v;
  }
}

#endif // OS-specific gatherers

} // namespace

PlatformInfo GatherPlatform() {
  PlatformInfo p;
  const unsigned hw = std::thread::hardware_concurrency();
  p.cores = hw == 0 ? 1 : static_cast<int>(hw);
  p.arch = HostArch();
  p.toolchain = CompilerToolchain() + ", openpit " + openpit::GetVersion();
#if defined(__APPLE__)
  GatherDarwin(p);
#elif defined(__linux__)
  GatherLinux(p);
#endif
  return p;
}

void PrintPlatform() {
  const PlatformInfo p = GatherPlatform();
  std::cout << "Platform:\n";
  std::cout << "  hardware : " << p.hardware << "\n";
  std::cout << "  cpu      : " << p.cpu << " (" << p.cores << " cores)\n";
  std::cout << "  memory   : " << p.memory << "\n";
  std::cout << "  disk     : " << p.disk << "\n";
  std::cout << "  os       : " << p.os << " (" << p.arch << ")\n";
  std::cout << "  toolchain: " << p.toolchain << "\n";
  std::cout << "\n";
}

} // namespace spot_table
