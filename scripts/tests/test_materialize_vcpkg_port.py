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

import importlib.util
import json
import sys
from pathlib import Path

import pytest

SCRIPT_PATH = Path(__file__).resolve().parents[1] / "materialize_vcpkg_port.py"
TEMPLATE_DIR = SCRIPT_PATH.parents[1] / "packaging" / "vcpkg" / "openpit"


def load_module():
    spec = importlib.util.spec_from_file_location("materialize_vcpkg_port", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def render_port(module, tmp_path, monkeypatch):
    runtime_dir = tmp_path / "runtime"
    runtime_dir.mkdir()
    for asset in module.RUNTIME_ASSETS.values():
        (runtime_dir / asset).write_bytes(asset.encode())
    destination = tmp_path / "port"
    options = {
        "--destination": destination,
        "--package-version": "1.2.3",
        "--runtime-dir": runtime_dir,
        "--runtime-url-base": "https://example.invalid/v1.2.3",
        "--runtime-version": "1.2.3",
        "--source-ref": "v1.2.3",
        "--source-sha512": "0" * 128,
        "--version": "1.2.3",
    }
    argv = ["materialize_vcpkg_port.py"]
    for option, value in options.items():
        argv += [option, str(value)]
    monkeypatch.setattr(sys, "argv", argv)
    module.main()
    return destination


def test_rendered_portfile_drops_the_template_license_header(tmp_path, monkeypatch):
    module = load_module()
    template = (TEMPLATE_DIR / "portfile.cmake.in").read_text(encoding="utf-8")

    port = render_port(module, tmp_path, monkeypatch)

    portfile = (port / "portfile.cmake").read_text(encoding="utf-8")
    assert "SPDX-License-Identifier:" in template
    assert "SPDX-License-Identifier:" not in portfile
    assert "vcpkg_check_linkage(ONLY_DYNAMIC_LIBRARY)" in portfile
    manifest = json.loads((port / "vcpkg.json").read_text(encoding="utf-8"))
    assert manifest["version"] == "1.2.3"


@pytest.mark.parametrize(
    "contents",
    [
        "set(A 1)\n\nset(B 2)\n",
        "# SPDX-License-Identifier: Apache-2.0\nset(A 1)\n",
        "# Unrelated comment\n\nset(A 1)\n",
    ],
    ids=["no-header", "no-blank-line-after-header", "comment-without-license"],
)
def test_template_without_license_header_is_rejected(contents):
    module = load_module()

    with pytest.raises(ValueError, match="does not open with the license header"):
        module.without_license_header(contents, Path("portfile.cmake.in"))
