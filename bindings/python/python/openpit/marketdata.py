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

"""Live market-data service bindings."""

from __future__ import annotations

from contextlib import suppress

from ._openpit import AlreadyRegistered as AlreadyRegistered
from ._openpit import MarketDataBuilder as MarketDataBuilder
from ._openpit import MarketDataError as MarketDataError
from ._openpit import MarketDataService as MarketDataService
from ._openpit import Quote as Quote
from ._openpit import QuoteExpired as QuoteExpired
from ._openpit import QuoteResolution as QuoteResolution
from ._openpit import QuoteTtl as QuoteTtl
from ._openpit import QuoteUnavailable as QuoteUnavailable
from ._openpit import RegistrationError as RegistrationError
from ._openpit import UnknownInstrument as UnknownInstrument
from ._openpit import UnknownInstrumentId as UnknownInstrumentId
from .core import InstrumentId as InstrumentId


def _set_doc(obj, doc: str) -> None:
    with suppress(AttributeError, TypeError):
        obj.__doc__ = doc


_set_doc(
    InstrumentId,
    """Core instrument identity, re-exported for market-data compatibility.""",
)
_set_doc(
    QuoteTtl,
    """Quote lifetime policy: infinite or within a finite duration.""",
)
_set_doc(
    Quote,
    """Market snapshot with optional mark, bid, and ask prices.""",
)
_set_doc(
    QuoteResolution,
    """Controls which quote buckets a read may fall through to.

    Variants (in order of breadth):

    - ``ACCOUNT_ONLY`` — only the per-account bucket.
    - ``ACCOUNT_THEN_GROUP`` — per-account, then the account's group bucket.
    - ``ACCOUNT_THEN_GROUP_THEN_DEFAULT`` — per-account, then group, then the
      default ("everyone-else") bucket.
    """,
)
_set_doc(
    MarketDataService,
    """Thread-shareable live market-data service.

    Reads are performed with ``get(instrument_id, account_id, account_info,
    resolution)``. ``QuoteExpired`` raised by ``get`` carries the stale quote as
    ``quote``. ``get_optional(...)`` keeps the older optional shape.
    ``account_info`` is any object that
    exposes an ``account_group`` property returning an ``AccountGroupId`` or
    ``None`` — engine contexts (e.g. ``PreTradeContext``, ``PostTradeContext``)
    satisfy this automatically. The group is resolved lazily: the service only
    reads ``account_group`` when the per-account bucket misses and the
    resolution mode needs the group.
    """,
)
_set_doc(
    MarketDataBuilder,
    """Builder for a market-data service.

    Obtain via ``SyncedEngineBuilder.market_data(default_ttl)`` or
    ``ReadyEngineBuilder.market_data(default_ttl)``. The synchronization mode
    is derived from the engine builder's sync policy:

    - ``no_sync()`` engine -> no-sync mode (no-op locks, zero overhead).
    - ``full_sync()`` or ``account_sync()`` engine -> Full mode (real locks,
      safe for a concurrent quote feed).

    Call ``.full_sync()`` on the builder to upgrade a no-sync builder to Full,
    or ``.no_sync()`` to explicitly downgrade to no-op locks (zero overhead,
    single-threaded use only), before calling ``.build()``.
    """,
)
_set_doc(MarketDataError, """Base exception for market-data read failures.""")
_set_doc(UnknownInstrument, """Raised when a market-data id is not registered.""")
_set_doc(QuoteUnavailable, """Raised when no usable quote is available.""")
_set_doc(
    QuoteExpired,
    """Raised when the selected quote aged past TTL.

    The stale quote selected before the TTL check is available as ``quote``.
    """,
)
_set_doc(AlreadyRegistered, """Raised when an instrument is already registered.""")
_set_doc(RegistrationError, """Raised when an explicit id registration conflicts.""")
_set_doc(UnknownInstrumentId, """Raised when an operation references an unknown id.""")


__all__ = [
    "AlreadyRegistered",
    "InstrumentId",
    "MarketDataBuilder",
    "MarketDataError",
    "MarketDataService",
    "Quote",
    "QuoteExpired",
    "QuoteResolution",
    "QuoteTtl",
    "QuoteUnavailable",
    "RegistrationError",
    "UnknownInstrument",
    "UnknownInstrumentId",
]
