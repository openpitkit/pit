# Examples

Runnable end-to-end examples that integrate the OpenPit SDK from each binding,
plus the scenario tables they share. Each example has its own README with the
full story; this page is the index and the local-source entry points.

## Layout

- `go/` - Go examples (`rate_pnl_killswitch`, `spot_funds`, `spot_table`).
- `python/` - the same examples for the Python binding.
- `js/` - the same Node examples for the JS binding, plus the
  `risk_playground` browser demo.
- `cpp/` - the same examples for the C++ binding.
- `tables/` - scenario tables consumed by the `spot_table` examples
  (see [`tables/spot/README.md`](tables/spot/README.md)).

## How to run

Just is optional. The commands below run against local sources, starting from
the repository root in a POSIX shell. Install the relevant language toolchains
from [Local Build and Test](../README.md#local-build-and-test) first.
For Windows commands and published-package usage, see each example's README.

### Go

Build the native runtime and set its absolute path before entering an example:

```sh
cargo build -p openpit-ffi --locked
export OPENPIT_RUNTIME_LIBRARY_PATH="$PWD/target/debug/libopenpit_ffi.dylib"
(cd examples/go/rate_pnl_killswitch && go run .)
(cd examples/go/spot_funds && go run .)
(cd examples/go/spot_table && go run . -table ../../tables/spot/coverage.md)
```

On Linux, use `libopenpit_ffi.so` instead of `libopenpit_ffi.dylib`.
The separate [`spot_loadtest`](go/spot_loadtest/README.md) harness requires a
release runtime and its own configuration.

### Python

With Rust and Python >=3.10 installed:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements.txt
python -m maturin develop --manifest-path bindings/python/Cargo.toml
python examples/python/rate_pnl_killswitch/main.py
python examples/python/spot_funds/main.py
python examples/python/spot_table/main.py --table examples/tables/spot/coverage.md
```

### JavaScript

Install the JS/WASM toolchain and build the local `@openpit/engine` package
using the [JS prerequisites](js/README.md#prerequisites). Then, from the
repository root:

```sh
cd examples/js
npm ci
npx tsx rate_pnl_killswitch/main.ts
npx tsx spot_funds/main.ts
npx tsx spot_table/main.ts --table ../tables/spot/coverage.md
```

The browser [`risk_playground`](js/risk_playground/README.md) has its own npm
commands.

### C++

With Rust, CMake >=3.21, and a C++17 compiler installed:

```sh
cargo build -p openpit-ffi --locked
cmake -S examples/cpp -B examples/cpp/build-debug \
  -DCMAKE_BUILD_TYPE=Debug \
  -DOPENPIT_RUNTIME_LIBRARY="$PWD/target/debug/libopenpit_ffi.dylib"
cmake --build examples/cpp/build-debug --config Debug
./examples/cpp/build-debug/rate_pnl_killswitch/rate_pnl_killswitch
./examples/cpp/build-debug/spot_funds/spot_funds
./examples/cpp/build-debug/spot_table/spot_table --table examples/tables/spot/coverage.md
```

On Linux, use `libopenpit_ffi.so` instead of `libopenpit_ffi.dylib`.
See [`spot_loadtest`](cpp/spot_loadtest/README.md) for the separate load harness.

### Optional Just shortcuts

From the repository root, [Just](https://just.systems/) wraps these flows:

```sh
just run-examples-debug        # every language
just run-examples-go-debug     # Go only
just run-examples-python-debug # Python only
just run-examples-js           # JS only (one WASM build mode)
just run-examples-cpp-debug    # C++ only
```

To run a single example standalone, see its own README, e.g.
[`python/spot_funds`](python/spot_funds/README.md) (published package),
[`js/spot_funds`](js/spot_funds/README.md) (local build), or
[`cpp/spot_funds`](cpp/spot_funds/README.md) (published package).

## How to run unit tests

After the corresponding setup above, run from the repository root:

```sh
# Go: retain OPENPIT_RUNTIME_LIBRARY_PATH from the setup.
(cd bindings/go && go test ./...)
(cd examples/go/rate_pnl_killswitch && go test ./...)
(cd examples/go/spot_funds && go test ./...)
(cd examples/go/spot_table && go test ./...)

# Python: retain the activated environment.
python -m pytest bindings/python/tests
python -m pytest examples/python/rate_pnl_killswitch
python -m pytest examples/python/spot_funds
python -m pytest examples/python/spot_table

# JavaScript:
npm --prefix bindings/js test
npm --prefix examples/js test

# C++:
ctest --test-dir examples/cpp/build-debug -C Debug --output-on-failure
```

Optional Just shortcuts include these examples in the per-language suites:

```sh
just test-go-debug            # Go examples' tests (plus the binding tests)
just test-python-debug        # Python examples' tests (plus the binding tests)
just test-js                  # JS examples' tests (plus the binding tests)
just test-examples-cpp-debug  # build the C++ examples and run their smoke tests
```

The JS risk playground is checked separately, with `npm test` and
`npm run build` (or optional `just check`) in
[`js/risk_playground`](js/risk_playground/README.md).
