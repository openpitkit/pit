# spot_funds

The smallest end-to-end integration of OpenPit's built-in **SpotFunds**
pre-trade policy. `main()` reads top-to-bottom as a story: build a limit-only
engine, seed an account with 100000 USD, accept a BUY of 30 AAPL @ 2000 (which
holds 60000 USD), watch an identical second BUY get rejected with
`InsufficientFunds` because that cash is still held, then fill the first order
so its reservation settles. The point is the reservation mechanic - a committed
order reduces available funds until it fills - and how a fill is tied back to
its reservation by carrying the pre-trade lock on the execution report. The
last two steps then switch SpotFunds to track-only at runtime and configure its
single account-wide P&L barrier, then correct its live account P&L state,
without rebuilding the engine.

## Running

The example imports `@openpit/engine`, a self-contained WASM package - there is
no native add-on to compile and no `await` in the common path.

The package must be built first. From the repository root:

```sh
just install                         # provisions the toolchain (once)
cd bindings/js && npm run build      # builds dist/ consumed by these examples
```

### With [Just](https://just.systems/)

From the repository root:

```sh
# Run this example (also run by just run-examples-js):
just run-examples-js

# Run the JS test suite (this example's smoke test included):
just test-js
```

### Manual

From `examples/js/` (one `npm install` covers all three examples):

```sh
npm install                  # links the locally built package
npx tsx spot_funds/main.ts   # run the scenario
npm test                     # run every example's smoke test
```

Or from this directory (`examples/js/spot_funds/`) once `npm install` has run
in `examples/js/`:

```sh
npm start    # run the scenario (tsx main.ts)
npm test     # run this example's smoke test (vitest)
```

## A note on inputs

Decimals cross the boundary as strings, the lossless form (`"2000"`, never
`2000.0`). Beyond that, the engine takes idiomatic plain inputs: this example
builds every order and execution report as a plain object literal, passes the
account id as a bigint, and passes prices and quantities as decimal strings - no
wrapper objects to construct. The one wrapper it keeps is the reservation
`Lock`, captured at commit time and carried back on the fill so SpotFunds
settles that exact reservation.

The wrapper classes (`AccountId`, `Price`, ...) remain available as a typed
alternative. Ordinary setters and input coercions borrow or clone wrapper
values, so passing a scalar wrapper, `Order`, or `ExecutionReport` does not
invalidate the caller's handle. Use `.clone()` only when the application needs
an independent copy. Lifecycle handles and staged builders are the intentional
exception: terminal operations consume `Request`, `Reservation`, `Mutation`,
and consumed engine builders.

## See also

- [SpotFunds wiki page](https://github.com/openpitkit/pit/wiki/Spot-Funds) -
  the full policy reference (market orders, slippage, pricing source, fee
  conventions).
- [`../spot_table`](../spot_table) - a table-driven harness around the same
  policy, covering market orders, account groups, and addressed market data.
