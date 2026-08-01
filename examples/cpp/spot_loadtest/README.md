<!--
Copyright The Pit Project Owners. All rights reserved.
SPDX-License-Identifier: Apache-2.0
-->

# openpit Spot-Limit Load Test (C++)

Measures the **open-loop** `intended arrival -> decision` latency for the
openpit pre-trade engine running a spot-limit funds policy at high offered
rates. The load test uses the `openpit::asyncengine` typed async surface
(Dynamic, per-account) and reports both order-check (stages 1-2) and settlement
(stages 3-4) latencies as HdrHistogram percentiles across sliding windows, with
coordinated-omission defence over a deterministic virtual causal timeline.

This is the C++ port of the Go harness at `examples/go/spot_loadtest/`. Every
internal layer mirrors a Go package one-to-one
(`measurement` / `progress` / `config` / `reporter` / `driver` / `env` /
`generator`); each header and source carries a `Mirror of:` cross-reference to
the Go file it ports.

Sample output: see [sample_report.txt](sample_report.txt).

---

## Contents

- [How it works — and how to read the results](#how-it-works--and-how-to-read-the-results)
- [Prerequisites](#prerequisites)
- [Build and run](#build-and-run)
- [Flags](#flags)
- [Configuration](#configuration)
- [Methodology and honesty notes](#methodology-and-honesty-notes)
- [C++-specific notes](#c-specific-notes)
- [What is and is not measured](#what-is-and-is-not-measured)

---

## How it works — and how to read the results

**In one sentence:** this tool tells you, in microseconds, how long openpit
takes to accept or reject a spot order when driven from C++ — measured honestly
under realistic continuous load, not cherry-picked light traffic.

### The order lifecycle and where latency is measured

Every order travels through four stages. The diagram shows where each timer
starts and stops:

```text
 Stage 1           Stage 2                 Stage 3          Stage 4
 ─────────         ─────────────────────   ────────────────  ─────────
 Client            Gateway (this harness)  Trading System    Client
   │                   │                       │               │
   │  order created    │                       │               │
   │  t0 stamped ──────►                       │               │
   │                   │  asyncengine queues   │               │
   │                   │  submit to engine ────►               │
   │                   │  ◄── allow / reject ──┤               │
   │                   │                       │               │
   │                   ├────── MEASURED ───────┘               │
   │    order-check latency = decision time − t0               │
   │    (includes queue wait; honest under load)               │
   │                   │                                       │
   │                   │  execution report arrives             │
   │                   │  (for accepted orders only)           │
   │                   │◄──────────────────────┤               │
   │                   │  settlement decision ──────────────►  │
   │                                                           │
   │                   ├──────────── MEASURED ─────────────────┘
   │                   settlement latency = decision time − t0
```

- **t0** (intended arrival): stamped from the virtual causal timeline, NOT
  from when the submit call was actually made. This is the crucial detail
  that makes the measurement honest (see below).
- **Order-check latency** (stages 1-2): how long until the engine says
  "allow" or "reject". Includes time waiting in the per-account dispatch
  queue before the submit even happens.
- **Settlement latency** (stages 3-4): for accepted orders, how long until
  the execution report is processed and the funds position is settled.

### What "measured honestly" means

#### Open-loop + intended arrival (coordinated-omission defence)

Imagine a queue at a service counter. The honest way to measure wait time
is to record when you *intended* to be served (when you joined the queue),
not when you finally reached the front. If the counter is swamped, your
wait is long — and that long wait must appear in the numbers.

This harness does exactly that. Every order is assigned a planned arrival
time on a *virtual causal timeline* (derived from `seed` + `offered_rate`).
That planned time is **t0**. The harness submits orders as fast as the
timeline says, without ever pausing to wait for a previous decision. If
the engine is busy and a decision comes back late, the extra wait is already
baked into `decision time − t0`.

The term for the alternative (measuring from when you actually submitted,
not from when you intended to) is *coordinated omission* — a known flaw in
many load-testing tools where the tool inadvertently hides saturation by
only measuring the queue front, not the queue length. This harness defends
against it.

#### Headline vs service-time

The report contains two numbers:

- **Headline** (`intended arrival -> decision`): the honest open-loop
  latency-under-load. **This is the number to trust.**
- **Service-time** (`resolve - ACTUAL submit`): the bare time once the
  submit actually happened, stripped of the pre-submit wait. This is
  printed as a clearly-labelled **DIAGNOSTIC** in the Diagnostics block,
  never as the headline. The gap between service-time and the headline **is**
  the coordinated-omission tail that the headline exposes and service-time
  conceals.

#### Why you can trust it

- **Real bindings**: drives the same public `openpit::` C++ binding your
  production code would use, not a mock.
- **Strict per-op oracle**: the harness pre-computes the expected
  accept/reject outcome (and, for funding/settlement, the exact post-op
  balances) for every single order. If the engine disagrees with even one
  prediction, the run fails hard.
- **Release-build guard**: the harness refuses to start against a debug
  build of the native core (latency from debug builds is meaningless). Pass
  `--allow-debug-core` only in development.
- **Anti-DCE checksum**: every decision is XOR-folded into a running
  checksum printed in the report. A zero checksum on a non-empty run aborts
  the run as INVALID (no headline, non-zero exit).
- **No self-tuning**: `seed + config` fully determine the event stream; the
  same inputs produce identical runs.

The report blocks (Headline, Environment, Workload, Trajectory, Distribution,
Diagnostics, Disclaimer) are identical in structure to the Go harness; see the
Go [README](../../go/spot_loadtest/README.md) for the per-block walkthrough and
the full settings glossary, which apply verbatim.

---

## Prerequisites

- A **C++17** compiler (Clang or GCC) and **CMake 3.21+**.
- A **release-built** openpit native core (the harness refuses to run against a
  debug build; see [Debug-core guard](#debug-core-guard) below).

---

## Build and run

<!-- Test mirror: examples/cpp/spot_loadtest/test/driver_test.cpp
     Driver.DocBackingBaselineRecipe -->

### 1. Build the native core in release mode

```sh
cd <repo>
cargo build --release
```

The core **must** be built in release mode. The harness refuses to run against a
debug core because latency numbers from such a build are meaningless.

### 2. Point CMake at the native runtime

The example consumes the header-only OpenPit C++ binding, which links the
prebuilt native runtime. Provide it exactly as the binding documents — either an
explicit library file or letting the binding download the matching release:

```sh
# macOS
cmake -S examples/cpp/spot_loadtest -B build -DCMAKE_BUILD_TYPE=Release \
  -DOPENPIT_RUNTIME_LIBRARY=$(pwd)/target/release/libopenpit_ffi.dylib

# Linux
cmake -S examples/cpp/spot_loadtest -B build -DCMAKE_BUILD_TYPE=Release \
  -DOPENPIT_RUNTIME_LIBRARY=$(pwd)/target/release/libopenpit_ffi.so
```

### 3. Build and run

```sh
cmake --build build
./build/spot_loadtest --config configs/baseline.ini
```

On platforms that resolve shared libraries at load time you may also need to
expose the runtime directory, e.g. `DYLD_LIBRARY_PATH` (macOS) or
`LD_LIBRARY_PATH` (Linux) pointing at `target/release`.

The report is written to **stdout**; live progress goes to **stderr** so the
report can be piped or redirected cleanly:

```sh
./build/spot_loadtest --config configs/baseline.ini > report.txt
```

---

## Flags

| Flag | Default | Description |
| --- | --- | --- |
| `--config` | (required) | Path to the INI configuration file |
| `--allow-debug-core` | false | Override the debug-core guard (dev only) |
| `--progress=false` | (progress on) | Disable live per-second progress |

---

## Configuration

The committed `configs/baseline.ini` is byte-identical to the Go harness's
baseline and is the reference baseline. Every knob is documented inline in that
file and in the Go README's [Settings
glossary](../../go/spot_loadtest/README.md#settings-glossary), which applies
verbatim. The report always echoes the config path and SHA-256 hash so every run
is reproducible by `config + seed`.

To run an alternative scenario, copy the baseline and pass the copy:

```sh
cp configs/baseline.ini configs/my_scenario.ini
# edit my_scenario.ini
./build/spot_loadtest --config configs/my_scenario.ini
```

---

## Methodology and honesty notes

The methodology is identical to the Go harness — headline = open-loop
`intended arrival -> decision`, true open-loop over a virtual causal timeline,
strict per-op oracle, HdrHistogram with a full percentile set and saturation
clamping, the debug-core guard, the anti-DCE checksum, and the bounded-
concurrency model. See the Go
[README](../../go/spot_loadtest/README.md#methodology-and-honesty-notes) for the
full text; the C++ port reproduces each property faithfully.

### Debug-core guard

The harness reads the native core's build profile via the
`openpit::GetBuildProfile()` accessor and refuses to run if the core was built
with debug settings (`debug_assertions=true`, `opt_level=0`, or
`profile=debug`). Pass `--allow-debug-core` to override (development only). The
build profile is always printed in the Environment block.

---

## C++-specific notes

The port is a faithful one-to-one mirror, with three deliberate adaptations to
the C++ binding surface:

- **Concurrency surface.** The driver drives the engine concurrently through
  `openpit::asyncengine::TypedAsyncEngine` — the C++ analogue of Go's
  `asyncengine` package (per-account queues, futures, `Dynamic`/`Sharded`
  strategies). The harness's own submitter / collector / finalizer pools are
  `std::thread` pools mirroring the Go goroutine roster. Per-account dispatch
  uses one OS thread per live queue, which is heavier than a goroutine; in
  practice this binding sustains a lower offered rate before saturating than the
  Go harness, which the open-loop tail honestly reflects.

- **Settlement fill lock.** The spot-funds policy resolves a BUY fill's held leg
  from a pre-trade lock attached to the execution report (built from the
  reserved price). The public `openpit::model::Fill` leaves the report's fill
  lock null, so the driver builds the report through a small
  `ReportWithLock` payload whose `Raw()` sets the lock pointer from a real
  `openpit::pretrade::PreTradeLock` — the exact analogue of the Go harness's
  `pretrade.NewLockFromEntries`. See `src/driver_build.hpp`.

- **Exact money.** The shadow ledger uses a self-contained scale-2 fixed-point
  `Decimal` (the Go harness uses `shopspring/decimal`). The value space is
  pinned exactly as the Go `money.go` — integer lots, two-decimal prices — so
  `q*p` is exact and the shadow ledger stays bit-for-bit against the engine.
  Values cross into the engine via `openpit::param` value types constructed from
  their exact string form.

The HdrHistogram, the deterministic RNG, the SHA-256 config hash, and the INI
parser are self-contained (no third-party dependencies beyond the OpenPit
binding and, for tests, GoogleTest fetched by CMake).

### Tests

The gtest suite mirrors the Go `*_test.go` files and is registered in this
example's own CMake (`SPOT_LOADTEST_BUILD_TESTS=ON`, the default):

| Test target | Mirrors |
| --- | --- |
| `spot_loadtest_config_tests` | `config/*_test.go` |
| `spot_loadtest_measurement_tests` | `measurement/measurement_test.go` |
| `spot_loadtest_progress_tests` | `progress/progress_test.go` |
| `spot_loadtest_generator_tests` | `generator/{generator,ledger}_test.go` |
| `spot_loadtest_driver_tests` | `driver/{driver,doc_backing}_test.go` |

```sh
cmake --build build
ctest --test-dir build --output-on-failure
```

The driver tests drive the real engine, so the native runtime must be resolvable
at load time (e.g. `DYLD_LIBRARY_PATH` / `LD_LIBRARY_PATH` pointing at
`target/release`).

---

## What is and is not measured

**Measured (HEADLINE = open-loop latency-under-load):**

- `intended arrival -> decision` latency for `ExecutePreTrade` (order-check,
  stages 1-2), including all pre-submit queue wait and the per-account async
  queue wait, through the C++ binding boundary. `t0` is the event's virtual
  arrival on the offline causal timeline, so queueing and stalls are counted.
- `intended arrival -> decision` latency for `ApplyExecutionReport` (settlement,
  stages 3-4), measured the same way.
- Both latencies are TRUE OPEN-LOOP with coordinated-omission defence over the
  virtual causal timeline.
- Service-time (`resolve - ACTUAL submit`) as a labelled DIAGNOSTIC in the
  Diagnostics block. It hides the coordinated-omission tail and is never the
  headline.

**Not measured:**

- Client or TS network latency.
- Serialization or protocol overhead beyond the C++ binding boundary.
- OS scheduling jitter beyond what the monotonic clock already captures.
- Any TS-side processing other than the pit core.
- Multi-host or multi-process throughput.
