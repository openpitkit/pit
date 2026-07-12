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

import openpit
import pytest


def instrument(symbol: str = "AAPL") -> openpit.core.Instrument:
    return openpit.core.Instrument(
        openpit.param.Asset(symbol),
        openpit.param.Asset("USD"),
    )


@pytest.mark.unit
def test_reference_book_resolves_ids_and_preserves_separate_settlement_legs() -> None:
    book = openpit.core.ReferenceBook()
    identifier = openpit.core.InstrumentId(42)
    aapl = instrument()

    assert book.register_with_id(aapl, identifier) == identifier
    assert book.resolve(aapl) == identifier
    assert book.settlement_scheme(identifier) is None

    scheme = openpit.core.SettlementScheme(
        openpit.core.SettlementLag(
            2,
            openpit.core.SettlementUnit.BUSINESS_DAYS,
        ),
        openpit.core.SettlementLag(
            1,
            openpit.core.SettlementUnit.CALENDAR_DAYS,
        ),
    )
    book.set_settlement_scheme(identifier, scheme)
    assert book.settlement_scheme(identifier) == scheme
    assert scheme.delivery.n == 2
    assert scheme.delivery.unit == openpit.core.SettlementUnit.BUSINESS_DAYS
    assert scheme.payment.n == 1
    assert scheme.payment.unit == openpit.core.SettlementUnit.CALENDAR_DAYS


@pytest.mark.unit
def test_reference_book_reports_duplicate_and_unknown_ids() -> None:
    book = openpit.core.ReferenceBook()
    identifier = openpit.core.InstrumentId(42)
    book.register_with_id(instrument(), identifier)

    with pytest.raises(openpit.core.ReferenceBookRegistrationError) as duplicate_id:
        book.register_with_id(instrument("MSFT"), identifier)
    assert duplicate_id.value.instrument_id == identifier
    assert duplicate_id.value.instrument is None

    with pytest.raises(
        openpit.core.ReferenceBookRegistrationError,
    ) as duplicate_instrument:
        book.register_with_id(instrument(), openpit.core.InstrumentId(43))
    assert duplicate_instrument.value.instrument_id is None
    assert duplicate_instrument.value.instrument.underlying_asset == "AAPL"
    with pytest.raises(openpit.core.UnknownReferenceBookInstrumentId) as error:
        book.set_settlement_scheme(
            openpit.core.InstrumentId(99),
            openpit.core.SettlementScheme.uniform(1),
        )
    assert error.value.instrument_id == openpit.core.InstrumentId(99)
    with pytest.raises(openpit.core.UnknownReferenceBookInstrumentId):
        book.settlement_scheme(openpit.core.InstrumentId(99))


@pytest.mark.unit
def test_uniform_settlement_defaults_to_business_days_and_marketdata_aliases_id() -> (
    None
):
    scheme = openpit.core.SettlementScheme.uniform(2)
    assert scheme.delivery.n == 2
    assert scheme.payment.n == 2
    assert scheme.delivery.unit == openpit.core.SettlementUnit.BUSINESS_DAYS
    assert scheme.payment.unit == openpit.core.SettlementUnit.BUSINESS_DAYS
    assert (
        openpit.core.SettlementLag(
            0,
            openpit.core.SettlementUnit.BUSINESS_DAYS,
        ).unit
        == openpit.core.SettlementUnit.BUSINESS_DAYS
    )
    assert openpit.marketdata.InstrumentId is openpit.core.InstrumentId
