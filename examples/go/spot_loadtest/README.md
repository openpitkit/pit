# openpit Spot-Limit Load Test (Go)

Measures the **open-loop** `intended arrival -> decision` latency for the
openpit pre-trade engine running a spot-limit funds policy at high offered
rates. The load test uses the `asyncengine` (Dynamic, per-account) and reports
both order-check (stages 1-2) and settlement (stages 3-4) latencies as
HdrHistogram percentiles across sliding windows, with coordinated-omission
defence over a deterministic virtual causal timeline.

Sample output: see [sample_report.txt](sample_report.txt).

---

## Contents

- [How it works — and how to read the results](#how-it-works--and-how-to-read-the-results)
- [Prerequisites](#prerequisites)
- [Build and run](#build-and-run)
- [Flags](#flags)
- [Configuration](#configuration)
- [Methodology and honesty notes](#methodology-and-honesty-notes)
- [What is and is not measured](#what-is-and-is-not-measured)

---

## How it works — and how to read the results

**In one sentence:** this tool tells you, in microseconds, how long openpit
takes to accept or reject a spot order when driven from Go — measured honestly
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
  never as the headline. It is useful for decomposition (how much was
  engine compute vs queue wait?) but it hides the saturation tail — the
  exact number that matters most under load.

The gap between service-time and the headline **is** the coordinated-
omission tail that the headline exposes and service-time conceals.

#### Why you can trust it

- **Real bindings**: drives the same public Go FFI bindings your production
  code would use, not a mock.
- **Strict per-op oracle**: the harness pre-computes the expected
  accept/reject outcome for every single order. If the engine disagrees
  with even one prediction, the run fails hard. You can be sure the
  numbers describe a correctly-behaving engine.
- **Release-build guard**: the harness refuses to start against a debug
  build of the native core (latency from debug builds is orders of magnitude
  higher and meaningless). Pass `-allow-debug-core` only in development.
- **Anti-DCE checksum**: every decision is XOR-folded into a running
  checksum printed in the report. This proves the compiler did not optimize
  away the measurement loop. A zero checksum on a non-empty run aborts the
  run as INVALID (no headline, non-zero exit).
- **No self-tuning**: the harness never adjusts itself to look good.
  `seed + config` fully determine the event stream; the same inputs produce
  identical runs.

---

### How to read each report block

The report is written to **stdout** as plain text divided into named blocks.
Progress noise is strictly on **stderr** and never appears in the report.

#### Headline

The most important block. Shows the **steady-state open-loop** order-check
latency (the honest number). Steady-state means the first measurement
window (warmup: JIT + cache + engine ramp-up) is excluded so warmup spikes
cannot pollute the result.

```text
=== Headline: Open-Loop Order-Check Latency (intended arrival -> decision) ===

  Order-check p50 (steady-state, open-loop): 24.6µs
  Order-check p99 (steady-state, open-loop): 299.893ms

  Full tail (steady-state, so no warmup spike is hidden):
    p99   : 299.893ms
    p99.9 : ...
    max   : ...
```

**What the percentiles mean:** if the tool ran 1 000 000 orders, p50 is
the latency that half of them were faster than (the typical case). p99 is
the latency that 99% were faster than — only 1% were slower. p99.9 means
99.9% were faster; the slowest 0.1% were above this line. `max` is the
single slowest measurement.

**Healthy values:** p50 in the tens-of-microseconds range; p99 under a
few milliseconds. A p99 in the hundreds of milliseconds means the engine
is saturated at the offered rate — check the Trajectory block for whether
it is consistent (system overloaded) or spiky (GC pause or scheduler
jitter). Also printed: throughput (decided ops/s) and max in-flight
(open-loop depth witness, should be > 1 to confirm true pipelining).

#### Environment

Host CPU, RAM, OS, kernel; Go version, architecture, CGO flag; the pit
repository commit (clean/dirty); the native core version and **build
profile** (profile, opt\_level, debug\_assertions, target, LTO). Always
present so you can audit exactly which binary was under test.

#### Workload

What the run actually did:

- **Op counts**: total resolved ops, with per-class counters: order-checks
  (with their own accepts/rejects); settlements (accepts/blocked);
  funding adjustments (accepted/rejected, only shown when > 0). Accepts
  never exceed the order-check count. The achieved reject rate is computed
  over order-checks only (settlements and funding adjustments are
  excluded from the denominator).
- **Concurrency model**: total account population vs bounded active working
  set (max accounts hot at once) and the percentage of the population that
  was active.
- **Engine dispatch sizing**: which async-engine strategy was used
  (`dynamic` or `sharded`), and the resource knobs (`max_queues`,
  `idle_cleanup`, `sharded_workers`, `queue_capacity`,
  `slow_submit_threshold`). These are capacity limits, not correctness
  knobs; the per-account ordering guarantees are unaffected.
- **Reject rate**: achieved vs target, and the deviation. A healthy run
  should be within the configured tolerance.
- **Backpressure**: how many submits the engine refused with an
  `ErrQueueLimit` signal because the live-queue cap was reached.
  **A healthy run always shows 0.** Any non-zero count makes the run
  **INVALID**: the headline and all latency-percentile blocks (Trajectory,
  Distribution) are suppressed, `Run` returns `ErrBackpressureInvalidRun`,
  and the process exits non-zero, printing a `*** RUN INVALID ***` banner
  together with the backpressure count and the non-latency diagnostics
  (environment, workload counts, dispatch sizing). Fix the dispatch sizing
  (raise `max_queues` or lower `active_accounts`) and re-run.
- **Cohort summary**: the weight, activity level, and burst length for each
  account cohort (chatty / steady / dormant).

#### Trajectory

Per-window percentile evolution, oldest window first. The first row is
marked `w` (warmup) and excluded from the steady-state headline. Use this
block to distinguish a consistently fast system from one with occasional
latency spikes:

```text
  Order-check (open-loop: intended arrival -> decision, stage 1->2):
  win  | ops   | p50        | p99        | p99.9      | wall
  -------------------------------------------------------------------
     1w| 100000| 22.0µs     | 65.0µs     | 303.1µs    | 12:00:01-12:00:03
     2 | 100000| 23.0µs     | 63.0µs     | 93.7µs     | 12:00:03-12:00:05
```

If p99 is low and stable across windows, the engine handles the offered
rate well. If p99 jumps in one window and recovers, that is a transient
event (GC, scheduler). If p99 is consistently high, the offered rate
exceeds the engine's throughput capacity.

#### Distribution

Final merged percentiles (p50, p90, p99, p99.9, max) for both order-check
and settlement, over all windows combined (including warmup — the full
picture). Also shows:

- **Harness self-overhead**: 200 probes via `ApplyAccountAdjustment`
  through a quiescent engine, before the workload runs. This is the
  **adjustment-path** FFI+queue floor — it is NOT the order-check
  (`ExecutePreTrade`) path overhead. Use it as a bare FFI+queue floor;
  do not subtract it from order-check latency as if it were the same
  path. Typical healthy overhead is a few microseconds at p99.
- **Clamped samples (`Clamped samples (> hist max): N`)**: latencies above
  the histogram ceiling (60 s) are saturated at the ceiling and counted,
  never dropped. When N > 0, tail percentiles at or above the ceiling are
  a lower bound on true latency.
- **Anti-DCE checksum**: a non-zero hex value proves every decision was
  consumed by the measurement loop. A zero checksum on a non-empty run aborts
  the run as INVALID (no headline, non-zero exit).

#### Diagnostics

Two diagnostic decompositions — neither is the headline:

**Service-time** (`resolve - ACTUAL submit`): the wall time from the actual
submit call to the decision. Explicitly labelled as `DIAGNOSTIC, NOT the
headline`. It strips out the pre-submit queue wait, so it hides the
saturation tail by design. It is useful for comparison: if service-time is
low but the headline p99 is high, the engine itself is fast and the latency
tail is mostly queue wait — you need more dispatch capacity or a lower
offered rate.

**Inner metrics** (when observer is enabled): per-account aggregate
distributions of `queue_wait` (time a task spent waiting in the async
engine queue) and `engine_compute` (wall time of the engine call itself),
plus queue lifecycle counters (queues created / removed). The residual
(order-check p50 minus queue\_wait p50 plus engine\_compute p50) is an
approximate FFI + handoff overhead, labelled as such. These are per-account
aggregate distributions, not per-order figures; use them as a decomposition
clue, not a precise budget.

#### Disclaimer

What is and is not measured (see the
[What is and is not measured](#what-is-and-is-not-measured) section), plus
the exact one-line reproduction recipe with the config path and hash, so
any run can be reproduced byte-for-byte from `seed + config`.

---

### Settings glossary

Every knob in `configs/baseline.ini`, grouped by section. Copy the
baseline and edit the copy — never edit the committed baseline directly.

#### `[run]`

- **`seed`** — RNG seed for the deterministic event stream. Same seed +
  same config = identical stream and identical oracle predictions. Change
  to explore a different random draw.
- **`total_ops`** — Number of order-check operations to run. Mutually
  exclusive with `duration`. Larger values give more stable tail
  percentiles but take longer.
- **`duration`** — Wall-clock run duration (alternative to `total_ops`).
- **`window`** — Sliding-window size (in ops when `window_unit = ops`, or
  seconds when `window_unit = wall`). Each window becomes one row in the
  Trajectory block; steady-state excludes the first window.
- **`window_unit`** — `ops` (count-based window, default) or `wall`
  (time-based).
- **`observer`** — `on` / `off`. When on, the asyncengine fires callbacks
  that populate the inner metrics (`queue_wait`, `engine_compute`) in the
  Diagnostics block. Adds a small per-op callback overhead; turn off for
  the purest headline.

#### `[arrival]`

- **`offered_rate`** — Target events per second on the virtual causal
  timeline. This is the *offered* rate — the rate the harness tries to
  drive, regardless of whether the engine can keep up. The engine's
  achieved throughput is reported separately. Saturation shows up as
  rising p99/tail growth in the Trajectory block. Hitting `ErrQueueLimit`
  (only possible with a finite `max_queues`) **invalidates** the run, so
  size dispatch capacity to keep backpressure at 0.

#### `[report_delay]`

Controls how long after an accepted order's t0 the settlement event is
scheduled on the virtual timeline. Simulates the round-trip time a real
execution report would take from the trading system.

- **`distribution`** — Shape of the delay distribution. `lognormal`
  produces a realistic right-skewed delay (most reports arrive quickly,
  a few are slow).
- **`mean`** — Mean report-return delay (e.g. `2ms`).
- **`sigma`** — Log-space standard deviation. Higher values produce longer
  and more variable tails in the report-delay distribution.

#### `[reject]`

- **`target_rate`** — Fraction of order-checks that should be rejected
  (0.05 = 5%). The generator uses this to mix forced-insufficient-funds
  rejects with natural accepts. The achieved rate is reported in the
  Workload block; a healthy run stays within tolerance.
- **`tolerance`** — Acceptable deviation from `target_rate` (±). If the
  achieved rate drifts further, the run is considered misconfigured.

#### `[accounts]`

- **`count`** — Total account population. Most accounts are idle most of
  the time; only a bounded active set (`concurrency.active_accounts`) is
  hot at any moment. A large population (e.g. 10 000) with a small active
  set is realistic; making all accounts simultaneously hot is not.

#### `[instruments]`

- **`symbols`** — Comma-separated list of trading symbols (e.g.
  `AAPL,SPX,...`). The generator distributes orders across these, with
  per-cohort skew (uniform or Zipf).
- **`settlement`** — The cash/settlement asset (e.g. `USD`). All
  spot-funds balances are denominated in this asset.

#### `[concurrency]`

- **`active_accounts`** — Maximum number of accounts concurrently active
  (hot) at any moment. Acts as a chain-gate: at most this many per-account
  submitter chains run simultaneously. This bounds the engine's live
  per-account dispatch queues near the active-set size rather than the
  full population. Must be ≤ `accounts.count`.

#### `[lifecycle]`

Controls the probability that each wake of an account generates a
particular order action (open a new position, add to it, partially close,
or fully close). Each is an independent probability in \[0, 1\].

- **`p_open`** — Probability of opening a new position (0.40 = 40%).
- **`p_add`** — Probability of adding to an existing position.
- **`p_partial_close`** — Probability of partially closing an existing
  position.
- **`p_full_close`** — Probability of fully closing an existing position.

#### `[funding]`

- **`seed`** — Absolute starting settlement balance per account
  (e.g. 1 000 000 USD). Accounts begin well-funded; top-ups fire only
  when the available balance falls below `amount`.
- **`trigger`** — When to top up: `balance_below` fires when available
  balance drops below `amount`.
- **`amount`** — Top-up trigger threshold and default top-up size.
- **`top_up`** — Delta added to an account's balance each time the
  trigger fires.

#### Cohorts (`[cohort.chatty]`, `[cohort.steady]`, `[cohort.dormant]`)

The account population is partitioned into named cohorts. Each cohort is
assigned a fraction of accounts (by `weight`) and a behavioral profile.

- **`weight`** — Unnormalized share of the population. A cohort with
  weight 0.20 out of a total weight of 1.00 gets ~20% of accounts.
- **`activity`** — Probability that the account acts on each scheduling
  opportunity (0.90 = almost always active; 0.10 = rarely).
- **`reject_propensity`** — How likely the cohort is to be assigned a
  forced-reject event when the reject budget allows. High-propensity
  cohorts absorb most of the configured reject rate.
- **`burst_len`** — Number of orders fired per wake. A chatty cohort
  with `burst_len = 4` submits up to 4 orders each time it wakes.
- **`size_weights`** — Bucket distribution of order sizes as `qty:weight`
  pairs. `1:1,10:4,100:2` means small (qty 1) gets weight 1, medium
  (qty 10) gets weight 4, large (qty 100) gets weight 2.
- **`symbol_skew`** — How symbols are chosen: `uniform` (equal
  probability) or `zipf` (first symbols heavily preferred, configurable
  via `zipf_s`).
- **`zipf_s`** — Zipf exponent (only used when `symbol_skew = zipf`).
  Higher values concentrate traffic on fewer symbols.

#### `[async_engine]`

These are **resource limits** on the async engine dispatcher — analogous
to a connection pool cap. They control capacity and cleanup policy, not
correctness: per-account ordering guarantees are fixed regardless of these
settings.

- **`strategy`** — `dynamic` (default): one lazily-created queue and
  worker per account, full per-account isolation. `sharded`: a fixed pool
  of N shared workers; cheaper hot path but no per-account isolation. Use
  `dynamic` for latency benchmarking; try `sharded` to measure the
  overhead difference.
- **`max_queues`** — **Dynamic only.** Maximum number of live per-account
  queues. `0` = unlimited (baseline default). Setting a finite cap (must
  be ≥ `active_accounts`) limits memory when the active set is large;
  submits that exceed the cap return `ErrQueueLimit` (counted as
  backpressure). **Any backpressure invalidates the run**: the headline is
  suppressed and the process exits non-zero. Leave at `0` for short runs
  where idle cleanup has not yet fired.
- **`idle_cleanup`** — **Dynamic only.** How long a per-account queue
  must be idle before it is retired and its memory freed. `0` disables
  cleanup. `5s` means the cleanup scan fires roughly every second (scan
  period = idle/5) and retires queues idle for > 5 s. Has no effect on
  runs shorter than this threshold.
- **`sharded_workers`** — **Sharded only.** Number of fixed worker shards
  (must be > 0 when `strategy = sharded`; ignored under `dynamic`). More
  shards reduce contention at the cost of more goroutines.
- **`queue_capacity`** — Both strategies. Per-queue buffered channel size.
  `0` uses the engine default (1 024). Larger values smooth bursts but
  increase memory and lengthen graceful-stop tail.
- **`slow_submit_threshold`** — Both strategies. If a submit call blocks
  longer than this threshold, the engine emits a warning. `0` uses the
  engine default (1 minute). Lower values (e.g. `100ms`) help detect
  producer stalls in latency-sensitive scenarios.

---

## Prerequisites

- **Go 1.23+** with CGO enabled (required for the FFI boundary).
- A **release-built** openpit native core (the harness refuses to run against a
  debug build; see [Debug-core guard](#debug-core-guard) below).

---

## Build and run

<!-- Test mirror: internal/driver/doc_backing_test.go
     TestDocBackingBaselineRecipe -->

### 1. Build the native core in release mode

```sh
cd <repo>
cargo build --release
```

Or, if the project's `justfile` is available:

```sh
just _build-ffi
```

The core **must** be built in release mode. The harness refuses to run against a
debug core because latency numbers from such a build are meaningless.

### 2. Set `OPENPIT_RUNTIME_LIBRARY_PATH`

Point the Go runtime at the built native library:

```sh
# macOS
export OPENPIT_RUNTIME_LIBRARY_PATH=$(pwd)/target/release/libopenpit_ffi.dylib

# Linux
export OPENPIT_RUNTIME_LIBRARY_PATH=$(pwd)/target/release/libopenpit_ffi.so
```

> **Embedded-runtime alternative:** the `just _build-ffi` recipe embeds the
> runtime into the Go binary so that `OPENPIT_RUNTIME_LIBRARY_PATH` is not
> required at run time. Windows uses this same embedded-runtime mechanism as
> the other Go bindings.

### 3. Build and run

```sh
cd examples/go/spot_loadtest
go build ./...
./spot_loadtest -config configs/baseline.ini
```

The report is written to **stdout**; live progress goes to **stderr** so the
report can be piped or redirected cleanly:

```sh
./spot_loadtest -config configs/baseline.ini > report.txt
```

---

## Flags

| Flag | Default | Description |
| --- | --- | --- |
| `-config` | (required) | Path to the INI configuration file |
| `-allow-debug-core` | false | Override the debug-core guard (dev only) |
| `-progress` | true | Show live per-second progress on stderr |

---

## Configuration

The committed `configs/baseline.ini` is the reference baseline. Every knob is
documented inline in that file and explained in the
[Settings glossary](#settings-glossary) above. The report always echoes the
config path and SHA-256 hash so every run is reproducible by `config + seed`.

To run an alternative scenario, copy the baseline and pass the copy with
`-config`:

```sh
cp configs/baseline.ini configs/my_scenario.ini
# edit my_scenario.ini
./spot_loadtest -config configs/my_scenario.ini
```

---

## Methodology and honesty notes

### Headline = open-loop `intended arrival -> decision`

The headline is the wall-clock interval from an event's **intended arrival** on
the virtual causal timeline to the moment the decision resolves, **including**
all time spent waiting in the per-account async dispatch queue — even time that
elapsed before the submit call was issued. This is the full latency a gateway
process would observe under offered load, and it is the right metric for
evaluating the risk-check path end to end.

The inner metrics (`queue_wait`, `engine_compute`) are a diagnostic
decomposition of the engine-side component only. The service-time figure
(`resolve - ACTUAL submit`) is a labelled diagnostic that hides the pre-submit
wait and therefore hides the coordinated-omission tail; it is never the
headline.

### True open-loop over a virtual causal timeline

The harness is **TRUE OPEN-LOOP**: the driver submits every event immediately
when its virtual clock fires, never blocking on a prior decision. The generator
assigns each event a deterministic **virtual arrival time** on an offline causal
timeline:

- Order-check arrivals are paced by the offered rate across accounts.
- A settlement's virtual time = its order's virtual arrival + a report-return
  delay.
- A causally-dependent order (e.g., a sell after the buy that funds it) has
  virtual arrival >= its dependency's virtual settlement time + a gap.

All times are derived deterministically from `(seed, config)`, so the stream is
byte-identical per seed. The driver paces submissions to each event's virtual
arrival but **never blocks on a decision**: submissions pipeline across accounts
(`MaxInFlight >> 1`), and the per-account dependency ordering is preserved by
the engine's FIFO-per-account guarantee.

`t0` for every measurement is that event's **virtual arrival**, stamped
independently of when the submit actually happened. Any time that accrues
between the virtual arrival and the actual submit (pre-submit queue wait, pacing
jitter) is therefore **inside** `resolve - t0` and is counted, not omitted.
This is the coordinated-omission defence: a stalled submitter does not
undercount latency.

### Why the strict per-op oracle survives open-loop

The `asyncengine` `Dynamic` dispatcher keeps one channel-backed queue and one
worker per account, so tasks (including reservation commits and closes) run
strictly in submission order, never concurrently for one account (FIFO per
account). The spot-funds hold is applied **in-place at Execute**: the
`available -> held` transition is written to live storage synchronously inside
`perform_pre_trade_check`; `Reservation.Commit()` is intentionally a no-op, and
only `Close`-without-commit rolls the hold back.

Because the driver submits each account's causal sequence in order and the
engine is FIFO per account with in-place holds, a subsequent same-account pre-
trade check observes the hold without any intervening commit. The shadow model's
offline-ordered, precomputed per-event predictions therefore match the live
engine's decisions exactly, without needing a separate replay or per-op blocking.
**A single live run with a strict per-op oracle is correct.**

### HdrHistogram - full percentile set

All latencies are recorded into HdrHistogram windows. The report always shows
p50, p90, p99, p99.9, and max. The tail (p99.9 and max) is always printed and
cannot be hidden. The headline uses a lossless `Merge` of the raw per-window
histograms for the steady-state range; aggregating per-window percentile
point-values (percentile-of-percentiles) is statistically invalid and is never
done here.

Latencies above the histogram ceiling (60 s) are **clamped** (saturated at the
ceiling value; the sample count is preserved, not dropped). The Distribution
block reports the clamped count on a `Clamped samples (> hist max): N` line.
When N > 0, tail percentiles at or above the ceiling are a lower bound on true
latency, not an exact value.

### Debug-core guard

The harness reads the native core's build profile via the
`openpit_get_runtime_build_profile` FFI accessor and refuses to run if the core
was built with debug settings (`debug_assertions=true`, `opt_level=0`, or
`profile=debug`). Latency numbers from a debug build are orders of magnitude
higher and are meaningless for comparison purposes. Pass `-allow-debug-core` to
override (development only).

The build profile is always printed in the Environment block so the reader can
audit which binary was under test.

### Anti-DCE checksum

Every decision (accept or reject) is XOR-folded into a running checksum that is
printed in the Distribution block. This proves that the compiler did not
optimize away the measurement loop and that every operation was actually
consumed. A zero checksum on a non-empty run aborts the run as INVALID (the
headline is suppressed and the process exits non-zero).

### Harness self-overhead

Before the workload runs, the harness measures its own round-trip cost with
200 probes via `ApplyAccountAdjustment` against a quiescent engine. The
overhead distribution is printed in the Distribution block under the label
`Harness self-overhead (adjustment-path FFI+queue floor, quiescent engine)`.

This probe goes through the **adjustment path** (`ApplyAccountAdjustment`),
not the order-check path (`ExecutePreTrade`), so it is the bare FFI+queue
floor for that path, not the order-check overhead. It quantifies the Go-side
housekeeping cost (goroutine scheduling, channel ops, `time.Now()` calls)
as a floor reference; do not subtract it from order-check latency as if it
were an equivalent measurement.

### Bounded-concurrency model and engine dispatch sizing

The harness models a realistic gateway: a large account population (e.g. 10 000)
but only a bounded **active working set** hot at any moment (e.g. 1 024). The
rest act rarely (steady cohort) or almost never (dormant cohort).

`concurrency.active_accounts` is the chain-gate limit: at most this many
per-account submitter chains run concurrently. This bounds the engine's live
per-account dispatch queues near the active set rather than the whole population.

`async_engine.max_queues` and `async_engine.idle_cleanup` are **resource limits**
on the Dynamic dispatcher (analogous to a connection cap), not synchronization
semantics. The per-account `AccountSync` semantics are fixed and are not
affected by these knobs. `max_queues=0` (unlimited, the baseline default) is
appropriate for runs shorter than `idle_cleanup` because the idle-cleanup scan
has not fired yet and live queues can temporarily exceed the active-set size as
the submitter rotates through distinct accounts.

---

## What is and is not measured

**Measured (HEADLINE = open-loop latency-under-load):**

- `intended arrival -> decision` latency for `ExecutePreTrade` (order-check,
  stages 1-2), including all pre-submit queue wait and the per-account async
  queue wait, through the Go FFI boundary. `t0` is the event's virtual arrival
  on the offline causal timeline, so queueing and stalls are counted.
- `intended arrival -> decision` latency for `ApplyExecutionReport` (settlement,
  stages 3-4), measured the same way.
- Both latencies are TRUE OPEN-LOOP with coordinated-omission defence over the
  virtual causal timeline.
- Service-time (`resolve - ACTUAL submit`) as a labelled DIAGNOSTIC in the
  Diagnostics block. It hides the coordinated-omission tail and is never the
  headline.

**Not measured:**

- Client or TS network latency.
- Serialization or protocol overhead beyond the Go binding boundary.
- OS scheduling jitter beyond what `time.Now()` already captures.
- Any TS-side processing other than the pit core.
- Multi-host or multi-process throughput.
