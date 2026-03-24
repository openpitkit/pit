# Pit: Pre-trade Integrity Toolkit

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://github.com/openpitkit/pit/actions/workflows/release.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/release.yml) [![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://crates.io/crates/openpit) [![crates.io](https://img.shields.io/crates/v/openpit)](https://crates.io/crates/openpit) [![docs.rs](https://img.shields.io/docsrs/openpit)](https://docs.rs/openpit/latest/openpit/) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](../../LICENSE)
<!-- markdownlint-enable MD013 -->

`openpit` is an embeddable pre-trade risk SDK for integrating policy-driven
risk checks into trading systems.

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

Visit the [crate page on crates.io](https://crates.io/crates/openpit) and the
[API documentation on docs.rs](https://docs.rs/openpit/latest/openpit/).

## Install

Run the following Cargo command in your project directory:

```bash
cargo add openpit
```

## Engine

### Overview

The engine evaluates an order through a deterministic pre-trade pipeline:

- `start_pre_trade(order)` runs lightweight start-stage policies
- `Request::execute()` runs main-stage policies
- `Reservation::commit()` applies reserved state
- dropping `Reservation` rolls state back automatically
- `apply_execution_report(report)` updates post-trade policy state

Start-stage policies stop on the first reject. Main-stage policies aggregate
rejects and roll back registered mutations in reverse order when any reject is
produced.

Built-in start-stage policies currently include:

- `OrderValidationPolicy`
- `PnlKillSwitchPolicy`
- `RateLimitPolicy`
- `OrderSizeLimitPolicy`

These built-ins are intentionally small. The primary integration model is to
write project-specific policies against the public Rust policy API described in
the wiki:
[Custom Rust policies](https://github.com/openpitkit/pit/wiki/Policies#rust-custom-policy-api).

There are two types of rejections: a full kill switch for the account and a
rejection of only the current request. This is useful in algorithmic trading
when automatic order submission must be halted until the situation is analyzed.

## Usage

```rust
use std::time::Duration;

use openpit::{
    FinancialImpact, ExecutionReportOperation, OrderOperation,
    WithFinancialImpact, WithExecutionReportOperation,
};
use openpit::param::{Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::{Engine, Instrument};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let usd = Asset::new("USD")?;

// 1. Configure policies.
let pnl_policy = PnlKillSwitchPolicy::new(
    (usd.clone(), Pnl::from_str("1000")?),
    [],
)?;

let rate_limit_policy = RateLimitPolicy::new(100, Duration::from_secs(1));

let size_policy = OrderSizeLimitPolicy::new(
    OrderSizeLimit {
        settlement_asset: usd.clone(),
        max_quantity: Quantity::from_str("500")?,
        max_notional: Volume::from_str("100000")?,
    },
    [],
);

// 2. Build the engine (one time at the platform initialization).
let engine = Engine::builder()
    .check_pre_trade_start_policy(OrderValidationPolicy::new())
    .check_pre_trade_start_policy(pnl_policy)
    .check_pre_trade_start_policy(rate_limit_policy)
    .check_pre_trade_start_policy(size_policy)
    .build()?;

// 3. Check an order.
let order = OrderOperation {
    instrument: Instrument::new(
        Asset::new("AAPL")?,
        usd.clone(),
    ),
    side: Side::Buy,
    trade_amount: TradeAmount::Quantity(
        Quantity::from_str("100")?,
    ),
    price: Some(Price::from_str("185")?),
};

let request = engine.start_pre_trade(order)?;

// 4. Quick, lightweight checks, such as fat-finger scope or enabled killswitch,
// were performed during pre-trade request creation. The system state has not
// yet changed, except in cases where each request, even rejected ones, must be
// considered (for example, to prevent frequent transfers). Before the
// heavy-duty checks, other work on the request can be performed simply by
// holding the request object.

// 5. Real pre-trade and risk control.
let reservation = request.execute()?;

// 6. If the request is successfully sent to the venue, it must be committed.
// The rollback must be called otherwise to revert all performed reservations.
reservation.commit();

// 5. The order goes to the venue and returns with an execution report.
let report = WithExecutionReportOperation {
    inner: WithFinancialImpact {
        inner: (),
        financial_impact: FinancialImpact {
            pnl: Pnl::from_str("-50")?,
            fee: Fee::from_str("3.4")?,
        },
    },
    operation: ExecutionReportOperation {
        instrument: Instrument::new(
            Asset::new("AAPL")?,
            usd,
        ),
        side: Side::Buy,
    },
};

let result = engine.apply_execution_report(&report);

// 6. After each execution report is applied, the system may report that it has
// been determined in advance that all subsequent requests will be rejected if
// the account status does not change.
assert!(!result.kill_switch_triggered);
# Ok(())
# }
```

## Errors

Rejects from `start_pre_trade(order)` and `Request::execute()` are returned as
`Err(Reject)` and `Result<Reservation, Vec<Reject>>`.

Each `Reject` contains:

- `policy`: policy name
- `code`: stable machine-readable code (for example `RejectCode::OrderQtyExceedsLimit`)
- `reason`: short human-readable reject type (for example `"order quantity exceeded"`)
- `details`: concrete case details (for example `"requested 11, max allowed: 10"`)
- `scope`: `RejectScope::Order` or `RejectScope::Account`

`RejectCode` values are standardized and stable across Rust, Python, and C FFI.
