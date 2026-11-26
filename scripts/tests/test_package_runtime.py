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

import os
import subprocess
from pathlib import Path

import pytest

pytestmark = pytest.mark.skipif(os.name == "nt", reason="Exercises the Unix recipe")

REPO = Path(__file__).resolve().parents[2]
LIB_NAME = "libopenpit_ffi.so"


def fake_strip(directory, name, flag, fail):
    strip = directory / "bin" / name
    strip.parent.mkdir(exist_ok=True)
    help_output = "  --strip-unneeded" if flag == "--strip-unneeded" else ""
    strip_command = f"echo 'fake {name} failed' >&2\nexit 7" if fail else "exit 0"
    strip.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = --help ]; then\n'
        f"    echo '{help_output}'\n"
        "    exit 0\n"
        "fi\n"
        f'if [ "$1" != "{flag}" ]; then\n'
        '    echo "unexpected strip flag: $1" >&2\n'
        "    exit 2\n"
        "fi\n"
        f"{strip_command}\n",
        encoding="utf-8",
    )
    strip.chmod(0o755)


def package_runtime(directory, target):
    for path in (
        directory / "target" / target / "release" / LIB_NAME,
        directory / "bindings/c/openpit.h",
        directory / "LICENSE",
        directory / "OWNERS",
        directory / "THIRD-PARTY-LICENSES",
    ):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.touch()

    return subprocess.run(
        [
            "just",
            "--justfile",
            str(REPO / "pipeline.just"),
            "--working-directory",
            str(directory),
            "package-runtime",
            target,
            "linux",
            "amd64",
            LIB_NAME,
        ],
        # The justfile puts PIT_NODE_DIR/bin first on the recipe's PATH.
        env={**os.environ, "PIT_NODE_DIR": str(directory)},
        capture_output=True,
        text=True,
    )


@pytest.mark.parametrize("flag", ["--strip-unneeded", "-x"])
@pytest.mark.parametrize("fail", [False, True], ids=["success", "failure"])
def test_package_runtime_with_native_strip(tmp_path, flag, fail):
    fake_strip(tmp_path, "strip", flag, fail)
    target = f"{os.uname().machine}-unknown-linux-gnu"
    result = package_runtime(tmp_path, target)

    if fail:
        assert "fake strip failed" in result.stderr
        assert (
            result.returncode != 0
        ), "package-runtime succeeded despite a failing strip"
    else:
        assert result.returncode == 0, result.stderr
        assert (tmp_path / "dist" / f"openpit-ffi--linux-amd64-{LIB_NAME}").is_file()


@pytest.mark.parametrize("fail", [False, True], ids=["success", "failure"])
def test_package_runtime_with_cross_strip(tmp_path, fail):
    target_arch = "aarch64" if os.uname().machine == "x86_64" else "x86_64"
    tool = f"{target_arch}-linux-gnu-strip"
    fake_strip(tmp_path, "strip", "--strip-unneeded", fail=True)
    fake_strip(tmp_path, tool, "--strip-unneeded", fail)
    result = package_runtime(tmp_path, f"{target_arch}-unknown-linux-gnu")

    if fail:
        assert f"fake {tool} failed" in result.stderr, result.stderr
        assert (
            result.returncode != 0
        ), "package-runtime succeeded despite a failing cross strip"
    else:
        assert result.returncode == 0, result.stderr
        assert (tmp_path / "dist" / f"openpit-ffi--linux-amd64-{LIB_NAME}").is_file()
