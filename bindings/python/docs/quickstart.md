# Quickstart

```python
import openpit

engine = (
    openpit.Engine.builder()
    .check_pre_trade_start_policy(
        policy=openpit.pretrade.policies.OrderValidationPolicy(),
    )
    .build()
)

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity(100.0),
        price=openpit.param.Price(185.0),
    ),
)

result = engine.start_pre_trade(order=order)
```
