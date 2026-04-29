from .._openpit import (
    OrderSizeLimit,
    OrderSizeLimitPolicy,
    OrderValidationPolicy,
    PnlBoundsKillSwitchPolicy,
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

PnlBoundsKillSwitchPolicy.__doc__ = """
Built-in start-stage kill-switch policy driven by accumulated settlement P&L.

The policy tracks accumulated outcome per settlement asset and blocks new
requests when P&L moves outside configured bounds.

`lower_bound` is typically negative and represents the loss limit.
`upper_bound` is typically positive and represents the profit-taking limit.
The constructor does not validate signs, ordering, or whether `initial_pnl` is
inside bounds.
If `initial_pnl` is outside the band, the first `start_pre_trade` is rejected.
If `lower_bound > upper_bound`, `start_pre_trade` keeps rejecting until
`apply_execution_report` moves the accumulator into bounds or the engine
instance is rebuilt.
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
    "PnlBoundsKillSwitchPolicy",
    "RateLimitPolicy",
]
