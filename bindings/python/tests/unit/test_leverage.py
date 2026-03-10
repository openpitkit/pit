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
def test_leverage_from_multiplier() -> None:
    leverage = openpit.Leverage.from_multiplier(100)

    assert leverage.value == pytest.approx(100.0)


@pytest.mark.unit
def test_leverage_margin_required() -> None:
    leverage = openpit.Leverage.from_multiplier(100)

    assert leverage.margin_required(notional=1000.0) == pytest.approx(10.0)


@pytest.mark.unit
def test_leverage_boundaries() -> None:
    min_leverage = openpit.Leverage.from_stored(1)
    max_leverage = openpit.Leverage.from_stored(65_535)

    assert min_leverage.value == pytest.approx(0.01)
    assert max_leverage.value == pytest.approx(655.35)


@pytest.mark.unit
def test_leverage_from_stored_rejects_zero() -> None:
    with pytest.raises(ValueError, match="invalid leverage value"):
        openpit.Leverage.from_stored(0)


@pytest.mark.unit
def test_leverage_from_multiplier_reports_overflow() -> None:
    with pytest.raises(ValueError, match="overflow"):
        openpit.Leverage.from_multiplier(656)
