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
