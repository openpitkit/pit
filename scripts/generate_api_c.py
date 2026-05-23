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

from __future__ import annotations

import argparse
import traceback

import _generate_api_c_dlsym as dlsym
import _generate_api_c_h as header


def main(*, dlsym_only: bool = False) -> None:
    if dlsym_only:
        dlsym.generate()
        return

    header.main()
    dlsym.generate()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--dlsym-only",
        action="store_true",
        help="Generate only bindings/go/internal/native/openpit_dlsym.c.",
    )
    args = parser.parse_args()
    try:
        main(dlsym_only=args.dlsym_only)
    except (
        header.UnmappedRustTypeError,
        header.UnsupportedStructShapeError,
    ) as error:
        frame = traceback.extract_tb(error.__traceback__)[-1]
        raise SystemExit(f"{frame.filename}:{frame.lineno}: {error}") from None
