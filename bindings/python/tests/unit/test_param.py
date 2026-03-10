import openpit
import pytest


@pytest.mark.unit
def test_param_types_are_exported_from_openpit_and_param_module() -> None:
    names = [
        "Asset",
        "Side",
        "PositionSide",
        "Quantity",
        "Price",
        "Pnl",
        "Fee",
        "Volume",
        "CashFlow",
        "PositionSize",
        "Leverage",
        "ParamKind",
        "RoundingStrategy",
    ]

    for name in names:
        assert hasattr(openpit.param, name)

    root_only_param = [
        "Asset",
        "Side",
        "PositionSide",
        "Quantity",
        "Price",
        "Pnl",
        "Fee",
        "Volume",
        "CashFlow",
        "PositionSize",
        "ParamKind",
        "RoundingStrategy",
    ]
    for name in root_only_param:
        assert not hasattr(openpit, name)

    assert hasattr(openpit, "Leverage")


@pytest.mark.unit
def test_param_types_have_rust_like_module_paths() -> None:
    assert openpit.param.Asset.__module__ == "openpit.param"
    assert openpit.param.Side.__module__ == "openpit.param"
    assert openpit.param.PositionSide.__module__ == "openpit.param"
    assert openpit.param.Quantity.__module__ == "openpit.param"
    assert openpit.param.Price.__module__ == "openpit.param"
    assert openpit.param.Pnl.__module__ == "openpit.param"
    assert openpit.param.Fee.__module__ == "openpit.param"
    assert openpit.param.Volume.__module__ == "openpit.param"
    assert openpit.param.CashFlow.__module__ == "openpit.param"
    assert openpit.param.PositionSize.__module__ == "openpit.param"
    assert openpit.Leverage.__module__ == "openpit.param"
    assert openpit.param.ParamKind.__module__ == "openpit.param"
    assert openpit.param.RoundingStrategy.__module__ == "openpit.param"


@pytest.mark.unit
def test_param_numeric_wrappers_accept_and_validate_values() -> None:
    assert openpit.param.Quantity("10.5").value == "10.5"
    assert openpit.param.Price(123).value == "123"
    assert openpit.param.Pnl(-1.25).value == "-1.25"
    assert openpit.param.Fee("0.01").value == "0.01"
    assert openpit.param.Volume(42).value == "42"
    assert openpit.param.CashFlow("-99.9").value == "-99.9"
    assert openpit.param.PositionSize("-7").value == "-7"

    with pytest.raises(ValueError, match="value must be non-negative for Quantity"):
        openpit.param.Quantity("-1")
    with pytest.raises(ValueError, match="value must be non-negative for Volume"):
        openpit.param.Volume("-1")


@pytest.mark.unit
def test_param_directional_and_identifier_wrappers() -> None:
    asset = openpit.param.Asset("AAPL")
    side = openpit.param.Side("buy")
    position_side = openpit.param.PositionSide("long")

    assert asset.value == "AAPL"
    assert side.value == "buy"
    assert side.is_buy()
    assert side.opposite().value == "sell"
    assert side.sign() == 1
    assert position_side.value == "long"
    assert position_side.opposite().value == "short"

    with pytest.raises(ValueError, match="asset must not be empty"):
        openpit.param.Asset(" ")
    with pytest.raises(ValueError, match="expected 'buy' or 'sell'"):
        openpit.param.Side("hold")
    with pytest.raises(ValueError, match="expected 'long' or 'short'"):
        openpit.param.PositionSide("flat")


@pytest.mark.unit
def test_param_constant_classes_expose_stable_names() -> None:
    assert openpit.param.ParamKind.QUANTITY == "Quantity"
    assert openpit.param.ParamKind.CASH_FLOW == "CashFlow"
    assert openpit.param.ParamKind.POSITION_SIZE == "PositionSize"
    assert openpit.param.RoundingStrategy.DEFAULT == "MidpointNearestEven"
    assert openpit.param.RoundingStrategy.BANKER == "MidpointNearestEven"
    assert openpit.param.RoundingStrategy.CONSERVATIVE_PROFIT == "Down"
    assert openpit.param.RoundingStrategy.CONSERVATIVE_LOSS == "Down"
