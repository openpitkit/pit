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

import os
import re
import subprocess
from pathlib import Path

PYTHON_BINDINGS_ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = PYTHON_BINDINGS_ROOT.parents[1]
PYTHON_SOURCE = PYTHON_BINDINGS_ROOT / "python"

if PYTHON_SOURCE.exists():
    import sys

    sys.path.insert(0, str(PYTHON_SOURCE))

project = "openpit Python API"
author = "The Pit Project Owners"
copyright = "2026, The Pit Project Owners"


def _release_from_git() -> str:
    ref_name = os.environ.get("GITHUB_REF_NAME")
    if ref_name:
        return ref_name.removeprefix("v")

    try:
        tag = subprocess.check_output(
            ["git", "describe", "--tags", "--exact-match"],
            cwd=PROJECT_ROOT,
            stderr=subprocess.DEVNULL,
            text=True,
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return "0.0.0+local"

    return tag.removeprefix("v")


release = os.environ.get("OPENPIT_DOCS_VERSION", _release_from_git()).removeprefix("v")
version = release

extensions = [
    "myst_parser",
    "sphinx.ext.autodoc",
]

source_suffix = {
    ".md": "markdown",
    ".rst": "restructuredtext",
}

master_doc = "index"
html_theme = "furo"
html_title = f"openpit Python API {release}"
html_static_path = []
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

nitpicky = True

autodoc_typehints = "description"
autodoc_typehints_format = "short"
autodoc_member_order = "bysource"
autodoc_default_options: dict[str, bool] = {}

autodoc_mock_imports = [
    item.strip()
    for item in os.environ.get("SPHINX_AUTODOC_MOCK_IMPORTS", "").split(",")
    if item.strip()
]

nitpick_ignore = [
    ("py:class", "abc.ABC"),
    ("py:class", "collections.abc.Iterable"),
    ("py:class", "enum.StrEnum"),
    ("py:class", "Mutation"),
    ("py:class", "openpit.Order"),
    ("py:class", "openpit.core.AccountAdjustment"),
    ("py:class", "openpit.core.AccountAdjustmentAmount"),
    ("py:class", "openpit.core.AccountAdjustmentBalanceOperation"),
    ("py:class", "openpit.core.AccountAdjustmentBounds"),
    ("py:class", "openpit.core.AccountAdjustmentPositionOperation"),
    ("py:class", "openpit.core.Mutation"),
    ("py:class", "PreTradeLock"),
    ("py:class", "PreTradeContext"),
    ("py:meth", "Policy.perform_pre_trade_check"),
]


def _normalize_python_docstrings(_app, _what, _name, _obj, _options, lines) -> None:
    in_markdown_fence = False
    in_google_section = False
    normalized: list[str] = []

    for line in lines:
        stripped = line.strip()

        if stripped.startswith("```"):
            if not in_markdown_fence:
                normalized.extend(["::", ""])
            in_markdown_fence = not in_markdown_fence
            continue

        if in_markdown_fence:
            normalized.append(f"    {line}")
            continue

        if stripped in {"Args:", "Attributes:", "Returns:"}:
            in_google_section = True
            normalized.extend([stripped, ""])
            continue

        field_match = re.match(r"^ {4}([A-Za-z_][A-Za-z0-9_.\[\]]*): ?(.*)$", line)
        if in_google_section and field_match:
            field_name, field_description = field_match.groups()
            normalized.append(f"- ``{field_name}``: {field_description}".rstrip())
            continue

        if in_google_section and line.startswith(" " * 4):
            normalized.append(f"  {line.strip()}")
            continue

        if in_google_section and stripped and not line[:1].isspace():
            in_google_section = False

        if (
            normalized
            and normalized[-1].startswith("- ")
            and stripped
            and not stripped.startswith("- ")
        ):
            normalized.append("")

        if normalized and normalized[-1].endswith(":") and stripped.startswith("- "):
            normalized.append("")

        normalized.append(line)

    lines[:] = normalized


def setup(app) -> None:
    app.connect("autodoc-process-docstring", _normalize_python_docstrings)
