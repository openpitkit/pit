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

"""Market-data feed wrapper for the spot_table example."""

from __future__ import annotations

import openpit
import openpit.marketdata as marketdata
from builder import parse_instrument
from table import Row


class MarketFeed:
    """Wraps a live MarketDataService and replays TICK rows against it.

    Each execution mode owns one feed over its own service: the runner
    registers every instrument that any TICK row mentions up front, then
    pushes quotes live at each TICK row's position. The feed also remembers
    the last price pushed per instrument so a FILL row may omit its price
    and reuse the latest quote as the lock price.
    """

    def __init__(self, service: marketdata.MarketDataService) -> None:
        self.service = service
        self._ids: dict[str, marketdata.InstrumentId] = {}
        self._latest: dict[str, str] = {}

    def register_instruments(self, rows: list[Row]) -> None:
        """Register every instrument named by a TICK row.

        Registration only creates the slot; quotes are published later by
        push/push_for.
        """
        for row in rows:
            if row.action != "TICK":
                continue
            if row.instrument in self._ids:
                continue
            try:
                instrument = parse_instrument(row.instrument)
            except ValueError as exc:
                raise ValueError(f"line {row.line}: {exc}") from None
            self._ids[row.instrument] = self.service.register(instrument)

    def push(self, instrument: str, price: str) -> None:
        """Publish a global mark-price snapshot for instrument."""
        iid, quote = self._quote(instrument, price)
        self.service.push(iid, quote)
        self._latest[instrument] = price

    def push_for(
        self,
        instrument: str,
        price: str,
        accounts: list[openpit.param.AccountId],
        groups: list[openpit.param.AccountGroupId],
    ) -> None:
        """Publish an addressed mark-price snapshot for specific targets."""
        iid, quote = self._quote(instrument, price)
        self.service.push_for(iid, quote, accounts, groups)
        self._latest[instrument] = price

    def latest_price(self, instrument: str) -> str | None:
        """Return the last price string pushed for instrument, or None."""
        return self._latest.get(instrument)

    def _quote(
        self, instrument: str, price: str
    ) -> tuple[marketdata.InstrumentId, marketdata.Quote]:
        iid = self._ids.get(instrument)
        if iid is None:
            raise ValueError(
                f"instrument {instrument} is not registered"
                " (every TICK instrument must appear in the table)"
            )
        return iid, marketdata.Quote(mark=price)
