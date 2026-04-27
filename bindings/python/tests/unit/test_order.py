import openpit
import pytest

_ACCOUNT_ID = openpit.param.AccountId.from_u64(99224416)


@pytest.mark.unit
def test_order_operation_accepts_keyword_arguments_and_numeric_variants() -> None:
    op = openpit.OrderOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity(10.5),
        account_id=_ACCOUNT_ID,
        price=openpit.param.Price(185.0),
    )
    order = openpit.Order(operation=op)

    assert order.operation.instrument.underlying_asset == "AAPL"
    assert order.operation.instrument.settlement_asset == "USD"
    assert order.operation.side is openpit.param.Side.BUY
    assert str(order.operation.trade_amount.as_quantity) == "10.5"
    assert str(order.operation.price) == "185"
    assert "Order(" in repr(order)


@pytest.mark.unit
def test_order_uses_optional_defaults() -> None:
    order = openpit.Order()

    assert order.operation is None
    assert order.position is None
    assert order.margin is None


@pytest.mark.unit
def test_order_operation_rejects_invalid_side() -> None:
    with pytest.raises(TypeError):
        openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side="hold",  # type: ignore[arg-type]
            trade_amount=openpit.param.TradeAmount.quantity(1),
            account_id=_ACCOUNT_ID,
            price=openpit.param.Price(10),
        )


@pytest.mark.unit
def test_order_operation_rejects_bool_quantity() -> None:
    with pytest.raises(TypeError):
        openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            trade_amount=True,  # type: ignore[arg-type]
            account_id=_ACCOUNT_ID,
            price=openpit.param.Price(10),
        )


@pytest.mark.unit
def test_order_operation_rejects_quantity_without_trade_amount_wrapper() -> None:
    with pytest.raises(TypeError):
        openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Quantity(1),  # type: ignore[arg-type]
            account_id=_ACCOUNT_ID,
            price=openpit.param.Price(10),
        )


@pytest.mark.unit
def test_order_operation_rejects_volume_without_trade_amount_wrapper() -> None:
    with pytest.raises(TypeError):
        openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Volume(1),  # type: ignore[arg-type]
            account_id=_ACCOUNT_ID,
            price=openpit.param.Price(10),
        )


@pytest.mark.unit
def test_order_operation_accepts_volume_without_price() -> None:
    op = openpit.OrderOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.volume(250),
        account_id=_ACCOUNT_ID,
        price=None,
    )
    order = openpit.Order(operation=op)

    assert str(order.operation.trade_amount.as_volume) == "250"
    assert order.operation.price is None


@pytest.mark.unit
def test_order_rejects_positional_arguments_for_keyword_only_constructor() -> None:
    with pytest.raises(TypeError):
        openpit.Order("operation")  # type: ignore[call-arg]


@pytest.mark.unit
def test_instrument_rejects_empty_settlement_asset() -> None:
    with pytest.raises(ValueError):
        openpit.Instrument(
            "AAPL",
            "",
        )


@pytest.mark.unit
def test_instrument_accepts_string_assets() -> None:
    instrument = openpit.Instrument("AAPL", "USD")

    assert instrument.underlying_asset == "AAPL"
    assert instrument.settlement_asset == "USD"


@pytest.mark.unit
def test_order_margin_fields_are_optional() -> None:
    margin = openpit.OrderMargin()
    order = openpit.Order(margin=margin)

    assert order.margin.leverage is None
    assert order.margin.collateral_asset is None
    assert order.margin.auto_borrow is False


@pytest.mark.unit
def test_order_margin_accepts_leverage_value_object() -> None:
    margin = openpit.OrderMargin(leverage=openpit.param.Leverage(10.1))

    assert margin.leverage is not None
    assert margin.leverage.value == pytest.approx(10.1)


@pytest.mark.unit
def test_order_margin_accepts_plain_multiplier_int_leverage() -> None:
    margin = openpit.OrderMargin(leverage=10)

    assert margin.leverage is not None
    assert margin.leverage.value == pytest.approx(10.0)


@pytest.mark.unit
def test_order_margin_accepts_plain_multiplier_float_leverage() -> None:
    margin = openpit.OrderMargin(leverage=10.1)

    assert margin.leverage is not None
    assert margin.leverage.value == pytest.approx(10.1)


@pytest.mark.unit
def test_order_margin_rejects_bool_leverage() -> None:
    with pytest.raises(
        ValueError, match="leverage must be openpit.param.Leverage, int, or float"
    ):
        openpit.OrderMargin(leverage=True)  # type: ignore[arg-type]


@pytest.mark.unit
def test_order_position_bool_flags_default_to_false() -> None:
    position = openpit.OrderPosition()
    order = openpit.Order(position=position)

    assert order.position.reduce_only is False
    assert order.position.close_position is False


@pytest.mark.unit
def test_order_operation_accepts_account_id_from_u64() -> None:
    op = openpit.OrderOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity(1),
        account_id=openpit.param.AccountId.from_u64(99224416),
    )
    assert op.account_id.value == 99224416


@pytest.mark.unit
def test_order_operation_accepts_account_id_from_str() -> None:
    op = openpit.OrderOperation(
        instrument=openpit.Instrument(
            "AAPL",
            "USD",
        ),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity(1),
        account_id=openpit.param.AccountId.from_str("my-account"),
    )
    assert op.account_id.value == openpit.param.AccountId.from_str("my-account").value


@pytest.mark.unit
def test_order_operation_rejects_raw_account_id_int() -> None:
    with pytest.raises(TypeError):
        openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(1),
            account_id=99224416,  # type: ignore[arg-type]
        )


@pytest.mark.unit
def test_order_operation_rejects_raw_account_id_str() -> None:
    with pytest.raises(TypeError):
        openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(1),
            account_id="my-account",  # type: ignore[arg-type]
        )


@pytest.mark.unit
def test_order_operation_rejects_missing_account_id() -> None:
    with pytest.raises(TypeError):
        openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(1),
        )  # type: ignore[call-arg]


@pytest.mark.unit
def test_order_operation_rejects_non_wrapper_instrument() -> None:
    with pytest.raises(TypeError, match="instrument must be openpit.core.Instrument"):
        openpit.OrderOperation(
            instrument=object(),  # type: ignore[arg-type]
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(1),
            account_id=_ACCOUNT_ID,
        )


@pytest.mark.unit
def test_order_rejects_non_wrapper_operation() -> None:
    with pytest.raises(
        TypeError, match="operation must be openpit.core.OrderOperation"
    ):
        openpit.Order(operation=object())  # type: ignore[arg-type]
