import openpit


def make_order(
    *,
    side: openpit.param.Side = openpit.param.Side.BUY,
    trade_amount: openpit.param.Quantity | openpit.param.Volume | None = None,
    price: openpit.param.Price | None = None,
    instrument: openpit.Instrument | None = None,
) -> openpit.Order:
    if trade_amount is None:
        trade_amount = openpit.param.Quantity("1")
    if price is None:
        price = openpit.param.Price("10")
    if instrument is None:
        instrument = openpit.Instrument(
            openpit.param.Asset("AAPL"),
            openpit.param.Asset("USD"),
        )

    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=instrument,
            side=side,
            trade_amount=trade_amount,
            price=price,
        ),
    )


def make_report(
    *,
    pnl: openpit.param.Pnl,
    fee: openpit.param.Fee | None = None,
    instrument: openpit.Instrument | None = None,
    side: openpit.param.Side = openpit.param.Side.BUY,
) -> openpit.ExecutionReport:
    if fee is None:
        fee = openpit.param.Fee("0")
    if instrument is None:
        instrument = openpit.Instrument(
            openpit.param.Asset("AAPL"),
            openpit.param.Asset("USD"),
        )

    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=instrument,
            side=side,
        ),
        financial_impact=openpit.FinancialImpact(pnl=pnl, fee=fee),
    )
