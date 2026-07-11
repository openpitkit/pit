# Examples

Runnable end-to-end examples that integrate the OpenPit SDK from each binding,
plus the scenario tables they share. Each example has its own README with the
full story; this page is the index and the one-command entry points.

## Layout

- `go/` — Go examples (`rate_pnl_killswitch`, `spot_funds`, `spot_table`).
- `python/` — the same examples for the Python binding.
- `js/` — the same Node examples for the JS binding, plus the
  `browser_terminal` demo.
- `cpp/` — the same examples for the C++ binding.
- `tables/` — scenario tables consumed by the `spot_table` examples
  (see [`tables/spot/README.md`](tables/spot/README.md)).

## How to run

From the repository root, run every example against local sources:

```sh
just run-examples-debug        # every language
just run-examples-go-debug     # Go only
just run-examples-python-debug # Python only
just run-examples-js           # JS only (one WASM build mode)
just run-examples-cpp-debug    # C++ only
```

The JS examples consume the locally built `@openpit/engine` package; build it
once first with `just install` and `cd bindings/js && npm run build` (see
[`js/README.md`](js/README.md)).

To run a single example standalone, see its own README, e.g.
[`python/spot_funds`](python/spot_funds/README.md) (published package),
[`js/spot_funds`](js/spot_funds/README.md) (local build), or
[`cpp/spot_funds`](cpp/spot_funds/README.md) (published package).

## How to run unit tests

Each example ships a smoke test, included in the per-language suites:

```sh
just test-go-debug            # Go examples' tests (plus the binding tests)
just test-python-debug        # Python examples' tests (plus the binding tests)
just test-js                  # JS examples' tests (plus the binding tests)
just test-examples-cpp-debug  # build the C++ examples and run their smoke tests
```
