# Domain types

OpenPit uses typed value objects instead of raw numbers for financial fields.
The goal is to make units visible at the Python boundary and to reject accidental
cross-type arithmetic.

## Numeric value objects

Common numeric types include:

- `Quantity`: unsigned instrument amount.
- `Price`: per-unit price.
- `Volume`: unsigned notional amount.
- `Pnl`: signed profit and loss.
- `Fee`: execution fee or rebate.
- `CashFlow`: signed settlement-currency flow.
- `PositionSize`: signed position amount.
- `Leverage`: leverage multiplier.

Construct values from `str` or `decimal.Decimal` at integration boundaries:

```python
from decimal import Decimal

import openpit

quantity = openpit.param.Quantity("10.5")
price = openpit.param.Price(Decimal("185"))
volume = price.calculate_volume(quantity)

assert isinstance(volume, openpit.param.Volume)
assert volume.to_json_value() == "1942.5"
```

`int` and `float` constructors are accepted. Prefer `str` or `Decimal` for
external monetary inputs when exact textual representation matters.

## Directional enums

```python
import openpit

side = openpit.param.Side.BUY
assert side.is_buy()
assert side.opposite() is openpit.param.Side.SELL
assert side.sign() == 1

position_side = openpit.param.PositionSide.LONG
assert position_side.opposite() is openpit.param.PositionSide.SHORT
```

## Account identifiers

Use exactly one account-id source model per runtime:

- `AccountId.from_u64(...)` for numeric IDs assigned by the caller.
- `AccountId.from_str(...)` when only string IDs are available.

Do not mix both models in one runtime state. A hashed string-derived ID can equal
a direct numeric ID.
