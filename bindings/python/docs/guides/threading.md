# Threading contract

The Python binding follows the SDK threading contract: the engine does not spawn
OS threads, and each public method runs on the OS thread that invoked it.

Do not invoke public methods concurrently on the same engine handle. Sequential
calls on the same handle from different OS threads are supported by the SDK
contract, but the caller must serialize access.

Python policy callbacks execute on the calling thread. Treat policy instances as
owned by the engine that registered them and protect any external state they
access with the caller's own synchronization model.

Snapshot semantics apply to submitted orders, reports, and account adjustments:
mutating the Python object after submission does not change the in-flight engine
operation.
