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
        "AccountId",
        "Asset",
        "Side",
        "PositionSide",
        "PositionEffect",
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
def test_account_id_constructors() -> None:
    from_int = openpit.param.AccountId.from_u64(12345)
    from_string = openpit.param.AccountId.from_str("my-account")

    assert from_int.value == 12345
    assert openpit.param.AccountId.from_u64(12345).value == from_int.value
    assert openpit.param.AccountId.from_str("my-account").value == from_string.value
    assert from_int.value != from_string.value

    # FNV-1a of the empty string must equal the offset basis.
    assert openpit.param.AccountId.from_str("").value == 14_695_981_039_346_656_037

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
    side = openpit.param.Side.BUY
    position_side = openpit.param.PositionSide.LONG
    position_effect = openpit.param.PositionEffect.OPEN

    assert asset.value == "AAPL"
    assert side.value == "buy"
    assert isinstance(side, str)
    assert side.is_buy()
    assert side.opposite().value == "sell"
    assert side.sign() == 1
    assert position_side.value == "long"
    assert position_side.opposite().value == "short"
    assert position_effect.value == "open"

    with pytest.raises(ValueError, match="asset must not be empty"):
        openpit.param.Asset(" ")
    with pytest.raises(ValueError, match="expected 'buy' or 'sell'"):
        openpit.param.Side("hold")
    with pytest.raises(ValueError, match="expected 'long' or 'short'"):
        openpit.param.PositionSide("flat")
    with pytest.raises(ValueError, match="expected 'open' or 'close'"):
        openpit.param.PositionEffect("flip")


@pytest.mark.unit
def test_param_notional_helpers_are_exposed() -> None:
    quantity = openpit.param.Quantity("8")
    price = openpit.param.Price("-100")
    requested_notional = price.calculate_volume(quantity)

    assert requested_notional.value == "800"
    assert quantity.calculate_volume(price).value == "800"
    assert requested_notional.to_cash_flow_inflow().value == "800"
    assert requested_notional.to_cash_flow_outflow().value == "-800"
    assert requested_notional > openpit.param.Volume("700")
    assert requested_notional <= openpit.param.Volume("800")


@pytest.mark.unit
def test_param_constant_classes_expose_stable_names() -> None:
    assert openpit.param.ParamKind.QUANTITY == "Quantity"
    assert openpit.param.ParamKind.CASH_FLOW == "CashFlow"
    assert openpit.param.ParamKind.POSITION_SIZE == "PositionSize"
    assert openpit.param.RoundingStrategy.DEFAULT == "MidpointNearestEven"
    assert openpit.param.RoundingStrategy.BANKER == "MidpointNearestEven"
    assert openpit.param.RoundingStrategy.CONSERVATIVE_PROFIT == "Down"
    assert openpit.param.RoundingStrategy.CONSERVATIVE_LOSS == "Down"
