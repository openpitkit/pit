# rate_pnl_killswitch

An independent supervisor that wraps OpenPit's **RateLimit** and
**PnlBoundsKillSwitch** policies around a Python strategy, so a runaway
strategy is halted before it floods the venue with orders or burns through its
loss budget. `main()` builds an engine with the two kill-switch policies
side-by-side, feeds it a single `Event` stream (orders + fills), keeps
venue/strategy side-effects behind a `Reactor` protocol, and aggregates
accepted/rejected counts, pre-trade latency, and cumulative P&L over the run.
The point is the supervisor pattern: the engine decides, the reactor acts, and
the strategy is stopped the moment a kill switch returns an account block.

## Running

The example loads the native OpenPit binding at run time.

### With [Just](https://just.systems/)

From the repository root (`just python-develop-debug` installs the binding):

```sh
# Install the binding once:
just python-develop-debug

# Run every Python example, this one included:
just run-examples-python-debug

# Run the Python test suite (this example's smoke test included):
just test-python-debug
```

### Manual

After `just python-develop-debug`, from `examples/python/rate_pnl_killswitch/`:

```sh
python main.py          # run the scenario
python -m pytest .      # run the smoke test
```

### Standalone (against the published package)

To run the example on its own, without the repository-root tooling, install
its self-contained dependencies from this directory and run it:

```sh
pip install -r requirements.txt   # openpit (published wheel) + pytest
python main.py                    # run the scenario
python -m pytest .                # run the smoke test
```

## See also

- [RateLimitPolicy](https://github.com/openpitkit/pit/wiki/Policies#ratelimitpolicy)
  and [PnlBoundsKillSwitchPolicy](https://github.com/openpitkit/pit/wiki/Policies#pnlboundskillswitchpolicy) -
  the policy references for the two kill switches combined here.
- [`../spot_funds`](../spot_funds) - the smallest single-policy integration, a
  good starting point before this multi-policy supervisor.
