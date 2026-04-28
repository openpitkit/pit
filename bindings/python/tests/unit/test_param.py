from decimal import Decimal

import openpit
import pytest


@pytest.mark.unit
def test_param_types_are_exported_from_openpit_and_param_module() -> None:
    names = [
        "AccountId",
        "Asset",
        "Side",
        "PositionSide",
        "PositionEffect",
        "FillType",
        "Quantity",
        "Price",
        "Pnl",
        "Fee",
        "Volume",
        "CashFlow",
        "PositionSize",
        "Leverage",
        "Kind",
        "RoundingStrategy",
    ]

    for name in names:
        assert hasattr(openpit.param, name)

    root_only_param = [
        "AccountId",
        "Asset",
        "Side",
        "PositionSide",
        "PositionEffect",
        "FillType",
        "Quantity",
        "Price",
        "Pnl",
        "Fee",
        "Volume",
        "CashFlow",
        "PositionSize",
        "Kind",
        "RoundingStrategy",
    ]
    for name in root_only_param:
        assert not hasattr(openpit, name)

    assert hasattr(openpit, "Leverage")


@pytest.mark.unit
def test_account_id_constructors() -> None:
    from_int = openpit.param.AccountId.from_u64(12345)
    from_string = openpit.param.AccountId.from_str("my-account")

    assert from_int.value == 12345
    assert openpit.param.AccountId.from_u64(12345).value == from_int.value
    assert openpit.param.AccountId.from_str("my-account").value == from_string.value
    assert from_int.value != from_string.value

    with pytest.raises(ValueError, match="account id string must not be empty"):
        openpit.param.AccountId.from_str("")
    with pytest.raises(ValueError, match="account id string must not be empty"):
        openpit.param.AccountId.from_str("   ")

    # from_u64 and from_str of the same numeric string are NOT equal.
    assert (
        openpit.param.AccountId.from_u64(42).value
        != openpit.param.AccountId.from_str("42").value
    )


@pytest.mark.unit
def test_param_types_have_rust_like_module_paths() -> None:
    assert openpit.param.AccountId.__module__ == "openpit.param"
    assert openpit.param.Side.__module__ == "openpit.param"
    assert openpit.param.PositionSide.__module__ == "openpit.param"
    assert openpit.param.PositionEffect.__module__ == "openpit.param"
    assert openpit.param.FillType.__module__ == "openpit.param"
    assert openpit.param.Quantity.__module__ == "openpit.param"
    assert openpit.param.Price.__module__ == "openpit.param"
    assert openpit.param.Pnl.__module__ == "openpit.param"
    assert openpit.param.Fee.__module__ == "openpit.param"
    assert openpit.param.Volume.__module__ == "openpit.param"
    assert openpit.param.CashFlow.__module__ == "openpit.param"
    assert openpit.param.PositionSize.__module__ == "openpit.param"
    assert openpit.Leverage.__module__ == "openpit.param"
    assert openpit.param.Kind.__module__ == "openpit.param"
    assert openpit.param.RoundingStrategy.__module__ == "openpit.param"


@pytest.mark.unit
def test_param_numeric_wrappers_accept_and_validate_values() -> None:
    assert str(openpit.param.Quantity("10.5")) == "10.5"
    assert str(openpit.param.Price(123)) == "123"
    assert str(openpit.param.Pnl(-1.25)) == "-1.25"
    assert str(openpit.param.Fee("0.01")) == "0.01"
    assert str(openpit.param.Volume(42)) == "42"
    assert str(openpit.param.CashFlow("-99.9")) == "-99.9"
    assert str(openpit.param.PositionSize("-7")) == "-7"

    with pytest.raises(ValueError, match="value must be non-negative for Quantity"):
        openpit.param.Quantity("-1")
    with pytest.raises(ValueError, match="value must be non-negative for Volume"):
        openpit.param.Volume("-1")


