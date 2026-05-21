# Quickstart

This example builds an engine with payload validation, checks one order, commits
the reservation, and sends a post-trade report back to the engine.

```python
import openpit
import openpit.pretrade.policies


def send_order_to_venue(order: openpit.Order) -> None:
    # Replace this with the caller's venue or broker integration.
    _ = order


engine = (
    openpit.Engine.builder()
    .no_sync()
    .builtin(
        openpit.pretrade.policies.build_order_validation(),
    )
    .build()
)

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity("100"),
        price=openpit.param.Price("185"),
    ),
)

start_result = engine.start_pre_trade(order=order)
if not start_result:
    messages = ", ".join(
        f"{reject.policy} [{reject.code}]: {reject.reason}"
        for reject in start_result.rejects
    )
    raise RuntimeError(messages)

execute_result = start_result.request.execute()
if not execute_result:
    messages = ", ".join(
        f"{reject.policy} [{reject.code}]: {reject.reason}"
        for reject in execute_result.rejects
    )
    raise RuntimeError(messages)

reservation = execute_result.reservation
try:
    send_order_to_venue(order)
except Exception:
    reservation.rollback()
    raise
else:
    reservation.commit()

report = openpit.ExecutionReport(
    operation=openpit.ExecutionReportOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
    ),
    financial_impact=openpit.FinancialImpact(
        pnl=openpit.param.Pnl("-50"),
        fee=openpit.param.Fee("3.4"),
    ),
)

post_trade = engine.apply_execution_report(report=report)
assert not post_trade.account_blocks
```

Use `engine.execute_pre_trade(order=...)` when the caller does not need to keep
the start-stage request handle separate from main-stage execution. The returned
reservation still has to be committed or rolled back explicitly.
