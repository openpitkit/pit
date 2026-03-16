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

"""Embeddable pre-trade risk SDK for trading systems.

``openpit`` evaluates orders through a deterministic two-stage pipeline
before they leave the application. The package provides:

- :class:`Engine` — single-threaded pre-trade risk engine.
- :mod:`~openpit.param` — typed financial values (Price, Pnl, Quantity, etc.)
  with exact decimal arithmetic.
- :mod:`~openpit.pretrade` — pluggable policy interfaces, standard reject
  codes, deferred requests, and reservations.
- :mod:`~openpit.core` — order and execution-report group models.

Quickstart::

    import openpit

    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
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

    result = engine.start_pre_trade(order=order)
"""

from . import core, param, pretrade
from ._openpit import Engine, EngineBuilder, RejectError
from .core import (
    ExecutionReport,
    ExecutionReportFillDetails,
    ExecutionReportOperation,
    ExecutionReportPositionImpact,
    FinancialImpact,
    Instrument,
    Order,
    OrderMargin,
    OrderOperation,
    OrderPosition,
)
from .param import Leverage
from .pretrade import PostTradeResult

Engine.__doc__ = """
Single-threaded pre-trade risk engine.

The engine evaluates orders through an explicit two-stage pipeline and accepts
post-trade execution reports to update cumulative policy state.
"""

EngineBuilder.__doc__ = """
Builder used to register start-stage and main-stage policies before engine creation.

Policy names must be unique across both stages in a single engine instance.
"""

RejectError.__doc__ = """
Exception raised for Python binding misuse or callback-level failures.

Normal policy rejects are returned through result objects instead of raising
this exception.
"""

__all__ = [
    "Engine",
    "EngineBuilder",
    "ExecutionReport",
    "ExecutionReportFillDetails",
    "ExecutionReportOperation",
    "ExecutionReportPositionImpact",
    "FinancialImpact",
    "Instrument",
    "Leverage",
    "Order",
    "OrderMargin",
    "OrderOperation",
    "OrderPosition",
    "PostTradeResult",
    "RejectError",
    "core",
    "param",
    "pretrade",
]
