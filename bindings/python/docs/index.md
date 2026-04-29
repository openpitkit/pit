# OpenPit Python documentation

OpenPit is an embeddable pre-trade risk SDK for Python applications that need
policy-driven checks before an order leaves the process. It is designed for
trading systems, broker gateways, strategy runtimes, and test harnesses that
want deterministic order admission logic close to the caller.

The Python package is named `openpit`. It exposes a single-threaded risk engine,
strong domain value types, order and execution-report models, built-in policies,
and Python policy interfaces for project-specific checks.

## Main use case

Use OpenPit when an application needs to decide whether an order can be sent to
a downstream venue or broker adapter. A typical flow is:

1. Build an `openpit.Engine` during application startup.
2. Register built-in and custom policies.
3. Submit each `openpit.Order` to the start stage.
4. Execute the deferred main stage when the caller is ready.
5. Commit or roll back the returned reservation.
6. Feed `openpit.ExecutionReport` objects back into the engine.

This model keeps pre-trade decisions explicit: policy rejects are returned as
business results, while invalid API usage remains an exception.

## Key features

- Two-stage pre-trade pipeline with explicit request and reservation handles.
- Built-in order validation, rate limit, P&L kill switch, and size-limit
  policies.
- Python interfaces for start-stage, main-stage, and account-adjustment
  policies.
- Exact decimal-backed domain value types for prices, quantities, P&L, fees,
  cash flow, and position sizes.
- Extensible Python order and execution-report models for integration-specific
  metadata.
- Atomic account-adjustment batches with policy rejects and rollback mutations.
- Sphinx-generated API reference from the public Python layer.

## Minimal quickstart

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
        trade_amount=openpit.param.TradeAmount.quantity("100"),
        price=openpit.param.Price("185"),
    ),
)

result = engine.execute_pre_trade(order=order)
if not result:
    raise RuntimeError(result.rejects[0].reason)

try:
    # Send the order to the venue here.
    result.reservation.commit()
except Exception:
    result.reservation.rollback()
    raise
```

```{toctree}
:maxdepth: 2
:caption: Getting started

installation
quickstart
concepts
```

```{toctree}
:maxdepth: 2
:caption: User guide

guides/engine
guides/policies
guides/domain-types
guides/account-adjustments
guides/errors
guides/threading
```

```{toctree}
:maxdepth: 2
:caption: Examples

examples/index
```

```{toctree}
:maxdepth: 2
:caption: API reference

api/index
```
