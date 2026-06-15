# OpenPit: Pre-trade Integrity Toolkit

[![Pit](docs/assets/pit-readme-banner.png)](https://openpit.dev/)
<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![Go version](https://img.shields.io/badge/go-1.22%2B-00ADD8)](https://pkg.go.dev/go.openpit.dev/openpit) [![Module](https://img.shields.io/badge/module-go.openpit.dev%2Fopenpit-00ADD8)](https://pkg.go.dev/go.openpit.dev/openpit)
[![Python versions](https://img.shields.io/pypi/pyversions/openpit)](https://pypi.org/project/openpit/) [![PyPI](https://img.shields.io/pypi/v/openpit)](https://pypi.org/project/openpit/)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://crates.io/crates/openpit) [![crates.io](https://img.shields.io/crates/v/openpit)](https://crates.io/crates/openpit)
[![C API](https://img.shields.io/badge/C%20API-header%20%2B%20docs-4b5563)](docs/c-api/index.md)
<!-- markdownlint-enable MD013 -->

OpenPit is a workspace for embeddable pre-trade risk components. Before an
order reaches a venue, it passes through a deterministic risk pipeline that
can reject the request, reserve state, and later absorb post-trade outcomes
back into the same control system.

## How The Flow Works

1. The engine is configured once during platform initialization.
2. Each order first goes through lightweight start-stage checks. This stage
   makes fast decisions and runs controls that must observe every request,
   including rejected ones.
3. If the order passes the start stage, the caller receives a deferred
   request object. The heavier pre-trade stage has not run yet.
4. When the deferred request is executed, the engine runs the main-stage
   risk policies and collects all registered mutations. If any policy
   rejects, the collected state is rolled back. If the stage succeeds, the
   caller receives a reservation. The shortcut `engine.execute_pre_trade(order)`
   composes the two stages into one call when manual request handling is
   not needed.
5. The reservation must be finalized explicitly: `commit` keeps the
   reserved state, `rollback` cancels it. Dropping the reservation without
   finalization rolls it back automatically.
6. Post-trade reports are fed back into the engine so policies that depend
   on realized outcomes can update their state.

## Current Scope

The current implementation focuses on the pre-trade pipeline, a small set
of built-in controls, and an API for building project-specific strategy and
risk policies. Built-ins:

- [Spot Funds](https://github.com/openpitkit/pit/wiki/Spot-Funds) - per-account
  solvency gate over spendable funds.
- [Order Validation](https://github.com/openpitkit/pit/wiki/Policies#ordervalidationpolicy)
  \- structural integrity checks on every order.
- [Rate Limit](https://github.com/openpitkit/pit/wiki/Policies#ratelimitpolicy)
  \- throttle order flow per broker, asset, or account.
- [Order Size Limit](https://github.com/openpitkit/pit/wiki/Policies#ordersizelimitpolicy)
  \- fat-finger caps on quantity and notional.
- [P&L Kill Switch](https://github.com/openpitkit/pit/wiki/Policies#pnlboundskillswitchpolicy)
  \- halt an account when realized P&L breaches bounds.
- plus your own via the [policy SDK](https://github.com/openpitkit/pit/wiki/Policy-API).

Custom policies that maintain state across calls can use the built-in
[Storage](https://github.com/openpitkit/pit/wiki/Storage) abstraction.
Synchronization is selected once at engine construction and applied
transparently, with no overhead in single-threaded embeddings.

Controls can act both by account and by group of accounts: register accounts
under a compact identifier with [Account Groups](https://github.com/openpitkit/pit/wiki/Account-Groups)
and let a policy branch on the group instead of an enumerated account list.

The engine is intentionally in-memory and deterministic, designed to be
embedded into a larger trading system rather than replace one. For custom
policy APIs:

- [Go custom policies](https://github.com/openpitkit/pit/wiki/Policy-API#go-interface)
- [Python custom policies](https://github.com/openpitkit/pit/wiki/Policy-API#python-interface)
- [Rust custom policies](https://github.com/openpitkit/pit/wiki/Policy-API#rust-interface)

## Versioning Policy (Pre‑1.0)

Before the `1.0` release OpenPit follows a relaxed Semantic Versioning:

- `PATCH` releases carry bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the
  public interface**.

Breaking API changes can appear in minor releases before `1.0`. Pick version
constraints that tolerate API evolution during the pre-stable phase.

## Where To Start

- [openpit.dev](https://openpit.dev/) - project website with an overview and
  links to all documentation.
- [Go SDK README](bindings/go/README.md) - integrate OpenPit from Go.
- [Python SDK README](bindings/python/README.md) - the `openpit` Python
  package.
- [`openpit` crate README](crates/openpit/README.md) - Rust interface with a
  runnable example.
- [C SDK README](bindings/c/README.md) - C ABI for environments that
  integrate through C.
- [examples/](examples/) - end-to-end runnable scenarios.
- [Wiki](https://github.com/openpitkit/pit/wiki) - conceptual pages and
  architecture notes.

## Local Build And Test

### Prerequisites

- Rust toolchain
- Python `>=3.10` if you build or test Python bindings
- `maturin` and `pytest` if you build or test Python bindings
- Go `1.22` if you build or test Go bindings

```bash
python3.10 -m venv .venv
.venv/bin/python -m pip install -r ./requirements.txt
```

### Build

#### SDK

With [Just](https://just.systems/):

```bash
just build
```

Manual:

```bash
cargo build --workspace
```

#### Python

With [Just](https://just.systems/):

```bash
just python-develop
just python-develop-release
```

Manual:

```bash
.venv/bin/maturin develop --manifest-path bindings/python/Cargo.toml
.venv/bin/maturin develop --release --manifest-path bindings/python/Cargo.toml
```

The recommended Python test flow is to run `maturin develop` before
`pytest`. This runs against the current checkout (including a dirty
worktree).

#### Go

The Go SDK consumes the native runtime through CGo. Build the FFI library
first:

```bash
cargo build -p openpit-ffi --release --locked
```

Go tests then expect the path to that library through
`OPENPIT_RUNTIME_LIBRARY_PATH`. The variable is needed only for local
development inside the `pit` repository - consumers installing the SDK with
`go get` do not need to set it.

### Tests

With [Just](https://just.systems/):

```bash
# All tests:
just test-all

# Rust:
just test-rust

# Python:
just test-python
just test-python-unit
just test-python-integration

# Go:
just test-go
just test-go-race
```

Manual:

```bash
# All tests:
cargo test --workspace
.venv/bin/maturin develop --manifest-path bindings/python/Cargo.toml
.venv/bin/python -m pytest bindings/python/tests

# Rust:
cargo test --workspace

# Python
maturin develop --manifest-path bindings/python/Cargo.toml
.venv/bin/python -m pytest bindings/python/tests
.venv/bin/python -m pytest bindings/python/tests/unit
.venv/bin/python -m pytest bindings/python/tests/integration

# Go:
cargo build -p openpit-ffi --release --locked
cd bindings/go
# Linux:
export OPENPIT_RUNTIME_LIBRARY_PATH="$(pwd)/../../target/release/libopenpit_ffi.so"
# macOS: use libopenpit_ffi.dylib instead.
go test ./...
go test -race ./...
```
