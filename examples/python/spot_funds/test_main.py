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

"""Assertion-driven counterpart of ``main``.

Drives the same shared helpers main() uses and asserts the outcomes that
make the example a lesson: the first buy is accepted (reserving funds), the
second identical buy is rejected with InsufficientFunds (those funds are
held), the fill - carrying the first reservation's lock - settles without an
account block, and after switching to track-only a third buy that exceeds
available funds is accepted instead of rejected.
"""

from __future__ import annotations

import main as example
import openpit
from openpit import pretrade


def test_spot_funds_reservation_flow() -> None:
    account = openpit.param.AccountId.from_int(example.SCENARIO_ACCOUNT)

    engine = example.build_engine()

    example.seed_funds(engine, account, example.SCENARIO_SEED_FUNDS)

    # Buy #1 must be accepted and yield a non-None lock to carry to the fill.
    buy1 = example.build_order(account)
    lock1, rejects = example.place_order(engine, buy1)
    assert not rejects, f"buy #1 rejected: {example.describe(rejects)}"
    assert lock1 is not None, "buy #1 accepted but produced no pre-trade lock"

    # Buy #2 must be rejected with InsufficientFunds: 60000 is held by buy #1,
    # only 40000 is available, and the order needs 60000.
    buy2 = example.build_order(account)
    lock2, rejects = example.place_order(engine, buy2)
    assert lock2 is None, "buy #2 was accepted; expected an InsufficientFunds reject"
    assert example.contains_code(
        rejects, pretrade.RejectCode.INSUFFICIENT_FUNDS
    ), f"buy #2 reject codes = {[r.code for r in rejects]}, want InsufficientFunds"

    # The fill carries buy #1's lock, so SpotFunds settles that reservation;
    # a successful settlement produces no account block.
    fill = example.build_fill_report(account, lock1)
    result = example.apply_fill(engine, fill)
    assert (
        not result.account_blocks
    ), f"fill produced {len(result.account_blocks)} account block(s), want 0"

    # After switching to track-only, an identical buy that needs 60000 with
    # only 40000 available is accepted (and yields a lock) instead of rejected.
    example.enable_track_only(engine)
    buy3 = example.build_order(account)
    lock3, rejects = example.place_order(engine, buy3)
    assert (
        lock3 is not None
    ), f"buy #3 rejected in track-only mode: {example.describe(rejects)}"
