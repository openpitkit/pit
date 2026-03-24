import enum

from . import _enum
from ._openpit import (
    AccountId,
    Asset,
    CashFlow,
    Fee,
    Leverage,
    Pnl,
    PositionSize,
    Price,
    Quantity,
    Volume,
)
from ._openpit import (
    AdjustmentAmount as _AdjustmentAmount,
)


@enum.unique
class Side(_enum.StrEnum):
    """Trade direction for orders and execution reports."""

    BUY = "buy"
    SELL = "sell"

    # @typing.override
    @classmethod
    def _missing_(cls, value: object) -> "Side | None":
        raise ValueError("expected 'buy' or 'sell'")

    def is_buy(self) -> bool:
        """Return ``True`` when the side is buy."""
        return self is self.BUY

    def is_sell(self) -> bool:
        """Return ``True`` when the side is sell."""
        return self is self.SELL

    def opposite(self) -> "Side":
        """Return the opposite side."""
        return self.SELL if self is self.BUY else self.BUY

    def sign(self) -> int:
        """Return ``+1`` for buy and ``-1`` for sell."""
        return 1 if self is self.BUY else -1


@enum.unique
class PositionSide(_enum.StrEnum):
    """Hedge-mode leg for derivatives positions."""

    LONG = "long"
    SHORT = "short"

    # @typing.override
    @classmethod
    def _missing_(cls, value: object) -> "PositionSide | None":
        raise ValueError("expected 'long' or 'short'")

    def is_long(self) -> bool:
        """Return ``True`` when the position side is long."""
        return self is self.LONG

    def is_short(self) -> bool:
        """Return ``True`` when the position side is short."""
        return self is self.SHORT

    def opposite(self) -> "PositionSide":
        """Return the opposite hedge-mode leg."""
        return self.SHORT if self is self.LONG else self.LONG


@enum.unique
class PositionEffect(_enum.StrEnum):
    """Whether an execution opens or closes exposure."""

    OPEN = "open"
    CLOSE = "close"

    # @typing.override
    @classmethod
    def _missing_(cls, value: object) -> "PositionEffect | None":
        raise ValueError("expected 'open' or 'close'")


@enum.unique
class PositionMode(_enum.StrEnum):
    """Netting vs hedged position mode."""

    NETTING = "netting"
    HEDGED = "hedged"

    # @typing.override
    @classmethod
    def _missing_(cls, value: object) -> "PositionMode | None":
        raise ValueError("expected 'netting' or 'hedged'")


class AdjustmentAmount(_AdjustmentAmount):
    """Delta or absolute payload wrapper."""

    # @typing.override
    def __new__(cls, *args: object, **kwargs: object) -> "AdjustmentAmount":
        return _AdjustmentAmount.__new__(cls, *args, **kwargs)

    # @typing.override
    def __init__(self, *, kind: str, value: PositionSize) -> None:
        if kind not in {"delta", "absolute"}:
            raise ValueError("kind must be 'delta' or 'absolute'")
        if not isinstance(value, PositionSize):
            raise TypeError(
                f"value must be {PositionSize.__module__}.{PositionSize.__name__}"
            )

    @staticmethod
    def delta(value: PositionSize) -> "AdjustmentAmount":
        if not isinstance(value, PositionSize):
            raise TypeError(
                f"value must be {PositionSize.__module__}.{PositionSize.__name__}"
            )
        raw = _AdjustmentAmount.delta(value)
        return AdjustmentAmount(kind=raw.kind, value=raw.value)

    @staticmethod
    def absolute(value: PositionSize) -> "AdjustmentAmount":
        if not isinstance(value, PositionSize):
            raise TypeError(
                f"value must be {PositionSize.__module__}.{PositionSize.__name__}"
            )
        raw = _AdjustmentAmount.absolute(value)
        return AdjustmentAmount(kind=raw.kind, value=raw.value)

    # @typing.override
    @property
    def kind(self) -> str:
        return _AdjustmentAmount.kind.__get__(self, type(self))

    # @typing.override
    @property
    def value(self) -> PositionSize:
        return _AdjustmentAmount.value.__get__(self, type(self))


@enum.unique
class ParamKind(_enum.StrEnum):
    """Stable identifiers for numeric domain value categories."""

    QUANTITY = "Quantity"
    VOLUME = "Volume"
    PRICE = "Price"
    PNL = "Pnl"
    CASH_FLOW = "CashFlow"
    POSITION_SIZE = "PositionSize"
    FEE = "Fee"
    LEVERAGE = "Leverage"


