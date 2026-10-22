# spot_funds

The smallest end-to-end integration of OpenPit's built-in **SpotFunds**
pre-trade policy. `main()` reads top-to-bottom as a story: build a
limit-only engine, seed an account with 100000 USD, accept a BUY of
30 AAPL @ 2000 (which holds 60000 USD), watch an identical second BUY get
rejected with `InsufficientFunds` because that cash is still held, then
fill the first order so its reservation settles. The point is the
reservation mechanic - a committed order reduces available funds until it
fills - and how a fill is tied back to its reservation by carrying the
pre-trade lock on the execution report.

## Running

The example loads the native OpenPit binding at run time.

### With [Just](https://just.systems/)

From the repository root (`just python-develop-debug` installs the binding):

```sh
# Install the binding once:
just python-develop-debug

# Run this example (also run by just run-examples-python-debug):
just run-examples-python-debug

# Run the Python test suite (this example's smoke test included):
just test-python-debug
```

### Manual

From the repository root, with Rust and Python >=3.10 installed:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements.txt
python -m maturin develop --manifest-path bindings/python/Cargo.toml
cd examples/python/spot_funds
```

On Windows, create the environment with `py -3 -m venv .venv` and activate it
with `.venv\Scripts\Activate.ps1` in PowerShell. Just is optional.
Then, in the same activated environment:

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

- [SpotFunds wiki page](https://wiki.openpit.dev/Spot-Funds/) -
  the full policy reference (market orders, slippage, pricing source, fee
  conventions).
- [`../spot_table`](../spot_table) - a table-driven / load-testing harness
  around the same policy, covering market orders and concurrent execution.
