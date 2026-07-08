# spot_funds (C++)

The smallest end-to-end integration of OpenPit's built-in **SpotFunds**
pre-trade policy. `RunExample()` reads top-to-bottom as a story: build a
limit-only engine, seed an account with 100000 USD, accept a BUY of 30 AAPL @
2000 (which holds 60000 USD), watch an identical second BUY get rejected with
`InsufficientFunds` because that cash is still held, then fill the first order
so its reservation settles. The point is the reservation mechanic - a committed
order reduces available funds until it fills - and how a fill is tied back to
its reservation by carrying the pre-trade lock on the execution report.

This is the C++ mirror of [`examples/go/spot_funds`](../../go/spot_funds); the
scenario, constants, and outcomes are identical. See
[Mirror notes](#mirror-notes) for the one binding-shaped difference.

## Layout

- `src/spot_funds.hpp` - shared helpers (one per Go `main.go` helper): build the
  engine, seed funds, build an order, place it, build and apply a fill.
- `src/main.cpp` - the linear story plus `main()`.
- `test/smoke_test.cpp` - a GoogleTest smoke test mirroring `main_test.go`.

## Building and running

The example links the OpenPit C++ binding (`OpenPit::openpit`), a header-only
interface target that pulls in the native `openpit-ffi` runtime library. The
runtime is resolved by the binding's CMake: it downloads the matching release
from GitHub by default, or uses a local build when you point
`OPENPIT_RUNTIME_LIBRARY` at one.

### In-repo (from a pit checkout)

This example's `CMakeLists.txt` embeds the binding from `../../../bindings/cpp`,
so no installed package is needed. Build the native runtime once, then configure
and build:

```sh
# From the repository root: build the FFI runtime (.dylib on macOS, .so on Linux).
cargo build -p openpit-ffi --release

# Configure + build the example against that local runtime.
cmake -S examples/cpp/spot_funds -B examples/cpp/spot_funds/build \
  -DOPENPIT_RUNTIME_LIBRARY="$PWD/target/release/libopenpit_ffi.dylib"
cmake --build examples/cpp/spot_funds/build
```

Then run the scenario and the smoke test:

```sh
./examples/cpp/spot_funds/build/spot_funds        # run the scenario
ctest --test-dir examples/cpp/spot_funds/build --output-on-failure  # smoke test
```

### Standalone (external `find_package`)

Outside the repo, install the binding once (`cmake --install` on the
`bindings/cpp` project), then consume it from your own `CMakeLists.txt` exactly
as any other CMake package. Replace the in-repo `FetchContent` block with:

```cmake
cmake_minimum_required(VERSION 3.21)
project(my_spot_funds LANGUAGES CXX)

find_package(OpenPit CONFIG REQUIRED)

add_executable(spot_funds src/main.cpp)
target_include_directories(spot_funds PRIVATE src)
target_link_libraries(spot_funds PRIVATE OpenPit::openpit)
```

Point CMake at the installed package and (optionally) a local runtime:

```sh
cmake -S . -B build \
  -DCMAKE_PREFIX_PATH="/path/to/openpit/install" \
  -DOPENPIT_RUNTIME_LIBRARY="/path/to/libopenpit_ffi.dylib"
cmake --build build
```

## Mirror notes

The Go example reads `reservation.Lock().Bytes()` before committing and carries
those bytes on the fill's execution report. The C++ binding shapes this
differently in two spots, so the helpers in `spot_funds.hpp` take the path the
Go doc comments already point to:

- `pretrade::Reservation` surfaces only `Commit()` / `Rollback()`, not a lock
  accessor. `PlaceOrder` reconstructs the equivalent lock the engine would have
  produced - a single record under the default policy group at the reservation
  price - via `pretrade::PreTradeLock::Push`. This is the C++ form of Go's
  `pretrade.NewLockFromEntries({DefaultPolicyGroupID, lockPrice})`.
- `model::ExecutionReport` intentionally does not surface the fill's lock
  pointer (pre-trade locks are a separate handle slice). `ApplyFill` therefore
  attaches the lock to the borrowed C report view and applies it through the C
  ABI, draining account blocks back into `accounts::AccountBlock`.

Everything else - the engine build, the seed adjustment, the orders, the
reject-code check, and the no-block fill assertion - uses the high-level
`openpit::` C++ types directly.

## See also

- [SpotFunds wiki page](https://wiki.openpit.dev/Spot-Funds/) -
  the full policy reference (market orders, slippage, pricing source, fee
  conventions).
- [`examples/go/spot_funds`](../../go/spot_funds) - the Go mirror this example
  tracks one-to-one.
