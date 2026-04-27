import openpit

_DEFAULT_ACCOUNT_ID = openpit.param.AccountId.from_u64(99224416)


def make_order(
    *,
    side: openpit.param.Side = openpit.param.Side.BUY,
    trade_amount: openpit.param.TradeAmount | None = None,
    price: openpit.param.Price | None = None,
    instrument: openpit.Instrument | None = None,
    account_id: openpit.param.AccountId | None = None,
) -> openpit.Order:
    if trade_amount is None:
        trade_amount = openpit.param.TradeAmount.quantity(1)
    if price is None:
        price = openpit.param.Price("10")
    if instrument is None:
        instrument = openpit.Instrument(
            "AAPL",
            "USD",
        )
    if account_id is None:
        account_id = _DEFAULT_ACCOUNT_ID

    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=instrument,
            side=side,
            trade_amount=trade_amount,
            account_id=account_id,
            price=price,
        ),
    )


def make_report(
    *,
    pnl: openpit.param.Pnl,
    fee: openpit.param.Fee | None = None,
    instrument: openpit.Instrument | None = None,
    side: openpit.param.Side = openpit.param.Side.BUY,
    account_id: openpit.param.AccountId | None = None,
) -> openpit.ExecutionReport:
    if fee is None:
        fee = openpit.param.Fee("0")
    if instrument is None:
        instrument = openpit.Instrument(
            "AAPL",
            "USD",
        )
    if account_id is None:
        account_id = _DEFAULT_ACCOUNT_ID

    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=instrument,
            side=side,
            account_id=account_id,
        ),
        financial_impact=openpit.FinancialImpact(pnl=pnl, fee=fee),
    )
