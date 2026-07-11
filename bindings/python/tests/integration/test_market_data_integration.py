import time
import types
from datetime import timedelta

import openpit
import pytest

_DEFAULT = openpit.marketdata.QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT
_NO_GROUP = types.SimpleNamespace(account_group=None)


@pytest.mark.integration
def test_market_data_quote_expired_error_carries_stale_quote() -> None:
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.within(timedelta(milliseconds=20)))
        .build()
    )
    aapl_id = service.register(openpit.Instrument("AAPL", "USD"))
    account_id = openpit.param.AccountId.from_int(1)

    service.push(aapl_id, openpit.marketdata.Quote(mark="200"))
    time.sleep(0.04)

    with pytest.raises(openpit.marketdata.QuoteExpired) as exc_info:
        service.get(aapl_id, account_id, _NO_GROUP, _DEFAULT)

    assert exc_info.value.quote.mark == openpit.param.Price("200")


@pytest.mark.integration
def test_market_data_registration_errors_carry_variant_payloads() -> None:
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    aapl = openpit.Instrument("AAPL", "USD")
    msft = openpit.Instrument("MSFT", "USD")
    instrument_id = openpit.marketdata.InstrumentId(42)
    other_id = openpit.marketdata.InstrumentId(43)

    service.register_with_id(aapl, instrument_id)

    with pytest.raises(openpit.marketdata.AlreadyRegistered) as already:
        service.register(aapl)
    assert already.value.instrument.underlying_asset == "AAPL"
    assert already.value.instrument.settlement_asset == "USD"

    with pytest.raises(openpit.marketdata.RegistrationError) as duplicate_id:
        service.register_with_id(msft, instrument_id)
    assert duplicate_id.value.instrument_id == instrument_id
    assert duplicate_id.value.instrument is None

    with pytest.raises(openpit.marketdata.RegistrationError) as duplicate_instrument:
        service.register_with_id(aapl, other_id)
    assert duplicate_instrument.value.instrument.underlying_asset == "AAPL"
    assert duplicate_instrument.value.instrument.settlement_asset == "USD"
    assert duplicate_instrument.value.instrument_id is None

    unknown_id = openpit.marketdata.InstrumentId(999)
    with pytest.raises(openpit.marketdata.UnknownInstrumentId) as unknown:
        service.push(unknown_id, openpit.marketdata.Quote(mark="1"))
    assert unknown.value.instrument_id == unknown_id
