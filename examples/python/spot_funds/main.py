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

"""Example spot_funds.

The smallest end-to-end integration of OpenPit's built-in SpotFunds
pre-trade policy: it shows how a buy order reserves settlement cash, how
a second order is rejected because that cash is still held, and how a
fill settles the held reservation.

What is illustrated:

- building a limit-only engine with SpotFunds + OrderValidation
- seeding an account's available cash via apply_account_adjustment
- the reservation mechanic: a committed BUY holds settlement funds, so a
  follow-up BUY that needs the same cash is rejected with InsufficientFunds
- tying a fill back to its reservation by carrying the pre-trade lock on
  the execution report, so SpotFunds settles the right held amount

Audience: an integrator who wants to lift the SpotFunds call pattern into
their own order/fill pipeline.

What you typically change to adapt this example to your own application:

1. Engine policies - see ``build_engine`` below.
2. The seed balance and the orders - here they are hard-coded constants
   chosen so the reservation mechanic is the lesson; your system feeds
   real account state and strategy orders.
3. The print statements - replace them with your order-router and
   fill-handler side effects.

The example is deliberately flat: main() reads top-to-bottom as a story,
and every engine call is factored into a small named helper that the smoke
test reuses. For a table-driven / load-testing harness around the same
policy, see ../spot_table.
"""

from __future__ import annotations

import sys

import openpit
from openpit import pretrade

# =============================================================================
# Scenario constants. The numbers are picked so the reservation is the whole
# point: two identical 60000-notional buys do not both fit inside a 100000
# balance, because the first one's funds stay held until it fills.
# =============================================================================

SCENARIO_ACCOUNT = 99_224_416  # same account as rate_pnl_killswitch
SCENARIO_ASSET_TRADED = "AAPL"  # underlying
SCENARIO_ASSET_SETTLE = "USD"  # settlement asset whose funds are reserved
SCENARIO_SEED_FUNDS = "100000"  # initial available USD
SCENARIO_ORDER_PRICE = "2000"  # limit price; also the lock/reservation price
SCENARIO_ORDER_QTY = "30"  # each buy is 30 * 2000 = 60000 USD notional

# Derived amounts used only in the narration below: one buy's notional
# (qty * price) and what stays available after the first buy's funds are held.
ORDER_NOTIONAL = 60_000  # SCENARIO_ORDER_QTY * SCENARIO_ORDER_PRICE
AVAILABLE_AFTER_BUY1 = 40_000  # SCENARIO_SEED_FUNDS - ORDER_NOTIONAL


def main() -> int:
    account = openpit.param.AccountId.from_int(SCENARIO_ACCOUNT)

    # Step 1 - build the engine. Limit-only SpotFunds plus OrderValidation;
    # do this once at platform start-up.
    engine = build_engine()

    # Step 2 - seed the account's available settlement cash. SpotFunds has no
    # initial-balance builder option; the balance is established through the
    # account-adjustment pipeline, exactly as a deposit would be.
    seed_funds(engine, account, SCENARIO_SEED_FUNDS)
    print(
        f"seeded account with {SCENARIO_SEED_FUNDS} {SCENARIO_ASSET_SETTLE} available"
    )

    # Step 3 - Buy #1: BUY 30 AAPL @ 2000 (60000 USD notional). It fits inside
    # the 100000 balance, so the pre-trade check accepts it. Committing the
    # reservation moves 60000 from available to held. We capture the
    # reservation's pre-trade lock first - the fill in Step 5 must carry it
    # back so SpotFunds settles this exact reservation.
    buy1 = build_order(account)
    lock1, rejects = place_order(engine, buy1)
    if rejects:
        raise RuntimeError(f"buy #1 unexpectedly rejected: {describe(rejects)}")
    print(
        f"buy #1 accepted: held {ORDER_NOTIONAL} {SCENARIO_ASSET_SETTLE},"
        f" {AVAILABLE_AFTER_BUY1} {SCENARIO_ASSET_SETTLE} now available"
    )

    # Step 4 - Buy #2: an identical BUY 30 AAPL @ 2000. This is the teaching
    # point. Only 40000 USD is available now (60000 is held by Buy #1), but the
    # order needs 60000, so SpotFunds rejects it with InsufficientFunds. A
    # rejected order produces no reservation - there is nothing to commit.
    buy2 = build_order(account)
    lock2, rejects = place_order(engine, buy2)
    if lock2 is not None:
        raise RuntimeError("buy #2 unexpectedly accepted")
    if not contains_code(rejects, pretrade.RejectCode.INSUFFICIENT_FUNDS):
        raise RuntimeError(f"buy #2 rejected for the wrong reason: {describe(rejects)}")
    print(
        f"buy #2 rejected: {describe(rejects)}" " (held funds reduce what is available)"
    )

    # Step 5 - fill Buy #1 in full. The execution report carries the lock we
    # captured at commit time, so SpotFunds matches the fill to Buy #1's
    # reservation and settles the 60000 it was holding. No account block means
    # the settlement succeeded.
    fill = build_fill_report(account, lock1)
    result = apply_fill(engine, fill)
    if result.account_blocks:
        raise RuntimeError("fill produced an unexpected account block")
    print(
        f"buy #1 filled: {ORDER_NOTIONAL} {SCENARIO_ASSET_SETTLE} reservation settled,"
        " no account block"
    )

    return 0


