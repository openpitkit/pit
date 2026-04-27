import openpit
import pytest


@pytest.mark.unit
def test_balance_operation_construction() -> None:
    operation = openpit.AccountAdjustmentBalanceOperation(
        asset="USD",
        average_entry_price=openpit.param.Price(1),
    )

    assert operation.asset == "USD"
    assert str(operation.average_entry_price) == "1"


@pytest.mark.unit
def test_position_operation_construction() -> None:
    operation = openpit.AccountAdjustmentPositionOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        collateral_asset="USD",
        average_entry_price=openpit.param.Price(100),
        mode=openpit.param.PositionMode.HEDGED,
        leverage=openpit.param.Leverage.from_int(10),
    )

    assert operation.instrument.underlying_asset == "AAPL"
    assert operation.collateral_asset == "USD"
    assert operation.mode is openpit.param.PositionMode.HEDGED
    assert operation.leverage.value == 10.0


@pytest.mark.unit
def test_position_operation_accepts_leverage_value_object() -> None:
    operation = openpit.AccountAdjustmentPositionOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        collateral_asset="USD",
        average_entry_price=openpit.param.Price(100),
        mode=openpit.param.PositionMode.HEDGED,
        leverage=openpit.param.Leverage(10.1),
    )

    assert operation.leverage is not None
    assert operation.leverage.value == pytest.approx(10.1)


@pytest.mark.unit
def test_position_operation_accepts_plain_multiplier_int_leverage() -> None:
    operation = openpit.AccountAdjustmentPositionOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        collateral_asset="USD",
        average_entry_price=openpit.param.Price(100),
        mode=openpit.param.PositionMode.HEDGED,
        leverage=10,
    )

    assert operation.leverage is not None
    assert operation.leverage.value == pytest.approx(10.0)


@pytest.mark.unit
def test_position_operation_accepts_plain_multiplier_float_leverage() -> None:
    operation = openpit.AccountAdjustmentPositionOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        collateral_asset="USD",
        average_entry_price=openpit.param.Price(100),
        mode=openpit.param.PositionMode.HEDGED,
        leverage=10.1,
    )

    assert operation.leverage is not None
    assert operation.leverage.value == pytest.approx(10.1)


@pytest.mark.unit
def test_position_operation_rejects_bool_leverage() -> None:
    with pytest.raises(
        ValueError, match="leverage must be openpit.param.Leverage, int, or float"
    ):
        openpit.AccountAdjustmentPositionOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            collateral_asset="USD",
            average_entry_price=openpit.param.Price(100),
            mode=openpit.param.PositionMode.HEDGED,
            leverage=True,  # type: ignore[arg-type]
        )


@pytest.mark.unit
def test_position_operation_rejects_non_wrapper_instrument() -> None:
    with pytest.raises(TypeError, match="instrument must be openpit.core.Instrument"):
        openpit.AccountAdjustmentPositionOperation(
            instrument=object(),  # type: ignore[arg-type]
            collateral_asset="USD",
            average_entry_price=openpit.param.Price(100),
            mode=openpit.param.PositionMode.HEDGED,
            leverage=10,
        )


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

    assert amount.total is not None
    assert amount.total.is_absolute
    assert amount.total.as_absolute is not None
    assert str(amount.total.as_absolute) == "5"
    assert amount.reserved is None
    assert amount.pending is None


@pytest.mark.unit
def test_account_adjustment_bounds_sparse_optional_fields() -> None:
    bounds = openpit.AccountAdjustmentBounds(
        pending_lower=openpit.param.PositionSize("-2")
    )

    assert str(bounds.pending_lower) == "-2"
    assert bounds.total_upper is None
    assert bounds.reserved_upper is None


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

    assert value.is_delta
    assert value.as_delta is not None
    assert str(value.as_delta) == "-1"


@pytest.mark.unit
def test_adjustment_amount_absolute() -> None:
    value = openpit.param.AdjustmentAmount.absolute(openpit.param.PositionSize("8"))

    assert value.is_absolute
    assert value.as_absolute is not None
    assert str(value.as_absolute) == "8"


@pytest.mark.unit
def test_repr_and_basic_property_access() -> None:
    adjustment = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(
            asset="USD",
        ),
        amount=openpit.AccountAdjustmentAmount(
            pending=openpit.param.AdjustmentAmount.delta(
                openpit.param.PositionSize("1")
            )
        ),
    )

    assert "AccountAdjustment(" in repr(adjustment)
    assert adjustment.operation.asset == "USD"
    assert adjustment.amount.pending.is_delta
    assert adjustment.amount.pending.as_delta is not None
    assert str(adjustment.amount.pending.as_delta) == "1"
