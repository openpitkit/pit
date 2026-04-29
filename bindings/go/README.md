# Pit (Pre-trade Integrity Toolkit) for Go

> **Read-only mirror.** This repository is a mirror of
> [`bindings/go/`](https://github.com/openpitkit/pit/tree/main/bindings/go)
> from the [openpitkit/pit](https://github.com/openpitkit/pit) monorepo.
> **Do not open pull requests here** — contribute to the monorepo instead.

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![Go version](https://img.shields.io/badge/go-1.22%2B-00ADD8)](https://go.dev/) [![Module](https://img.shields.io/badge/module-go.openpit.dev%2Fopenpit-00ADD8)](https://go.openpit.dev/openpit) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
<!-- markdownlint-enable MD013 -->

`openpit` is an embeddable pre-trade risk SDK for integrating policy-driven
risk checks into trading systems from Go.

For an overview and links to all resources, see
the project website [openpit.dev](https://openpit.dev/).
For full project documentation, see
[the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see
[the project wiki](https://github.com/openpitkit/pit/wiki).
For the public Go module source, see
[go.openpit.dev/openpit](https://go.openpit.dev/openpit).

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

Visit the [Go module page](https://go.openpit.dev/openpit) and the
[project wiki](https://github.com/openpitkit/pit/wiki) for conceptual pages
and architecture notes.

## Install

```bash
go get go.openpit.dev/openpit
```

## Engine

### Overview

The engine evaluates an order through a deterministic pre-trade pipeline:

- `engine.StartPreTrade(order)` runs start-stage policies; returns
  `(*pretrade.Request, []reject.Reject, error)`
- `request.Execute()` runs main-stage policies; returns
  `(*pretrade.Reservation, []reject.Reject, error)`
- `reservation.Commit()` applies reserved state
- `reservation.Close()` rolls back any uncommitted reservation automatically
- `engine.ExecutePreTrade(order)` is a shortcut that composes both stages
- `engine.ApplyExecutionReport(report)` updates post-trade policy state

Start-stage policies stop on the first reject. Main-stage policies aggregate
rejects and roll back registered mutations in reverse order when any reject is
produced.

Built-in start-stage policies currently include:

- `policies.NewOrderValidation()`
- `policies.NewPnlBoundsKillSwitchPolicy(...)`
- `policies.NewRateLimitPolicy(...)`
- `policies.NewOrderSizeLimitPolicy(...)`

The primary integration model is to write project-specific policies against the
public Go policy API described in the wiki:
[Custom Go policies](https://github.com/openpitkit/pit/wiki/Policy-API#go-interface).

There are two types of rejections: a full kill switch for the account and a
rejection of only the current request. This is useful in algorithmic trading
when automatic order submission must be halted until the situation is analyzed.

## Threading

Canonical contract: [Threading Contract](https://github.com/openpitkit/pit/wiki/Threading-Contract).

The Go binding follows the same SDK threading contract. Goroutine migration
between OS threads during one SDK call is supported, and callbacks invoked by
the SDK may run on a different OS thread than the goroutine that initiated the
call.

## Usage

```go
package main

import (
 "fmt"
 "log"

 "go.openpit.dev/openpit"
 "go.openpit.dev/openpit/model"
 "go.openpit.dev/openpit/param"
 "go.openpit.dev/openpit/pkg/optional"
 "go.openpit.dev/openpit/pretrade/policies"
)

func main() {
 usd, err := param.NewAsset("USD")
 if err != nil {
  log.Fatal(err)
 }

 lowerBound, err := param.NewPnlFromString("-1000")
 if err != nil {
  log.Fatal(err)
 }
 maxQty, err := param.NewQuantityFromString("500")
 if err != nil {
  log.Fatal(err)
 }
 maxNotional, err := param.NewVolumeFromString("100000")
 if err != nil {
  log.Fatal(err)
 }

 // 1. Configure policies.
 pnlPolicy, err := policies.NewPnlBoundsKillSwitchPolicy(
  policies.PnlBoundsBarrier{
   SettlementAsset: usd,
   LowerBound:      optional.Some(lowerBound),
   InitialPnl:      param.PnlZero,
  },
 )
 if err != nil {
  log.Fatal(err)
 }
 defer pnlPolicy.Close()

 sizePolicy, err := policies.NewOrderSizeLimitPolicy(
  policies.OrderSizeLimit{
   SettlementAsset: usd,
   MaxQuantity:     maxQty,
   MaxNotional:     maxNotional,
  },
 )
 if err != nil {
  log.Fatal(err)
 }
 defer sizePolicy.Close()

 // 2. Build the engine (one time at the platform initialization).
 builder, err := openpit.NewEngineBuilder()
 if err != nil {
  log.Fatal(err)
 }
 builder.BuiltinCheckPreTradeStartPolicy(
  policies.NewOrderValidation(),
  pnlPolicy,
  policies.NewRateLimitPolicy(100, 1),
  sizePolicy,
 )
 engine, err := builder.Build()
 if err != nil {
  log.Fatal(err)
 }
 defer engine.Stop()

 // 3. Check an order.
 order := model.NewOrder()
 op := order.EnsureOperationView()
 aapl, err := param.NewAsset("AAPL")
 if err != nil {
  log.Fatal(err)
 }
 op.SetInstrument(param.NewInstrument(aapl, usd))
 op.SetAccountID(param.NewAccountIDFromInt(99224416))
 op.SetSide(param.SideBuy)
 price, _ := param.NewPriceFromString("185")
 qty, _ := param.NewQuantityFromString("100")
 op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
 op.SetPrice(price)

 request, rejects, err := engine.StartPreTrade(order)
 if err != nil {
  log.Fatal(err)
 }
 if rejects != nil {
  for _, r := range rejects {
   fmt.Printf("rejected by %s [%d]: %s (%s)\n", r.Policy, r.Code, r.Reason, r.Details)
  }
  return
 }
 defer request.Close()

 // 4. Quick, lightweight checks were performed during start stage. The system
 // state has not yet changed, except in cases where each request, even
 // rejected ones, must be considered. Before the heavy-duty checks, other
 // work on the request can be performed simply by holding the request object.

 // 5. Real pre-trade and risk control.
 reservation, rejects, err := request.Execute()
 if err != nil {
  log.Fatal(err)
 }
 if rejects != nil {
  for _, r := range rejects {
   fmt.Printf("rejected by %s [%d]: %s (%s)\n", r.Policy, r.Code, r.Reason, r.Details)
  }
  return
 }
 defer reservation.Close()

 // Optional shortcut for the same two-stage flow:
 // reservation, rejects, err := engine.ExecutePreTrade(order)

 // 6. If the request is successfully sent to the venue, it must be committed.
 // The rollback must be called otherwise to revert all performed reservations.
 reservation.Commit()

 // 7. The order goes to the venue and returns with an execution report.
 report := model.NewExecutionReport()
 reportOp := model.NewExecutionReportOperation()
 reportOp.SetInstrument(param.NewInstrument(aapl, usd))
 reportOp.SetAccountID(param.NewAccountIDFromInt(99224416))
 reportOp.SetSide(param.SideBuy)
 report.SetOperation(reportOp)

 pnl, _ := param.NewPnlFromString("-50")
 fee, _ := param.NewFeeFromString("3.4")
 impact := model.NewExecutionReportFinancialImpact()
 impact.SetPnl(pnl)
 impact.SetFee(fee)
 report.SetFinancialImpact(impact)

 result, err := engine.ApplyExecutionReport(report)
 if err != nil {
  log.Fatal(err)
 }

 // 8. After each execution report is applied, the system may report that it
 // has been determined in advance that all subsequent requests will be
 // rejected if the account status does not change.
 if result.KillSwitchTriggered {
  fmt.Println("halt new orders until the blocked state is cleared")
 }
}
```

## Errors

Policy rejects from `engine.StartPreTrade()` and `request.Execute()` are
returned as the second return value (`[]reject.Reject`). A non-nil list means the
request was rejected; a nil list means the stage passed.

Infrastructure failures and API misuse are returned as the third return value
(`error`):

- `error` from `engine.StartPreTrade()`, `request.Execute()`, or
  `engine.ApplyExecutionReport()` indicates a transport-level or lifecycle
  failure, not a business reject
- `error` from policy constructors such as `policies.NewPnlBoundsKillSwitchPolicy()`
  indicates an invalid configuration

Business rejects use stable codes, for example
`reject.CodeOrderQtyExceedsLimit` when an order quantity exceeds the configured
limit.

## Runtime Delivery

The native runtime library is embedded inside the Go module at build time using
Go's `embed` package. No network download happens at runtime.

On first use, the embedded library is extracted to the user cache directory
under a path that includes the SDK version and the `GOOS-GOARCH` target tuple.
Subsequent process starts find the cached file and skip extraction.

- Target selection uses `runtime.GOOS` and `runtime.GOARCH`.
- Extraction cache path: `<user-cache>/pit-go/<version>/<goos>-<goarch>/`.

Environment overrides:

- `OPENPIT_RUNTIME_LIBRARY_PATH` — use an explicit pre-extracted library path
  instead of the embedded copy; extraction is skipped entirely.
- `OPENPIT_RUNTIME_CACHE_DIR` — override the root directory for extraction
  instead of the OS user cache directory.
