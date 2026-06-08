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

From the repository root (`just python-develop` installs the binding):

```sh
# Install the binding once:
just python-develop

# Run this example (also run by just run-examples-python):
just run-examples-python

# Run the Python test suite (this example's smoke test included):
just test-python
```

### Manual

After `just python-develop`, from `examples/python/spot_funds/`:

```sh
python main.py          # run the scenario
python -m pytest .      # run the smoke test
```

## See also

- [SpotFunds wiki page](https://github.com/openpitkit/pit/wiki/Spot-Funds) -
  the full policy reference (market orders, slippage, pricing source, fee
  conventions).
- [`../spot_table`](../spot_table) - a table-driven / load-testing harness
  around the same policy, covering market orders and concurrent execution.
