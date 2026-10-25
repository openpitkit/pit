# Threading contract

The Python binding follows the SDK threading contract: the engine does not
spawn OS threads, and each public method runs on the OS thread that invoked
it.

The engine handle's capability depends on the sync policy selected by the
builder:

- `full_sync()` allows concurrent calls on the same engine handle and also
  supports sequential calls from different OS threads.
- `no_sync()` keeps all calls on the OS thread that created the engine.
- `account_sync()` allows concurrent calls on the same handle when the
  caller pins each account to one processing chain (calls for the same
  account are never concurrent). Sequential cross-thread invocation is
  always safe.

## Exclusive operations

A few rare operations change state that every account reads:
`register_group`, `unregister_group`, `set_group_currency`, and
`clear_group_currency` on `engine.accounts()`, and the Spot Funds retune
`spot_funds` and `spot_funds_pnl_bounds_killswitch` on `engine.configure()`.

- Under `full_sync()` the engine orders each of them itself.
- Under `no_sync()` and `account_sync()` the engine takes no lock for
  them, so ordinary calls pay nothing. Run each with no other call on the
  engine in flight on any thread; breaking that rule is undefined behavior.
  Moving an account between groups is `unregister_group` followed by
  `register_group`: run both in one such stretch.

Under `account_sync()`, policy callbacks for distinct accounts may run
concurrently, so a custom policy synchronizes any state of its own that is
not keyed by account.

## Recommended default

Prefer `no_sync()` when the embedding does not explicitly work with multiple
threads. It has zero synchronization overhead and is the right default for
applications driven from a single thread (synchronous code or one asyncio
loop). Use `full_sync()` when the engine is genuinely shared across threads.

**Warning:** under `no_sync()` the engine handle must stay on the OS
thread that created it. The type system does not enforce this from Python,
and violating it is undefined behavior.

Python policy callbacks execute on the calling thread. Treat policy
instances as owned by the engine that registered them, and protect any
external state they touch with the caller's own synchronization.

Snapshot semantics apply to submitted orders, reports, and account
adjustments: mutating the Python object after submission does not change
the in-flight engine operation.
