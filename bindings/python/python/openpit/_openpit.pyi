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
# Please see https://openpit.dev and the OWNERS file for details.

"""Typing stubs for the native ``openpit._openpit`` extension module."""

from __future__ import annotations

import datetime
import decimal
import typing

from . import param, pretrade
from .pretrade import Policy

class _AccountInfo(typing.Protocol):
    """Any object that exposes an ``account_group`` property.

    Engine contexts (``Context``, ``PostTradeContext``,
    ``AccountAdjustmentContext``) satisfy this protocol automatically via their
    ``account_group`` property.  A minimal stub for testing::

        class _Stub:
            @property
            def account_group(self) -> AccountGroupId | None:
                return None
    """

    @property
    def account_group(self) -> AccountGroupId | None: ...

_ROUNDING_STRATEGY_DEFAULT: str
_ROUNDING_STRATEGY_BANKER: str
_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT: str
_ROUNDING_STRATEGY_CONSERVATIVE_LOSS: str
_LEVERAGE_SCALE: int
_LEVERAGE_MIN: int
_LEVERAGE_MAX: int
_LEVERAGE_STEP: float

def _validate_asset(value: str) -> None: ...

class RejectError(Exception):
    """Base exception type raised by the library."""

class ParamError(ValueError):
    """Numeric parameter validation and arithmetic error."""

class MarketDataError(Exception):
    """Base exception for market-data read failures."""

class UnknownInstrument(MarketDataError):
    """Requested market-data instrument is not registered."""

class QuoteUnavailable(MarketDataError):
    """No usable quote is available."""

class QuoteExpired(MarketDataError):
    """The selected quote aged past TTL.

    The stale quote selected before the TTL check is available as ``quote``.
    """

    quote: Quote

class AlreadyRegistered(Exception):
    """Instrument is already registered."""

    instrument: Instrument

class RegistrationError(Exception):
    """Explicit market-data registration conflicts with existing state."""

    instrument: Instrument | None
    instrument_id: InstrumentId | None

class UnknownInstrumentId(Exception):
    """Instrument id is unknown to the market-data service."""

    instrument_id: InstrumentId

class AccountGroupRegistrationError(Exception):
    """Account-group registration conflicts with existing state."""

class AccountBlockError(Exception):
    """Admin block/unblock operation failed."""

class ConfigureErrorKind:
    """Classifies why a runtime policy reconfiguration failed.

    Integer values match the C ``OpenPitConfigureErrorKind`` and the Go
    ``ConfigureErrorKind`` so all bindings agree on the discriminants.
    """

    UNKNOWN: typing.ClassVar[ConfigureErrorKind]
    """No registered policy carries the requested name."""
    TYPE_MISMATCH: typing.ClassVar[ConfigureErrorKind]
    """The named policy exists but its settings type differs from the target."""
    VALIDATION: typing.ClassVar[ConfigureErrorKind]
    """The update was rejected; the prior value still applies."""
    NESTED_CONFIGURATION: typing.ClassVar[ConfigureErrorKind]
    """Configuration was re-entered on the same thread."""

class PolicyConfigureError(Exception):
    """Runtime policy reconfiguration failed.

    Raised by ``Configurator`` methods when the policy name is unknown,
    has the wrong type, the new settings fail validation, or configuration is
    re-entered on the same thread.
    """

    kind: ConfigureErrorKind
    """Classifies the configuration failure."""

