#!/usr/bin/env python3
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

"""Render a versioned OpenPit vcpkg port from the tracked registry template."""

from __future__ import annotations

import argparse
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--package-version", required=True)
    parser.add_argument("--runtime-version", required=True)
    parser.add_argument("--source-ref", required=True)
    parser.add_argument("--source-sha512", required=True)
    parser.add_argument("--version", required=True)
    return parser.parse_args()


def render(template: Path, replacements: dict[str, str]) -> str:
    contents = template.read_text(encoding="utf-8")
    for placeholder, value in replacements.items():
        contents = contents.replace(placeholder, value)
    if "@OPENPIT_" in contents:
        raise ValueError(f"unresolved OpenPit template placeholder in {template}")
    return contents


def main() -> None:
    args = parse_args()
    root = Path(__file__).resolve().parent.parent / "packaging" / "vcpkg" / "openpit"
    replacements = {
        "@OPENPIT_PACKAGE_VERSION@": args.package_version,
        "@OPENPIT_RUNTIME_VERSION@": args.runtime_version,
        "@OPENPIT_SOURCE_REF@": args.source_ref,
        "@OPENPIT_SOURCE_SHA512@": args.source_sha512,
        "@OPENPIT_VERSION@": args.version,
    }

    args.destination.mkdir(parents=True, exist_ok=True)
    (args.destination / "portfile.cmake").write_text(
        render(root / "portfile.cmake.in", replacements), encoding="utf-8"
    )
    (args.destination / "vcpkg.json").write_text(
        render(root / "vcpkg.json.in", replacements), encoding="utf-8"
    )


if __name__ == "__main__":
    main()
