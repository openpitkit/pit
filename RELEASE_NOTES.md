<!-- release-version: 0.10.0 -->
Adds exclusive sections to the Go and C++ asynchronous engines for coordinated changes across accounts. Corrects concurrent account-state handling, rate-limit counting, decimal subtraction, and C++ custom-policy dry runs. Tightens the threading contract and makes invalid fill-lock input recoverable in Go.

- New exclusive sections for account-group membership, group currency, Spot Funds reconfiguration, and supported account-retirement operations
- Fix for AccountSync transition races that could stall order processing or overlap account-state changes
- Fixes for rate-limit over-counting with late clock readings and decimal subtraction with different scales
- Fix for C++ custom-policy dry runs invoking a live start hook when the dry-run hook takes a context
- Support for seven interface languages in Risk Playground, with browser-language detection and corrected funds display while editing limits

### Breaking changes

- Go and C++ async account-group registration and removal, plus Go async retirement and group-currency chain steps, have been removed - call the synchronous driver inside `Exclusive` instead. Go requires a cancellable context; C++ requires a positive timeout.
- Under no-sync and account-sync modes, group membership, group currency, Spot Funds reconfiguration, and supported account retirement require no other call on the engine to be in flight, including callbacks - pause callers or use the async engine's `Exclusive` section.
- Stateful callback re-entry into the same engine is now a caller-contract violation, detected in debug builds and unchecked in release builds. Nested-configuration error variants and Go's `ErrReentrantLane` have been removed - move re-entrant work outside callbacks and prevent callback cycles between engines.
- Go fill-lock setters and `ExecutionReportFill.SetValues` now return errors, and `NewExecutionReportFillFromValues` returns `(ExecutionReportFill, error)` - handle invalid encoded locks explicitly; failed setters leave the fill unchanged.
- Custom Rust storage factories must implement `LockingPolicyFactory::serializes_state_transitions` - report whether the engine must serialize transitions or the caller guarantees exclusivity. Custom C++ async strategies must implement `Exclusive`, `SerializesAccount`, and `Terminate`.
- C++ start-policy adapters reject hooks that take a context instead of silently ignoring them - use a direct `CustomPolicy` handler for context-aware start and start dry-run hooks.
- C++ async reservation and drop-copy destruction now preserves account-lane ordering - keep the engine alive until all wrappers are released. Cleanup hand-off or result-delivery failures that cannot be reported terminate the process; an explicit `Close` returning `TaskFailed` may leave its handle open and can be retried.