@pytest.mark.unit
def test_trade_amount_quantity_factory_accepts_all_supported_inputs() -> None:
    from_quantity = openpit.param.TradeAmount.quantity(openpit.param.Quantity("10.5"))
    from_str = openpit.param.TradeAmount.quantity("11.5")
    from_int = openpit.param.TradeAmount.quantity(12)
    from_float = openpit.param.TradeAmount.quantity(13.25)

    assert from_quantity.is_quantity
    assert from_quantity.as_quantity is not None
    assert str(from_quantity.as_quantity) == "10.5"

    assert from_str.is_quantity
    assert from_str.as_quantity is not None
    assert str(from_str.as_quantity) == "11.5"

    assert from_int.is_quantity
    assert from_int.as_quantity is not None
    assert str(from_int.as_quantity) == "12"

    assert from_float.is_quantity
    assert from_float.as_quantity is not None
    assert str(from_float.as_quantity) == "13.25"

    with pytest.raises(
        ValueError, match="Quantity must be a Decimal, str, int, or float"
    ):
        openpit.param.TradeAmount.quantity(True)  # type: ignore[arg-type]


@pytest.mark.unit
def test_trade_amount_volume_factory_accepts_all_supported_inputs() -> None:
    from_volume = openpit.param.TradeAmount.volume(openpit.param.Volume("100.5"))
    from_str = openpit.param.TradeAmount.volume("101.5")
    from_int = openpit.param.TradeAmount.volume(102)
    from_float = openpit.param.TradeAmount.volume(103.25)

    assert from_volume.is_volume
    assert from_volume.as_volume is not None
    assert str(from_volume.as_volume) == "100.5"

    assert from_str.is_volume
    assert from_str.as_volume is not None
    assert str(from_str.as_volume) == "101.5"

    assert from_int.is_volume
    assert from_int.as_volume is not None
    assert str(from_int.as_volume) == "102"

    assert from_float.is_volume
    assert from_float.as_volume is not None
    assert str(from_float.as_volume) == "103.25"

    with pytest.raises(
        ValueError, match="Volume must be a Decimal, str, int, or float"
    ):
        openpit.param.TradeAmount.volume(True)  # type: ignore[arg-type]


@pytest.mark.unit
def test_param_directional_and_identifier_wrappers() -> None:
    asset = openpit.param.Asset("AAPL")
    side = openpit.param.Side.BUY
    position_side = openpit.param.PositionSide.LONG
    position_effect = openpit.param.PositionEffect.OPEN
    fill_type = openpit.param.FillType.TRADE

    assert asset == "AAPL"
    assert isinstance(asset, openpit.param.Asset)
    assert side.value == "buy"
    assert isinstance(side, str)
    assert side.is_buy()
    assert side.opposite().value == "sell"
    assert side.sign() == 1
    assert position_side.value == "long"
    assert position_side.opposite().value == "short"
    assert position_effect.value == "open"
    assert fill_type.value == "TRADE"

    with pytest.raises(ValueError, match="expected 'buy' or 'sell'"):
        openpit.param.Side("hold")
    with pytest.raises(ValueError, match="expected 'long' or 'short'"):
        openpit.param.PositionSide("flat")
    with pytest.raises(ValueError, match="expected 'open' or 'close'"):
        openpit.param.PositionEffect("flip")
    with pytest.raises(
        ValueError,
        match=(
            "expected 'TRADE', 'LIQUIDATION', 'AUTO_DELEVERAGE', 'SETTLEMENT'"
            ", or 'FUNDING'"
        ),
    ):
        openpit.param.FillType("manual")


@pytest.mark.unit
def test_asset_rejects_empty_or_whitespace() -> None:
    with pytest.raises(openpit.param.AssetError, match="asset must not be empty"):
        openpit.param.Asset("")
    with pytest.raises(openpit.param.AssetError, match="asset must not be empty"):
        openpit.param.Asset("  ")


@pytest.mark.unit
def test_param_notional_helpers_are_exposed() -> None:
    quantity = openpit.param.Quantity("8")
    price = openpit.param.Price("-100")
    requested_notional = price.calculate_volume(quantity)

    assert str(requested_notional) == "800"
    assert str(quantity.calculate_volume(price)) == "800"
    assert str(requested_notional.to_cash_flow_inflow()) == "800"
    assert str(requested_notional.to_cash_flow_outflow()) == "-800"
    assert requested_notional > openpit.param.Volume("700")
    assert requested_notional <= openpit.param.Volume("800")


