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

# Source: bindings/python/README.md


def send_order_to_venue(order: openpit.Order) -> None:
    _ = order


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

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument(
            openpit.param.Asset("AAPL"),
            openpit.param.Asset("USD"),
        ),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.Quantity("100"),
        price=openpit.param.Price("185"),
    ),
)

start_result = engine.start_pre_trade(order=order)
assert start_result.ok

request = start_result.request
execute_result = request.execute()
assert execute_result.ok

reservation = execute_result.reservation

try:
    send_order_to_venue(order)
except Exception:
    reservation.rollback()
    raise

reservation.commit()

report = openpit.ExecutionReport(
    operation=openpit.ExecutionReportOperation(
        instrument=openpit.Instrument(
            openpit.param.Asset("AAPL"),
            openpit.param.Asset("USD"),
        ),
        side=openpit.param.Side.BUY,
    ),
    financial_impact=openpit.FinancialImpact(
        pnl=openpit.param.Pnl("-50"),
        fee=openpit.param.Fee("3.4"),
    ),
)

result = engine.apply_execution_report(report=report)
assert result.kill_switch_triggered is False
