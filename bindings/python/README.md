# OpenPit (Pre-trade Integrity Toolkit) for Python

<!-- markdownlint-disable MD033 -->

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![Python versions](https://img.shields.io/pypi/pyversions/openpit)](https://pypi.org/project/openpit/) [![PyPI](https://img.shields.io/pypi/v/openpit)](https://pypi.org/project/openpit/) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](../../LICENSE)
<!-- markdownlint-enable MD013 -->

`openpit` is an embeddable pre-trade risk SDK for integrating policy-driven
risk checks into trading systems from Python.

For an overview and links to all resources, see the project website [openpit.dev](https://openpit.dev/).
For the Python API guide and reference, see [openpit.readthedocs.io](https://openpit.readthedocs.io/en/stable/).
For full project documentation, see [the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see [the project wiki](https://wiki.openpit.dev/).

## Versioning Policy (Pre‑1.0)

Before the `1.0` release OpenPit follows a relaxed Semantic Versioning:

- `PATCH` releases carry bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the
  public interface**.

Breaking API changes can appear in minor releases before `1.0`. Pick
version constraints that tolerate API evolution during the pre-stable
phase.

## Getting Started

Visit the [PyPI package](https://pypi.org/project/openpit/).

## Examples

Runnable end-to-end examples live in [`examples/python/`](https://github.com/openpitkit/pit/tree/main/examples/python):

- [`spot_funds`](https://github.com/openpitkit/pit/tree/main/examples/python/spot_funds)
  \- simplest SpotFunds policy integration (limit-only).
- [`spot_table`](https://github.com/openpitkit/pit/tree/main/examples/python/spot_table)
  \- table-driven test runner for the SpotFunds policy.
- [`rate_pnl_killswitch`](https://github.com/openpitkit/pit/tree/main/examples/python/rate_pnl_killswitch)
  \- rate-limit + P&L kill-switch supervisor.

## Install

For normal end-user installation, use the published
[PyPI package](https://pypi.org/project/openpit/):

```bash
pip install openpit
```

<details>
<summary>POSIX (Linux, macOS, etc)</summary>

Install [Rust](https://rustup.rs/) and
[Python 3.10+](https://www.python.org/downloads/). [Just](https://just.systems/)
is optional but recommended.

If you need local development/debugging, clone this repository and build from
source with [Maturin](https://github.com/PyO3/maturin):

With [Just](https://just.systems/):

```bash
just python-develop-debug
```

Local release build:

```bash
just python-develop-release
```

Manual:

```bash
python3 -m venv .venv
.venv/bin/python -m pip install -r ./requirements.txt
.venv/bin/python -m maturin develop --manifest-path bindings/python/Cargo.toml
.venv/bin/python -m maturin develop --release --manifest-path bindings/python/Cargo.toml
```

</details>

<details>
<summary>Windows</summary>

Install [rustup](https://rustup.rs/), target `x86_64-pc-windows-msvc`, and
[Python 3.10+](https://www.python.org/downloads/). [Just](https://just.systems/)
is optional but recommended.

With [Just](https://just.systems/):

```powershell
rustup target add x86_64-pc-windows-msvc
just python-develop-debug
just python-develop-release
```

Manual:

```powershell
rustup target add x86_64-pc-windows-msvc
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r .\requirements.txt
.\.venv\Scripts\python.exe -m maturin develop `
  --manifest-path bindings/python/Cargo.toml `
  --target x86_64-pc-windows-msvc
.\.venv\Scripts\python.exe -m maturin develop `
  --release `
  --manifest-path bindings/python/Cargo.toml `
  --target x86_64-pc-windows-msvc
```

</details>

## Engine

### Overview

The engine evaluates an order through a deterministic pre-trade pipeline:

- `engine.start_pre_trade(order=...)` runs lightweight start-stage checks
- `request.execute()` runs main-stage check policies
- `reservation.commit()` applies reserved state
- `reservation.rollback()` reverts reserved state
- `engine.apply_execution_report(report=...)` updates post-trade policy
  state

Start-stage checks aggregate rejects from all registered policies.
Main-stage checks aggregate rejects and run rollback mutations in reverse
order when any reject is produced.

Built-in policies:

- `SpotFundsPolicy` -
  [per-account solvency gate over spendable funds](https://wiki.openpit.dev/Spot-Funds/)
- `OrderValidationPolicy` - [structural integrity checks on every order](https://wiki.openpit.dev/Policies/#ordervalidationpolicy)
- `RateLimitPolicy` - [throttle order flow per broker, asset, or account](https://wiki.openpit.dev/Policies/#ratelimitpolicy)
- `OrderSizeLimitPolicy` - [fat-finger caps on quantity and notional](https://wiki.openpit.dev/Policies/#ordersizelimitpolicy)
- `PnlBoundsKillSwitchPolicy` - [halt an account when realized P&L breaches bounds](https://wiki.openpit.dev/Policies/#pnlboundskillswitchpolicy)

The primary integration model is to write project-specific policies against
the public Python policy API:
[Custom Python policies](https://wiki.openpit.dev/Policy-API/#python-interface).

## Threading

Canonical contract: [Threading Contract](https://wiki.openpit.dev/Threading-Contract/).

Public methods acquire the GIL when needed; the SDK does not release the
GIL across callback boundaries, so Python policies execute on the calling
thread.

Custom policies that need internal state across calls can use the built-in
[Storage](https://wiki.openpit.dev/Storage/) abstraction. In
typical Python usage (synchronous code or an asyncio loop pinned to one
thread) the no-sync policy is sufficient and the storage compiles down to
direct dictionary access. A synchronizing policy is needed only when the
engine is genuinely shared across OS threads.

## Usage

<!-- markdownlint-disable-next-line MD013 -->
<!-- Test mirror: pit/bindings/python/tests/integration/test_examples_readme.py -->
```python
import datetime
import openpit
import openpit.pretrade.policies

# 1. Configure policies.
pnl_policy = (
    openpit.pretrade.policies.build_pnl_bounds_killswitch()
    .broker_barriers(
        openpit.pretrade.policies.PnlBoundsBrokerBarrier(
            settlement_asset="USD",
            lower_bound=openpit.param.Pnl("-1000"),
        ),
    )
)

rate_limit_policy = (
    openpit.pretrade.policies.build_rate_limit()
    .broker_barrier(
        openpit.pretrade.policies.RateLimitBrokerBarrier(
            limit=openpit.pretrade.policies.RateLimit(
                max_orders=100,
                window=datetime.timedelta(seconds=1),
            ),
        ),
    )
)

max_qty = openpit.param.Quantity("500")
max_notional = openpit.param.Volume("100000")
order_size_policy = (
    openpit.pretrade.policies.build_order_size_limit()
    .broker_barrier(
        openpit.pretrade.policies.OrderSizeBrokerBarrier(
            limit=openpit.pretrade.policies.OrderSizeLimit(
                max_quantity=max_qty,
                max_notional=max_notional,
            ),
        )
    )
    .asset_barriers(
        openpit.pretrade.policies.OrderSizeAssetBarrier(
            limit=openpit.pretrade.policies.OrderSizeLimit(
                max_quantity=max_qty,
                max_notional=max_notional,
            ),
            settlement_asset="USD",
        ),
    )
)

# 2. Build the engine (one time at the platform initialization).
engine = (
    openpit.Engine.builder()
    .no_sync()
    .builtin(openpit.pretrade.policies.build_order_validation())
    .builtin(pnl_policy)
    .builtin(rate_limit_policy)
    .builtin(order_size_policy)
    .build()
)

# 3. Check an order.
order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_int(99224416),
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
        account_id=openpit.param.AccountId.from_int(99224416),
        side=openpit.param.Side.BUY,
    ),
    financial_impact=openpit.FinancialImpact(
        pnl=openpit.param.Pnl("-50"),
        fee=openpit.param.Fee("3.4"),
    ),
)

result = engine.apply_execution_report(report=report)
for outcome in result.account_pnls:
    print(f"account P&L outcome for {outcome.account_id}")
for outcome in result.account_adjustments:
    print(f"account adjustment from group {outcome.policy_group_id}")

# 8. After each execution report is applied, the system may report that it has
# been determined in advance that all subsequent requests will be rejected if
# the account status does not change.
assert not result.account_blocks
```

## Errors

Policy rejects from `engine.start_pre_trade()` and `request.execute()` are
returned as `StartResult` and `ExecuteResult`.

Input validation errors and API misuse still raise exceptions:

- `ValueError` for invalid assets/sides/malformed numeric inputs
- `RuntimeError` for lifecycle misuse, for example executing the same request
  twice or finalizing the same reservation twice
- Business rejects use stable reject codes such as
  `openpit.pretrade.RejectCode.ORDER_VALUE_CALCULATION_FAILED` when a policy
  cannot evaluate order value without `price`

## Local Testing

<details>
<summary>POSIX (Linux, macOS, etc)</summary>

Recommended local flow:

With [Just](https://just.systems/):

```bash
just test-python-debug
just test-python-unit-debug
just test-python-integration-debug
```

Manual:

```bash
python3 -m venv .venv
.venv/bin/python -m pip install -r ./requirements.txt
.venv/bin/python -m maturin develop --manifest-path bindings/python/Cargo.toml
.venv/bin/python -m pytest bindings/python/tests
```

Run only unit tests:

```bash
.venv/bin/python -m maturin develop --manifest-path bindings/python/Cargo.toml
.venv/bin/python -m pytest bindings/python/tests/unit
```

Run only integration test:

```bash
.venv/bin/python -m maturin develop --manifest-path bindings/python/Cargo.toml
.venv/bin/python -m pytest bindings/python/tests/integration
```

</details>

<details>
<summary>Windows</summary>

Optional: [Just](https://just.systems/).

With [Just](https://just.systems/):

```powershell
just test-python-debug
just test-python-unit-debug
just test-python-integration-debug
```

Manual:

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r .\requirements.txt
.\.venv\Scripts\python.exe -m maturin develop `
  --manifest-path bindings/python/Cargo.toml `
  --target x86_64-pc-windows-msvc
.\.venv\Scripts\python.exe -m pytest bindings/python/tests
.\.venv\Scripts\python.exe -m pytest bindings/python/tests/unit
.\.venv\Scripts\python.exe -m pytest bindings/python/tests/integration
```

</details>

For full build/test command matrix (manual and `just`), see
[the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
