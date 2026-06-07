# spot_table

A table-driven runner for OpenPit's spot funds policy.

The tool runs a scenario on a single-threaded `no_sync` engine with a `no_sync`
market-data service, operation by operation in the row order written in the
table. It prints a per-run summary report: total operations
(SEED/GROUP/ORDER/FILL rows; TICK rows are not counted), distinct accounts
touched, total wall-clock time to run the scenario, order-check latency
(min/avg/max, n = orders checked), and report latency (min/avg/max, n =
fills/execution reports applied). The run stops at the first verdict mismatch
and returns a partial report.

The scenario table format and the bundled tables are documented in
[`../../tables/spot/README.md`](../../tables/spot/README.md).

## Running

The runner loads the native OpenPit binding at run time. The `--table` argument
is required; it is a scenario path relative to the repository root.

### With [Just](https://just.systems/)

From the repository root (`just python-develop` installs the binding):

```sh
# Install the binding once:
just python-develop

# Run the coverage scenario (the default argument):
just run-examples-python-table

# Run a specific table:
just run-examples-python-table examples/tables/spot/coverage.md

# Repeat-run a scenario (runs for 3 minutes by default):
just run-examples-python-table-repeat

# Repeat for a specific duration:
just run-examples-python-table-repeat examples/tables/spot/coverage.md 5m

# Run the Python test suite (this example's fast test included):
just test-python
```

### Manual

After `just python-develop`, from `examples/python/spot_table/`:

```sh
python main.py --table ../../tables/spot/coverage.md                   # run once
python main.py --table ../../tables/spot/coverage.md --min-duration 3m  # repeat-run
python -m pytest examples/python/spot_table                           # run the test
```

Running with no `--table` prints a short usage message.

### Standalone (against the published package)

To run the example on its own, without the repository-root tooling, install
its self-contained dependencies from this directory and run it:

```sh
pip install -r requirements.txt                        # openpit (published wheel) + pytest
python main.py --table ../../tables/spot/coverage.md   # run once
python -m pytest .                                     # run the test
```

## Tests

- **`test_fast`** - the quick check (well under a second). It runs the coverage
  scenario once and asserts every row's verdict, so it doubles as an end-to-end
  check of the CLI's own scenario.

For a sustained-load / soak run, use the CLI repeat (`--min-duration`, or
`just run-examples-python-table-repeat`); see "Running" above.

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
