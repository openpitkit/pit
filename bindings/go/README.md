# OpenPit (Pre-trade Integrity Toolkit) for Go

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://img.shields.io/github/v/release/openpitkit/pit)](https://github.com/openpitkit/pit/releases) [![Go version](https://img.shields.io/badge/go-1.22%2B-00ADD8)](https://pkg.go.dev/go.openpit.dev/openpit) [![Module](https://img.shields.io/badge/module-go.openpit.dev%2Fopenpit-00ADD8)](https://pkg.go.dev/go.openpit.dev/openpit) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/openpitkit/pit/blob/main/LICENSE)
<!-- markdownlint-enable MD013 -->

> **Read-only mirror.** This repository is a mirror of [`bindings/go/`](https://github.com/openpitkit/pit/tree/main/bindings/go)
> from the [openpitkit/pit](https://github.com/openpitkit/pit) monorepo.
> **Do not open pull requests here** - contribute to the monorepo instead.

`openpit` is an embeddable pre-trade risk SDK for integrating policy-driven
risk checks into trading systems from Go.

For an overview and links to all resources, see the project website [openpit.dev](https://openpit.dev/).
For full project documentation, see [the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see [the project wiki](https://wiki.openpit.dev/).
For the public Go module source, see [go.openpit.dev/openpit](https://go.openpit.dev/openpit).

## Versioning Policy (Pre‑1.0)

Before the `1.0` release OpenPit follows a relaxed Semantic Versioning:

- `PATCH` releases carry bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the
  public interface**.

Breaking API changes can appear in minor releases before `1.0`. Pick
version constraints that tolerate API evolution during the pre-stable
phase.

## Install

```bash
go get go.openpit.dev/openpit
```

## Quick Start

<!-- Test mirror: bindings/go/examples_readme_test.go -->

```go
package main

import (
 "log"

 "go.openpit.dev/openpit"
 "go.openpit.dev/openpit/model"
 "go.openpit.dev/openpit/param"
 "go.openpit.dev/openpit/pretrade/policies"
)

func main() {
// Build the engine once, at platform initialization.
engine, err := openpit.NewEngineBuilder().
 FullSync().
 Builtin(policies.BuildOrderValidation()).
 Build()
if err != nil {
 log.Fatal(err)
}
defer engine.Stop()

aapl, err := param.NewAsset("AAPL")
if err != nil {
 log.Fatal(err)
}
usd, err := param.NewAsset("USD")
if err != nil {
 log.Fatal(err)
}
qty, err := param.NewQuantityFromString("100")
if err != nil {
 log.Fatal(err)
}
price, err := param.NewPriceFromString("185")
if err != nil {
 log.Fatal(err)
}

// Describe the order: buy 100 AAPL at 185 USD.
order := model.NewOrder()
operation := order.EnsureOperationView()
operation.SetInstrument(param.NewInstrument(aapl, usd))
operation.SetAccountID(param.NewAccountIDFromUint64(99224416))
operation.SetSide(param.SideBuy)
operation.SetTradeAmount(param.NewQuantityTradeAmount(qty))
operation.SetPrice(price)

// Run the pre-trade pipeline and read the verdict.
reservation, rejects, err := engine.ExecutePreTrade(order)
if err != nil {
 log.Fatal(err)
}
if rejects != nil {
 log.Fatalf("order rejected: %v", rejects)
}
// Close rolls the reservation back unless it was committed.
defer reservation.Close()

// The venue accepted the order, so the reserved state stays.
reservation.Commit()
}
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
- [Custom Go policies](https://wiki.openpit.dev/Policy-API/#go-interface)
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
- [Async Engine](https://wiki.openpit.dev/Async-Engine/) - per-account
  queues for concurrent submission.
- [Threading Contract](https://wiki.openpit.dev/Threading-Contract/) -
  goroutine migration and synchronization modes.
- [Rejects and errors](https://wiki.openpit.dev/Errors/) with
  [stable reject codes](https://wiki.openpit.dev/Reject-Codes/).

## Examples

Runnable end-to-end examples live in [`examples/go/`](https://github.com/openpitkit/pit/tree/main/examples/go):

- [`spot_funds`](https://github.com/openpitkit/pit/tree/main/examples/go/spot_funds)
  \- simplest SpotFunds policy integration (limit-only).
- [`spot_table`](https://github.com/openpitkit/pit/tree/main/examples/go/spot_table)
  \- table-driven test runner for the SpotFunds policy.
- [`rate_pnl_killswitch`](https://github.com/openpitkit/pit/tree/main/examples/go/rate_pnl_killswitch)
  \- rate-limit + P&L kill-switch supervisor.

## Runtime Delivery

The native runtime library is embedded inside the Go module at build time using
Go's `embed` package. No network download happens at runtime.

On first use, the embedded library is extracted to the user cache directory
under a path that includes the SDK version and the `GOOS-GOARCH` target tuple.
Subsequent process starts find the cached file and skip extraction.

- Target selection uses `runtime.GOOS` and `runtime.GOARCH`.
- Extraction cache path: `<user-cache>/pit-go/<version>/<goos>-<goarch>/`.

Environment overrides:

- `OPENPIT_RUNTIME_LIBRARY_PATH` - use an explicit pre-extracted library
  path instead of the embedded copy; extraction is skipped entirely.
- `OPENPIT_RUNTIME_CACHE_DIR` - override the root directory for extraction
  instead of the OS user cache directory.

## Local Repository Testing

Install [Go](https://go.dev/dl/) and
[golangci-lint](https://golangci-lint.run/welcome/install/) before running
local checks.

<!-- markdownlint-disable MD033 -->

<details>
<summary>POSIX (Linux, macOS, etc)</summary>

Install a C compiler through your OS package manager. The Go SDK uses cgo.
Optional: [Just](https://just.systems/).

With [Just](https://just.systems/):

```bash
just test-go-debug
just test-go-race
```

Manual:

```bash
cargo build -p openpit-ffi --release --locked
cd bindings/go
export OPENPIT_RUNTIME_LIBRARY_PATH="$(pwd)/../../target/release/libopenpit_ffi.so"
# macOS: use libopenpit_ffi.dylib instead.
go test ./...
go test -race ./...
```

</details>

<details>
<summary>Windows</summary>

Install [LLVM](https://github.com/llvm/llvm-project/releases) and add
`clang`/`lld` to `PATH`. Optional: [Just](https://just.systems/).

With [Just](https://just.systems/):

```powershell
just test-go-debug
just test-go-race
```

Manual:

```powershell
cargo build -p openpit-ffi --release --locked --target x86_64-pc-windows-msvc
$env:OPENPIT_RUNTIME_LIBRARY_PATH = `
  (Resolve-Path target\x86_64-pc-windows-msvc\release\openpit_ffi.dll)
$env:CGO_ENABLED = "1"
$env:CC = "clang -fuse-ld=lld"
$env:CXX = "clang++ -fuse-ld=lld"
Push-Location bindings\go
go test ./...
go test -race ./...
Pop-Location
```

</details>

<!-- markdownlint-enable MD033 -->
