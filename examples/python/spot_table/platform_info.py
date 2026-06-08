# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://github.com/openpitkit and the OWNERS file for details.

"""Host platform detection for the spot_table example."""

from __future__ import annotations

import contextlib
import os
import platform as _platform
import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass
class PlatformInfo:
    """Host platform summary.

    Detection is best-effort; unknown fields stay "unknown".
    """

    hardware: str
    cpu: str
    cores: int
    memory: str
    disk: str
    os: str
    arch: str
    python: str


def gather_platform() -> PlatformInfo:
    """Collect host information. Every field defaults to "unknown" on failure."""
    system = _platform.system()
    p = PlatformInfo(
        hardware="unknown",
        cpu="unknown",
        cores=os.cpu_count() or 0,
        memory="unknown",
        disk="unknown",
        os="unknown",
        arch=f"{system.lower()}/{_platform.machine()}",
        python=_platform.python_version(),
    )
    if system == "Darwin":
        _gather_darwin(p)
    elif system == "Linux":
        _gather_linux(p)
    return p


def print_platform() -> None:
    """Print the host summary block that heads each run's report."""
    p = gather_platform()
    print("Platform:")
    print(f"  hardware : {p.hardware}")
    print(f"  cpu      : {p.cpu} ({p.cores} cores)")
    print(f"  memory   : {p.memory}")
    print(f"  disk     : {p.disk}")
    print(f"  os       : {p.os} ({p.arch})")
    print(f"  python   : {p.python}")
    print()


# ---------------------------------------------------------------------------
# Darwin
# ---------------------------------------------------------------------------


def _gather_darwin(p: PlatformInfo) -> None:
    if v := _run(["sysctl", "-n", "hw.model"]):
        p.hardware = v
    if v := _run(["sysctl", "-n", "machdep.cpu.brand_string"]):
        p.cpu = v
    if v := _run(["sysctl", "-n", "hw.memsize"]):
        with contextlib.suppress(ValueError):
            p.memory = _format_gib(int(v))
    name = _run(["sw_vers", "-productName"])
    version = _run(["sw_vers", "-productVersion"])
    combined = (name + " " + version).strip()
    if combined:
        p.os = combined
    if v := _darwin_disk_interface():
        p.disk = v


def _darwin_disk_interface() -> str:
    """Report the boot volume's transport from diskutil."""
    output = _run(["diskutil", "info", "/"])
    for line in output.splitlines():
        stripped = line.strip()
        if stripped.startswith("Protocol:"):
            return stripped[len("Protocol:") :].strip()
    return ""


# ---------------------------------------------------------------------------
# Linux
# ---------------------------------------------------------------------------


def _gather_linux(p: PlatformInfo) -> None:
    if v := _linux_os_pretty():
        p.os = v
    if v := _linux_cpu_model():
        p.cpu = v
    if v := _linux_mem_total():
        p.memory = v
    hardware = _read_first_line("/sys/class/dmi/id/product_name")
    if hardware:
        p.hardware = hardware
    if v := _linux_disk_interface():
        p.disk = v


def _linux_os_pretty() -> str:
    for line in _read_lines("/etc/os-release"):
        if line.startswith("PRETTY_NAME="):
            return line[len("PRETTY_NAME=") :].strip('"')
    return ""


def _linux_cpu_model() -> str:
    for line in _read_lines("/proc/cpuinfo"):
        if ":" not in line:
            continue
        key, _, val = line.partition(":")
        key = key.strip()
        if key in ("model name", "Model", "Hardware"):
            return val.strip()
    return ""


def _linux_mem_total() -> str:
    for line in _read_lines("/proc/meminfo"):
        if line.startswith("MemTotal:"):
            fields = line[len("MemTotal:") :].split()
            if fields:
                try:
                    kb = int(fields[0])
                    return _format_gib(kb * 1024)
                except ValueError:
                    pass
    return ""


def _linux_disk_interface() -> str:
    """Report the transport of the disk backing the root filesystem."""
    src = _run(["findmnt", "-no", "SOURCE", "/"])
    if not src:
        return ""
    for line in _run(["lsblk", "-no", "TRAN", src]).splitlines():
        t = line.strip()
        if t:
            return t
    return ""


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


_BYTES_PER_GIB = 1024 * 1024 * 1024


def _format_gib(b: int) -> str:
    """Format bytes as "X.Y GiB"."""
    return f"{b / _BYTES_PER_GIB:.1f} GiB"


def _run(cmd: list[str]) -> str:
    """Run a fixed diagnostic command and return trimmed stdout, or "" on any error."""
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        return result.stdout.strip()
    except Exception:
        return ""


def _read_lines(path: str) -> list[str]:
    try:
        return Path(path).read_text().splitlines()
    except Exception:
        return []


def _read_first_line(path: str) -> str:
    lines = _read_lines(path)
    return lines[0].strip() if lines else ""
