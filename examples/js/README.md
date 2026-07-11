# JS examples

Runnable Node and browser examples that integrate the `@openpit/engine` SDK.
The Node examples cover the shared engine scenarios; the browser terminal is the
WASM-specific end-to-end demo.

- [`rate_pnl_killswitch`](rate_pnl_killswitch) - a RateLimit + PnlBoundsKillSwitch
  supervisor that halts a runaway strategy.
- [`spot_funds`](spot_funds) - the smallest SpotFunds integration: reserve,
  reject the duplicate, settle the fill.
- [`spot_table`](spot_table) - a table-driven SpotFunds runner over the shared
  scenario tables in [`../tables/spot`](../tables/spot).
- [`browser_terminal`](browser_terminal) - a Vite-built terminal that runs the
  engine entirely in the browser with inlined WASM.

## Prerequisites

`@openpit/engine` is a self-contained WASM package: no native add-on to compile
and no `await` in the common path. The examples depend on the locally built
package via a `file:` reference, so build it once first. From the repository
root:

```sh
just install                         # provisions the toolchain (once)
cd bindings/js && npm run build      # builds bindings/js/dist
```

## Running

From this directory, a single `npm install` links the built package into all
four examples (they are an npm workspace):

```sh
npm install

npx tsx rate_pnl_killswitch/main.ts
npx tsx spot_funds/main.ts
npx tsx spot_table/main.ts --table ../tables/spot/coverage.md

npm test     # Node smoke tests plus the browser production build
```

From the repository root, the `just` targets wrap the same commands:

```sh
just run-examples-js         # run all three Node examples
just run-examples-js-table   # spot_table over the coverage table
just test-js                 # run Node tests and build the browser demo
```

## Idiomatic inputs

Prices, quantities, and money cross the engine boundary as decimal **strings**,
the only lossless form (`"185"`, never `185.0`); value types serialize back to a
decimal string and never return a raw `number` for money. Beyond decimals, these
examples use the idiomatic plain inputs the engine accepts everywhere: orders,
reports, and adjustments are written as plain object literals, scalars pass as
plain values (an account id as a bigint, a side as a string). The wrapper classes
(`Price`, `AccountId`, ...) remain available as a typed alternative. See the
package [`README`](../../bindings/js/README.md) for the full input contract.