@pytest.mark.unit
def test_param_numeric_wrappers_accept_decimal() -> None:
    assert str(openpit.param.Quantity(Decimal("10.5"))) == "10.5"
    assert str(openpit.param.Price(Decimal("185"))) == "185"
    assert str(openpit.param.Pnl(Decimal("-1.25"))) == "-1.25"
    assert str(openpit.param.Fee(Decimal("0.01"))) == "0.01"
    assert str(openpit.param.Volume(Decimal("42"))) == "42"
    assert str(openpit.param.CashFlow(Decimal("-99.9"))) == "-99.9"
    assert str(openpit.param.PositionSize(Decimal("-7"))) == "-7"


@pytest.mark.unit
def test_param_decimal_property_returns_decimal() -> None:
    quantity = openpit.param.Quantity("10.5")
    assert quantity.decimal == Decimal("10.5")
    assert isinstance(quantity.decimal, Decimal)


@pytest.mark.unit
def test_param_same_type_arithmetic() -> None:
    price_1 = openpit.param.Price("100")
    price_2 = openpit.param.Price("50")

    result = price_1 + price_2
    assert isinstance(result, openpit.param.Price)
    assert str(result) == "150"

    result = price_1 - price_2
    assert isinstance(result, openpit.param.Price)
    assert str(result) == "50"

    result = price_1 * 3
    assert isinstance(result, openpit.param.Price)
    assert str(result) == "300"

    result = 3 * price_1
    assert isinstance(result, openpit.param.Price)
    assert str(result) == "300"


@pytest.mark.unit
def test_param_cross_type_arithmetic_rejected() -> None:
    price = openpit.param.Price("100")
    quantity = openpit.param.Quantity("10")
    with pytest.raises(TypeError):
        price + quantity  # type: ignore[operator]
    with pytest.raises(TypeError):
        price - quantity  # type: ignore[operator]


@pytest.mark.unit
def test_param_division_rules() -> None:
    price_1 = openpit.param.Price("100")
    price_2 = openpit.param.Price("25")

    with pytest.raises(TypeError):
        _ = price_1 / price_2  # type: ignore[operator]

    result = price_1 / 4
    assert isinstance(result, openpit.param.Price)
    assert str(result) == "25"


@pytest.mark.unit
def test_param_comparison_operators() -> None:
    price_1 = openpit.param.Price("100")
    price_2 = openpit.param.Price("200")
    price_3 = openpit.param.Price("100")

    assert price_1 < price_2
    assert price_2 > price_1
    assert price_1 <= price_3
    assert price_1 >= price_3
    assert price_1 == price_3
    assert price_1 != price_2


@pytest.mark.unit
def test_param_hash() -> None:
    price_1 = openpit.param.Price("100")
    price_2 = openpit.param.Price("100")
    assert hash(price_1) == hash(price_2)

    values = {price_1, price_2}
    assert len(values) == 1


@pytest.mark.unit
def test_param_signed_neg_abs() -> None:
    pnl = openpit.param.Pnl("-50")
    assert str(-pnl) == "50"
    assert str(abs(pnl)) == "50"


@pytest.mark.unit
def test_param_unsigned_no_neg() -> None:
    quantity = openpit.param.Quantity("10")
    with pytest.raises(TypeError):
        _ = -quantity  # type: ignore[operator]


@pytest.mark.unit
def test_param_bool() -> None:
    assert bool(openpit.param.Price("100"))
    assert not bool(openpit.param.Price.ZERO)


@pytest.mark.unit
def test_param_zero_constant() -> None:
    assert str(openpit.param.Price.ZERO) == "0"
    assert str(openpit.param.Quantity.ZERO) == "0"
    assert str(openpit.param.Volume.ZERO) == "0"
    assert str(openpit.param.Pnl.ZERO) == "0"
    assert str(openpit.param.Fee.ZERO) == "0"
    assert str(openpit.param.CashFlow.ZERO) == "0"
    assert str(openpit.param.PositionSize.ZERO) == "0"


