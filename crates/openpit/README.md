# OpenPit: Pre-trade Integrity Toolkit

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

Before the `1.0` release OpenPit follows a relaxed Semantic Versioning:

- `PATCH` releases carry bug fixes and small internal corrections.
- `MINOR` releases may introduce new features **and may also change the
  public interface**.

Breaking API changes can appear in minor releases before `1.0`. Pick
version constraints that tolerate API evolution during the pre-stable
phase.

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
- `PreTradeRequest::execute()` runs main-stage policies
- `PreTradeReservation::commit()` applies reserved state
- dropping `PreTradeReservation` rolls state back automatically
- `apply_execution_report(report)` updates post-trade policy state

Start-stage policies aggregate rejects from all registered policies.
Main-stage policies aggregate rejects and roll back registered mutations
in reverse order when any reject is produced.

Built-in policies:

- `SpotFundsPolicy` - [per-account solvency gate over spendable funds](https://github.com/openpitkit/pit/wiki/Spot-Funds)
- `OrderValidationPolicy` - [structural integrity checks on every order](https://github.com/openpitkit/pit/wiki/Policies#ordervalidationpolicy)
- `RateLimitPolicy` - [throttle order flow per broker, asset, or account](https://github.com/openpitkit/pit/wiki/Policies#ratelimitpolicy)
- `OrderSizeLimitPolicy` - [fat-finger caps on quantity and notional](https://github.com/openpitkit/pit/wiki/Policies#ordersizelimitpolicy)
- `PnlBoundsKillSwitchPolicy` - [halt an account when realized P&L breaches bounds](https://github.com/openpitkit/pit/wiki/Policies#pnlboundskillswitchpolicy)

The primary integration model is to write project-specific policies against
the public Rust policy API: [Custom Rust policies](https://github.com/openpitkit/pit/wiki/Policy-API#rust-interface).

Two types of rejections are supported: a full kill switch for the account
and a rejection of only the current request. Kill switches are intended
for algorithmic trading where automatic order submission must be halted
until the situation is analyzed.

## Threading

Canonical contract: [Threading Contract](https://github.com/openpitkit/pit/wiki/Threading-Contract).

Custom policies that need internal state across calls use the built-in
[Storage](https://github.com/openpitkit/pit/wiki/Storage) abstraction. The
synchronization policy - no-sync, full-sync, or caller-sharded for per-key
parallelism - is selected once at engine construction and applied
transparently. Policy code never names a lock primitive; misuse is
prevented at compile time.

1. The SDK never spawns OS threads. Every public method runs on the OS
   thread that invoked it.
2. Preventing concurrent invocation of any public method on the same SDK
   handle is the caller's responsibility. Entering one handle concurrently
   from multiple threads is undefined behavior.
3. Sequential calls to public methods on the same handle from different OS
   threads are supported. Handles, contexts, and callbacks are not pinned
   to a specific thread.
4. `Reject.user_data` / `Order.user_data` / `ExecutionReport.user_data` /
   `AccountAdjustment.user_data` are opaque caller tokens. The SDK never
   inspects, dereferences, or frees them. Lifetime, thread-safety, and
   meaning are entirely caller-managed.

## Usage

<!-- Test mirror: pit/crates/openpit/tests/examples_readme.rs -->
```rust
use std::time::Duration;

use openpit::{
    FinancialImpact, ExecutionReportOperation, OrderOperation,
    WithFinancialImpact, WithExecutionReportOperation,
};
use openpit::param::{
    AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume,
};
use openpit::pretrade::policies::{
    OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy,
    OrderSizeLimitSettings, OrderValidationPolicy,
    PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings,
    RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::storage::NoLocking;
use openpit::{Engine, Instrument};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let usd = Asset::new("USD")?;

// 1. Build the engine builder.
type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;
let builder = Engine::builder::<OrderOperation, Report, ()>().no_sync();

// 2. Configure policies.
let pnl_policy = PnlBoundsKillSwitchPolicy::new(
    PnlBoundsKillSwitchSettings::new(
        [PnlBoundsBrokerBarrier {
            settlement_asset: usd.clone(),
            lower_bound: Some(Pnl::from_str("-1000")?),
            upper_bound: None,
        }],
        [],
    )?,
    builder.storage_builder(),
);

let rate_limit_policy = RateLimitPolicy::new(
    RateLimitSettings::new(
        Some(RateLimitBrokerBarrier {
            limit: RateLimit {
                max_orders: 100,
                window: Duration::from_secs(1),
            },
        }),
        [],
        [],
        [],
    )?,
    builder.storage_builder(),
);

// 3. Build the engine (one time at the platform initialization).
let engine = builder
    .pre_trade(OrderValidationPolicy::new())
    .pre_trade(pnl_policy)
    .pre_trade(rate_limit_policy)
    .pre_trade(OrderSizeLimitPolicy::<NoLocking>::new(
        OrderSizeLimitSettings::new(
            Some(OrderSizeBrokerBarrier {
                limit: OrderSizeLimit {
                    max_quantity: Quantity::from_str("500")?,
                    max_notional: Volume::from_str("100000")?,
                },
            }),
            [OrderSizeAssetBarrier {
                limit: OrderSizeLimit {
                    max_quantity: Quantity::from_str("500")?,
                    max_notional: Volume::from_str("100000")?,
                },
                settlement_asset: usd.clone(),
            }],
            [],
        )?,
    ))
    .build()?;

// 3. Check an order.
let order = OrderOperation {
    instrument: Instrument::new(
        Asset::new("AAPL")?,
        usd.clone(),
    ),
    account_id: AccountId::from_u64(99224416),
    side: Side::Buy,
    trade_amount: TradeAmount::Quantity(
        Quantity::from_f64(100.0)?,
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
let mut reservation = request.execute()?;

// Optional shortcut for the same two-stage flow:
// let reservation = engine.execute_pre_trade(order)?;

// 6. If the request is successfully sent to the venue, it must be committed.
// The rollback must be called otherwise to revert all performed reservations.
reservation.commit();

// 7. The order goes to the venue and returns with an execution report.
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
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
    },
};

let result = engine.apply_execution_report(&report);

// 8. After each execution report is applied, the system may report that it has
// been determined in advance that all subsequent requests will be rejected if
// the account status does not change.
assert!(result.account_blocks.is_empty());
# Ok(())
# }
```

## Errors

Rejects from `start_pre_trade(order)` and `PreTradeRequest::execute()` are
returned as
`Err(Reject)` and `Result<PreTradeReservation, Vec<Reject>>`.

Each `Reject` contains:

- `policy`: policy name
- `code`: stable machine-readable code (for example `RejectCode::OrderQtyExceedsLimit`)
- `reason`: short human-readable reject type (for example `"order quantity exceeded"`)
- `details`: concrete case details (for example `"requested 11, max allowed: 10"`)
- `scope`: `RejectScope::Order` or `RejectScope::Account`
- `user_data`: opaque caller-defined pointer payload (`null` by default)

`RejectCode` values are standardized and stable across Rust, Python, and C FFI.