class InstrumentId:
    """Market-data instrument identifier."""

    def __init__(self, value: int) -> None: ...
    @property
    def value(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

class QuoteTtl:
    """Quote lifetime policy."""

    @staticmethod
    def infinite() -> QuoteTtl: ...
    @staticmethod
    def within(duration: datetime.timedelta) -> QuoteTtl: ...

class Quote:
    """Market snapshot with optional mark, bid, and ask prices."""

    def __init__(
        self,
        *,
        mark: Price | decimal.Decimal | str | int | float | None = None,
        bid: Price | decimal.Decimal | str | int | float | None = None,
        ask: Price | decimal.Decimal | str | int | float | None = None,
    ) -> None: ...
    @property
    def mark(self) -> Price | None: ...
    @property
    def bid(self) -> Price | None: ...
    @property
    def ask(self) -> Price | None: ...
    def __eq__(self, other: object) -> bool: ...

class QuoteResolution:
    """Controls which quote buckets a read may fall through to."""

    ACCOUNT_ONLY: typing.ClassVar[QuoteResolution]
    """Consult only the per-account bucket."""
    ACCOUNT_THEN_GROUP: typing.ClassVar[QuoteResolution]
    """Consult the per-account bucket, then the account's group bucket."""
    ACCOUNT_THEN_GROUP_THEN_DEFAULT: typing.ClassVar[QuoteResolution]
    """Consult per-account, then group, then the default ("everyone-else") bucket."""

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class MarketDataService:
    """Live market-data service."""

    def register(self, instrument: Instrument) -> InstrumentId: ...
    def register_with_ttl(
        self,
        instrument: Instrument,
        ttl: QuoteTtl,
    ) -> InstrumentId: ...
    def register_with_id(
        self,
        instrument: Instrument,
        id: InstrumentId,
    ) -> InstrumentId: ...
    def register_with_id_and_ttl(
        self,
        instrument: Instrument,
        id: InstrumentId,
        ttl: QuoteTtl,
    ) -> InstrumentId: ...
    # ── TTL setters / clearers ────────────────────────────────────────────────
    def set_account_ttl(self, account_id: AccountId, ttl: QuoteTtl) -> None: ...
    def clear_account_ttl(self, account_id: AccountId) -> None: ...
    def set_account_group_ttl(
        self, account_group_id: AccountGroupId, ttl: QuoteTtl
    ) -> None:
        """Set the TTL override for the given account group.

        Passing ``AccountGroupId.DEFAULT`` targets the service-level
        default-group TTL (the "everyone-else" bucket).
        """

    def clear_account_group_ttl(self, account_group_id: AccountGroupId) -> None:
        """Clear the TTL override for the given account group.

        Passing ``AccountGroupId.DEFAULT`` targets the service-level
        default-group TTL (the "everyone-else" bucket).
        """

    def set_instrument_ttl(
        self, instrument_id: InstrumentId, ttl: QuoteTtl
    ) -> None: ...
    def clear_instrument_ttl(self, instrument_id: InstrumentId) -> None: ...
    def set_instrument_account_ttl(
        self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        ttl: QuoteTtl,
    ) -> None: ...
    def clear_instrument_account_ttl(
        self,
        instrument_id: InstrumentId,
        account_id: AccountId,
    ) -> None: ...
    def set_instrument_account_group_ttl(
        self,
        instrument_id: InstrumentId,
        account_group_id: AccountGroupId,
        ttl: QuoteTtl,
    ) -> None:
        """Set the per-instrument TTL override for the given account group.

        Passing ``AccountGroupId.DEFAULT`` targets the instrument-level
        default-group TTL (the "everyone-else" bucket for this instrument).
        """

    def clear_instrument_account_group_ttl(
        self,
        instrument_id: InstrumentId,
        account_group_id: AccountGroupId,
    ) -> None:
        """Clear the per-instrument TTL override for the given account group.

        Passing ``AccountGroupId.DEFAULT`` targets the instrument-level
        default-group TTL (the "everyone-else" bucket for this instrument).
        """
    # ── Clear ─────────────────────────────────────────────────────────────────
    def clear(self, instrument_id: InstrumentId) -> None: ...
    # ── Push (default bucket) ─────────────────────────────────────────────────
    def push(self, instrument_id: InstrumentId, quote: Quote) -> None: ...
    def push_patch(self, instrument_id: InstrumentId, quote: Quote) -> None: ...
    def push_by_instrument(
        self, instrument: Instrument, quote: Quote
    ) -> InstrumentId: ...
    def push_by_instrument_patch(
        self,
        instrument: Instrument,
        quote: Quote,
    ) -> InstrumentId: ...
    # ── Targeted fan-out push ─────────────────────────────────────────────────
    def push_for(
        self,
        instrument_id: InstrumentId,
        quote: Quote,
        account_ids: typing.Iterable[AccountId],
        account_group_ids: typing.Iterable[AccountGroupId],
    ) -> None:
        """Push a full quote snapshot to specific accounts and/or groups.

        To target the default ("everyone-else") bucket, include
        ``AccountGroupId.DEFAULT`` in ``account_group_ids``.
        """

    def push_for_patch(
        self,
        instrument_id: InstrumentId,
        quote: Quote,
        account_ids: typing.Iterable[AccountId],
        account_group_ids: typing.Iterable[AccountGroupId],
    ) -> None:
        """Push a partial quote patch to specific accounts and/or groups.

        To target the default ("everyone-else") bucket, include
        ``AccountGroupId.DEFAULT`` in ``account_group_ids``.
        """
    # ── Get ───────────────────────────────────────────────────────────────────
    def get(
        self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: _AccountInfo,
        resolution: QuoteResolution,
    ) -> Quote:
        """Return the best available quote for the instrument.

        Raises:
            QuoteExpired: when the quote exists but its TTL has elapsed;
                the stale quote is available as ``exception.quote``.
        """

    def get_optional(
        self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: _AccountInfo,
        resolution: QuoteResolution,
    ) -> Quote | None: ...
    # ── Resolve ───────────────────────────────────────────────────────────────
    def resolve(self, instrument: Instrument) -> InstrumentId | None: ...

class MarketDataBuilder:
    """Builder for a market-data service.

    Obtain via ``SyncedEngineBuilder.market_data(default_ttl)`` or
    ``ReadyEngineBuilder.market_data(default_ttl)``. Do not construct directly.
    """

    def no_sync(self) -> MarketDataBuilder:
        """Downgrade to no-sync mode (no-op locks, zero overhead).

        Use only when the market-data service is written from a single thread
        and never read concurrently. Returns ``self`` for chaining.
        """
        ...

    def full_sync(self) -> MarketDataBuilder:
        """Upgrade to Full synchronization (real locks, safe for a concurrent feed).

        No-op when already Full. Returns ``self`` for chaining.
        """
        ...

    def build(self) -> MarketDataService: ...

class Reject:
    """Business reject returned by pre-trade checks."""

    @property
    def code(self) -> str:
        """Reject code string."""

    @property
    def reason(self) -> str:
        """Human-readable reason."""

    @property
    def details(self) -> str:
        """Additional reject details."""

    @property
    def policy(self) -> str:
        """Policy name that produced the reject."""

    @property
    def scope(self) -> str:
        """Reject scope (``order`` or ``account``)."""

    @property
    def user_data(self) -> int:
        """Opaque caller-defined integer token. ``0`` means "not set". The SDK never
        inspects, dereferences, or frees it."""

class Leverage:
    """Per-order leverage multiplier."""

    SCALE: typing.ClassVar[int]
    MIN: typing.ClassVar[int]
    MAX: typing.ClassVar[int]
    STEP: typing.ClassVar[float]

    def __init__(self, value: int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``int`` or ``from_int``."""

    @staticmethod
    def from_int(value: int) -> Leverage: ...
    @staticmethod
    def from_float(value: float) -> Leverage:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``from_int`` with an integer value."""

    @property
    def value(self) -> float: ...
    def calculate_margin_required(
        self,
        notional: Notional | decimal.Decimal | str | int | float,
    ) -> Notional: ...

class AccountId:
    """Type-safe account identifier."""

    @staticmethod
    def from_int(value: int) -> AccountId: ...
    @staticmethod
    def from_string(value: str) -> AccountId: ...
    @property
    def value(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

class AccountGroupId:
    """Type-safe account-group identifier."""

    DEFAULT: typing.ClassVar[AccountGroupId]
    """The reserved default group an account belongs to until assigned."""

    @staticmethod
    def from_int(value: int) -> AccountGroupId:
        """Raises ``ValueError`` when ``value`` is the reserved default group (0).
        Raises ``OverflowError`` for values outside ``1..=4294967295``."""

    @staticmethod
    def from_string(value: str) -> AccountGroupId: ...
    @property
    def value(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

class Quantity:
    """Instrument quantity value type."""

    ZERO: typing.ClassVar[Quantity]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Quantity: ...
    @staticmethod
    def from_string(value: str) -> Quantity: ...
    @staticmethod
    def from_int(value: int) -> Quantity: ...
    @staticmethod
    def from_float(value: float) -> Quantity:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> Quantity: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Quantity:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Quantity: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def calculate_volume(self, price: Price) -> Volume: ...
    def to_position_size(self) -> PositionSize: ...
    def __add__(self, other: Quantity) -> Quantity: ...
    def __sub__(self, other: Quantity) -> Quantity: ...
    def __mul__(self, other: int | float) -> Quantity: ...
    def __rmul__(self, other: int | float) -> Quantity: ...
    def __truediv__(self, other: int | float) -> Quantity: ...
    def __mod__(self, other: int | float) -> Quantity: ...

class Price:
    """Instrument price value type."""

    ZERO: typing.ClassVar[Price]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Price: ...
    @staticmethod
    def from_string(value: str) -> Price: ...
    @staticmethod
    def from_int(value: int) -> Price: ...
    @staticmethod
    def from_float(value: float) -> Price:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> Price: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Price:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Price: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def calculate_volume(self, quantity: Quantity) -> Volume: ...
    def calculate_position_size(self, quantity: Quantity) -> PositionSize: ...
    def __add__(self, other: Price) -> Price: ...
    def __sub__(self, other: Price) -> Price: ...
    def __neg__(self) -> Price: ...
    def __mul__(self, other: int | float) -> Price: ...
    def __rmul__(self, other: int | float) -> Price: ...
    def __truediv__(self, other: int | float) -> Price: ...
    def __mod__(self, other: int | float) -> Price: ...

class Pnl:
    """Profit and loss value type."""

    ZERO: typing.ClassVar[Pnl]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Pnl: ...
    @staticmethod
    def from_string(value: str) -> Pnl: ...
    @staticmethod
    def from_int(value: int) -> Pnl: ...
    @staticmethod
    def from_float(value: float) -> Pnl:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> Pnl: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Pnl:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Pnl: ...
    @staticmethod
    def from_fee(fee: Fee) -> Pnl: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_cash_flow(self) -> CashFlow: ...
    def to_position_size(self) -> PositionSize: ...
    def __add__(self, other: Pnl) -> Pnl: ...
    def __sub__(self, other: Pnl) -> Pnl: ...
    def __neg__(self) -> Pnl: ...
    def __mul__(self, other: int | float) -> Pnl: ...
    def __rmul__(self, other: int | float) -> Pnl: ...
    def __truediv__(self, other: int | float) -> Pnl: ...
    def __mod__(self, other: int | float) -> Pnl: ...

class Fee:
    """Fee value type."""

    ZERO: typing.ClassVar[Fee]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Fee: ...
    @staticmethod
    def from_string(value: str) -> Fee: ...
    @staticmethod
    def from_int(value: int) -> Fee: ...
    @staticmethod
    def from_float(value: float) -> Fee:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> Fee: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Fee:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Fee: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_pnl(self) -> Pnl: ...
    def to_position_size(self) -> PositionSize: ...
    def to_cash_flow(self) -> CashFlow: ...
    def __add__(self, other: Fee) -> Fee: ...
    def __sub__(self, other: Fee) -> Fee: ...
    def __neg__(self) -> Fee: ...
    def __mul__(self, other: int | float) -> Fee: ...
    def __rmul__(self, other: int | float) -> Fee: ...
    def __truediv__(self, other: int | float) -> Fee: ...
    def __mod__(self, other: int | float) -> Fee: ...

class Volume:
    """Notional volume value type."""

    ZERO: typing.ClassVar[Volume]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Volume: ...
    @staticmethod
    def from_string(value: str) -> Volume: ...
    @staticmethod
    def from_int(value: int) -> Volume: ...
    @staticmethod
    def from_float(value: float) -> Volume:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> Volume: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Volume:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Volume: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_cash_flow_inflow(self) -> CashFlow: ...
    def to_cash_flow_outflow(self) -> CashFlow: ...
    def calculate_quantity(self, price: Price) -> Quantity: ...
    def to_position_size(self) -> PositionSize: ...
    def __add__(self, other: Volume) -> Volume: ...
    def __sub__(self, other: Volume) -> Volume: ...
    def __mul__(self, other: int | float) -> Volume: ...
    def __rmul__(self, other: int | float) -> Volume: ...
    def __truediv__(self, other: int | float) -> Volume: ...
    def __mod__(self, other: int | float) -> Volume: ...

class Notional:
    """Monetary position exposure value type.

    Represents the absolute monetary value of a position in the settlement
    currency: ``|price| × quantity``. Always non-negative. Used for margin
    and risk calculation.
    """

    ZERO: typing.ClassVar[Notional]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> Notional: ...
    @staticmethod
    def from_string(value: str) -> Notional: ...
    @staticmethod
    def from_int(value: int) -> Notional: ...
    @staticmethod
    def from_float(value: float) -> Notional:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> Notional: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> Notional:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> Notional: ...
    @staticmethod
    def from_volume(volume: Volume) -> Notional: ...
    @staticmethod
    def from_price_quantity(price: Price, quantity: Quantity) -> Notional: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_volume(self) -> Volume: ...
    def calculate_margin_required(
        self,
        leverage: Leverage | int | float,
    ) -> Notional: ...
    def __add__(self, other: Notional) -> Notional: ...
    def __sub__(self, other: Notional) -> Notional: ...
    def __mul__(self, other: int | float) -> Notional: ...
    def __rmul__(self, other: int | float) -> Notional: ...
    def __truediv__(self, other: int | float) -> Notional: ...
    def __mod__(self, other: int | float) -> Notional: ...

class CashFlow:
    """Cash-flow contribution value type."""

    ZERO: typing.ClassVar[CashFlow]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> CashFlow: ...
    @staticmethod
    def from_string(value: str) -> CashFlow: ...
    @staticmethod
    def from_int(value: int) -> CashFlow: ...
    @staticmethod
    def from_float(value: float) -> CashFlow:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> CashFlow: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> CashFlow:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> CashFlow: ...
    @staticmethod
    def from_pnl(pnl: Pnl) -> CashFlow: ...
    @staticmethod
    def from_fee(fee: Fee) -> CashFlow: ...
    @staticmethod
    def from_volume_inflow(volume: Volume) -> CashFlow: ...
    @staticmethod
    def from_volume_outflow(volume: Volume) -> CashFlow: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def __add__(self, other: CashFlow) -> CashFlow: ...
    def __sub__(self, other: CashFlow) -> CashFlow: ...
    def __neg__(self) -> CashFlow: ...
    def __mul__(self, other: int | float) -> CashFlow: ...
    def __rmul__(self, other: int | float) -> CashFlow: ...
    def __truediv__(self, other: int | float) -> CashFlow: ...
    def __mod__(self, other: int | float) -> CashFlow: ...

class PositionSize:
    """Signed position-size value type."""

    ZERO: typing.ClassVar[PositionSize]

    def __init__(self, value: decimal.Decimal | str | int | float) -> None:
        """WARNING: passing ``float`` is imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal(value: decimal.Decimal) -> PositionSize: ...
    @staticmethod
    def from_string(value: str) -> PositionSize: ...
    @staticmethod
    def from_int(value: int) -> PositionSize: ...
    @staticmethod
    def from_float(value: float) -> PositionSize:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_string_rounded(value: str, scale: int, strategy: str) -> PositionSize: ...
    @staticmethod
    def from_float_rounded(value: float, scale: int, strategy: str) -> PositionSize:
        """WARNING: float inputs are imprecise and may yield inconsistent
        results across platforms; prefer ``str`` or ``decimal.Decimal``."""

    @staticmethod
    def from_decimal_rounded(
        value: decimal.Decimal,
        scale: int,
        strategy: str,
    ) -> PositionSize: ...
    @staticmethod
    def from_quantity_and_side(quantity: Quantity, side: str) -> PositionSize: ...
    @staticmethod
    def from_pnl(pnl: Pnl) -> PositionSize: ...
    @staticmethod
    def from_fee(fee: Fee) -> PositionSize: ...
    @property
    def decimal(self) -> decimal.Decimal: ...
    def to_json_value(self) -> str: ...
    def to_open_quantity(self) -> tuple[Quantity, str]: ...
    def to_close_quantity(self) -> tuple[Quantity, str | None]: ...
    def checked_add_quantity(self, qty: Quantity, side: str) -> PositionSize: ...
    def __add__(self, other: PositionSize) -> PositionSize: ...
    def __sub__(self, other: PositionSize) -> PositionSize: ...
    def __neg__(self) -> PositionSize: ...
    def __mul__(self, other: int | float) -> PositionSize: ...
    def __rmul__(self, other: int | float) -> PositionSize: ...
    def __truediv__(self, other: int | float) -> PositionSize: ...
    def __mod__(self, other: int | float) -> PositionSize: ...

class Instrument:
    """Trading instrument with underlying and settlement assets."""

    def __init__(
        self,
        underlying_asset: param.Asset,
        settlement_asset: param.Asset,
    ) -> None:
        """Create an instrument."""
        _ = (underlying_asset, settlement_asset)

    @property
    def underlying_asset(self) -> param.Asset:
        """Underlying asset symbol string."""

    @property
    def settlement_asset(self) -> param.Asset:
        """Settlement asset symbol string."""

class OrderOperation:
    """Main order parameters group."""

    def __init__(
        self,
        *,
        underlying_asset: param.Asset | None = None,
        settlement_asset: param.Asset | None = None,
        account_id: param.AccountId | None = None,
        side: param.Side | None = None,
        trade_amount: TradeAmount | None = None,
        price: param.Price | None = None,
    ) -> None:
        """Create an order operation group."""
        _ = (underlying_asset, settlement_asset, account_id, side, trade_amount, price)

    @property
    def underlying_asset(self) -> param.Asset | None:
        """Underlying asset symbol string."""

    @underlying_asset.setter
    def underlying_asset(self, value: param.Asset | None) -> None: ...
    @property
    def settlement_asset(self) -> param.Asset | None:
        """Settlement asset symbol string."""

    @settlement_asset.setter
    def settlement_asset(self, value: param.Asset | None) -> None: ...
    @property
    def account_id(self) -> param.AccountId | None:
        """Account identifier."""

    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def side(self) -> param.Side | None:
        """Order side."""

    @side.setter
    def side(self, value: param.Side | None) -> None: ...
    @property
    def trade_amount(self) -> TradeAmount | None: ...
    @trade_amount.setter
    def trade_amount(self, value: TradeAmount | None) -> None: ...
    @property
    def price(self) -> param.Price | None:
        """Order price."""

    @price.setter
    def price(self, value: param.Price | None) -> None: ...

class OrderPosition:
    """Position-management parameters group."""

    def __init__(
        self,
        *,
        position_side: param.PositionSide | None = None,
        reduce_only: bool = False,
        close_position: bool = False,
    ) -> None:
        """Create an order position group."""
        _ = (position_side, reduce_only, close_position)

    @property
    def position_side(self) -> param.PositionSide | None:
        """Position side."""

    @position_side.setter
    def position_side(self, value: param.PositionSide | None) -> None: ...
    @property
    def reduce_only(self) -> bool:
        """Reduce-only flag."""

    @reduce_only.setter
    def reduce_only(self, value: bool) -> None: ...
    @property
    def close_position(self) -> bool:
        """Close-position flag."""

    @close_position.setter
    def close_position(self, value: bool) -> None: ...

class OrderMargin:
    """Margin-trading parameters group."""

    def __init__(
        self,
        *,
        leverage: param.Leverage | int | float | None = None,
        collateral_asset: param.Asset | None = None,
        auto_borrow: bool = False,
    ) -> None:
        """Create an order margin group."""
        _ = (leverage, collateral_asset, auto_borrow)

    @property
    def leverage(self) -> param.Leverage | None:
        """Optional leverage override."""

    @leverage.setter
    def leverage(self, value: param.Leverage | int | float | None) -> None: ...
    @property
    def collateral_asset(self) -> param.Asset | None:
        """Collateral asset string."""

    @collateral_asset.setter
    def collateral_asset(self, value: param.Asset | None) -> None: ...
    @property
    def auto_borrow(self) -> bool:
        """Auto-borrow flag."""

    @auto_borrow.setter
    def auto_borrow(self, value: bool) -> None: ...

class Order:
    """Extensible order model accepted by ``Engine.start_pre_trade``."""

    def __init__(
        self,
        *,
        operation: OrderOperation | None = None,
        position: OrderPosition | None = None,
        margin: OrderMargin | None = None,
    ) -> None:
        """Create an order with optional groups."""
        _ = (operation, position, margin)

    @property
    def operation(self) -> OrderOperation | None:
        """Main order parameters group."""

    @operation.setter
    def operation(self, value: OrderOperation | None) -> None: ...
    @property
    def position(self) -> OrderPosition | None:
        """Position-management parameters group."""

    @position.setter
    def position(self, value: OrderPosition | None) -> None: ...
    @property
    def margin(self) -> OrderMargin | None:
        """Margin-trading parameters group."""

    @margin.setter
    def margin(self, value: OrderMargin | None) -> None: ...

class ExecutionReportOperation:
    """Execution-report instrument and side group."""

    def __init__(
        self,
        *,
        underlying_asset: param.Asset | None = None,
        settlement_asset: param.Asset | None = None,
        account_id: param.AccountId | None = None,
        side: param.Side | None = None,
    ) -> None:
        """Create an execution report operation group."""
        _ = (underlying_asset, settlement_asset, account_id, side)

    @property
    def underlying_asset(self) -> param.Asset | None:
        """Underlying asset symbol string."""

    @underlying_asset.setter
    def underlying_asset(self, value: param.Asset | None) -> None: ...
    @property
    def settlement_asset(self) -> param.Asset | None:
        """Settlement asset symbol string."""

    @settlement_asset.setter
    def settlement_asset(self, value: param.Asset | None) -> None: ...
    @property
    def account_id(self) -> param.AccountId | None:
        """Account identifier."""

    @account_id.setter
    def account_id(self, value: param.AccountId | None) -> None: ...
    @property
    def side(self) -> param.Side | None:
        """Trade side."""

    @side.setter
    def side(self, value: param.Side | None) -> None: ...

class FinancialImpact:
    """Realized P&L and fee group."""

    def __init__(
        self,
        *,
        pnl: param.Pnl,
        fee: param.Fee,
    ) -> None:
        """Create a financial impact group."""
        _ = (pnl, fee)

    @property
    def pnl(self) -> param.Pnl:
        """Realized PnL value."""

    @pnl.setter
    def pnl(self, value: param.Pnl) -> None: ...
    @property
    def fee(self) -> param.Fee:
        """Fee value."""

    @fee.setter
    def fee(self, value: param.Fee) -> None: ...

class ExecutionReportFillDetails:
    """Fill execution details group."""

    def __init__(
        self,
        *,
        last_trade: Trade | None = None,
        fee: param.MonetaryAmount | None = None,
        leaves_quantity: param.Quantity | None = None,
        lock: Lock,
        is_final: bool | None = None,
    ) -> None:
        """Create a fill details group."""
        _ = (last_trade, fee, leaves_quantity, lock, is_final)

    @property
    def last_trade(self) -> Trade | None:
        """Last executed trade."""

    @last_trade.setter
    def last_trade(self, value: Trade | None) -> None: ...
    @property
    def fee(self) -> param.MonetaryAmount | None:
        """Fee amount and currency."""

    @fee.setter
    def fee(self, value: param.MonetaryAmount | None) -> None: ...
    @property
    def leaves_quantity(self) -> param.Quantity | None:
        """Remaining order quantity."""

    @leaves_quantity.setter
    def leaves_quantity(self, value: param.Quantity | None) -> None: ...
    @property
    def lock(self) -> Lock:
        """Order lock payload."""

    @lock.setter
    def lock(self, value: Lock) -> None: ...
    @property
    def is_final(self) -> bool | None:
        """Whether this report closes the order's report stream.

        The order is filled, cancelled, or rejected.
        """

    @is_final.setter
    def is_final(self, value: bool | None) -> None: ...

class ExecutionReportPositionImpact:
    """Position-impact data group."""

    def __init__(
        self,
        *,
        position_effect: param.PositionEffect | None = None,
        position_side: param.PositionSide | None = None,
    ) -> None:
        """Create a position impact group."""
        _ = (position_effect, position_side)

    @property
    def position_effect(self) -> param.PositionEffect | None:
        """Position effect."""

    @position_effect.setter
    def position_effect(self, value: param.PositionEffect | None) -> None: ...
    @property
    def position_side(self) -> param.PositionSide | None:
        """Position side."""

    @position_side.setter
    def position_side(self, value: param.PositionSide | None) -> None: ...

class ExecutionReport:
    """Extensible execution report model.

    Accepted by ``Engine.apply_execution_report``.
    """

    def __init__(
        self,
        *,
        operation: ExecutionReportOperation | None = None,
        financial_impact: FinancialImpact | None = None,
        fill: ExecutionReportFillDetails | None = None,
        position_impact: ExecutionReportPositionImpact | None = None,
    ) -> None:
        """Create an execution report with optional groups."""
        _ = (operation, financial_impact, fill, position_impact)

    @property
    def operation(self) -> ExecutionReportOperation | None:
        """Execution-report instrument and side group."""

    @operation.setter
    def operation(self, value: ExecutionReportOperation | None) -> None: ...
    @property
    def financial_impact(self) -> FinancialImpact | None:
        """Realized P&L and fee group."""

    @financial_impact.setter
    def financial_impact(self, value: FinancialImpact | None) -> None: ...
    @property
    def fill(self) -> ExecutionReportFillDetails | None:
        """Fill execution details group."""

    @fill.setter
    def fill(self, value: ExecutionReportFillDetails | None) -> None: ...
    @property
    def position_impact(self) -> ExecutionReportPositionImpact | None:
        """Position-impact data group."""

    @position_impact.setter
    def position_impact(self, value: ExecutionReportPositionImpact | None) -> None: ...

class TradeAmount:
    """Quantity- or volume-based trade amount."""

    def __init__(self, other: TradeAmount) -> None:
        """Copy / subclass constructor."""
        _ = other

    @staticmethod
    def quantity(value: param.Quantity | str | int | float) -> TradeAmount:
        """Create a quantity-based trade amount."""
        _ = value

    @staticmethod
    def volume(value: param.Volume | str | int | float) -> TradeAmount:
        """Create a volume-based trade amount."""
        _ = value

    @property
    def is_quantity(self) -> bool:
        """True when the amount is expressed as quantity (instrument units)."""

    @property
    def is_volume(self) -> bool:
        """True when the amount is expressed as volume (notional units)."""

    @property
    def as_quantity(self) -> param.Quantity | None:
        """Inner quantity, or None when the amount is volume-based."""

    @property
    def as_volume(self) -> param.Volume | None:
        """Inner volume, or None when the amount is quantity-based."""

class AdjustmentAmount:
    """Delta-or-absolute adjustment payload."""

    def __init__(self, other: AdjustmentAmount) -> None:
        """Copy / subclass constructor."""
        _ = other

    @staticmethod
    def delta(value: param.PositionSize) -> AdjustmentAmount:
        _ = value

    @staticmethod
    def absolute(value: param.PositionSize) -> AdjustmentAmount:
        """Create an absolute-type adjustment."""
        _ = value

    @property
    def is_delta(self) -> bool:
        """True when the adjustment is a signed delta."""

    @property
    def is_absolute(self) -> bool:
        """True when the adjustment sets an absolute value."""

    @property
    def as_delta(self) -> param.PositionSize | None:
        """Inner position size when delta, otherwise None."""

    @property
    def as_absolute(self) -> param.PositionSize | None:
        """Inner position size when absolute, otherwise None."""

class Trade:
    """Trade payload with price and quantity."""

    def __init__(self, *, price: param.Price, quantity: param.Quantity) -> None:
        _ = (price, quantity)

    @property
    def price(self) -> param.Price:
        """Trade price."""

    @property
    def quantity(self) -> param.Quantity:
        """Trade quantity."""

class AccountAdjustmentAmount:
    """Grouped amount payload (balance + held + incoming)."""

    def __init__(
        self,
        *,
        balance: AdjustmentAmount | None = None,
        held: AdjustmentAmount | None = None,
        incoming: AdjustmentAmount | None = None,
    ) -> None:
        _ = (balance, held, incoming)

    @property
    def balance(self) -> AdjustmentAmount | None: ...
    @balance.setter
    def balance(self, value: AdjustmentAmount | None) -> None: ...
    @property
    def held(self) -> AdjustmentAmount | None: ...
    @held.setter
    def held(self, value: AdjustmentAmount | None) -> None: ...
    @property
    def incoming(self) -> AdjustmentAmount | None: ...
    @incoming.setter
    def incoming(self, value: AdjustmentAmount | None) -> None: ...

class AccountAdjustmentBalanceOperation:
    """Physical-balance account-adjustment operation group."""

    def __init__(
        self,
        *,
        asset: param.Asset | None = None,
        average_entry_price: param.Price | None = None,
        realized_pnl: param.Pnl | None = None,
    ) -> None:
        _ = (asset, average_entry_price, realized_pnl)

    @property
    def asset(self) -> param.Asset | None: ...
    @asset.setter
    def asset(self, value: param.Asset | None) -> None: ...
    @property
    def average_entry_price(self) -> param.Price | None:
        """Average entry price force-set in account currency.

        The value is caller supplied; no FX is applied.
        """

    @average_entry_price.setter
    def average_entry_price(self, value: param.Price | None) -> None: ...
    @property
    def realized_pnl(self) -> param.Pnl | None:
        """Absolute realized PnL force-set in account currency.

        The value is caller supplied; no FX is applied.
        """

    @realized_pnl.setter
    def realized_pnl(self, value: param.Pnl | None) -> None: ...

class AccountAdjustmentPositionOperation:
    """Derivatives-like position account-adjustment operation group."""

    def __init__(
        self,
        *,
        underlying_asset: param.Asset | None = None,
        settlement_asset: param.Asset | None = None,
        collateral_asset: param.Asset | None = None,
        average_entry_price: param.Price | None = None,
        mode: param.PositionMode | None = None,
        leverage: param.Leverage | int | float | None = None,
    ) -> None:
        _ = (
            underlying_asset,
            settlement_asset,
            collateral_asset,
            average_entry_price,
            mode,
            leverage,
        )

    @property
    def underlying_asset(self) -> param.Asset | None: ...
    @underlying_asset.setter
    def underlying_asset(self, value: param.Asset | None) -> None: ...
    @property
    def settlement_asset(self) -> param.Asset | None: ...
    @settlement_asset.setter
    def settlement_asset(self, value: param.Asset | None) -> None: ...
    @property
    def collateral_asset(self) -> param.Asset | None: ...
    @collateral_asset.setter
    def collateral_asset(self, value: param.Asset | None) -> None: ...
    @property
    def average_entry_price(self) -> param.Price | None:
        """Average entry price force-set in account currency.

        The value is caller supplied; no FX is applied.
        """

    @average_entry_price.setter
    def average_entry_price(self, value: param.Price | None) -> None: ...
    @property
    def mode(self) -> param.PositionMode | None: ...
    @mode.setter
    def mode(self, value: param.PositionMode | None) -> None: ...
    @property
    def leverage(self) -> param.Leverage | None: ...
    @leverage.setter
    def leverage(self, value: param.Leverage | int | float | None) -> None: ...

class AccountAdjustmentBounds:
    """Optional post-adjustment bounds group."""

    def __init__(
        self,
        *,
        balance_upper: param.PositionSize | None = None,
        balance_lower: param.PositionSize | None = None,
        held_upper: param.PositionSize | None = None,
        held_lower: param.PositionSize | None = None,
        incoming_upper: param.PositionSize | None = None,
        incoming_lower: param.PositionSize | None = None,
    ) -> None:
        _ = (
            balance_upper,
            balance_lower,
            held_upper,
            held_lower,
            incoming_upper,
            incoming_lower,
        )

    @property
    def balance_upper(self) -> param.PositionSize | None: ...
    @balance_upper.setter
    def balance_upper(self, value: param.PositionSize | None) -> None: ...
    @property
    def balance_lower(self) -> param.PositionSize | None: ...
    @balance_lower.setter
    def balance_lower(self, value: param.PositionSize | None) -> None: ...
    @property
    def held_upper(self) -> param.PositionSize | None: ...
    @held_upper.setter
    def held_upper(self, value: param.PositionSize | None) -> None: ...
    @property
    def held_lower(self) -> param.PositionSize | None: ...
    @held_lower.setter
    def held_lower(self, value: param.PositionSize | None) -> None: ...
    @property
    def incoming_upper(self) -> param.PositionSize | None: ...
    @incoming_upper.setter
    def incoming_upper(self, value: param.PositionSize | None) -> None: ...
    @property
    def incoming_lower(self) -> param.PositionSize | None: ...
    @incoming_lower.setter
    def incoming_lower(self, value: param.PositionSize | None) -> None: ...

class AccountAdjustment:
    """Extensible non-trading account-adjustment record."""

    def __init__(
        self,
        *,
        operation: (
            AccountAdjustmentBalanceOperation
            | AccountAdjustmentPositionOperation
            | None
        ) = None,
        amount: AccountAdjustmentAmount | None = None,
        bounds: AccountAdjustmentBounds | None = None,
    ) -> None:
        _ = (operation, amount, bounds)

    @property
    def operation(
        self,
    ) -> (
        AccountAdjustmentBalanceOperation | AccountAdjustmentPositionOperation | None
    ): ...
    @operation.setter
    def operation(
        self,
        value: (
            AccountAdjustmentBalanceOperation
            | AccountAdjustmentPositionOperation
            | None
        ),
    ) -> None: ...
    @property
    def amount(self) -> AccountAdjustmentAmount | None: ...
    @amount.setter
    def amount(self, value: AccountAdjustmentAmount | None) -> None: ...
    @property
    def bounds(self) -> AccountAdjustmentBounds | None: ...
    @bounds.setter
    def bounds(self, value: AccountAdjustmentBounds | None) -> None: ...

class Request:
    """
    Deferred main-stage request handle produced by ``Engine.start_pre_trade``.

    The handle is single-use: calling ``execute`` more than once is a lifecycle
    error.
    """

    def execute(self) -> ExecuteResult:
        """Run main-stage pre-trade checks."""

PolicyGroupId: typing.TypeAlias = int
"""Group identifier used to tag policies and their outcomes."""

class Lock:
    """Pre-trade lock payload: grouped `(policy_group_id, price)` records."""

    def __init__(
        self,
        entries: (
            Lock | typing.Iterable[tuple[PolicyGroupId, param.Price]] | None
        ) = None,
    ) -> None:
        _ = entries

    def push(self, policy_group_id: PolicyGroupId, price: param.Price) -> None:
        """Append `price` under `policy_group_id`."""

    def push_many(
        self,
        policy_group_id: PolicyGroupId,
        prices: typing.Iterable[param.Price],
    ) -> None:
        """Append every price under `policy_group_id`."""

    def extend(
        self, entries: typing.Iterable[tuple[PolicyGroupId, param.Price]]
    ) -> None:
        """Append every `(policy_group_id, price)` pair from the iterable."""

    def merge(self, other: Lock) -> None:
        """Append all entries from another lock."""

    def __len__(self) -> int:
        """Number of stored price entries."""

    def prices_of(self, policy_group_id: PolicyGroupId) -> list[param.Price]:
        """All prices stored under `policy_group_id`, in insertion order."""

    def entries(self) -> list[tuple[PolicyGroupId, param.Price]]:
        """Every `(policy_group_id, price)` pair, default group first."""

    def to_json(self) -> str:
        """Serialize to compact JSON."""

    @staticmethod
    def from_json(text: str) -> Lock:
        """Deserialize from compact JSON."""

    def to_msgpack(self) -> bytes:
        """Serialize to MessagePack."""

    @staticmethod
    def from_msgpack(data: bytes) -> Lock:
        """Deserialize from MessagePack."""

    def to_cbor(self) -> bytes:
        """Serialize to CBOR."""

    @staticmethod
    def from_cbor(data: bytes) -> Lock:
        """Deserialize from CBOR."""

class Reservation:
    """
    Single-use reservation handle returned by successful main-stage execution.

    Exactly one of ``commit`` or ``rollback`` must be called to finalize the
    reserved state.
    """

    def lock(self) -> Lock:
        """Current reservation lock payload."""

    def account_adjustments(self) -> list[AccountAdjustmentOutcome]:
        """Account adjustment outcomes captured by the reservation."""

    def commit(self) -> None:
        """Finalize reservation as committed."""

    def rollback(self) -> None:
        """Finalize reservation as rolled back."""

class StartResult:
    """
    Result of ``Engine.start_pre_trade``.

    On success it exposes a deferred request handle; on failure it exposes the
    merged reject list from all rejecting start-stage checks.
    """

    @property
    def ok(self) -> bool:
        """Whether start-stage checks passed."""

    @property
    def request(self) -> Request | None:
        """Request handle when checks pass."""

    @property
    def rejects(self) -> list[Reject]:
        """Reject list when checks fail."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``ok``."""

class ExecuteResult:
    """
    Result of ``Request.execute``.

    This object reports whether main-stage checks accepted the request and,
    on success, carries the single-use reservation handle that must later be
    committed or rolled back.
    """

    @property
    def ok(self) -> bool:
        """Whether main-stage checks passed."""

    @property
    def reservation(self) -> Reservation | None:
        """Reservation when checks pass."""

    @property
    def rejects(self) -> list[Reject]:
        """Reject list when checks fail."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``ok``."""

class DryRunReport:
    """
    Inert verdict of a non-mutating pre-trade dry-run.

    Describes what a real pre-trade call would have produced for the same order
    and engine state, with zero effect on that state.
    """

    @property
    def is_pass(self) -> bool:
        """Whether the order would have been admitted by all stages."""

    @property
    def rejects(self) -> list[Reject] | None:
        """Reject list when checks would fail."""

    def lock(self) -> Lock:
        """Lock payload the main stage would have produced."""

    def account_adjustments(self) -> list[AccountAdjustmentOutcome]:
        """Account adjustment outcomes the main stage would have produced."""

    @property
    def account_block(self) -> AccountBlock | None:
        """Account block a real account-scope reject would have recorded."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``is_pass``."""

class OutcomeAmount:
    """Delta and absolute values for one account position field."""

    def __init__(
        self,
        *,
        delta: param.PositionSize,
        absolute: param.PositionSize,
    ) -> None: ...
    @property
    def delta(self) -> param.PositionSize:
        """Signed change applied by this operation."""

    @property
    def absolute(self) -> param.PositionSize:
        """Field value at the moment the policy returned."""

class PnlOutcomeAmount:
    """Account-currency delta and absolute values for one realized PnL field."""

    def __init__(
        self,
        *,
        delta: param.Pnl,
        absolute: param.Pnl,
    ) -> None: ...
    @property
    def delta(self) -> param.Pnl:
        """Signed account-currency PnL change applied by this operation."""

    @property
    def absolute(self) -> param.Pnl:
        """Cumulative account-currency realized PnL after this operation."""

class AccountOutcomeEntry:
    """Account position outcome for one asset."""

    def __init__(
        self,
        *,
        asset: str,
        balance: OutcomeAmount | None = None,
        held: OutcomeAmount | None = None,
        incoming: OutcomeAmount | None = None,
        realized_pnl: PnlOutcomeAmount | None = None,
        average_entry_price: param.Price | None = None,
    ) -> None: ...
    @property
    def asset(self) -> str:
        """Asset this outcome refers to."""

    @property
    def balance(self) -> OutcomeAmount | None:
        """Balance outcome."""

    @property
    def held(self) -> OutcomeAmount | None:
        """Held amount outcome."""

    @property
    def incoming(self) -> OutcomeAmount | None:
        """Incoming amount outcome."""

    @property
    def realized_pnl(self) -> PnlOutcomeAmount | None:
        """Account-currency realized PnL outcome.

        ``None`` means realized PnL was not tracked or not emitted. Missing
        account currency or FX stops tracking without reject/block.
        """

    @property
    def average_entry_price(self) -> param.Price | None:
        """Account-currency average entry price outcome.

        ``None`` means average entry price was not tracked or not emitted.
        Missing account currency or FX stops tracking without reject/block.
        """

class AccountAdjustmentOutcome:
    """Account position outcome tagged with a policy group."""

    def __init__(
        self,
        *,
        policy_group_id: PolicyGroupId,
        entry: AccountOutcomeEntry,
    ) -> None: ...
    @property
    def policy_group_id(self) -> PolicyGroupId:
        """Policy group tag of the policy that produced this outcome."""

    @property
    def entry(self) -> AccountOutcomeEntry:
        """Account outcome entry."""

class AccountAdjustmentBatchResult:
    """Result of ``Engine.apply_account_adjustment``."""

    @property
    def ok(self) -> bool:
        """Whether the full batch passed."""

    @property
    def failed_index(self) -> int | None:
        """Zero-based index of the failing adjustment."""

    @property
    def rejects(self) -> list[Reject]:
        """Reject list when validation fails."""

    @property
    def outcomes(self) -> list[AccountAdjustmentOutcome]:
        """Account position outcomes returned on success."""

    def __bool__(self) -> bool:
        """Boolean convenience alias for ``ok``."""

class AccountBlock:
    """An account-level block record returned by a policy callback."""

    def __init__(
        self,
        *,
        policy: str,
        code: str,
        reason: str,
        details: str,
        user_data: int = 0,
    ) -> None: ...
    @property
    def code(self) -> str:
        """Stable machine-readable reject code."""

    @property
    def policy(self) -> str:
        """Policy name that produced the block."""

    @property
    def reason(self) -> str:
        """Human-readable reject reason."""

    @property
    def details(self) -> str:
        """Case-specific reject details."""

    @property
    def user_data(self) -> int:
        """Opaque caller-defined integer token."""

class PostTradeResult:
    """
    Result of ``Engine.apply_execution_report``.

    A non-empty ``account_blocks`` list means at least one policy entered a
    blocked state after the report was applied.

    Post-trade processing is **not** atomic: ``account_adjustments`` reflect
    storage mutations that have already been applied, so callers must propagate
    them downstream even when ``account_blocks`` is non-empty.
    """

    def __init__(
        self,
        *,
        account_blocks: typing.Iterable[AccountBlock] | None = None,
        account_adjustments: typing.Iterable[AccountAdjustmentOutcome] | None = None,
    ) -> None: ...
    @property
    def account_blocks(self) -> list[AccountBlock]:
        """Account blocks reported by policies. Non-empty when a kill switch fired."""

    @property
    def account_adjustments(self) -> list[AccountAdjustmentOutcome]:
        """Per-asset position outcomes reported by policies, in registration order."""

class AccountControl:
    """Per-account handle to the engine's account-block facility.

    Valid to use only within the pre-trade processing of the request it belongs
    to — from the callback that produced it through the commit or rollback of
    that request's reservation (so it may be retained for a deferred mutation
    commit/rollback callback). Recording a block through it after that pre-trade
    transaction has completed is unspecified and must not be relied upon.
    """

    def block(self, block: AccountBlock) -> None: ...

class Context:
    """Context of the current pre-trade operation."""

    @property
    def account_control(self) -> AccountControl | None: ...
    @property
    def account_group(self) -> AccountGroupId | None: ...

class AccountAdjustmentContext:
    """Context of the current account-adjustment operation."""

    @property
    def account_control(self) -> AccountControl: ...
    @property
    def account_group(self) -> AccountGroupId | None: ...

class PostTradeContext:
    """Context of the current post-trade operation."""

    @property
    def account_group(self) -> AccountGroupId | None: ...

class OrderSizeLimit:
    """Order size limits."""

    def __init__(
        self,
        *,
        max_quantity: param.Quantity,
        max_notional: param.Volume,
    ) -> None:
        """Create order size limits."""
        _ = (max_quantity, max_notional)

class SyncedEngineBuilder:
    """Second stage of the engine builder (sync policy already chosen). Add at least one
    policy to obtain a ReadyEngineBuilder."""

    def pre_trade(
        self,
        policy: Policy,
    ) -> ReadyEngineBuilder:
        """Register a pre-trade policy."""
        _ = policy

    def builtin(self, builtin_ready_builder: typing.Any) -> ReadyEngineBuilder:
        """Register a built-in policy via its ready builder."""
        _ = builtin_ready_builder

    def market_data(self, default_ttl: QuoteTtl) -> MarketDataBuilder:
        """Create a market-data service builder."""

class ReadyEngineBuilder:
    """Third stage of the engine builder (at least one policy registered). Accepts more
    policies and builds the engine."""

    def pre_trade(
        self,
        policy: Policy,
    ) -> ReadyEngineBuilder:
        """Register an additional pre-trade policy."""
        _ = policy

    def builtin(self, builtin_ready_builder: typing.Any) -> ReadyEngineBuilder:
        """Register a built-in policy via its ready builder."""
        _ = builtin_ready_builder

    def market_data(self, default_ttl: QuoteTtl) -> MarketDataBuilder:
        """Create a market-data service builder."""

    def build(self) -> Engine:
        """Build an engine instance."""

class Engine:
    """Pre-trade risk engine."""

    @staticmethod
    def builder() -> EngineBuilder:
        """Create a new EngineBuilder."""

    def start_pre_trade(self, order: object) -> StartResult: ...
    def start_pre_trade_dry_run(self, order: object) -> DryRunReport: ...
    def execute_pre_trade(self, order: object) -> ExecuteResult: ...
    def execute_pre_trade_dry_run(self, order: object) -> DryRunReport: ...
    def apply_execution_report(self, report: object) -> PostTradeResult: ...
    def apply_account_adjustment(
        self,
        account_id: object,
        adjustments: object,
    ) -> AccountAdjustmentBatchResult: ...
    def accounts(self) -> Accounts:
        """Return a handle to the engine's account controls."""

    def configure(self) -> Configurator:
        """Return a handle to the engine's runtime policy settings registry."""

class Accounts:
    """Handle to the engine's account groups and pre-trade block controls."""

    def register_group(
        self,
        accounts: typing.Iterable[AccountId],
        group: AccountGroupId,
    ) -> None: ...
    def unregister_group(
        self,
        accounts: typing.Iterable[AccountId],
        group: AccountGroupId,
    ) -> None: ...
    def group_of(self, account: AccountId) -> AccountGroupId | None: ...
    def set_currency(self, account: AccountId, asset: param.Asset | str) -> None:
        """Set an account's explicit currency.

        Setting or changing currency does not validate existing holdings and
        does not recompute stored average entry price or realized PnL. The
        caller owns the risk of changing currency on live state.
        """

    def clear_currency(self, account: AccountId) -> None:
        """Clear an account's explicit currency.

        Clearing currency does not validate existing holdings and does not
        recompute stored average entry price or realized PnL.
        """

    def set_group_currency(
        self, group: AccountGroupId | int, asset: param.Asset | str
    ) -> None:
        """Set the currency inherited by accounts in a group.

        ``AccountGroupId.DEFAULT`` is allowed and represents the global
        default tier. Setting or changing currency does not validate existing
        holdings and does not recompute stored average entry price or realized
        PnL.
        """

    def clear_group_currency(self, group: AccountGroupId | int) -> None:
        """Clear the currency inherited by accounts in a group.

        ``AccountGroupId.DEFAULT`` is allowed and represents the global
        default tier. Clearing currency does not validate existing holdings and
        does not recompute stored average entry price or realized PnL.
        """

    def block(self, account: AccountId, reason: str) -> None: ...
    def unblock(self, account: AccountId) -> None: ...
    def replace_block_reason(self, account: AccountId, reason: str) -> None: ...
    def block_group(self, group: AccountGroupId, reason: str) -> None: ...
    def unblock_group(self, group: AccountGroupId) -> None: ...
    def replace_group_block_reason(
        self, group: AccountGroupId, reason: str
    ) -> None: ...

class Configurator:
    """Handle to the engine's runtime policy settings registry.

    Obtained from ``Engine.configure()``. The handle shares the engine's single
    settings registry, so changes made through it are visible to running
    policies immediately. It inherits the engine's synchronization mode.
    """

    def rate_limit(
        self,
        name: str,
        *,
        broker: pretrade.policies.RateLimitBrokerBarrier | None = None,
        clear_broker: bool = False,
        asset_barriers: list[pretrade.policies.RateLimitAssetBarrier] | None = None,
        account_barriers: list[pretrade.policies.RateLimitAccountBarrier] | None = None,
        account_asset_barriers: (
            list[pretrade.policies.RateLimitAccountAssetBarrier] | None
        ) = None,
    ) -> None:
        """Retune a registered rate-limit policy at runtime.

        *name* must match the name given to the policy at registration time.

        An axis passed as ``None`` is left unchanged.  A supplied list REPLACES
        that axis wholesale: an empty list clears it (subject to the
        at-least-one-barrier rule enforced by the core).  Barriers may be added
        and removed at runtime; a barrier key that survives a replacement keeps
        its live counter (no reset).  *broker* replaces the broker barrier when
        provided and leaves it unchanged when ``None``. Pass
        ``clear_broker=True`` to remove it.

        Policy settings use the named rate-limit entities from
        ``openpit.pretrade.policies``.

        Raises:
            PolicyConfigureError: If the policy is not found, has the wrong
                type, or the new settings fail validation.
        """

    def pnl_bounds_killswitch(
        self,
        name: str,
        *,
        broker_barriers: list[pretrade.policies.PnlBoundsBrokerBarrier] | None = None,
        account_barriers: (
            list[pretrade.policies.PnlBoundsAccountAssetBarrierUpdate] | None
        ) = None,
    ) -> None:
        """Retune a registered P&L bounds kill-switch policy at runtime.

        *name* must match the name given to the policy at registration time.

        The barrier arguments mirror those of
        ``ReadyEngineBuilder._add_builtin_pnl_bounds_killswitch``.

        An axis passed as ``None`` is left unchanged; an empty list replaces
        the axis with an empty set (subject to the at-least-one-barrier rule).

        Raises:
            PolicyConfigureError: If the policy is not found, has the wrong
                type, or the new settings fail validation.
        """

    def set_account_pnl(
        self,
        name: str,
        *,
        account: param.AccountId,
        settlement_asset: param.Asset,
        pnl: param.Pnl,
    ) -> None:
        """Force-set the live accumulated P&L for one account entry.

        *name* must match the name given to the P&L bounds kill-switch policy at
        registration time.

        Unlike :meth:`pnl_bounds_killswitch`, which retunes bounds and never
        touches accumulated P&L, this is an absolute assignment (upsert) of the
        live accumulator for ``(account, settlement_asset)``; the new value is
        evaluated against the live bounds on the next order. Forcing the
        accumulator past a bound trips the kill switch, which latches an
        engine-level account block that this call does not clear.

        Raises:
            PolicyConfigureError: If the policy is not found or has a different
                type than a P&L bounds kill-switch policy.
        """

    def order_size_limit(
        self,
        name: str,
        *,
        broker: pretrade.policies.OrderSizeBrokerBarrier | None = None,
        clear_broker: bool = False,
        asset_barriers: list[pretrade.policies.OrderSizeAssetBarrier] | None = None,
        account_asset_barriers: (
            list[pretrade.policies.OrderSizeAccountAssetBarrier] | None
        ) = None,
    ) -> None:
        """Retune a registered order-size-limit policy at runtime.

        *name* must match the name given to the policy at registration time.

        Policy settings use the named order-size entities from
        ``openpit.pretrade.policies``.

        ``broker=None`` and axis arguments passed as ``None`` are left
        unchanged; ``clear_broker=True`` removes the broker barrier. An empty
        list replaces that axis with an empty set (subject to the
        at-least-one-barrier rule).

        Raises:
            PolicyConfigureError: If the policy is not found, has the wrong
                type, or the new settings fail validation.
        """

    def spot_funds(
        self,
        name: str,
        *,
        global_slippage_bps: int | None = None,
        pricing_source: pretrade.policies.SpotFundsPricingSource | None = None,
        overrides: list[pretrade.policies.SpotFundsOverrideEntry] = ...,
        global_limit_mode: pretrade.policies.SpotFundsLimitMode | None = None,
        account_limit_modes: list[
            pretrade.policies.SpotFundsLimitModeAccountEntry
        ] = ...,
        account_group_limit_modes: list[
            pretrade.policies.SpotFundsLimitModeAccountGroupEntry
        ] = ...,
    ) -> None:
        """Retune a registered spot-funds policy at runtime.

        *name* must match the name given to the policy at registration time.

        *global_slippage_bps* replaces the global slippage when provided.

        *pricing_source* is a
        :class:`~openpit.pretrade.policies.SpotFundsPricingSource`, or ``None``
        to leave the current source unchanged.

        *overrides* contains
        :class:`~openpit.pretrade.policies.SpotFundsOverrideEntry` entries that
        replace individual cascade entries.

        *global_limit_mode* is a
        :class:`~openpit.pretrade.policies.SpotFundsLimitMode`, or ``None`` to
        leave the global insufficient-funds limit mode unchanged.

        *account_limit_modes* and *account_group_limit_modes* contain
        :class:`~openpit.pretrade.policies.SpotFundsLimitModeAccountEntry` and
        :class:`~openpit.pretrade.policies.SpotFundsLimitModeAccountGroupEntry`
        entries; each pins the entry's ``mode`` for that scope, or clears the
        pin (deferring to the next cascade tier) when ``mode`` is ``None``.

        Raises:
            PolicyConfigureError: If the policy is not found, has the wrong
                type, or the new settings fail validation.
        """

    def spot_funds_pnl_bounds_killswitch(
        self,
        name: str,
        *,
        global_barriers: (
            list[pretrade.policies.SpotFundsPnlBoundsBarrier] | None
        ) = None,
        account_group_barriers: (
            list[pretrade.policies.SpotFundsPnlBoundsAccountGroupBarrier] | None
        ) = None,
        account_barriers: (
            list[pretrade.policies.SpotFundsPnlBoundsAccountBarrierUpdate] | None
        ) = None,
    ) -> None:
        """Retune the account-currency P&L bounds axis of a spot-funds policy.

        *name* must match the name given to the spot-funds policy at
        registration time.

        The barrier arguments mirror those of
        ``ReadyEngineBuilder._add_builtin_spot_funds_pnl_bounds_killswitch``,
        except that the account axis takes
        :class:`~openpit.pretrade.policies.SpotFundsPnlBoundsAccountBarrierUpdate`
        entries: runtime account updates preserve the live accumulated P&L and
        cannot seed or reset it (no ``initial_pnl``).

        An axis passed as ``None`` is left unchanged.  A supplied list REPLACES
        that axis wholesale: an empty list clears it.  Each barrier must still
        configure at least one bound.

        Raises:
            PolicyConfigureError: If the policy is not found, has the wrong
                type, or the new settings fail validation.
        """

    def set_spot_funds_account_pnl(
        self,
        name: str,
        *,
        account: param.AccountId,
        account_currency: param.Asset,
        pnl: param.Pnl,
    ) -> None:
        """Force-set live spot-funds account-currency P&L for one account.

        *name* must match the name given to the spot-funds policy at
        registration time.

        Unlike :meth:`spot_funds_pnl_bounds_killswitch`, which retunes bounds
        and never touches accumulated P&L, this is an absolute assignment
        (upsert) of the live accumulator for ``(account, account_currency)``;
        the new value is evaluated against the live bounds when the next P&L
        delta is applied by an execution report. A breach trips the kill
        switch, which latches an engine-level account block that this call
        does not clear.

        Raises:
            PolicyConfigureError: If the policy is not found or has a different
                type than a spot-funds policy.
        """

class EngineBuilder:
    """First stage of the engine builder."""

    def full_sync(self) -> SyncedEngineBuilder:
        """Use full synchronization (concurrent cross-thread calls safe)."""

    def no_sync(self) -> SyncedEngineBuilder:
        """Use no synchronization (zero overhead; handle stays on creating thread)."""

    def account_sync(self) -> SyncedEngineBuilder:
        """Use account synchronization (concurrent when caller pins each account to one
        chain)."""
