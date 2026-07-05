# rate_pnl_killswitch

An independent supervisor that wraps OpenPit's **RateLimit** and
**PnlBoundsKillSwitch** policies around a C++ strategy, so a runaway strategy is
halted before it floods the venue with orders or burns through its loss budget.
`main()` builds an engine with the two kill-switch policies side-by-side, feeds
it a single `Event` stream (orders + fills), keeps venue/strategy side-effects
behind a `Reactor` interface, and aggregates accepted/rejected counts,
pre-trade latency, and cumulative P&L over the run. The point is the supervisor
pattern: the engine decides, the reactor acts, and the strategy is stopped the
moment a kill switch returns an account block.

## Usage

The engine wiring and the policy-agnostic loop are the parts you reuse. Build
the engine once with the two kill switches plus order validation, then drive it
with your event stream and reactor:

<!-- Test mirror: examples/cpp/rate_pnl_killswitch/test/killswitch_test.cpp -->

```cpp
namespace policies = openpit::pretrade::policies;

// Wire the engine with the two killswitch policies plus order validation.
openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
builder.Add(policies::OrderValidationPolicy{});

openpit::pretrade::policies::PnlBoundsBrokerBarrier pnlBarrier("USD");
pnlBarrier.lowerBound = openpit::param::Pnl::FromString("-500");
pnlBarrier.upperBound = openpit::param::Pnl::FromString("500");
builder.Add(policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(std::move(pnlBarrier)));

builder.Add(policies::RateLimitPolicy{}.BrokerBarrier(
    policies::RateLimitBrokerBarrier(policies::RateLimit(
        /*maxOrders=*/1'000, /*windowNanoseconds=*/10'000'000'000))));

const openpit::Engine engine = builder.Build();

// Drive the engine: ExecutePreTrade reserves accepted orders (Commit to keep
// them), ApplyExecutionReport feeds fills back. A non-empty accountBlocks list
// on the report result means a kill switch fired - halt the strategy.
openpit::pretrade::ExecuteResult preTrade = engine.ExecutePreTrade(order);
if (preTrade.Passed()) {
  preTrade.reservation->Commit();
}

const openpit::PostTradeResult result = engine.ApplyExecutionReport(report);
const bool killSwitched = !result.accountBlocks.empty();
```

## Running

The example links the native OpenPit runtime (`libopenpit_ffi`). Its
`CMakeLists.txt` consumes the binding the community-standard way: in-repo it
`add_subdirectory`s the binding at `bindings/cpp`; standalone it resolves the
installed package with `find_package(OpenPit)`.

### In-repo, against local sources

Build the native runtime once, then configure and build the example against it
(the path mirrors the workspace `just build-cpp-debug` recipe):

```sh
# From the repository root:
cargo build -p openpit-ffi --release

# Resolve the freshly built runtime by absolute path.
lib="$PWD/target/release/libopenpit_ffi.dylib"   # .so on Linux

cmake -S examples/cpp/rate_pnl_killswitch -B examples/cpp/rate_pnl_killswitch/build \
  -DOPENPIT_RUNTIME_LIBRARY="$lib"
cmake --build examples/cpp/rate_pnl_killswitch/build
```

Then run the scenario and the smoke test:

```sh
./examples/cpp/rate_pnl_killswitch/build/rate_pnl_killswitch   # run the scenario
ctest --test-dir examples/cpp/rate_pnl_killswitch/build --output-on-failure  # smoke test
```

The build embeds the resolved runtime directory into the binary's RPATH, so no
`OPENPIT_RUNTIME_LIBRARY_PATH` export is needed to launch it. If you omit
`-DOPENPIT_RUNTIME_LIBRARY`, the binding downloads the matching prebuilt runtime
release instead, so no Rust toolchain is required.

### Standalone, against an installed binding

Install the C++ binding (`cmake --install` of `bindings/cpp`), then build this
example on its own, resolving the binding with `find_package(OpenPit)`:

```sh
cmake -S examples/cpp/rate_pnl_killswitch -B build \
  -DOPENPIT_EXAMPLE_USE_INSTALLED_PACKAGE=ON \
  -DCMAKE_PREFIX_PATH="/path/to/openpit-install"
cmake --build build
ctest --test-dir build --output-on-failure
```

`find_package(OpenPit)` brings in the same `OpenPit::openpit` target and the
runtime resolver, so the example exercises exactly what an SDK consumer sees.

## See also

- [RateLimitPolicy](https://github.com/openpitkit/pit/wiki/Policies#ratelimitpolicy)
  and [PnlBoundsKillSwitchPolicy](https://github.com/openpitkit/pit/wiki/Policies#pnlboundskillswitchpolicy) -
  the policy references for the two kill switches combined here.
- [`../../python/rate_pnl_killswitch`](../../python/rate_pnl_killswitch) and
  [`../../go/rate_pnl_killswitch`](../../go/rate_pnl_killswitch) - the same
  supervisor in Python and Go.
