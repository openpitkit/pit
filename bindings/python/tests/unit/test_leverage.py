import openpit
import pytest


@pytest.mark.unit
def test_leverage_creation_and_value() -> None:
    leverage = openpit.Leverage(100)

    assert leverage.value == pytest.approx(100.0)
    assert "Leverage(" in repr(leverage)


@pytest.mark.unit
def test_leverage_rejects_zero_raw_value() -> None:
    with pytest.raises(ValueError, match="invalid leverage value"):
        openpit.Leverage(0)


@pytest.mark.unit
def test_leverage_from_u16() -> None:
    leverage = openpit.Leverage.from_u16(100)

    assert leverage.value == pytest.approx(100.0)


@pytest.mark.unit
def test_leverage_from_float_table() -> None:
    cases = [
        (1.1, 1.1),
        (100.5, 100.5),
        (2999.9, 2999.9),
    ]

    for input_value, expected in cases:
        leverage = openpit.Leverage.from_f64(input_value)
        assert leverage.value == pytest.approx(expected)


@pytest.mark.unit
def test_leverage_from_float_rejects_invalid_step_or_range_table() -> None:
    cases = [0.0, 0.9, 1.111, 3000.1]

    for input_value in cases:
        with pytest.raises(ValueError, match="invalid leverage value"):
            openpit.Leverage.from_f64(input_value)


@pytest.mark.unit
def test_leverage_margin_required() -> None:
    leverage = openpit.Leverage.from_u16(100)

    assert leverage.margin_required(notional=1000.0) == pytest.approx(10.0)


@pytest.mark.unit
def test_leverage_boundaries() -> None:
    min_leverage = openpit.Leverage.from_f64(1.0)
    max_leverage = openpit.Leverage.from_f64(3000.0)

    assert min_leverage.value == pytest.approx(1.0)
    assert max_leverage.value == pytest.approx(3000.0)


@pytest.mark.unit
def test_leverage_from_multiplier_accepts_business_max() -> None:
    leverage = openpit.Leverage.from_u16(3000)

    assert leverage.value == pytest.approx(3000.0)


@pytest.mark.unit
def test_leverage_from_multiplier_rejects_values_above_business_limit() -> None:
    with pytest.raises(ValueError, match="invalid leverage value"):
        openpit.Leverage.from_u16(3001)
