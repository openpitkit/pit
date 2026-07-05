# spot_table (Go)

Runs a scenario table through two OpenPit engines — a sequential one and a
parallel async one — and prints how fast each was. Use it to get a feel for the
engine's speed without writing any code.

## Build & run

From the repository root (this builds the native library and wires up the
runtime path for you, then runs the bundled `coverage` scenario):

```sh
just run-examples-go-table-debug
```

Other tables, and a soak run:

```sh
just run-examples-go-table-debug examples/tables/spot/coverage.md        # pick a table
just run-examples-go-table-repeat-debug examples/tables/spot/coverage.md 5m # 5m soak
just test-go-debug                                                       # run the tests
```

No `just`? Build the native library once, point the loader at it, then run from
`examples/go/spot_table/`:

```sh
cargo build -p openpit-ffi --release
# .so on Linux, .dll on Windows:
export OPENPIT_RUNTIME_LIBRARY_PATH="$PWD/target/release/libopenpit_ffi.dylib"

go run . -table ../../tables/spot/coverage.md
```

`-table` is required. Add `-min-duration 3m` to repeat-run for at least the
given time (soak / sustained-load).

## Reading the report

You get one block per engine — **sync** (sequential) and **async** (parallel):

- **operations** — orders/fills/etc. applied (price ticks excluded)
- **accounts** — distinct accounts touched
- **total time** — wall-clock to run the whole scenario
- **order check** — pre-trade decision latency, as n / min / avg / max
- **reports** — fill-application latency, as n / min / avg / max
- **result** — `ALL PASS`, or the first row that disagreed (line, account, action)

The two engines' numbers are **not** comparable: async times the full
submit-to-result round trip (dispatch + queue wait), sync times the direct call.

A repeat run prints progress every ~10 s and ends with a host summary plus
per-engine aggregates over all iterations.

The scenario table format is documented in
[`examples/tables/spot/README.md`](../../tables/spot/README.md).
