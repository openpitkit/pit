# rate_pnl_killswitch

An independent supervisor that wraps OpenPit's **RateLimit** and
**PnlBoundsKillSwitch** policies around a TypeScript strategy, so a runaway
strategy is halted before it floods the venue with orders or burns through its
loss budget. `main()` builds an engine with the two kill-switch policies
side-by-side, feeds it a single `Event` stream (orders + fills), keeps
venue/strategy side-effects behind a `Reactor` interface, and aggregates
accepted/rejected counts, pre-trade latency, and cumulative P&L over the run.
The point is the supervisor pattern: the engine decides, the reactor acts, and
the strategy is stopped the moment a kill switch returns an account block.

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
# Run every JS example, this one included:
just run-examples-js

# Run the JS test suite (this example's smoke test included):
just test-js
```

### Manual

From `examples/js/` (one `npm install` covers all three examples):

```sh
npm install                              # links the locally built package
npx tsx rate_pnl_killswitch/main.ts      # run the scenario
npm test                                 # run every example's smoke test
```

Or from this directory (`examples/js/rate_pnl_killswitch/`) once
`npm install` has run in `examples/js/`:

```sh
npm start    # run the scenario (tsx main.ts)
npm test     # run this example's smoke test (vitest)
```

## Expected output

The burst of 105 attempts overshoots the rate-limit ceiling of 100, so the tail
of the burst is rejected with `RateLimitExceeded`. The 100 accepted orders then
produce 99 small-loss reports plus one large-loss report; cumulative P&L reaches
-509.50 USD, past the -500 floor, and the kill switch trips on the final trade.

## See also

- [RateLimitPolicy](https://wiki.openpit.dev/Policies/#ratelimitpolicy)
  and [PnlBoundsKillSwitchPolicy][pnl-policy] -
  the policy references for the two kill switches combined here.
- [`../spot_funds`](../spot_funds) - the smallest single-policy integration, a
  good starting point before this multi-policy supervisor.

[pnl-policy]: https://wiki.openpit.dev/Policies/#pnlboundskillswitchpolicy
