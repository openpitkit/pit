# OpenPit: Pre-trade Integrity Toolkit

<!-- markdownlint-disable MD013 -->
[![Verify](https://github.com/openpitkit/pit/actions/workflows/verify.yml/badge.svg)](https://github.com/openpitkit/pit/actions/workflows/verify.yml) [![Release](https://img.shields.io/github/v/release/openpitkit/pit)](https://github.com/openpitkit/pit/releases) [![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://crates.io/crates/openpit) [![crates.io](https://img.shields.io/crates/v/openpit)](https://crates.io/crates/openpit) [![docs.rs](https://img.shields.io/docsrs/openpit)](https://docs.rs/openpit/latest/openpit/) [![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/openpitkit/pit/blob/main/LICENSE)
<!-- markdownlint-enable MD013 -->

`openpit` is an embeddable pre-trade risk SDK for integrating policy-driven
risk checks into trading systems.

For an overview and links to all resources, see
the project website [openpit.dev](https://openpit.dev/).
For full project documentation, see
[the repository README](https://github.com/openpitkit/pit/blob/main/README.md).
For conceptual and architectural pages, see
[the project wiki](https://wiki.openpit.dev/).

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

## Quick Start

<!-- Test mirror: crates/openpit/tests/examples_readme.rs -->

```rust
use openpit::param::{AccountId, Asset, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::{
    OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitSettings,
};
use openpit::storage::NoLocking;
use openpit::{Engine, Instrument, OrderOperation};

// One fat-finger control, wired once at startup.
let engine = Engine::builder::<OrderOperation, (), ()>()
    .no_sync()
    .pre_trade(OrderSizeLimitPolicy::<NoLocking>::new(
        OrderSizeLimitSettings::new(
            Some(OrderSizeBrokerBarrier {
                limit: OrderSizeLimit {
                    max_quantity: Quantity::from_str("500")?,
                    max_notional: Volume::from_str("100000")?,
                },
            }),
            [],
            [],
        )?,
    ))
    .build()?;

let order = OrderOperation {
    instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
    account_id: AccountId::from_u64(99224416),
    side: Side::Buy,
    trade_amount: TradeAmount::Quantity(Quantity::from_str("1000")?),
    price: Some(Price::from_str("185")?),
};

// The verdict: either a reservation to finalize once the venue answers,
// or the rejects that stopped the order.
match engine.execute_pre_trade(order) {
    Ok(mut reservation) => reservation.commit(),
    Err(rejects) => println!("order rejected: {rejects}"),
}
```

The explicit two-stage flow, drop copy, and post-trade reports are described
on the [Pre-trade Pipeline](https://wiki.openpit.dev/Pre-trade-Pipeline/)
page.

## What Is Inside

- `SpotFundsPolicy` - [per-account solvency gate over spendable funds](https://wiki.openpit.dev/Spot-Funds/)
- `OrderValidationPolicy` - [structural integrity checks on every order](https://wiki.openpit.dev/Policies/#ordervalidationpolicy)
- `RateLimitPolicy` - [throttle order flow per broker, asset, or account](https://wiki.openpit.dev/Policies/#ratelimitpolicy)
- `OrderSizeLimitPolicy` - [fat-finger caps on quantity and notional](https://wiki.openpit.dev/Policies/#ordersizelimitpolicy)
- `PnlBoundsKillSwitchPolicy` - [halt an account when realized P&L breaches bounds](https://wiki.openpit.dev/Policies/#pnlboundskillswitchpolicy)
- [Custom Rust policies](https://wiki.openpit.dev/Policy-API/#rust-interface)
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
- [Storage](https://wiki.openpit.dev/Storage/) for policy state, with the
  synchronization mode selected once at engine construction - see the
  [Threading Contract](https://wiki.openpit.dev/Threading-Contract/).
- [Rejects and errors](https://wiki.openpit.dev/Errors/) with
  [stable reject codes](https://wiki.openpit.dev/Reject-Codes/).
