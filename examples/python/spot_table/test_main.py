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

"""Tests for the spot_table example.

``test_fast`` runs the shared coverage scenario once and asserts every verdict
against the single sequential engine.
"""

from __future__ import annotations

import os

import pytest
import runner
import table

# The shared scenario lives outside the example so one file backs the CLI, the
# tests, and the just targets.
COVERAGE_TABLE = os.path.join(
    os.path.dirname(__file__), "..", "..", "tables", "spot", "coverage.md"
)


def test_fast() -> None:
    """Run the coverage scenario once and assert every row's verdict.

    The scenario uses every feature of the runner, so a green run covers the
    whole tool end to end in well under a second.
    """
    t = table.parse_file(COVERAGE_TABLE)
    report = runner.run_sync(t.fm, t.rows, None)
    if report.first_fail is not None:
        fail = report.first_fail
        pytest.fail(
            f"verdict mismatch at line {fail.row.line}"
            f" ({fail.row.account} {fail.row.action}): {fail.message}"
        )
    assert report.total > 0, f"zero executable rows in {COVERAGE_TABLE}"
