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

from __future__ import annotations

import argparse
import traceback

import _generate_api_c_dlsym as dlsym
import _generate_api_c_h as header


def main(*, mode: str = "headers") -> None:
    if mode == "dlsym":
        dlsym.generate()
        return
    if mode == "docs":
        header.generate_docs()
        return

    # Default: the FFI headers/stubs the local delivery gate keeps committed.
    header.generate_headers()
    dlsym.generate()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--headers-only",
        dest="mode",
        action="store_const",
        const="headers",
        help=(
            "Generate only the FFI C header, its Go copy, and the dlsym stub "
            "(the default)."
        ),
    )
    mode.add_argument(
        "--docs",
        dest="mode",
        action="store_const",
        const="docs",
        help="Generate only the C API HTML docs under docs/c-api.",
    )
    mode.add_argument(
        "--dlsym-only",
        dest="mode",
        action="store_const",
        const="dlsym",
        help="Generate only bindings/go/internal/native/openpit_dlsym.c.",
    )
    parser.set_defaults(mode="headers")
    args = parser.parse_args()
    try:
        main(mode=args.mode)
    except (
        header.UnmappedRustTypeError,
        header.UnsupportedStructShapeError,
        header.UnsupportedDocMarkupError,
        header.MissingSitePartialError,
    ) as error:
        frame = traceback.extract_tb(error.__traceback__)[-1]
        raise SystemExit(f"{frame.filename}:{frame.lineno}: {error}") from None
