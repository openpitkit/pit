import openpit
import pytest


@pytest.mark.unit
def test_leverage_creation_and_value() -> None:
    leverage = openpit.Leverage(100)
    fractional = openpit.Leverage(100.5)

    assert leverage.value == pytest.approx(100.0)
    assert fractional.value == pytest.approx(100.5)
    assert "Leverage(" in repr(leverage)


@pytest.mark.unit
def test_leverage_constructor_accepts_leverage_instance() -> None:
    source = openpit.Leverage.from_int(100)
    copied = openpit.Leverage(source)

    assert copied.value == pytest.approx(100.0)


@pytest.mark.unit
def test_leverage_rejects_zero_raw_value() -> None:
    with pytest.raises(ValueError, match="invalid leverage value"):
        openpit.Leverage(0)


@pytest.mark.unit
def test_leverage_rejects_bool_input() -> None:
    with pytest.raises(
        ValueError, match="leverage must be openpit.param.Leverage, int, or float"
    ):
        openpit.Leverage(True)


@pytest.mark.unit
def test_leverage_from_int() -> None:
    leverage = openpit.Leverage.from_int(100)

    assert leverage.value == pytest.approx(100.0)
    assert not hasattr(openpit.Leverage, "from_u16")


@pytest.mark.unit
def test_leverage_from_float_table() -> None:
    cases = [
        (1.1, 1.1),
        (100.5, 100.5),
        (2999.9, 2999.9),
    ]

    for input_value, expected in cases:
        leverage = openpit.Leverage.from_float(input_value)
        assert leverage.value == pytest.approx(expected)


@pytest.mark.unit
def test_leverage_from_float_rejects_invalid_step_or_range_table() -> None:
    cases = [0.0, 0.9, 1.111, 3000.1]

    for input_value in cases:
        with pytest.raises(ValueError, match="invalid leverage value"):
            openpit.Leverage.from_float(input_value)


@pytest.mark.unit
def test_leverage_calculate_margin_required() -> None:
    leverage = openpit.Leverage.from_int(100)
    notional = openpit.param.Notional("1000.0")
    margin = leverage.calculate_margin_required(notional=notional)

    assert isinstance(margin, openpit.param.Notional)
    assert str(margin) == "10.0"


@pytest.mark.unit
def test_leverage_boundaries() -> None:
    min_leverage = openpit.Leverage.from_float(1.0)
    max_leverage = openpit.Leverage.from_float(3000.0)

    assert min_leverage.value == pytest.approx(1.0)
    assert max_leverage.value == pytest.approx(3000.0)


@pytest.mark.unit
def test_leverage_from_multiplier_accepts_business_max() -> None:
    leverage = openpit.Leverage.from_int(3000)

    assert leverage.value == pytest.approx(3000.0)


@pytest.mark.unit
def test_leverage_from_int_rejects_invalid_values() -> None:
    for value in [-1, 0, 3001, 65536]:
        with pytest.raises(ValueError, match="invalid leverage value"):
            openpit.Leverage.from_int(value)


@pytest.mark.unit
def test_leverage_constants_are_exposed() -> None:
    assert openpit.param.Leverage.SCALE == 10
    assert openpit.param.Leverage.MIN == 1
    assert openpit.param.Leverage.MAX == 3000
