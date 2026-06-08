# spot_table

A table-driven test runner for OpenPit's spot funds policy.

The tool runs the table twice in parallel, on two isolated engines — each with
its own market-data service — and prints a per-engine summary report:

1. **sync**  — single-threaded `NoSync` engine with a `NoSync` market-data
   service, operation by operation, in the row order written in the table.
2. **async** — `AccountSync` engine wrapped in `asyncengine.Dynamic`, with a
   `FullSync` market-data service safe for the concurrent feed. Operations for
   the same account are serialized; operations across accounts run in parallel.

Each engine's report shows: total operations (SEED/GROUP/ORDER/FILL rows; TICK
rows are not counted), distinct accounts touched, total wall-clock time to run
the scenario, order-check latency (min/avg/max, n = orders checked), and report
latency (min/avg/max, n = fills/execution reports applied). Each engine stops at
the first verdict mismatch and returns a partial report. The CLI waits for the
slower engine before printing, so both reports are always shown together.

The scenario table format and the bundled tables are documented in
[`examples/tables/spot/README.md`](../../tables/spot/README.md).

## Running

The runner loads the native OpenPit library at run time. The `-table` argument
is required; it is a scenario path relative to the repository root.

### With [Just](https://just.systems/)

From the repository root (`just` builds the native library and wires up the
runtime path for you):

```sh
# Run the coverage scenario (the default argument):
just run-examples-go-table

# Run a specific table:
just run-examples-go-table examples/tables/spot/coverage.md

# Repeat-run a scenario (runs for 3 minutes by default):
just run-examples-go-table-repeat

# Repeat for a specific duration:
just run-examples-go-table-repeat examples/tables/spot/coverage.md 5m

# Run the Go test suite (this example's fast test included):
just test-go
```

### Manual

Build the native library once and point the loader at it:

```sh
cargo build -p openpit-ffi --release
export OPENPIT_RUNTIME_LIBRARY_PATH="$PWD/target/release/libopenpit_ffi.dylib"  # .so on Linux
```

Then, from `examples/go/spot_table/`:

```sh
go run . -table ../../tables/spot/coverage.md                  # run a scenario once
go run . -table ../../tables/spot/coverage.md -min-duration 3m  # repeat-run for 3 minutes
go test ./...                                                  # run the test
```

Running the binary with no `-table` prints a short usage message.

## Reading the report

Each run prints a legend followed by a per-engine block. The legend describes
every field. The per-engine block shows:

- **operations** — SEED/GROUP/ORDER/FILL rows applied (TICK rows excluded)
- **accounts** — distinct accounts touched
- **total time** — wall-clock to complete the scenario on that engine
- **order check** — time to decide one order (the pre-trade check); n/min/avg/max
- **reports** — time to apply one fill / execution report; n/min/avg/max
- **result** — ALL PASS, or the first mismatch with its line, account, and action

A repeat run (`-min-duration d`) re-runs the scenario until at least `d` of
wall-clock has elapsed. Every ~10 s it prints a progress block showing the
current time, iteration count, elapsed and remaining time, and each engine's
running order/report min/avg/max. On completion it prints the host platform
summary and a per-engine aggregate over all iterations.

The two engines' timings are **not** directly comparable: the parallel engine
reports each operation's full submit-to-result round-trip — async dispatch (the
per-account worker handoff) plus any queue wait — while the sequential engine
times only the direct call.
