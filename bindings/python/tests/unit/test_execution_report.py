import openpit
import pytest


@pytest.mark.unit
def test_execution_report_exposes_fields_and_optional_defaults() -> None:
    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            account_id=openpit.param.AccountId.from_u64(99224416),
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl(-5),
            fee=openpit.param.Fee(0),
        ),
    )

    assert report.operation.instrument.underlying_asset == "AAPL"
    assert report.operation.instrument.settlement_asset == "USD"
    assert report.operation.side is openpit.param.Side.BUY
    assert str(report.financial_impact.pnl) == "-5"
    assert str(report.financial_impact.fee) == "0"
    assert report.fill is None
    assert "ExecutionReport(" in repr(report)


@pytest.mark.unit
def test_execution_report_operation_rejects_invalid_asset() -> None:
    with pytest.raises(ValueError):
        openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "",
            ),
            side=openpit.param.Side.BUY,
            account_id=openpit.param.AccountId.from_u64(99224416),
        )


@pytest.mark.unit
def test_execution_report_rejects_positional_args_for_keyword_only_constructor() -> (
    None
):
    with pytest.raises(TypeError):
        openpit.ExecutionReport("operation")  # type: ignore[call-arg]


@pytest.mark.unit
def test_execution_report_optional_groups_default_to_none() -> None:
    report = openpit.ExecutionReport()

    assert report.operation is None
    assert report.financial_impact is None
    assert report.fill is None
    assert report.position_impact is None


@pytest.mark.unit
def test_execution_report_operation_accepts_account_id_from_u64() -> None:
    op = openpit.ExecutionReportOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        side=openpit.param.Side.BUY,
        account_id=openpit.param.AccountId.from_u64(99224416),
    )
    assert op.account_id.value == 99224416


@pytest.mark.unit
def test_execution_report_operation_accepts_account_id_from_str() -> None:
    op = openpit.ExecutionReportOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        side=openpit.param.Side.BUY,
        account_id=openpit.param.AccountId.from_str("my-account"),
    )
    assert op.account_id.value == openpit.param.AccountId.from_str("my-account").value


@pytest.mark.unit
def test_execution_report_operation_rejects_raw_account_id_int() -> None:
    with pytest.raises(TypeError):
        openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            account_id=99224416,  # type: ignore[arg-type]
        )


@pytest.mark.unit
def test_execution_report_operation_rejects_raw_account_id_str() -> None:
    with pytest.raises(TypeError):
        openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            account_id="my-account",  # type: ignore[arg-type]
        )


@pytest.mark.unit
def test_financial_impact_requires_explicit_fee() -> None:
    with pytest.raises(TypeError):
        openpit.FinancialImpact(
            pnl=openpit.param.Pnl(-5),
        )  # type: ignore[call-arg]


@pytest.mark.unit
def test_fill_details_requires_explicit_lock() -> None:
    with pytest.raises(TypeError):
        openpit.ExecutionReportFillDetails(
            leaves_quantity=openpit.param.Quantity(0),
        )  # type: ignore[call-arg]


@pytest.mark.unit
def test_fill_details_happy_path_without_last_trade() -> None:
    fill = openpit.ExecutionReportFillDetails(
        leaves_quantity=openpit.param.Quantity(3),
        lock=openpit.pretrade.PreTradeLock(price=openpit.param.Price(101)),
    )

    assert str(fill.leaves_quantity) == "3"
    assert str(fill.lock.price) == "101"
    assert fill.last_trade is None
    assert fill.is_final is None


@pytest.mark.unit
def test_fill_details_happy_path_with_last_trade_and_final_flag() -> None:
    fill = openpit.ExecutionReportFillDetails(
        last_trade=openpit.param.Trade(
            price=openpit.param.Price(102),
            quantity=openpit.param.Quantity(7),
        ),
        leaves_quantity=openpit.param.Quantity(0),
        lock=openpit.pretrade.PreTradeLock(price=openpit.param.Price(102)),
        is_final=True,
    )

    assert str(fill.leaves_quantity) == "0"
    assert str(fill.lock.price) == "102"
    assert fill.last_trade is not None
    assert str(fill.last_trade.price) == "102"
    assert str(fill.last_trade.quantity) == "7"
    assert fill.is_final is True


@pytest.mark.unit
def test_fill_details_happy_path_without_leaves_quantity() -> None:
    fill = openpit.ExecutionReportFillDetails(
        lock=openpit.pretrade.PreTradeLock(price=openpit.param.Price(101)),
    )

    assert fill.leaves_quantity is None


@pytest.mark.unit
def test_fill_details_accepts_explicit_non_final_flag() -> None:
    fill = openpit.ExecutionReportFillDetails(
        leaves_quantity=openpit.param.Quantity(3),
        lock=openpit.pretrade.PreTradeLock(price=openpit.param.Price(101)),
        is_final=False,
    )

    assert fill.is_final is False


@pytest.mark.unit
def test_execution_report_operation_rejects_missing_account_id() -> None:
    with pytest.raises(TypeError):
        openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
        )  # type: ignore[call-arg]


@pytest.mark.unit
def test_execution_report_rejects_non_wrapper_fill() -> None:
    with pytest.raises(
        TypeError, match="fill must be openpit.core.ExecutionReportFillDetails"
    ):
        openpit.ExecutionReport(fill=object())  # type: ignore[arg-type]
