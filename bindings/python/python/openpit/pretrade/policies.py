from .._openpit import (
    OrderSizeLimit,
    OrderSizeLimitPolicy,
    OrderValidationPolicy,
    PnlKillSwitchPolicy,
    RateLimitPolicy,
)

OrderSizeLimit.__doc__ = """
Order-size limits for one settlement asset.

This helper groups the settlement asset together with the maximum permitted
quantity and notional. ``OrderSizeLimitPolicy`` consumes one or more of these
objects to enforce per-asset admission limits.
"""

OrderSizeLimitPolicy.__doc__ = """
Built-in start-stage policy that enforces per-settlement-asset size limits.

The policy checks quantity and notional independently and emits standard reject
codes when a configured limit is missing or exceeded.
"""

OrderValidationPolicy.__doc__ = """
Built-in start-stage policy that validates the order payload shape.

Use this as the first start-stage policy when the engine should reject missing
required fields or malformed values before any strategy-specific logic runs.
"""

PnlKillSwitchPolicy.__doc__ = """
Built-in start-stage kill-switch policy driven by realized settlement P&L.

The policy tracks realized outcome by settlement asset and blocks new requests
once the configured loss barrier is reached or crossed.
"""

RateLimitPolicy.__doc__ = """
Built-in start-stage rate-limit policy.

It counts attempts in a sliding time window and rejects further requests once
the configured per-window limit has been exhausted.
"""

__all__ = [
    "OrderSizeLimit",
    "OrderSizeLimitPolicy",
    "OrderValidationPolicy",
    "PnlKillSwitchPolicy",
    "RateLimitPolicy",
]
