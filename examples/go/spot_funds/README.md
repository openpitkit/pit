# spot_funds

The smallest end-to-end integration of OpenPit's built-in **SpotFunds**
pre-trade policy. `main()` reads top-to-bottom as a story: build a limit-only
engine, seed an account with 100000 USD, accept a BUY of 30 AAPL @ 2000 (which
holds 60000 USD), watch an identical second BUY get rejected with
`InsufficientFunds` because that cash is still held, then fill the first order
so its reservation settles. The point is the reservation mechanic - a committed
order reduces available funds until it fills - and how a fill is tied back to
its reservation by carrying the pre-trade lock on the execution report.

## Running

The example loads the native OpenPit library at run time.

### With [Just](https://just.systems/)

From the repository root (`just` builds the native library and wires up the
runtime path for you):

```sh
# Run every Go example, this one included:
just run-examples-go

# Run the Go test suite (this example's smoke test included):
just test-go
```

### Manual

Build the native library once and point the loader at it:

```sh
cargo build -p openpit-ffi --release
export OPENPIT_RUNTIME_LIBRARY_PATH="$PWD/target/release/libopenpit_ffi.dylib"  # .so on Linux
```

Then, from `examples/go/spot_funds/`:

```sh
go run .          # run the scenario
go test ./...     # run the smoke test
```

## See also

- [SpotFunds wiki page](https://github.com/openpitkit/pit/wiki/Spot-Funds) -
  the full policy reference (market orders, slippage, pricing source, fee
  conventions).
- [`../spot_table`](../spot_table) - a table-driven / load-testing harness
  around the same policy, covering market orders and concurrent execution.
