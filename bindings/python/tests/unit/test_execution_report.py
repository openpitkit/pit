import openpit
import pytest


@pytest.mark.unit
def test_execution_report_exposes_fields_and_optional_defaults() -> None:
    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl("-5"),
            fee=openpit.param.Fee("0"),
        ),
    )

    assert report.operation.instrument.underlying_asset.value == "AAPL"
    assert report.operation.instrument.settlement_asset.value == "USD"
    assert report.operation.side is openpit.param.Side.BUY
    assert report.financial_impact.pnl.value == "-5"
    assert report.financial_impact.fee.value == "0"
    assert report.fill is None
    assert "ExecutionReport(" in repr(report)


@pytest.mark.unit
def test_execution_report_operation_rejects_invalid_asset() -> None:
    with pytest.raises(ValueError):
        openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset(""),
            ),
            side=openpit.param.Side.BUY,
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
