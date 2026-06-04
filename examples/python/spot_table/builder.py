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

"""Domain-object builders for the spot_table example."""

from __future__ import annotations

import typing

import openpit
from openpit import pretrade
from table import Row

if typing.TYPE_CHECKING:
    from market_feed import MarketFeed


def split_instrument(s: str) -> tuple[str, str]:
    """Split "BASE/QUOTE" into (base, quote). Raises ValueError if malformed."""
    i = s.find("/")
    if i <= 0 or i == len(s) - 1:
        raise ValueError(f"instrument {s!r} must be BASE/QUOTE")
    return s[:i], s[i + 1 :]


def parse_instrument(s: str) -> openpit.Instrument:
    """Turn "BASE/QUOTE" into an engine Instrument."""
    base, quote = split_instrument(s)
    return openpit.Instrument(base, quote)


def parse_side(s: str) -> openpit.param.Side:
    """Convert BUY/SELL to a Side enum value."""
    if s == "BUY":
        return openpit.param.Side.BUY
    if s == "SELL":
        return openpit.param.Side.SELL
    raise ValueError(f"side must be BUY or SELL, got {s!r}")


def account_id(s: str) -> openpit.param.AccountId:
    """Convert a free-form account label to a stable AccountId.

    The engine hashes the string via FNV-1a; the runner keeps the source
    string for diagnostics.
    """
    if not s:
        raise ValueError("account is required")
    return openpit.param.AccountId.from_string(s)


def account_group_id(s: str) -> openpit.param.AccountGroupId:
    """Convert a free-form group label to a stable AccountGroupId."""
    if not s:
        raise ValueError("group is required")
    return openpit.param.AccountGroupId.from_string(s)


def build_seed_adjustment(row: Row) -> openpit.AccountAdjustment:
    """Turn a SEED row into an AccountAdjustment seeding an absolute balance."""
    return openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset=row.asset),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(row.amount)
            )
        ),
    )


def build_trade_amount(row: Row) -> openpit.param.TradeAmount:
    """Turn an ORDER row's qty or volume cell into a TradeAmount.

    Exactly one of the two is set; the parser already enforced that.
    """
    if row.volume:
        return openpit.param.TradeAmount.volume(row.volume)
    return openpit.param.TradeAmount.quantity(row.qty)


def build_order(row: Row, acc: openpit.param.AccountId) -> openpit.Order:
    """Turn an ORDER row into an Order. Empty price means market order."""
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=parse_instrument(row.instrument),
            account_id=acc,
            side=parse_side(row.side),
            trade_amount=build_trade_amount(row),
            price=(None if row.price == "" else openpit.param.Price(row.price)),
        )
    )


def build_fill_report(
    row: Row, acc: openpit.param.AccountId, feed: MarketFeed
) -> openpit.ExecutionReport:
    """Turn a FILL row into a final ExecutionReport.

    The price column on a FILL is the lock price (limit price for limit
    orders, mark price for market orders). When it is empty the most recent
    quote pushed for the instrument is reused.
    """
    price_str = row.price or feed.latest_price(row.instrument)
    if not price_str:
        raise ValueError(f"FILL needs a price or a prior TICK for {row.instrument}")
    price = openpit.param.Price(price_str)
    # The fill carries the pre-trade lock that ties it back to the reservation
    # the matching ORDER committed: one entry under the spot funds policy's
    # default group at the lock/reservation price.
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=parse_instrument(row.instrument),
            account_id=acc,
            side=parse_side(row.side),
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl(row.pnl or "0"),
            fee=openpit.param.Fee(row.fee or "0"),
        ),
        fill=openpit.ExecutionReportFillDetails(
            last_trade=openpit.param.Trade(
                price=price,
                quantity=openpit.param.Quantity(row.qty),
            ),
            leaves_quantity=openpit.param.Quantity("0"),
            lock=pretrade.Lock(entries=[(pretrade.DEFAULT_POLICY_GROUP_ID, price)]),
            is_final=True,
        ),
    )