@enum.unique
class RoundingStrategy(_enum.StrEnum):
    """Named rounding strategies used by the Rust value-type layer."""

    MIDPOINT_NEAREST_EVEN = "MidpointNearestEven"
    MIDPOINT_AWAY_FROM_ZERO = "MidpointAwayFromZero"
    UP = "Up"
    DOWN = "Down"

    @_enum.classproperty
    def DEFAULT(cls) -> "RoundingStrategy":
        """Default rounding strategy used by the Python binding."""
        return cls.MIDPOINT_NEAREST_EVEN

    @_enum.classproperty
    def BANKER(cls) -> "RoundingStrategy":
        """Alias for banker-style midpoint-to-even rounding."""
        return cls.MIDPOINT_NEAREST_EVEN

    @_enum.classproperty
    def CONSERVATIVE_PROFIT(cls) -> "RoundingStrategy":
        """Conservative rounding direction for profit-sensitive flows."""
        return cls.DOWN

    @_enum.classproperty
    def CONSERVATIVE_LOSS(cls) -> "RoundingStrategy":
        """Conservative rounding direction for loss-sensitive flows."""
        return cls.DOWN


AccountId.__doc__ = """
Type-safe account identifier.

Use :meth:`from_u64` when the broker or venue assigns numeric account IDs —
zero cost, zero collision risk.

Use :meth:`from_str` when only a string identifier is available. The string is
hashed with FNV-1a 64-bit, so hash collisions are theoretically possible.
For n distinct account strings the probability of at least one collision is
approximately n² / 2⁶⁵. If that risk is unacceptable, maintain a
collision-free string-to-integer mapping on your side and use
:meth:`from_u64`. See <http://www.isthe.com/chongo/tech/comp/fnv/> for the algorithm
specification.
"""

Asset.__doc__ = """
Validated asset code used across instruments, collateral fields, and settlement keys.

Use this wrapper when the host application wants an explicit asset value object
instead of a plain string while preserving the same validation rules as the
Rust core.
"""

CashFlow.__doc__ = """
Signed cash-flow amount expressed in the settlement currency.

Positive values represent inflow and negative values represent outflow.
Policies and adapters can use this type when they need explicit cash-flow
semantics instead of a raw numeric literal.
"""

Fee.__doc__ = """
Execution fee or rebate expressed in the settlement currency.

Use this value type for venue commissions, maker rebates, and similar
post-trade charges instead of passing untyped numeric values around.
"""

Leverage.__doc__ = """
Per-order leverage multiplier with `0.1` step and range `1..=3000`.

The Python wrapper exposes constructors from integer and float multipliers and
returns the normalized multiplier through ``value``. Use it for explicit
leverage overrides on margin orders and margin-parameter objects.
"""

Pnl.__doc__ = """
Signed profit-and-loss amount expressed in the settlement currency.

Positive values represent realized profit and negative values represent
realized loss. Policies such as kill switches consume this type when tracking
account-level realized outcome.
"""

PositionSize.__doc__ = """
Signed position size in instrument units.

This value type is useful when the host integration needs explicit directional
position semantics instead of a generic numeric payload.
"""

Price.__doc__ = """
Per-unit instrument price.

Use ``Price.calculate_volume(quantity)`` to derive notional volume through the
same exact arithmetic as the Rust core instead of multiplying raw numbers in
Python.
"""

Quantity.__doc__ = """
Unsigned order or fill size in instrument units.

Use ``Quantity.calculate_volume(price)`` to derive notional volume through the
engine's domain arithmetic rather than manual float or decimal math.
"""

Volume.__doc__ = """
Unsigned notional volume in the settlement currency.

This wrapper also exposes helpers that convert notional into signed cash-flow
contributions for inflow/outflow bookkeeping.
"""

Side.BUY.__doc__ = "Buy direction."
Side.SELL.__doc__ = "Sell direction."

PositionSide.LONG.__doc__ = "Long hedge-mode leg."
PositionSide.SHORT.__doc__ = "Short hedge-mode leg."

PositionEffect.OPEN.__doc__ = "Execution opens exposure."
PositionEffect.CLOSE.__doc__ = "Execution closes exposure."
PositionMode.NETTING.__doc__ = "Single net position."
PositionMode.HEDGED.__doc__ = "Separate long and short legs."

AdjustmentAmount.__doc__ = """
Delta or absolute payload wrapper.
"""

PositionMode.__doc__ = """
Netting vs hedged position mode.
"""


__all__ = [
    "AccountId",
    "AdjustmentAmount",
    "Asset",
    "CashFlow",
    "Fee",
    "Leverage",
    "ParamKind",
    "Pnl",
    "PositionEffect",
    "PositionMode",
    "PositionSide",
    "PositionSize",
    "Price",
    "Quantity",
    "RoundingStrategy",
    "Side",
    "Volume",
]
