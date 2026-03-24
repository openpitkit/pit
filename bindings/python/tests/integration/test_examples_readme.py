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

import openpit
import pytest


def send_order_to_venue(order: openpit.Order) -> None:
    _ = order


@pytest.mark.integration
def test_readme_quickstart() -> None:
    # Source: bindings/python/README.md — Usage

    # 1. Configure policies.
    pnl_policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset=openpit.param.Asset("USD"),
        barrier=openpit.param.Pnl("1000"),
    )

    rate_limit_policy = openpit.pretrade.policies.RateLimitPolicy(
        max_orders=100,
        window_seconds=1,
    )

    order_size_policy = openpit.pretrade.policies.OrderSizeLimitPolicy(
        limit=openpit.pretrade.policies.OrderSizeLimit(
            settlement_asset=openpit.param.Asset("USD"),
            max_quantity=openpit.param.Quantity("500"),
            max_notional=openpit.param.Volume("100000"),
        ),
    )

    # 2. Build the engine (one time at the platform initialization).
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .check_pre_trade_start_policy(policy=pnl_policy)
        .check_pre_trade_start_policy(policy=rate_limit_policy)
        .check_pre_trade_start_policy(policy=order_size_policy)
        .build()
    )

    # 3. Check an order.
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Quantity("100"),
            price=openpit.param.Price("185"),
        ),
    )

    start_result = engine.start_pre_trade(order=order)

    if not start_result:
        reject = start_result.reject
        raise RuntimeError(
            f"{reject.policy} [{reject.code}]: {reject.reason}: {reject.details}"
        )

    request = start_result.request

    # 4. Quick, lightweight checks, such as fat-finger scope or enabled kill
    # switch, were performed during pre-trade request creation. The system state
    # has not yet changed, except in cases where each request, even rejected ones,
    # must be considered. Before the heavy-duty checks, other work on the request
    # can be performed simply by holding the request object.

    # 5. Real pre-trade and risk control.
    execute_result = request.execute()

    if not execute_result:
        messages = ", ".join(
            f"{reject.policy} [{reject.code}]: {reject.reason}: {reject.details}"
            for reject in execute_result.rejects
        )
        raise RuntimeError(messages)

    reservation = execute_result.reservation

    # 6. If the request is successfully sent to the venue, it must be committed.
    # The rollback must be called otherwise to revert all performed reservations.
    try:
        send_order_to_venue(order)
    except Exception:
        reservation.rollback()
        raise

    reservation.commit()

    # 7. The order goes to the venue and returns with an execution report.
    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl("-50"),
            fee=openpit.param.Fee("3.4"),
        ),
    )

    result = engine.apply_execution_report(report=report)

    # 8. After each execution report is applied, the system may report that it has
    # been determined in advance that all subsequent requests will be rejected if
    # the account status does not change.
    assert result.kill_switch_triggered is False
