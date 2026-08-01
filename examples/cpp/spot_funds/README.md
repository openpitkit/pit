# spot_funds (C++)

The smallest end-to-end integration of OpenPit's built-in **SpotFunds**
pre-trade policy. `RunExample()` reads top-to-bottom as a story: build a
limit-only engine, seed an account with 100000 USD, accept a BUY of 30 AAPL @
2000 (which holds 60000 USD), watch an identical second BUY get rejected with
`InsufficientFunds` because that cash is still held, fill the first order so its
reservation settles, then switch the policy to track-only mode on the running
engine and watch a third identical BUY get accepted against the 40000 USD the
fill left available. The point is the reservation mechanic - a committed order
reduces available funds until it fills - how a fill is tied back to its
reservation by carrying the pre-trade lock on the execution report, and how the
limit mode is retuned without rebuilding the engine.

Everything runs against the `openpit::` C++ API: the engine is an RAII value,
failures surface as `openpit::Error`, and every engine call is factored into a
helper in `src/spot_funds.hpp`. See
[Carrying the pre-trade lock](#carrying-the-pre-trade-lock) for the one piece of
state those helpers have to thread by hand.

## Layout

- `src/spot_funds.hpp` - the shared helpers, one per engine interaction: build
  the engine, seed funds, build an order, place it, build and apply a fill, and
  switch the limit mode.
- `src/main.cpp` - the linear story plus `main()`.
- `test/smoke_test.cpp` - a GoogleTest smoke test over the same helpers.

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

## Carrying the pre-trade lock

A fill settles a reservation only when its execution report carries that
reservation's pre-trade lock, so the lock is the one piece of state the helpers
in `src/spot_funds.hpp` thread from placement to fill:

- `PlaceOrder` reads that lock off the reservation with
  `pretrade::Reservation::Lock()`, then commits and returns it. The accessor
  gives back an owned snapshot detached from the reservation, valid before or
  after `Commit()`, and throws `openpit::Error` on an empty
  (default-constructed or moved-from) handle.
- `ApplyFill` copies the report, sets `model::Fill::lock` - a
  `std::shared_ptr<pretrade::PreTradeLock>` - to a `Clone()` of that lock, and
  passes the copy to `Engine::ApplyExecutionReport`. The returned
  `PostTradeResult::accountBlocks` is empty when settlement succeeded.

Everything else - the engine build, the seed adjustment, the orders, the
reject-code check, and the no-block fill assertion - uses the high-level
`openpit::` C++ types directly.

## See also

- [SpotFunds wiki page](https://wiki.openpit.dev/Spot-Funds/) -
  the full policy reference (market orders, slippage, pricing source, fee
  conventions).
- [`examples/go/spot_funds`](../../go/spot_funds) - the same scenario in Go.
