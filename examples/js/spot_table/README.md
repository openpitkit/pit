# spot_table

A table-driven runner for OpenPit's spot funds policy.

The tool runs a scenario on a single-threaded engine with a no-op-locking
market-data service, operation by operation in the row order written in the
table. It prints a per-run summary report: total operations
(SEED/GROUP/ORDER/FILL rows; TICK rows are not counted), distinct accounts
touched, total wall-clock time to run the scenario, order-check latency
(min/avg/max, n = orders checked), and report latency (min/avg/max, n =
fills/execution reports applied). The run stops at the first verdict mismatch
and returns a partial report.

The scenario table format and the bundled tables are documented in
[`../../tables/spot/README.md`](../../tables/spot/README.md). This example
consumes the shared `coverage.md` scenario that defines the common domain
contract exercised by every runner.

## Layout

The example is split into focused modules:

- `main.ts` - CLI: argument parsing, run-once / repeat-run, report printing.
- `table.ts` - the scenario-table parser (front-matter + GFM pipe table).
- `builder.ts` - domain-object builders (orders, reports, seed adjustments).
- `marketFeed.ts` - the market-data feed that replays TICK rows live.
- `runner.ts` - the sequential engine, run loop, and verdict checks.
- `platformInfo.ts` - the host platform summary printed atop each run.

## Running

The runner imports `@openpit/engine`, a self-contained WASM package - there is
no native add-on to compile. The `--table` argument is required; it is a
scenario path that resolves whether you run from the repository root or from
this directory.

The package must be built first. From the repository root:

```sh
just install                         # provisions the toolchain (once)
cd bindings/js && npm run build      # builds dist/ consumed by these examples
```

### With [Just](https://just.systems/)

From the repository root:

```sh
# Run the coverage scenario (the default argument):
just run-examples-js-table

# Run a specific table:
just run-examples-js-table examples/tables/spot/coverage.md

# Run the JS test suite (this example's fast test included):
just test-js
```

### Manual

From `examples/js/` (one `npm install` covers all three examples):

```sh
npm install                                                   # links the package
npx tsx spot_table/main.ts --table ../tables/spot/coverage.md # run once
npx tsx spot_table/main.ts --table ../tables/spot/coverage.md --min-duration 3m
npm test                                                      # run the test
```

Or from this directory (`examples/js/spot_table/`) once `npm install` has run
in `examples/js/`:

```sh
npm start    # run the coverage scenario (tsx main.ts --table ...)
npm test     # run this example's fast test (vitest)
```

Running with no `--table` prints a short usage message.

## Tests

- **`spot_table`** - the quick check (well under a second). It runs the coverage
  scenario once and asserts every row's verdict, so it doubles as an end-to-end
  check of the CLI's own scenario.

For a sustained-load / soak run, use the CLI repeat (`--min-duration`); see
"Running" above.

## Reading the report

Each run prints a legend followed by the engine's report block. The legend
describes every field. The block shows:

- **operations** - SEED/GROUP/ORDER/FILL rows applied (TICK rows excluded)
- **accounts** - distinct accounts touched
- **total time** - wall-clock to complete the scenario
- **order check** - time to decide one order (the pre-trade check); n/min/avg/max
- **reports** - time to apply one fill / execution report; n/min/avg/max
- **result** - ALL PASS, or the first mismatch with its line, account, and action

A repeat run (`--min-duration d`) re-runs the scenario until at least `d` of
wall-clock has elapsed. Every ~10 s it prints a progress block showing the
current time, iteration count, elapsed and remaining time, and the engine's
running order/report min/avg/max. On completion it prints the host platform
summary and an aggregate over all iterations.
