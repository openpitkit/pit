# OpenPit (Pre-trade Integrity Toolkit) for Python

<!-- markdownlint-disable MD033 -->

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://img.shields.io/github/v/release/openpitkit/pit)](https://github.com/openpitkit/pit/releases) [![Python versions](https://img.shields.io/pypi/pyversions/openpit)](https://pypi.org/project/openpit/) [![PyPI](https://img.shields.io/pypi/v/openpit)](https://pypi.org/project/openpit/) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/openpitkit/pit/blob/main/LICENSE)
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

## Quick Start

<!-- Test mirror: bindings/python/tests/integration/test_examples_readme.py -->

```python
import openpit
from openpit.param import AccountId, Price, Quantity, Side, TradeAmount, Volume
from openpit.pretrade.policies import (
    OrderSizeBrokerBarrier,
    OrderSizeLimit,
    build_order_size_limit,
)

# Build the engine once, at start-up: one broker-wide fat-finger cap.
engine = (
    openpit.Engine.builder()
    .no_sync()
    .builtin(
        build_order_size_limit().broker_barrier(
            OrderSizeBrokerBarrier(
                limit=OrderSizeLimit(
                    max_quantity=Quantity("500"),
                    max_notional=Volume("1000000"),
                ),
            ),
        ),
    )
    .build()
)

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=AccountId.from_int(99224416),
        side=Side.BUY,
        trade_amount=TradeAmount.quantity("1000"),
        price=Price("185"),
    ),
)

result = engine.execute_pre_trade(order=order)
if result.ok:
    # Send the order to the venue, then commit or roll the reservation back.
    result.reservation.commit()
else:
    for reject in result.rejects:
        # OrderSizeLimitPolicy [OrderQtyExceedsLimit]: order quantity exceeded
        print(f"{reject.policy} [{reject.code}]: {reject.reason}")
```

The explicit two-stage flow, drop copy, and post-trade reports are described
on the [Pre-trade Pipeline](https://wiki.openpit.dev/Pre-trade-Pipeline/)
page.

## What Is Inside

- [Spot Funds](https://wiki.openpit.dev/Spot-Funds/) - per-account
  solvency gate over spendable funds.
- [Order Validation](https://wiki.openpit.dev/Policies/#ordervalidationpolicy)
  \- structural integrity checks on every order.
- [Rate Limit](https://wiki.openpit.dev/Policies/#ratelimitpolicy)
  \- throttle order flow per broker, asset, or account.
- [Order Size Limit](https://wiki.openpit.dev/Policies/#ordersizelimitpolicy)
  \- fat-finger caps on quantity and notional.
- [P&L Kill Switch](https://wiki.openpit.dev/Policies/#pnlboundskillswitchpolicy)
  \- halt an account when realized P&L breaches bounds.
- [Custom Python policies](https://wiki.openpit.dev/Policy-API/#python-interface)
  \- the primary integration model.
- [Account Blocking](https://wiki.openpit.dev/Account-Blocking/),
  [Account Groups](https://wiki.openpit.dev/Account-Groups/),
  [Account Adjustments](https://wiki.openpit.dev/Account-Adjustments/), and
  [Balance Reconciliation](https://wiki.openpit.dev/Balance-Reconciliation/).
- [Drop Copy](https://wiki.openpit.dev/Pre-trade-Pipeline/#drop-copy) -
  record already executed orders without pre-trade enforcement.
- [Market Data](https://wiki.openpit.dev/Market-Data/).
- [Dynamic Reconfiguration](https://wiki.openpit.dev/Dynamic-Policy-Reconfiguration/)
  of a live policy.
- [Threading Contract](https://wiki.openpit.dev/Threading-Contract/) - the GIL
  is held across callbacks, so Python policies run on the calling thread.
- [Rejects and errors](https://wiki.openpit.dev/Errors/) with
  [stable reject codes](https://wiki.openpit.dev/Reject-Codes/); the full
  Python exception guide is on
  [readthedocs](https://openpit.readthedocs.io/en/stable/guides/errors.html).

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
