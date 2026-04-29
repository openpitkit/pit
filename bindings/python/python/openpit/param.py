import enum

from . import _enum
from ._openpit import (
    _LEVERAGE_MAX,
    _LEVERAGE_MIN,
    _LEVERAGE_SCALE,
    _LEVERAGE_STEP,
    _ROUNDING_STRATEGY_BANKER,
    _ROUNDING_STRATEGY_CONSERVATIVE_LOSS,
    _ROUNDING_STRATEGY_CONSERVATIVE_PROFIT,
    _ROUNDING_STRATEGY_DEFAULT,
    AccountId,
    CashFlow,
    Fee,
    Leverage,
    Notional,
    ParamError,
    Pnl,
    PositionSize,
    Price,
    Quantity,
    Volume,
    _validate_asset,
)
from ._openpit import (
    AdjustmentAmount as _AdjustmentAmount,
)
from ._openpit import (
    Trade as _Trade,
)
from ._openpit import (
    TradeAmount as _TradeAmount,
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


@enum.unique
class FillType(_enum.StrEnum):
    """Type of fill event reported by the venue."""

    TRADE = "TRADE"
    LIQUIDATION = "LIQUIDATION"
    AUTO_DELEVERAGE = "AUTO_DELEVERAGE"
    SETTLEMENT = "SETTLEMENT"
    FUNDING = "FUNDING"

    # @typing.override
    @classmethod
    def _missing_(cls, value: object) -> "FillType | None":
        raise ValueError(
            "expected 'TRADE', 'LIQUIDATION', 'AUTO_DELEVERAGE', "
            "'SETTLEMENT', or 'FUNDING'",
        )


class AssetError(ValueError):
    """Error type for asset validation failures."""


class AccountIdError(ValueError):
    """Error type for account identifier validation failures."""


class Asset(str):
    """Validated asset identifier string."""

    # @typing.override
    def __new__(cls, value: str) -> "Asset":
        if not isinstance(value, str):
            raise TypeError("asset must be str")
        try:
            _validate_asset(value)
        except ValueError as error:
            raise AssetError(str(error)) from None
        return str.__new__(cls, value)


class AdjustmentAmount(_AdjustmentAmount):
    """Delta-or-absolute adjustment payload.

    Use factory methods :meth:`delta` and :meth:`absolute` to construct;
    inspect the variant with :attr:`is_delta` / :attr:`is_absolute` and
    extract the inner value with :attr:`as_delta` / :attr:`as_absolute`.
    """

    # @typing.override
    def __new__(cls, *args: object, **kwargs: object) -> "AdjustmentAmount":
        return _AdjustmentAmount.__new__(cls, *args, **kwargs)

    @staticmethod
    def delta(value: "PositionSize") -> "AdjustmentAmount":
        """Create a delta-type adjustment amount."""
        base = _AdjustmentAmount.delta(value)
        return AdjustmentAmount.__new__(AdjustmentAmount, base)

    @staticmethod
    def absolute(value: "PositionSize") -> "AdjustmentAmount":
        """Create an absolute-type adjustment amount."""
        base = _AdjustmentAmount.absolute(value)
        return AdjustmentAmount.__new__(AdjustmentAmount, base)

    @property
    def is_delta(self) -> bool:
        """True when the adjustment is a signed delta."""
        return _AdjustmentAmount.is_delta.__get__(self, type(self))

    @property
    def is_absolute(self) -> bool:
        """True when the adjustment sets an absolute value."""
        return _AdjustmentAmount.is_absolute.__get__(self, type(self))

    @property
    def as_delta(self) -> "PositionSize | None":
        """Inner position size when delta, otherwise None."""
        return _AdjustmentAmount.as_delta.__get__(self, type(self))

    @property
    def as_absolute(self) -> "PositionSize | None":
        """Inner position size when absolute, otherwise None."""
        return _AdjustmentAmount.as_absolute.__get__(self, type(self))

    # @typing.override
    def __repr__(self) -> str:
        return _AdjustmentAmount.__repr__(self)


class Trade(_Trade):
    """Last executed trade details."""

    # @typing.override
    def __new__(cls, *args: object, **kwargs: object) -> "Trade":
        return _Trade.__new__(cls, *args, **kwargs)

    # @typing.override
    def __init__(self, *, price: Price, quantity: Quantity) -> None:
        if not isinstance(price, Price):
            raise TypeError(f"price must be {Price.__module__}.{Price.__name__}")
        if not isinstance(quantity, Quantity):
            raise TypeError(
                f"quantity must be {Quantity.__module__}.{Quantity.__name__}",
            )

    # @typing.override
    @property
    def price(self) -> Price:
        return _Trade.price.__get__(self, type(self))

    # @typing.override
    @property
    def quantity(self) -> Quantity:
        return _Trade.quantity.__get__(self, type(self))

    # @typing.override
    def __repr__(self) -> str:
        return f"Trade(price={self.price!r}, quantity={self.quantity!r})"


class TradeAmount(_TradeAmount):
    """Quantity- or volume-based trade amount wrapper.

    Use factory methods :meth:`quantity` and :meth:`volume` to construct.
    Factory arguments accept either value objects (:class:`Quantity` /
    :class:`Volume`) or primitive numeric/string literals.

    inspect the variant with :attr:`is_quantity` / :attr:`is_volume` and
    extract the inner value with :attr:`as_quantity` / :attr:`as_volume`.
    """

    # @typing.override
    def __new__(cls, *args: object, **kwargs: object) -> "TradeAmount":
        return _TradeAmount.__new__(cls, *args, **kwargs)

    @staticmethod
    def quantity(value: "Quantity | str | int | float") -> "TradeAmount":
        """Create a quantity-based trade amount."""
        base = _TradeAmount.quantity(value)
        return TradeAmount.__new__(TradeAmount, base)

    @staticmethod
    def volume(value: "Volume | str | int | float") -> "TradeAmount":
        """Create a volume-based trade amount."""
        base = _TradeAmount.volume(value)
        return TradeAmount.__new__(TradeAmount, base)

    @property
    def is_quantity(self) -> bool:
        """True when the amount is expressed as quantity."""
        return _TradeAmount.is_quantity.__get__(self, type(self))

    @property
    def is_volume(self) -> bool:
        """True when the amount is expressed as volume."""
        return _TradeAmount.is_volume.__get__(self, type(self))

    @property
    def as_quantity(self) -> "Quantity | None":
        """Inner quantity, or None when volume-based."""
        return _TradeAmount.as_quantity.__get__(self, type(self))

    @property
    def as_volume(self) -> "Volume | None":
        """Inner volume, or None when quantity-based."""
        return _TradeAmount.as_volume.__get__(self, type(self))

    # @typing.override
    def __repr__(self) -> str:
        return _TradeAmount.__repr__(self)


@enum.unique
class Kind(_enum.StrEnum):
    """Stable identifiers for numeric domain value categories."""

    QUANTITY = "Quantity"
    VOLUME = "Volume"
    NOTIONAL = "Notional"
    PRICE = "Price"
    PNL = "Pnl"
    CASH_FLOW = "CashFlow"
    POSITION_SIZE = "PositionSize"
    FEE = "Fee"
    LEVERAGE = "Leverage"


@enum.unique
class RoundingStrategy(_enum.StrEnum):
    """Named rounding strategies used by the SDK value-type layer."""

    MIDPOINT_NEAREST_EVEN = "MidpointNearestEven"
    MIDPOINT_AWAY_FROM_ZERO = "MidpointAwayFromZero"
    UP = "Up"
    DOWN = "Down"

    @_enum.classproperty
    def DEFAULT(cls) -> "RoundingStrategy":
        """Default rounding strategy used by the Python binding."""
        return cls(_ROUNDING_STRATEGY_DEFAULT)

    @_enum.classproperty
    def BANKER(cls) -> "RoundingStrategy":
        """Alias for banker-style midpoint-to-even rounding."""
        return cls(_ROUNDING_STRATEGY_BANKER)

    @_enum.classproperty
    def CONSERVATIVE_PROFIT(cls) -> "RoundingStrategy":
        """Conservative rounding direction for profit-sensitive flows."""
        return cls(_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT)

    @_enum.classproperty
    def CONSERVATIVE_LOSS(cls) -> "RoundingStrategy":
        """Conservative rounding direction for loss-sensitive flows."""
        return cls(_ROUNDING_STRATEGY_CONSERVATIVE_LOSS)


_native_account_id_from_str = AccountId.from_str


def _account_id_from_str(value: str) -> AccountId:
    try:
        return _native_account_id_from_str(value)
    except ValueError as error:
        if str(error) == "account id string must not be empty":
            raise AccountIdError("account id string must not be empty") from None
        raise


AccountId.from_str = staticmethod(_account_id_from_str)


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

WARNING:
Use exactly one source model per runtime:
- either only ``AccountId.from_u64(...)``,
- or only ``AccountId.from_str(...)``.
Do not mix both in one runtime state. A hashed string-derived ID can equal a
direct numeric ID, and then two distinct accounts become one logical key.
"""

CashFlow.__doc__ = """
Signed cash-flow amount expressed in the settlement currency.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``CashFlow`` operands of the same type.
Cross-type arithmetic (for example ``CashFlow + Fee``) returns ``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use :meth:`from_pnl` and :meth:`from_fee` for explicit domain conversions.
Positive values represent inflow and negative values represent outflow.
"""

Fee.__doc__ = """
Execution fee or rebate expressed in the settlement currency.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``Fee`` operands of the same type.
Cross-type arithmetic (for example ``Fee + Pnl``) returns ``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use :meth:`to_pnl` and :meth:`to_position_size` for domain conversions.
"""

Leverage.__doc__ = """
Per-order leverage multiplier with `0.1` step and range `1..=3000`.

Use ``Leverage(10)`` or ``Leverage(10.1)`` for direct multiplier
construction. ``from_int`` and ``from_float`` remain available when explicit
constructor intent is preferred.

The normalized multiplier is exposed through ``value``. Use this type for
explicit leverage overrides on margin orders and margin-parameter objects.
"""
Leverage.SCALE = _LEVERAGE_SCALE
Leverage.MIN = _LEVERAGE_MIN
Leverage.MAX = _LEVERAGE_MAX
Leverage.STEP = _LEVERAGE_STEP

Pnl.__doc__ = """
Signed profit-and-loss amount expressed in the settlement currency.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``Pnl`` operands of the same type.
Cross-type arithmetic (for example ``Pnl + Fee``) returns ``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use :meth:`to_cash_flow` and :meth:`to_position_size` for domain conversions.
Positive values represent realized profit and negative values represent
realized loss.
"""

PositionSize.__doc__ = """
Signed position size in instrument units.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``PositionSize`` operands of the same type.
Cross-type arithmetic (for example ``PositionSize + Quantity``) returns
``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use :meth:`from_quantity_and_side`, :meth:`to_open_quantity`,
:meth:`to_close_quantity`, and :meth:`checked_add_quantity` for directional
position workflows.
"""

Price.__doc__ = """
Per-unit instrument price.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``Price`` operands of the same type.
Cross-type arithmetic (for example ``Price + Quantity``) returns ``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use ``Price.calculate_volume(quantity)`` to derive notional volume through the
same exact arithmetic as the SDK core instead of multiplying raw numbers.
"""

Quantity.__doc__ = """
Unsigned order or fill size in instrument units.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``Quantity`` operands of the same type.
Cross-type arithmetic (for example ``Quantity + Price``) returns ``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use ``Quantity.calculate_volume(price)`` to derive notional volume through the
engine's domain arithmetic rather than manual number math.
"""

Notional.__doc__ = """
Monetary position exposure in the settlement currency.

Represents the absolute face value of a position: ``|price| × quantity``.
Always non-negative. Used to calculate required margin and evaluate risk.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``Notional`` operands of the same type.
Cross-type arithmetic (for example ``Notional + Volume``) returns ``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use :meth:`from_price_quantity` to compute notional from a trade;
use :meth:`calculate_margin_required` to derive the required margin.
"""

Volume.__doc__ = """
Unsigned notional volume in the settlement currency.

Behaves like ``decimal.Decimal`` with domain-type safety: arithmetic accepts
only ``Volume`` operands of the same type.
Cross-type arithmetic (for example ``Volume + Quantity``) returns ``TypeError``.

Constructor accepts ``Decimal``, ``str``, ``int``, or ``float``.

WARNING:
``float`` is inherently imprecise. The same numeric literal passed as
``float`` can differ from its ``str``/``Decimal`` representation by one ULP
and may produce platform-dependent results. For external monetary inputs,
prefer ``str`` or ``Decimal`` values.

Use ``.decimal`` to access the underlying ``decimal.Decimal`` and
``.to_json_value()`` for canonical JSON serialization.

Use :meth:`calculate_quantity`, :meth:`to_cash_flow_inflow`, and
:meth:`to_cash_flow_outflow` for domain conversions.
"""

Side.BUY.__doc__ = "Buy direction."
Side.SELL.__doc__ = "Sell direction."

PositionSide.LONG.__doc__ = "Long hedge-mode leg."
PositionSide.SHORT.__doc__ = "Short hedge-mode leg."

PositionEffect.OPEN.__doc__ = "Execution opens exposure."
PositionEffect.CLOSE.__doc__ = "Execution closes exposure."
PositionMode.NETTING.__doc__ = "Single net position."
PositionMode.HEDGED.__doc__ = "Separate long and short legs."
FillType.TRADE.__doc__ = "Normal trade execution."
FillType.LIQUIDATION.__doc__ = "Forced liquidation by the venue."
FillType.AUTO_DELEVERAGE.__doc__ = "Auto-deleveraging event."
FillType.SETTLEMENT.__doc__ = "Settlement at expiry or delivery."
FillType.FUNDING.__doc__ = "Funding payment."
AdjustmentAmount.__doc__ = """
Delta or absolute payload wrapper.
"""

PositionMode.__doc__ = """
Netting vs hedged position mode.
"""


__all__ = [
    "AccountId",
    "AccountIdError",
    "AdjustmentAmount",
    "Asset",
    "AssetError",
    "CashFlow",
    "Fee",
    "FillType",
    "Kind",
    "Leverage",
    "Notional",
    "ParamError",
    "Pnl",
    "PositionEffect",
    "PositionMode",
    "PositionSide",
    "PositionSize",
    "Price",
    "Quantity",
    "RoundingStrategy",
    "Side",
    "Trade",
    "TradeAmount",
    "Volume",
]
