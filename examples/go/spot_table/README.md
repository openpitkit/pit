# spot_table (Go)

Runs a scenario table through two OpenPit engines - a sequential one and a
parallel async one - and prints how fast each was. Use it to get a feel for the
engine's speed without writing any code.

## Build & run

From the repository root (this builds the native library and wires up the
runtime path for you, then runs the bundled `coverage` scenario):

```sh
just run-examples-go-table-debug
```

Other tables, and a soak run:

```sh
# Pick a table.
just run-examples-go-table-debug examples/tables/spot/coverage.md
# Run a five-minute soak.
just run-examples-go-table-repeat-debug examples/tables/spot/coverage.md 5m
# Run the tests.
just test-go-debug
```

[Just](https://just.systems/) is optional. Without it, start from the
repository root, build the native library once, and point the loader at it:

```sh
cargo build -p openpit-ffi --release
# .so on Linux, .dll on Windows:
export OPENPIT_RUNTIME_LIBRARY_PATH="$PWD/target/release/libopenpit_ffi.dylib"
cd examples/go/spot_table
go run . -table ../../tables/spot/coverage.md
go test ./...
```

`-table` is required. Add `-min-duration 3m` to repeat-run for at least the
given time (soak / sustained-load).

## Reading the report

You get one block per engine - **sync** (sequential) and **async** (parallel):

- **operations** - orders/fills/etc. applied (price ticks excluded)
- **accounts** - distinct accounts touched
- **total time** - wall-clock to run the whole scenario
- **order check** - pre-trade decision latency, as n / min / avg / max
- **reports** - fill-application latency, as n / min / avg / max
- **result** - `ALL PASS`, or the first row that disagreed (line, account, action)

The two engines' numbers are **not** comparable: async times the full
submit-to-result round trip (dispatch + queue wait), sync times the direct call.

A repeat run prints progress every ~10 s and ends with a host summary plus
per-engine aggregates over all iterations.

The scenario table format is documented in
[`examples/tables/spot/README.md`](../../tables/spot/README.md).
