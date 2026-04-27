# Pit (Pre-trade Integrity Toolkit) for Python

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![Python versions](https://img.shields.io/pypi/pyversions/openpit)](https://pypi.org/project/openpit/) [![PyPI](https://img.shields.io/pypi/v/openpit)](https://pypi.org/project/openpit/) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](../../LICENSE)
<!-- markdownlint-enable MD013 -->

`openpit` is an embeddable pre-trade risk SDK for integrating policy-driven
risk checks into trading systems from Python.

For an overview and links to all resources, see
the project website [openpit.dev](https://openpit.dev/).
For full project documentation, see
[the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see
[the project wiki](https://github.com/openpitkit/pit/wiki).

## Versioning Policy (Pre‑1.0)

Until Pit reaches a stable `1.0` release, the project follows a relaxed
interpretation of Semantic Versioning.

During this phase:

- `PATCH` releases are used for bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the public
  interface**.

This means that breaking API changes can appear in minor releases before `1.0`.
Consumers of the library should take this into account when declaring
dependencies and consider using version constraints that tolerate API
evolution during the pre‑stable phase.

## Getting Started

Visit the [PyPI package](https://pypi.org/project/openpit/).

## Install

For normal end-user installation, use the published
[PyPI package](https://pypi.org/project/openpit/):

```bash
pip install openpit
```

If you need local development/debugging, clone this repository and build from
source with [Maturin](https://github.com/PyO3/maturin):

```bash
maturin develop --manifest-path bindings/python/Cargo.toml
```

Local release build:

```bash
maturin develop --release --manifest-path bindings/python/Cargo.toml
```

## Engine

### Overview

The engine evaluates an order through a deterministic pre-trade pipeline:

- `engine.start_pre_trade(order=...)` runs start-stage policies and makes
  lightweight check policies
- `request.execute()` runs main-stage check policies
- `reservation.commit()` applies reserved state
- `reservation.rollback()` reverts reserved state
- `engine.apply_execution_report(report=...)` updates post-trade policy state

Start-stage policies stop on the first reject. Main-stage policies aggregate
rejects and run rollback mutations in reverse order when any reject is produced.

Built-in policies currently include:

- `OrderValidationPolicy`
- `PnlKillSwitchPolicy`
- `RateLimitPolicy`
- `OrderSizeLimitPolicy`

The primary integration model is to write project-specific policies against the
public Python policy API described in the manual: [Custom Python policies](https://github.com/openpitkit/pit/wiki/Policies#python-custom-policy-api).

There are two types of rejections: a full kill switch for the account and a
rejection of only the current request. This is useful in algorithmic trading
when automatic order submission must be halted until the situation is analyzed.

## Threading

Canonical contract: [Threading Contract](https://github.com/openpitkit/pit/wiki/Threading-Contract).

The Python binding follows the same SDK threading contract.
Public methods acquire the GIL when needed; the SDK does not release the GIL
across callback boundaries, so Python policies execute on the calling thread.

## Usage

```python
import openpit

# 1. Configure policies.
pnl_policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
    settlement_asset="USD",
    barrier=openpit.param.Pnl("1000"),
)

rate_limit_policy = openpit.pretrade.policies.RateLimitPolicy(
    max_orders=100,
    window_seconds=1,
)

order_size_policy = openpit.pretrade.policies.OrderSizeLimitPolicy(
    limit=openpit.pretrade.policies.OrderSizeLimit(
        settlement_asset="USD",
        max_quantity=openpit.param.Quantity("500"),
        max_notional=openpit.param.Volume("100000"),
    ),
)

# 2. Build the engine (one time at the platform initialization).
engine = (
    openpit.Engine.builder()
    .check_pre_trade_start_policy(
        policy=openpit.pretrade.policies.OrderValidationPolicy(),
    )
    .check_pre_trade_start_policy(policy=pnl_policy)
    .check_pre_trade_start_policy(policy=rate_limit_policy)
    .check_pre_trade_start_policy(policy=order_size_policy)
    .build()
)

# 3. Check an order.
order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity(100.0),
        price=openpit.param.Price(185.0),
    ),
)

start_result = engine.start_pre_trade(order=order)

if not start_result:
    messages = ", ".join(
        f"{r.policy} [{r.code}]: {r.reason}: {r.details}"
        for r in start_result.rejects
    )
    raise RuntimeError(messages)

request = start_result.request

# 4. Quick, lightweight checks, such as fat-finger scope or enabled kill
# switch, were performed during pre-trade request creation. The system state
# has not yet changed, except in cases where each request, even rejected ones,
# must be considered. Before the heavy-duty checks, other work on the request
# can be performed simply by holding the request object.

# 5. Real pre-trade and risk control.
execute_result = request.execute()

# Optional shortcut for the same two-stage flow:
# execute_result = engine.execute_pre_trade(order=order)

if not execute_result:
    messages = ", ".join(
        f"{reject.policy} [{reject.code}]: {reject.reason}: {reject.details}"
        for reject in execute_result.rejects
    )
    raise RuntimeError(messages)

reservation = execute_result.reservation

# 6. If the request is successfully sent to the venue, it must be committed.
# The rollback must be called otherwise to revert all performed reservations.
try:
    send_order_to_venue(order)
except Exception:
    reservation.rollback()
    raise

reservation.commit()

# 7. The order goes to the venue and returns with an execution report.
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

result = engine.apply_execution_report(report=report)

# 8. After each execution report is applied, the system may report that it has
# been determined in advance that all subsequent requests will be rejected if
# the account status does not change.
assert result.kill_switch_triggered is False
```

## Errors

Policy rejects from `engine.start_pre_trade()` and `request.execute()` are
returned as `StartPreTradeResult` and `ExecuteResult`.

Input validation errors and API misuse still raise exceptions:

- `ValueError` for invalid assets/sides/malformed numeric inputs
- `RuntimeError` for lifecycle misuse, for example executing the same request
  twice or finalizing the same reservation twice
- Business rejects use stable reject codes such as
  `openpit.pretrade.RejectCode.ORDER_VALUE_CALCULATION_FAILED` when a policy
  cannot evaluate order value without `price`

## Local Testing

Recommended local flow:

```bash
maturin develop --manifest-path bindings/python/Cargo.toml
python -m pytest bindings/python/tests
```

Run only unit tests:

```bash
maturin develop --manifest-path bindings/python/Cargo.toml
python -m pytest bindings/python/tests/unit
```

Run only integration test:

```bash
maturin develop --manifest-path bindings/python/Cargo.toml
python -m pytest bindings/python/tests/integration
```

For full build/test command matrix (manual and `just`), see
[the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
