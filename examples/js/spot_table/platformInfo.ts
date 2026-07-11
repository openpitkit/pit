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

// Host platform detection for the spot_table example.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { cpus, totalmem } from "node:os";
import { arch, platform, version, versions } from "node:process";

/** Host platform summary. Detection is best-effort; unknown fields stay "unknown". */
export interface PlatformInfo {
  hardware: string;
  cpu: string;
  cores: number;
  memory: string;
  os: string;
  arch: string;
  node: string;
}

const BYTES_PER_GIB = 1024 * 1024 * 1024;

/** Collect host information. Every field defaults to "unknown" on failure. */
export function gatherPlatform(): PlatformInfo {
  const cpuList = cpus();
  const info: PlatformInfo = {
    hardware: "unknown",
    cpu: cpuList.length > 0 ? cpuList[0]!.model : "unknown",
    cores: cpuList.length,
    memory: formatGib(totalmem()),
    os: "unknown",
    arch: `${platform}/${arch}`,
    node: `${version} (V8 ${versions.v8})`,
  };
  if (platform === "darwin") {
    gatherDarwin(info);
  } else if (platform === "linux") {
    gatherLinux(info);
  }
  return info;
}

/** Print the host summary block that heads each run's report. */
export function printPlatform(): void {
  const info = gatherPlatform();
  console.log("Platform:");
  console.log(`  hardware : ${info.hardware}`);
  console.log(`  cpu      : ${info.cpu} (${info.cores} cores)`);
  console.log(`  memory   : ${info.memory}`);
  console.log(`  os       : ${info.os} (${info.arch})`);
  console.log(`  node     : ${info.node}`);
  console.log();
}

// ---------------------------------------------------------------------------
// Darwin
// ---------------------------------------------------------------------------

function gatherDarwin(info: PlatformInfo): void {
  const model = run("sysctl", ["-n", "hw.model"]);
  if (model) {
    info.hardware = model;
  }
  const brand = run("sysctl", ["-n", "machdep.cpu.brand_string"]);
  if (brand) {
    info.cpu = brand;
  }
  const name = run("sw_vers", ["-productName"]);
  const ver = run("sw_vers", ["-productVersion"]);
  const combined = `${name} ${ver}`.trim();
  if (combined) {
    info.os = combined;
  }
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

function gatherLinux(info: PlatformInfo): void {
  const pretty = linuxOsPretty();
  if (pretty) {
    info.os = pretty;
  }
}

function linuxOsPretty(): string {
  for (const line of readLines("/etc/os-release")) {
    if (line.startsWith("PRETTY_NAME=")) {
      return line.slice("PRETTY_NAME=".length).replace(/^"|"$/g, "");
    }
  }
  return "";
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatGib(bytes: number): string {
  return `${(bytes / BYTES_PER_GIB).toFixed(1)} GiB`;
}

/** Run a fixed diagnostic command and return trimmed stdout, or "" on any error. */
function run(cmd: string, args: string[]): string {
  try {
    return execFileSync(cmd, args, {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return "";
  }
}

function readLines(path: string): string[] {
  try {
    return readFileSync(path, "utf8").split("\n");
  } catch {
    return [];
  }
}
