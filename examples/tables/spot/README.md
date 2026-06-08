# Spot scenario tables

Markdown scenario tables for the OpenPit spot test utilities (the [`spot_table`](../../go/spot_table)
runner). This document describes only the table format; each file under this
directory is one self-contained scenario.

## Files

- `coverage.md` — one scenario that uses every feature: all five actions
  (`SEED`, `GROUP`, `TICK`, `ORDER`, `FILL`), global / account- / group-addressed
  market data, quantity- and volume-denominated orders, market and limit orders,
  a buy and a sell of each kind, and the two deliberate rejects (over-budget buy,
  oversold sell).

## Table format

A scenario file is a Markdown document with an optional front-matter block and
one [GFM](https://github.github.com/gfm/) pipe-table.

### Front-matter

The front-matter is a small `key: value` block delimited by `---` lines. It is
**not** full YAML — there are no nested fields, lists, quoting, or anchors. It
supports exactly two keys, `name` and `slippage_bps`, and any line starting with
`#` is treated as a comment.

```text
---
name: human-readable scenario label
slippage_bps: 1500   # worst-case slippage applied to market orders, in bps
---
```

`slippage_bps = 1500` means 15 % (1 bps = 0.01 %). Set `0` to disable slippage;
market orders are still accepted.

### Columns

Columns are matched by header name, so their order is free and extra columns are
ignored. A table must declare at least `account`, `action`, and `expect`; every
other column below is read when present.

| column       | meaning                                                      |
|--------------|--------------------------------------------------------------|
| `#`          | row number, documentary only                                 |
| `account`    | free-form account label; reused engine-side as a stable ID   |
| `action`     | `SEED`, `GROUP`, `TICK`, `ORDER`, or `FILL`                  |
| `instrument` | `BASE/QUOTE`, for example `AAPL/USD`                         |
| `side`       | `BUY` or `SELL`                                              |
| `qty`        | decimal quantity (one order-amount denomination)             |
| `volume`     | decimal settlement notional (the other order denomination)   |
| `price`      | decimal limit / mark / lock price; empty on ORDER = market   |
| `asset`      | settlement-asset code for `SEED`                             |
| `amount`     | absolute starting amount for `SEED`                          |
| `fee`        | execution fee on `FILL` (defaults to `0`)                    |
| `pnl`        | realized PnL on `FILL` (defaults to `0`)                     |
| `group`      | account-group label for `GROUP` and addressed `TICK`         |
| `expect`     | `OK`, `ACCEPT`, or `REJECT`                                  |
| `reject`     | expected reject code when `expect = REJECT`                  |
| `note`       | free-form comment, ignored                                   |

### Actions

- `SEED`  — credits `amount` of `asset` to `account` as an absolute starting
  balance. `expect = OK` (or `REJECT` to assert a refusal).
- `GROUP` — registers `account` into account-group `group`. All `GROUP` rows are
  aggregated and registered **before** any `ORDER`, `FILL`, or addressed `TICK`,
  so later rows can rely on the membership. `expect = OK`.
- `TICK`  — publishes a live mark price of `instrument` to the market-data
  service at the row's position. Addressing:
  - empty `account` **and** empty `group` → a global snapshot every account
    reads by default;
  - a non-empty `account` and/or `group` → an addressed snapshot that replaces
    the quote for those targets only, so it sizes only their market orders.

  `expect = OK`.
- `ORDER` — places a buy/sell order. Set **exactly one** of `qty` or `volume`:
  `qty` reserves against a base quantity, `volume` reserves the settlement
  notional directly. Empty `price` means a market order, sized from the live
  quote the ordering account reads; a non-empty `price` is a limit order (which
  may itself be quantity- or volume-denominated). `expect = ACCEPT` or `REJECT`;
  on `REJECT` the `reject` cell carries the expected reject code
  (case-insensitive). Recognised codes include `InsufficientFunds`,
  `MarkPriceUnavailable`, `UnsupportedOrderType`,
  `AccountAdjustmentBoundsExceeded`, `OrderValueCalculationFailed`,
  `InvalidFieldFormat`, `InvalidFieldValue`, `InsufficientPosition`,
  `InsufficientMargin`, `MissingRequiredField`.
- `FILL`  — applies a final execution report. `qty` is the filled quantity
  (fills are always quantity-based; `volume` is not used), and `price` is the
  lock / reservation price (the limit price for limit orders, the mark price for
  market orders). When `price` is omitted the most recent quote pushed for the
  instrument is reused. `fee` and `pnl` are the financial impact.

### TICK determinism

`TICK` rows are replayed live, in row order, against each engine's own
market-data service — there is no load-time pre-aggregation.

- **Addressed** ticks (with an `account` and/or `group`) are safe anywhere. In
  the parallel engine an addressed tick is replayed only after the outstanding
  operations of its target account(s) have executed, and before that account's
  later rows are submitted: the new quote reaches the target's later orders
  without rewriting its earlier ones, while other accounts keep running.
- **Global** (unaddressed) ticks are only safe in the setup block, before the
  first `ORDER`. A global tick changes every account at once, and in the parallel
  engine its timing relative to in-flight orders on other accounts is ambiguous,
  which can produce a spurious verdict mismatch between the two engines. When a
  tick must take effect after orders have started, address it to the account or
  group it concerns.

## Limitations

- `FILL` rows always emit final (`IsFinal = true`) execution reports with
  `LeavesQuantity = 0`. Partial fills and cancel-with-leftover scenarios are not
  modelled.
- Account IDs are derived from the table label via `param.NewAccountIDFromString`,
  and account-group IDs via `param.NewAccountGroupIDFromString`; both FNV-1a hash
  the input. Two different labels produce two different engine identifiers.