# =============================================================================
# Shared helpers. main() and the smoke test both call these; each wraps one
# engine interaction so the flow above stays readable.
# =============================================================================


def build_engine() -> openpit.Engine:
    """Wire a limit-only engine with the SpotFunds policy.

    OrderValidation is registered first so the engine refuses malformed
    orders before SpotFunds sees them. SpotFunds is not given
    ``.market_data(...)``, so market orders (no limit price) are rejected
    with UnsupportedOrderType - this example only sends limit orders.
    """
    policies = pretrade.policies
    return (
        openpit.Engine.builder()
        .full_sync()
        .builtin(policies.build_order_validation())
        .builtin(policies.build_spot_funds())
        .build()
    )


def seed_funds(
    engine: openpit.Engine,
    account: openpit.param.AccountId,
    funds: str,
) -> None:
    """Set the account's available settlement balance to an absolute amount.

    An absolute adjustment overwrites the balance (unlike a relative delta),
    so it reads as "set available USD to funds".
    """
    result = engine.apply_account_adjustment(
        account_id=account,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(
                    asset=SCENARIO_ASSET_SETTLE,
                ),
                amount=openpit.AccountAdjustmentAmount(
                    balance=openpit.param.AdjustmentAmount.absolute(
                        openpit.param.PositionSize(funds)
                    ),
                ),
            )
        ],
    )
    if not result.ok:
        raise RuntimeError(f"seed adjustment rejected: {result.rejects}")


def build_order(account: openpit.param.AccountId) -> openpit.Order:
    """Assemble a BUY limit order for the scenario instrument.

    A real strategy builds this from a signal and current market data.
    """
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(SCENARIO_ASSET_TRADED, SCENARIO_ASSET_SETTLE),
            account_id=account,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(SCENARIO_ORDER_QTY),
            price=openpit.param.Price(SCENARIO_ORDER_PRICE),
        ),
    )


def place_order(
    engine: openpit.Engine,
    order: openpit.Order,
) -> tuple[pretrade.Lock | None, list[pretrade.Reject]]:
    """Run the pre-trade check and, on accept, commit the reservation.

    Returns the committed reservation's pre-trade lock so the caller can
    later attach it to the matching fill; on reject it returns ``None`` lock
    and the rejects. The lock MUST be read before ``commit()``, because
    ``reservation.lock()`` raises once the reservation is finalized.
    """
    result = engine.execute_pre_trade(order=order)
    if not result:
        # A rejected order reserves nothing; there is no lock and nothing
        # to commit.
        return None, list(result.rejects)
    # Snapshot the lock the engine assigned to this reservation, then commit.
    # commit() moves the reserved settlement funds from available to held;
    # rollback() would release them instead.
    lock = result.reservation.lock()
    result.reservation.commit()
    return lock, []


def build_fill_report(
    account: openpit.param.AccountId,
    lock: pretrade.Lock,
) -> openpit.ExecutionReport:
    """Assemble a full, final execution report for a buy order.

    The pre-trade lock captured when the reservation was committed is
    attached to the fill. Carrying that lock is what ties the fill back to
    the reservation: SpotFunds reads the lock to find which held funds to
    settle. Reusing the stored Lock object is more faithful than rebuilding
    the lock - it is exactly what the engine produced - but an equivalent
    lock can be reconstructed with
    ``pretrade.Lock(entries=[(pretrade.DEFAULT_POLICY_GROUP_ID, price)])``
    when the caller did not keep the reservation's lock (see ../spot_table).
    """
    price = openpit.param.Price(SCENARIO_ORDER_PRICE)
    qty = openpit.param.Quantity(SCENARIO_ORDER_QTY)
    # A full fill of a 30-lot order leaves nothing outstanding.
    leaves = openpit.param.Quantity("0")
    # Combined-mode impact: the fee is embedded in pnl, so both are zero for
    # a plain settlement. See the SpotFunds wiki page for the "separate" fee
    # convention.
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(SCENARIO_ASSET_TRADED, SCENARIO_ASSET_SETTLE),
            account_id=account,
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl("0"),
            fee=openpit.param.Fee("0"),
        ),
        fill=openpit.ExecutionReportFillDetails(
            last_trade=openpit.param.Trade(price=price, quantity=qty),
            leaves_quantity=leaves,
            lock=lock,
            is_final=True,
        ),
    )


def apply_fill(
    engine: openpit.Engine,
    report: openpit.ExecutionReport,
) -> openpit.PostTradeResult:
    """Feed a completed execution report to the engine.

    The returned ``PostTradeResult.account_blocks`` is empty when settlement
    succeeds; a non-empty list would mean a policy permanently blocked the
    account.
    """
    return engine.apply_execution_report(report=report)


def contains_code(rejects: list[pretrade.Reject], want: pretrade.RejectCode) -> bool:
    """Report whether the rejects include the given business code."""
    return any(r.code == want for r in rejects)


def describe(rejects: list[pretrade.Reject]) -> str:
    """Render rejects as "reason (details)" pairs for a one-line message."""
    if not rejects:
        return "no rejects"
    return "; ".join(f"{r.reason} ({r.details})" for r in rejects)


if __name__ == "__main__":
    sys.exit(main())
