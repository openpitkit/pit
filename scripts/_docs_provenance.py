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
# Please see https://openpit.dev and the OWNERS file for details.

"""Identify the source checkout used to build public documentation."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path


def read_provenance(root: Path) -> dict[str, str]:
    """Read the package version and exact source revision, without guessing."""
    manifest = root / "crates" / "openpit" / "Cargo.toml"
    try:
        text = manifest.read_text(encoding="utf-8")
    except OSError as error:
        raise ValueError(
            f"Cannot read documentation package version: {manifest}"
        ) from error
    package = re.search(r"(?ms)^\[package\]\s*\n(.*?)(?=^\[|\Z)", text)
    version = (
        None
        if package is None
        else re.search(r'^version\s*=\s*"([^"]+)"\s*$', package.group(1), re.MULTILINE)
    )
    if (
        version is None
        or re.fullmatch(
            r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?",
            version.group(1),
        )
        is None
    ):
        raise ValueError(
            f"Missing or invalid documentation package version: {manifest}"
        )
    try:
        revision = subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            text=True,
            stderr=subprocess.PIPE,
        ).strip()
        status = subprocess.check_output(
            [
                "git",
                "-C",
                str(root),
                "status",
                "--porcelain",
                "--untracked-files=normal",
            ],
            text=True,
            stderr=subprocess.PIPE,
        ).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise ValueError("Cannot resolve documentation source revision") from error
    if re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", revision) is None:
        raise ValueError(f"Invalid documentation source revision: {revision!r}")
    return {
        "version": version.group(1),
        "revision": revision,
        "source_state": "working-tree" if status else "committed",
    }


def write_provenance(root: Path, output: Path) -> None:
    """Mark a completed Python HTML build with its source identity."""
    if not (output / "index.html").is_file():
        raise ValueError(f"Python documentation output is missing: {output}")
    provenance = read_provenance(root)
    (output / "docs-provenance.json").write_text(
        json.dumps(provenance, indent=2) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    write_provenance(Path(__file__).resolve().parents[1], arguments.output)
