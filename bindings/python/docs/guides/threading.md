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