@pytest.mark.unit
def test_param_repr_format() -> None:
    price = openpit.param.Price("185.5")
    assert repr(price) == "Price(Decimal('185.5'))"


@pytest.mark.unit
def test_param_to_json_value() -> None:
    price = openpit.param.Price("185.500")
    assert price.to_json_value() == "185.500"
    assert isinstance(price.to_json_value(), str)


@pytest.mark.unit
def test_param_float_constructor_avoids_precision_artifacts() -> None:
    price = openpit.param.Price(0.1)
    assert str(price) == "0.1"


@pytest.mark.unit
def test_param_domain_conversion_methods() -> None:
    quantity = openpit.param.Quantity("8")
    price = openpit.param.Price("-100")
    volume = price.calculate_volume(quantity)
    assert str(volume) == "800"

    assert str(volume.to_cash_flow_inflow()) == "800"
    assert str(volume.to_cash_flow_outflow()) == "-800"

    pnl = openpit.param.Pnl("-50")
    cash_flow = pnl.to_cash_flow()
    assert isinstance(cash_flow, openpit.param.CashFlow)
    assert str(cash_flow) == "-50"

    position_size = pnl.to_position_size()
    assert isinstance(position_size, openpit.param.PositionSize)
    assert str(position_size) == "-50"

    fee = openpit.param.Fee("3.18")
    pnl_from_fee = fee.to_pnl()
    assert isinstance(pnl_from_fee, openpit.param.Pnl)
    assert str(pnl_from_fee) == "-3.18"

    position_size_from_fee = fee.to_position_size()
    assert isinstance(position_size_from_fee, openpit.param.PositionSize)
    assert str(position_size_from_fee) == "-3.18"

    cash_flow_from_pnl = openpit.param.CashFlow.from_pnl(openpit.param.Pnl("1.25"))
    assert str(cash_flow_from_pnl) == "1.25"

    cash_flow_from_fee = openpit.param.CashFlow.from_fee(openpit.param.Fee("1.25"))
    assert str(cash_flow_from_fee) == "-1.25"

    volume_2 = openpit.param.Volume("6352.6125")
    price_2 = openpit.param.Price("42350.75")
    quantity_2 = volume_2.calculate_quantity(price_2)
    assert isinstance(quantity_2, openpit.param.Quantity)
    assert str(quantity_2) == "0.15"


@pytest.mark.unit
def test_param_position_size_domain_methods() -> None:
    position_size = openpit.param.PositionSize.from_quantity_and_side(
        openpit.param.Quantity("2"), "buy"
    )
    assert str(position_size) == "2"

    position_size_sell = openpit.param.PositionSize.from_quantity_and_side(
        openpit.param.Quantity("2"), "sell"
    )
    assert str(position_size_sell) == "-2"

    quantity, side = position_size.to_open_quantity()
    assert str(quantity) == "2"
    assert side == "buy"

    quantity, side = position_size_sell.to_close_quantity()
    assert str(quantity) == "2"
    assert side == "buy"

    result = position_size.checked_add_quantity(openpit.param.Quantity("1"), "sell")
    assert str(result) == "1"


@pytest.mark.unit
def test_param_constant_classes_expose_stable_names() -> None:
    assert openpit.param.Kind.QUANTITY == "Quantity"
    assert openpit.param.Kind.CASH_FLOW == "CashFlow"
    assert openpit.param.Kind.POSITION_SIZE == "PositionSize"
    assert openpit.param.RoundingStrategy.DEFAULT == "MidpointNearestEven"
    assert openpit.param.RoundingStrategy.BANKER == "MidpointNearestEven"
    assert openpit.param.RoundingStrategy.CONSERVATIVE_PROFIT == "Down"
    assert openpit.param.RoundingStrategy.CONSERVATIVE_LOSS == "Down"
