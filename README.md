# Pit: Pre-trade Integrity Toolkit

[![Pit](docs/assets/pit-readme-banner.png)](https://openpit.dev/)
<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://crates.io/crates/openpit) [![Python versions](https://img.shields.io/pypi/pyversions/openpit)](https://pypi.org/project/openpit/) [![crates.io](https://img.shields.io/crates/v/openpit)](https://crates.io/crates/openpit) [![PyPI](https://img.shields.io/pypi/v/openpit)](https://pypi.org/project/openpit/) [![C API](https://img.shields.io/badge/C%20API-header%20%2B%20docs-4b5563)](docs/c-api/index.md) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
<!-- markdownlint-enable MD013 -->

Pit is a workspace for embeddable pre-trade risk components.

The project is built around a simple idea: before an order reaches a venue, it
should pass through a deterministic risk pipeline that can reject, reserve
state, and later absorb post-trade outcomes back into the same control system.

## How The Flow Works

1. The engine is configured once during platform initialization.
2. Each order first goes through lightweight start-stage checks. This stage is
   meant for fast decisions and for controls that must observe every request,
   even when the order is rejected early.
3. If the order passes the start stage, the caller receives a deferred request
   object. That object represents the heavier pre-trade stage, but the heavy
   checks have not run yet.
4. When the deferred request is executed, the engine runs the main-stage risk
   policies and collects all registered mutations. If the stage fails, the
   collected state is rolled back. If it succeeds, the caller receives a
   reservation.
   For a compact path without manual request handling, use
   `engine.execute_pre_trade(order)` as a shortcut.
5. The reservation must then be finalized explicitly. Commit keeps the reserved
   state. Rollback cancels it. Dropping the reservation without finalization
   rolls it back automatically.
6. After execution, post-trade reports are fed back into the engine so that
   policies depending on realized outcomes can update their internal state.

## Current Scope

The current implementation focuses on the pre-trade pipeline, a small set of
foundational built-in controls, and an API for building project-specific
strategy and risk policies:

- P&L kill switch
- sliding-window rate limit
- per-settlement order size limits

The engine is intentionally in-memory and deterministic. It is designed to be
embedded into a larger trading system rather than replace one. For custom
policy APIs, see the wiki:

- [Go custom policies](https://github.com/openpitkit/pit/wiki/Policy-API#go-interface)
- [Python custom policies](https://github.com/openpitkit/pit/wiki/Policy-API#python-interface)
- [Rust custom policies](https://github.com/openpitkit/pit/wiki/Policy-API#rust-interface)

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

## Where To Start

The project website [openpit.dev](https://openpit.dev/) for an overview
and links to all documentation.

[The Go SDK README](bindings/go/README.md) if you want to integrate Pit from Go.

[The Python SDK README](bindings/python/README.md) if you want to work
with Pit from Python via the `openpit` package.

[The `openpit` crate README](crates/openpit/README.md) if you want to start
with the Rust interface and a runnable example.

[The C SDK README](bindings/c/README.md) if you want to integrate Pit
from C or from environments that integrate through a C ABI.

[Conceptual pages and longer architecture notes wiki](https://github.com/openpitkit/pit/wiki).

## Local Build And Test

### Prerequisites

- Rust toolchain
- Python `>=3.10` if you build or test Python bindings
- `maturin` and `pytest` if you build or test Python bindings
- Go `1.22` if you build or test Go bindings

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r ./requirements.txt
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
maturin develop --manifest-path bindings/python/Cargo.toml
maturin develop --release --manifest-path bindings/python/Cargo.toml
```

The recommended Python test flow is to run `maturin develop` before `pytest`.
This runs against the current checkout (including a dirty worktree). If sources
did not change, Cargo can reuse cached artifacts and avoid a full rebuild.

#### Go

Verified with Go `1.22`.

With [Just](https://just.systems/):

```bash
just test-go
just test-go-race
```

Manual:

```bash
cargo build -p pit-ffi --release --locked
cd bindings/go
export OPENPIT_RUNTIME_LIBRARY_PATH="$(pwd)/../../target/release/libpit_ffi.so"
export LD_LIBRARY_PATH="$(pwd)/../../target/release${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
export CGO_LDFLAGS="-L$(pwd)/../../target/release -lpit_ffi"
go test ./...
go test -race ./...
```

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
maturin develop --manifest-path bindings/python/Cargo.toml
python -m pytest bindings/python/tests

# Rust:
cargo test --workspace

# Python
maturin develop --manifest-path bindings/python/Cargo.toml
python -m pytest bindings/python/tests
python -m pytest bindings/python/tests/unit
python -m pytest bindings/python/tests/integration

# Go:
cargo build -p pit-ffi --release --locked
cd bindings/go
export OPENPIT_RUNTIME_LIBRARY_PATH="$(pwd)/../../target/release/libpit_ffi.so"
export LD_LIBRARY_PATH="$(pwd)/../../target/release${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
export CGO_LDFLAGS="-L$(pwd)/../../target/release -lpit_ffi"
go test ./...
go test -race ./...
```
