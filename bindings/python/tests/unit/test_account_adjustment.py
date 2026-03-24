import openpit
import pytest


@pytest.mark.unit
def test_balance_operation_construction() -> None:
    operation = openpit.AccountAdjustmentBalanceOperation(
        asset=openpit.param.Asset("USD"),
        average_entry_price=openpit.param.Price("1"),
    )

    assert operation.asset.value == "USD"
    assert operation.average_entry_price.value == "1"


@pytest.mark.unit
def test_position_operation_construction() -> None:
    operation = openpit.AccountAdjustmentPositionOperation(
        instrument=openpit.Instrument(
            openpit.param.Asset("BTC"),
            openpit.param.Asset("USD"),
        ),
        collateral_asset=openpit.param.Asset("USDT"),
        average_entry_price=openpit.param.Price("100"),
        mode=openpit.param.PositionMode.HEDGED,
        leverage=openpit.param.Leverage.from_u16(10),
    )

    assert operation.instrument.underlying_asset.value == "BTC"
    assert operation.collateral_asset.value == "USDT"
    assert operation.mode is openpit.param.PositionMode.HEDGED
    assert operation.leverage.value == 10.0


@pytest.mark.unit
def test_account_adjustment_optional_defaults() -> None:
    adjustment = openpit.AccountAdjustment()

    assert adjustment.operation is None
    assert adjustment.amount is None
    assert adjustment.bounds is None


@pytest.mark.unit
def test_account_adjustment_amount_sparse_optional_fields() -> None:
    amount = openpit.AccountAdjustmentAmount(
        total=openpit.param.AdjustmentAmount.absolute(openpit.param.PositionSize("5"))
    )

    assert amount.total.kind == "absolute"
    assert amount.total.value.value == "5"
    assert amount.reserved is None
    assert amount.pending is None


@pytest.mark.unit
def test_account_adjustment_bounds_sparse_optional_fields() -> None:
    bounds = openpit.AccountAdjustmentBounds(
        pending_lower_bound=openpit.param.PositionSize("-2")
    )

    assert bounds.pending_lower_bound.value == "-2"
    assert bounds.total_upper_bound is None
    assert bounds.reserved_upper_bound is None


@pytest.mark.unit
def test_wrong_operation_type_rejected() -> None:
    with pytest.raises(TypeError):
        openpit.AccountAdjustment(operation=openpit.Order())  # type: ignore[arg-type]


@pytest.mark.unit
def test_wrong_amount_type_rejected() -> None:
    with pytest.raises(TypeError):
        openpit.AccountAdjustment(amount=openpit.OrderMargin())  # type: ignore[arg-type]


@pytest.mark.unit
def test_wrong_bounds_type_rejected() -> None:
    with pytest.raises(TypeError):
        openpit.AccountAdjustment(bounds=openpit.OrderPosition())  # type: ignore[arg-type]


@pytest.mark.unit
def test_position_mode_values() -> None:
    assert openpit.param.PositionMode.NETTING.value == "netting"
    assert openpit.param.PositionMode.HEDGED.value == "hedged"


@pytest.mark.unit
def test_adjustment_amount_delta() -> None:
    value = openpit.param.AdjustmentAmount.delta(openpit.param.PositionSize("-1"))

    assert value.kind == "delta"
    assert value.value.value == "-1"


@pytest.mark.unit
def test_adjustment_amount_absolute() -> None:
    value = openpit.param.AdjustmentAmount.absolute(openpit.param.PositionSize("8"))

    assert value.kind == "absolute"
    assert value.value.value == "8"


@pytest.mark.unit
def test_repr_and_basic_property_access() -> None:
    adjustment = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(
            asset=openpit.param.Asset("USD"),
        ),
        amount=openpit.AccountAdjustmentAmount(
            pending=openpit.param.AdjustmentAmount.delta(
                openpit.param.PositionSize("1")
            )
        ),
    )

    assert "AccountAdjustment(" in repr(adjustment)
    assert adjustment.operation.asset.value == "USD"
    assert adjustment.amount.pending.value.value == "1"
