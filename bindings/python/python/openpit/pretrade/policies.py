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

"""Built-in pre-trade policies for the Python binding."""

from .._openpit import (
    OrderSizeLimit,
    OrderSizeLimitPolicy,
    OrderValidationPolicy,
    PnlBoundsKillSwitchPolicy,
    RateLimitPolicy,
)

OrderSizeLimit.__doc__ = """
Order-size limits for one settlement asset.

Args:
    settlement_asset: Settlement asset to which the limits apply.
    max_quantity: Maximum allowed order quantity for this asset.
    max_notional: Maximum allowed notional volume for this asset.

Use with ``OrderSizeLimitPolicy`` to reject orders that are missing a matching
asset configuration, exceed quantity, exceed notional, or require price for
notional calculation but do not provide one.
"""

OrderSizeLimitPolicy.__doc__ = """
Built-in start-stage policy that enforces per-settlement-asset size limits.

Args:
    limit: Initial ``OrderSizeLimit`` configuration.

Methods:
    set_limit(limit): Replace or add a limit for the settlement asset carried
        by ``limit``.

Rejects:
    ``RISK_CONFIGURATION_MISSING`` when an order uses an unconfigured
    settlement asset, ``ORDER_QTY_EXCEEDS_LIMIT`` when quantity is above the
    configured limit, ``ORDER_NOTIONAL_EXCEEDS_LIMIT`` when notional is above
    the configured limit, and ``ORDER_VALUE_CALCULATION_FAILED`` when notional
    cannot be calculated from a quantity order without price.
"""

OrderValidationPolicy.__doc__ = """
Built-in start-stage policy that validates the order payload shape.

Use this as the first start-stage policy when the engine should reject missing
required fields or malformed values before strategy-specific logic runs.

Rejects:
    Standard validation reject codes such as ``MISSING_REQUIRED_FIELD`` and
    ``INVALID_FIELD_VALUE``.
"""

PnlBoundsKillSwitchPolicy.__doc__ = """
Built-in start-stage kill-switch policy driven by accumulated settlement P&L.

Args:
    settlement_asset: Settlement asset tracked by the initial barrier.
    lower_bound: Optional lower P&L bound. Usually a negative loss limit.
    upper_bound: Optional upper P&L bound. Usually a positive profit limit.
    initial_pnl: Initial accumulated P&L for ``settlement_asset``.

Methods:
    set_barrier(...): Add or replace bounds for a settlement asset.
    reset_pnl(settlement_asset): Reset accumulated P&L for one asset.

The policy tracks accumulated outcome per settlement asset and blocks new
requests when P&L moves outside configured bounds. At least one bound is
required. The constructor does not validate sign conventions, bound ordering,
or whether ``initial_pnl`` is inside bounds.
"""

RateLimitPolicy.__doc__ = """
Built-in start-stage rate-limit policy.

Args:
    max_orders: Maximum accepted attempts in the sliding window.
    window_seconds: Sliding-window length in seconds.

The policy counts attempts and returns ``RATE_LIMIT_EXCEEDED`` once the window
capacity has been exhausted.
"""

__all__ = [
    "OrderSizeLimit",
    "OrderSizeLimitPolicy",
    "OrderValidationPolicy",
    "PnlBoundsKillSwitchPolicy",
    "RateLimitPolicy",
]
