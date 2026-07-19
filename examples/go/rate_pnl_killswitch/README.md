# rate_pnl_killswitch

An independent supervisor that wraps OpenPit's **RateLimit** and
**PnlBoundsKillSwitch** policies around a Go strategy, so a runaway strategy is
halted before it floods the venue with orders or burns through its loss budget.
`main()` builds an engine with the two kill-switch policies side-by-side, feeds
it a single `Event` stream (orders + fills), keeps venue/strategy side-effects
behind a `Reactor` interface, and aggregates accepted/rejected counts,
pre-trade latency, and cumulative P&L over the run. The point is the supervisor
pattern: the engine decides, the reactor acts, and the strategy is stopped the
moment a kill switch returns an account block.

## Running

The example loads the native OpenPit library at run time.

### With [Just](https://just.systems/)

From the repository root (`just` builds the native library and wires up the
runtime path for you):

```sh
# Run every Go example, this one included:
just run-examples-go-debug

# Run the Go test suite (this example's smoke test included):
just test-go-debug
```

### Manual

Build the native library once and point the loader at it:

<!-- markdownlint-disable MD013 -->

```sh
cargo build -p openpit-ffi --release
export OPENPIT_RUNTIME_LIBRARY_PATH="$PWD/target/release/libopenpit_ffi.dylib"  # .so on Linux
```

<!-- markdownlint-enable MD013 -->

Then, from `examples/go/rate_pnl_killswitch/`:

```sh
go run .          # run the scenario
go test ./...     # run the smoke test
```

The example's own dependencies are declared in its `go.mod`; the release flow
drops the local `replace` directive and pins the require to the published
version so the example exercises exactly what an SDK consumer sees.

## See also

<!-- markdownlint-disable MD013 -->

- [RateLimitPolicy](https://wiki.openpit.dev/Policies/#ratelimitpolicy)
  and [PnlBoundsKillSwitchPolicy](https://wiki.openpit.dev/Policies/#pnlboundskillswitchpolicy) -
  the policy references for the two kill switches combined here.
- [`../spot_funds`](../spot_funds) - the smallest single-policy integration, a
  good starting point before this multi-policy supervisor.

<!-- markdownlint-enable MD013 -->
