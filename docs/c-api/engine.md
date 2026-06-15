# Engine

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitSyncPolicy`

Runtime selector for the engine's storage synchronization policy.

```c
typedef uint8_t OpenPitSyncPolicy;
/**
 * The handle stays on the OS thread that created it. Use this for
 * single-threaded embeddings where synchronization overhead must be zero.
 */
#define OpenPitSyncPolicy_None ((OpenPitSyncPolicy) 0)
/**
 * Concurrent invocation of public methods on the same handle is safe.
 * Sequential cross-thread access is also safe. Use this when the engine is
 * shared across threads.
 */
#define OpenPitSyncPolicy_Full ((OpenPitSyncPolicy) 1)
/**
 * Sequential cross-thread access on the same handle is safe; the caller pins
 * each account to a single processing chain (one queue or one worker at a
 * time). Concurrent invocation on the same handle is not supported in this
 * mode.
 */
#define OpenPitSyncPolicy_Account ((OpenPitSyncPolicy) 2)
```

## `OpenPitEngineBuilder`

Opaque builder pointer used to assemble an engine instance.

Ownership:

- returned by `openpit_create_engine_builder`;
- owned by the caller until passed to `openpit_destroy_engine_builder`;
- consumed by `openpit_engine_builder_build`.

```c
typedef struct OpenPitEngineBuilder OpenPitEngineBuilder;
```

## `OpenPitEngine`

Opaque engine pointer.

The engine stores policies and mutable risk state. The caller owns the pointer
until `openpit_destroy_engine`.

```c
typedef struct OpenPitEngine OpenPitEngine;
```

## `OpenPitPretradePreTradeRequest`

Opaque pointer for a deferred pre-trade request.

This is returned by `openpit_engine_start_pre_trade`. It can be executed once
with `openpit_pretrade_pre_trade_request_execute` or discarded with
`openpit_destroy_pretrade_pre_trade_request`.

```c
typedef struct OpenPitPretradePreTradeRequest OpenPitPretradePreTradeRequest;
```

## `OpenPitPretradePreTradeReservation`

Opaque reservation pointer returned by a successful pre-trade check.

A reservation represents resources that have been tentatively locked. The caller
must resolve it exactly once by calling
`openpit_pretrade_pre_trade_reservation_commit`,
`openpit_pretrade_pre_trade_reservation_rollback`, or
`openpit_destroy_pretrade_pre_trade_reservation`.

```c
typedef struct OpenPitPretradePreTradeReservation
    OpenPitPretradePreTradeReservation;
```

## `OpenPitPretradePreTradeDryRunReport`

Opaque report pointer returned by a pre-trade dry-run.

A dry-run reports the verdict the equivalent real call would produce with zero
effect on engine state. Unlike `OpenPitPretradePreTradeReservation`, this report
is inert: it carries no commit/rollback capability. The caller owns the pointer
and MUST release it with `openpit_destroy_pretrade_pre_trade_dry_run_report`.

```c
typedef struct OpenPitPretradePreTradeDryRunReport
    OpenPitPretradePreTradeDryRunReport;
```

## `OpenPitPretradeStatus`

Result status for pre-trade operations.

```c
typedef uint8_t OpenPitPretradeStatus;
/**
 * Order/request passed this stage; read the success out-pointer.
 */
#define OpenPitPretradeStatus_Passed ((OpenPitPretradeStatus) 0)
/**
 * Order/request was rejected; read the reject out-pointer.
 */
#define OpenPitPretradeStatus_Rejected ((OpenPitPretradeStatus) 1)
/**
 * Call failed due to invalid input; read the error out-pointer.
 */
#define OpenPitPretradeStatus_Error ((OpenPitPretradeStatus) 2)
```

## `OpenPitAccountAdjustmentBatchError`

Batch rejection details returned by account-adjustment apply API.

Ownership:

- created by `openpit_engine_apply_account_adjustment` on `Rejected`;
- owned by the caller;
- released with `openpit_destroy_account_adjustment_batch_error`.

```c
typedef struct OpenPitAccountAdjustmentBatchError
    OpenPitAccountAdjustmentBatchError;
```

## `OpenPitEngineBuildErrorCode`

Machine-readable discriminant describing why building an engine failed.

Each value identifies a distinct failure category. There is no success value: a
build-error object exists only when a build did not produce an engine.

```c
typedef uint8_t OpenPitEngineBuildErrorCode;
/**
 * Two or more registered policies declare the same name.
 */
#define OpenPitEngineBuildErrorCode_DuplicatePolicyName \
    ((OpenPitEngineBuildErrorCode) 0)
/**
 * Two or more registered policies declare the same non-default group id.
 */
#define OpenPitEngineBuildErrorCode_DuplicatePolicyGroupId \
    ((OpenPitEngineBuildErrorCode) 1)
/**
 * A failure category not covered by the above. Forward-compatible catch-all;
 * no structured payload is available.
 */
#define OpenPitEngineBuildErrorCode_Other ((OpenPitEngineBuildErrorCode) 2)
```

## `OpenPitEngineBuildError`

Structured build-failure details returned by engine construction.

Ownership:

- created by `openpit_engine_builder_build` when building does not produce an
  engine;
- owned by the caller;
- released with `openpit_destroy_engine_build_error`.

```c
typedef struct OpenPitEngineBuildError OpenPitEngineBuildError;
```

## `openpit_create_engine_builder`

Creates a new engine builder with the chosen synchronization policy.

Success:

- returns a non-null caller-owned builder object.

Error:

- returns null when `sync_policy` is not one of `OpenPitSyncPolicy_None` (0),
  `OpenPitSyncPolicy_Full` (1), or `OpenPitSyncPolicy_Account` (2);
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Cleanup:

- release the pointer with `openpit_destroy_engine_builder` if you stop before
  building;
- after a successful build the builder is consumed and must still be released
  with `openpit_destroy_engine_builder`.

```c
OpenPitEngineBuilder * openpit_create_engine_builder(
    uint8_t sync_policy,
    OpenPitOutError out_error
);
```

## `openpit_destroy_engine_builder`

Releases a builder pointer owned by the caller.

Contract:

- passing null is allowed;
- after this call the pointer is invalid;
- this function always succeeds.

```c
void openpit_destroy_engine_builder(
    OpenPitEngineBuilder * builder
);
```

## `openpit_engine_builder_build`

Finalizes a builder and creates an engine.

Success:

- returns a non-null engine pointer.

Error:

- returns null when `builder` is null, the builder was already consumed, or no
  policies were registered;
- for those non-domain failures, if `out_error` is not null, writes a
  caller-owned `OpenPitSharedString` error handle that MUST be released with
  `openpit_destroy_shared_string`; `out_build_error` is left untouched;
- returns null when the configuration is rejected during building (for
  example, duplicate policy names or duplicate group ids); in that case, if
  `out_build_error` is not null, writes a caller-owned
  `OpenPitEngineBuildError` pointer that carries the machine-readable failure
  code and the offending value, and MUST be released with
  `openpit_destroy_engine_build_error`; `out_error` is left untouched for this
  domain failure.

Ownership:

- on success the returned engine pointer is owned by the caller and must be
  released with `openpit_destroy_engine`; `out_build_error` is left untouched;
- the builder becomes consumed regardless of success and must not be reused.

```c
OpenPitEngine * openpit_engine_builder_build(
    OpenPitEngineBuilder * builder,
    OpenPitEngineBuildError ** out_build_error,
    OpenPitOutError out_error
);
```

## `openpit_destroy_engine_build_error`

Releases a build-error object returned by engine construction.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void openpit_destroy_engine_build_error(
    OpenPitEngineBuildError * build_error
);
```

## `openpit_engine_build_error_get_code`

Returns the machine-readable failure category of a build error.

Contract:

- `build_error` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
OpenPitEngineBuildErrorCode openpit_engine_build_error_get_code(
    const OpenPitEngineBuildError * build_error
);
```

## `openpit_engine_build_error_get_policy_name`

Returns a non-owning view of the offending policy name from a build error.

Contract:

- `build_error` must be a valid non-null pointer;
- the returned view points into memory owned by `build_error` and is valid
  while `build_error` is alive; it must not be used after the build error is
  destroyed;
- the view is empty unless the failure category is the duplicate-policy-name
  category;
- this function never fails;
- violating the pointer contract aborts the call.

```c
OpenPitStringView openpit_engine_build_error_get_policy_name(
    const OpenPitEngineBuildError * build_error
);
```

## `openpit_engine_build_error_get_policy_group_id`

Returns the offending policy group id from a build error.

Contract:

- `build_error` must be a valid non-null pointer;
- the value is zero unless the failure category is the
  duplicate-policy-group-id category;
- this function never fails;
- violating the pointer contract aborts the call.

```c
uint16_t openpit_engine_build_error_get_policy_group_id(
    const OpenPitEngineBuildError * build_error
);
```

## `openpit_destroy_engine`

Releases an engine pointer owned by the caller.

Contract:

- passing null is allowed;
- destroying the engine also releases any state and policies retained by that
  engine instance;
- this function always succeeds.

```c
void openpit_destroy_engine(
    OpenPitEngine * engine
);
```

## `openpit_engine_start_pre_trade`

Starts pre-trade processing and returns a deferred request pointer.

This stage validates whether the order can enter the full pre-trade flow.

Success:

- returns `Passed` when the order passed this stage; read `out_request`;
- returns `Rejected` when the order was rejected; read `out_rejects` if not
  null.

Error:

- returns `Error` when input pointers are invalid or the order payload cannot
  be decoded;
- on `Error`, if `out_error` is not null, it is filled with a caller-owned
  `OpenPitSharedString` that MUST be destroyed by the caller.

Cleanup:

- release a successful request with
  `openpit_pretrade_pre_trade_request_execute` or
  `openpit_destroy_pretrade_pre_trade_request`.

Output ownership contract:

- on `Passed`, a non-null request pointer is written to `out_request` if it is
  not null;
- on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to
  `out_rejects` if it is not null;
- the caller owns either returned object and MUST release it with the
  corresponding destroy function;
- no thread-local state is involved, and returned pointers are safe to read on
  any thread;
- on `Passed` and `Error`, `out_rejects` is left untouched;
- on `Rejected` and `Error`, `out_request` is left untouched.

Order lifetime contract:

- `order` is read as a borrowed view during this call;
- the operation snapshots that payload before returning, because the deferred
  request may outlive the source buffers.

```c
OpenPitPretradeStatus openpit_engine_start_pre_trade(
    OpenPitEngine * engine,
    const OpenPitOrder * order,
    OpenPitPretradePreTradeRequest ** out_request,
    OpenPitPretradeRejectList ** out_rejects,
    OpenPitOutError out_error
);
```

## `openpit_engine_execute_pre_trade`

Runs the complete pre-trade check in one call.

Success:

- returns `Passed` when the order passed this stage; read `out_reservation`;
- returns `Rejected` when the order was rejected is not null; read
  `out_rejects`.

Error:

- returns `Error` when input pointers are invalid or the order payload cannot
  be decoded;
- on `Error`, if `out_error` is not null, it is filled with a caller-owned
  `OpenPitSharedString` that MUST be destroyed by the caller.

Cleanup:

- release a successful reservation with
  `openpit_pretrade_pre_trade_reservation_commit`,
  `openpit_pretrade_pre_trade_reservation_rollback`, or
  `openpit_destroy_pretrade_pre_trade_reservation`.

Output ownership contract:

- on `Passed`, a non-null reservation pointer is written to `out_reservation`
  if it is not null;
- on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to
  `out_rejects` if it is not null;
- the caller owns either returned object and MUST release it with the
  corresponding destroy function;
- no thread-local state is involved, and returned pointers are safe to read on
  any thread;
- on `Passed` and `Error`, `out_rejects` is left untouched;
- on `Rejected` and `Error`, `out_reservation` is left untouched.

Order lifetime contract:

- `order` is read as a borrowed view during this call only;
- the operation does not retain any pointer into source memory after this
  function returns.

```c
OpenPitPretradeStatus openpit_engine_execute_pre_trade(
    OpenPitEngine * engine,
    const OpenPitOrder * order,
    OpenPitPretradePreTradeReservation ** out_reservation,
    OpenPitPretradeRejectList ** out_rejects,
    OpenPitOutError out_error
);
```

## `openpit_engine_start_pre_trade_dry_run`

Runs the start stage as a non-mutating pre-trade dry-run.

A dry-run reports the verdict the start stage *would* return for `order` with
zero effect on engine state: no policy side effect is applied and no account
block is recorded, even when an account-scope reject *would* latch one. The
reported lock and account adjustments are always empty, because they are
produced by the main stage, which this call does not run; see
`openpit_engine_execute_pre_trade_dry_run` for a full-pipeline dry-run.

Success:

- returns `true` on valid input and writes a non-null caller-owned report to
  `out_report` if it is not null; the pass/reject verdict lives inside the
  report (a dry-run never rejects the *call*).

Error:

- returns `false` when input pointers are invalid or the order payload cannot
  be decoded;
- on `false`, if `out_error` is not null, it is filled with a caller-owned
  `OpenPitSharedString` that MUST be destroyed by the caller.

Cleanup:

- release a produced report with
  `openpit_destroy_pretrade_pre_trade_dry_run_report`.

Output ownership contract:

- on success, a non-null report pointer is written to `out_report` if it is
  not null;
- the caller owns the report and MUST release it with the corresponding
  destroy function;
- no thread-local state is involved, and the returned pointer is safe to read
  on any thread;
- on `false`, `out_report` is left untouched.

Order lifetime contract:

- `order` is read as a borrowed view during this call only;
- the operation does not retain any pointer into source memory after this
  function returns.

```c
bool openpit_engine_start_pre_trade_dry_run(
    OpenPitEngine * engine,
    const OpenPitOrder * order,
    OpenPitPretradePreTradeDryRunReport ** out_report,
    OpenPitOutError out_error
);
```

## `openpit_engine_execute_pre_trade_dry_run`

Runs the full pre-trade pipeline as a non-mutating dry-run.

A dry-run reports the verdict, the lock, and the account adjustments both stages
*would* produce for `order` with zero effect on engine state: no rate-limit
budget is spent, no reservation or hold is applied, and no account block is
recorded. Any mutations a main-stage policy registers on the dry-run path are
dropped - neither committed nor rolled back - so a repeated dry-run never moves
engine state.

Success:

- returns `true` on valid input and writes a non-null caller-owned report to
  `out_report` if it is not null; the pass/reject verdict lives inside the
  report (a dry-run never rejects the *call*).

Error:

- returns `false` when input pointers are invalid or the order payload cannot
  be decoded;
- on `false`, if `out_error` is not null, it is filled with a caller-owned
  `OpenPitSharedString` that MUST be destroyed by the caller.

Cleanup:

- release a produced report with
  `openpit_destroy_pretrade_pre_trade_dry_run_report`.

Output ownership contract:

- on success, a non-null report pointer is written to `out_report` if it is
  not null;
- the caller owns the report and MUST release it with the corresponding
  destroy function;
- no thread-local state is involved, and the returned pointer is safe to read
  on any thread;
- on `false`, `out_report` is left untouched.

Order lifetime contract:

- `order` is read as a borrowed view during this call only;
- the operation does not retain any pointer into source memory after this
  function returns.

```c
bool openpit_engine_execute_pre_trade_dry_run(
    OpenPitEngine * engine,
    const OpenPitOrder * order,
    OpenPitPretradePreTradeDryRunReport ** out_report,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_pre_trade_request_execute`

Executes a deferred request returned by `openpit_engine_start_pre_trade`.

Success:

- returns `Passed` when the order passed this stage; read `out_reservation`;
- returns `Rejected` when the order was rejected and `out_rejects` is not
  null; read `out_rejects`.

Error:

- returns `Error` when input pointers are invalid or the order payload cannot
  be decoded;
- on `Error`, if `out_error` is not null, it is filled with a caller-owned
  `OpenPitSharedString` that MUST be destroyed by the caller.

Ownership:

- this call consumes the request object's content exactly once;
- after a successful or failed execute, the object itself may still be
  released with `openpit_destroy_pretrade_pre_trade_request`, but it cannot be
  executed again.

Output ownership contract:

- on `Passed`, a non-null reservation pointer is written to `out_reservation`
  if it is not null;
- on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to
  `out_rejects` if it is not null;
- the caller owns either returned object and MUST release it with the
  corresponding destroy function;
- no thread-local state is involved, and returned pointers are safe to read on
  any thread;
- on `Passed` and `Error`, `out_rejects` is left untouched;
- on `Rejected` and `Error`, `out_reservation` is left untouched.

```c
OpenPitPretradeStatus openpit_pretrade_pre_trade_request_execute(
    OpenPitPretradePreTradeRequest * request,
    OpenPitPretradePreTradeReservation ** out_reservation,
    OpenPitPretradeRejectList ** out_rejects,
    OpenPitOutError out_error
);
```

## `openpit_destroy_pretrade_pre_trade_request`

Releases a deferred request pointer owned by the caller.

Contract:

- passing null is allowed;
- destroying an unexecuted request abandons it without creating a reservation;
- this function always succeeds.

```c
void openpit_destroy_pretrade_pre_trade_request(
    OpenPitPretradePreTradeRequest * request
);
```

## `openpit_pretrade_pre_trade_reservation_commit`

Finalizes a reservation and applies the reserved state permanently.

This call is idempotent at the pointer level: if the reservation was already
consumed, nothing happens. Passing null is allowed.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void openpit_pretrade_pre_trade_reservation_commit(
    OpenPitPretradePreTradeReservation * reservation
);
```

## `openpit_pretrade_pre_trade_reservation_rollback`

Cancels a reservation and releases the reserved state.

This call is idempotent at the pointer level: if the reservation was already
consumed, nothing happens. Passing null is allowed.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void openpit_pretrade_pre_trade_reservation_rollback(
    OpenPitPretradePreTradeReservation * reservation
);
```

## `openpit_pretrade_pre_trade_reservation_get_lock`

Returns a snapshot of the lock attached to a reservation.

Contract:

- `reservation` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails.

Lifetime contract:

- the returned snapshot is detached from the reservation state.

```c
OpenPitPretradePreTradeLock * openpit_pretrade_pre_trade_reservation_get_lock(
    const OpenPitPretradePreTradeReservation * reservation
);
```

## `openpit_pretrade_pre_trade_reservation_get_account_adjustments`

Returns the account-adjustment outcomes collected by the reservation.

Contract:

- `reservation` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails;
- always returns a caller-owned `OpenPitAccountAdjustmentOutcomeList`
  (possibly empty); release it with
  `openpit_destroy_account_adjustment_outcome_list`.

Lifetime contract:

- the returned list is detached from the reservation state.

```c
OpenPitAccountAdjustmentOutcomeList *
openpit_pretrade_pre_trade_reservation_get_account_adjustments(
    const OpenPitPretradePreTradeReservation * reservation
);
```

## `openpit_destroy_pretrade_pre_trade_reservation`

Releases a reservation pointer owned by the caller.

Contract:

- passing null is allowed;
- destroying an unresolved reservation triggers rollback of any pending
  mutations;
- callers that need explicit resolution should call commit or rollback first;
- this function always succeeds.

```c
void openpit_destroy_pretrade_pre_trade_reservation(
    OpenPitPretradePreTradeReservation * reservation
);
```

## `openpit_pretrade_pre_trade_dry_run_report_is_pass`

Returns whether the order would have passed every pre-trade stage.

Contract:

- `report` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails;
- equivalent to "the rejects list returned by
  `openpit_pretrade_pre_trade_dry_run_report_get_rejects` is empty".

```c
bool openpit_pretrade_pre_trade_dry_run_report_is_pass(
    const OpenPitPretradePreTradeDryRunReport * report
);
```

## `openpit_pretrade_pre_trade_dry_run_report_get_rejects`

Returns the rejects the order would have collected.

Contract:

- `report` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails;
- always returns a caller-owned `OpenPitPretradeRejectList` (empty when the
  order would have passed); release it with
  `openpit_pretrade_destroy_reject_list`.

Lifetime contract:

- the returned list is detached from the report state.

```c
OpenPitPretradeRejectList *
openpit_pretrade_pre_trade_dry_run_report_get_rejects(
    const OpenPitPretradePreTradeDryRunReport * report
);
```

## `openpit_pretrade_pre_trade_dry_run_report_get_lock`

Returns a snapshot of the lock the main stage would have produced.

Contract:

- `report` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails;
- the lock is empty when the start stage would have rejected (the main stage
  never runs in that case) or when no policy locks a price; release the
  returned handle with `openpit_destroy_pretrade_pre_trade_lock`.

Lifetime contract:

- the returned snapshot is detached from the report state.

```c
OpenPitPretradePreTradeLock *
openpit_pretrade_pre_trade_dry_run_report_get_lock(
    const OpenPitPretradePreTradeDryRunReport * report
);
```

## `openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments`

Returns the account-adjustment outcomes the main stage would have produced.

Contract:

- `report` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails;
- always returns a caller-owned `OpenPitAccountAdjustmentOutcomeList`
  (possibly empty); release it with
  `openpit_destroy_account_adjustment_outcome_list`. The numbers match what a
  real reservation reports for the same order and engine state.

Lifetime contract:

- the returned list is detached from the report state.

```c
OpenPitAccountAdjustmentOutcomeList *
openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(
    const OpenPitPretradePreTradeDryRunReport * report
);
```

## `openpit_pretrade_pre_trade_dry_run_report_get_account_block`

Returns the account block an account-scope reject would have latched.

Contract:

- `report` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails;
- always returns a caller-owned `OpenPitPretradeAccountBlockList` carrying the
  single would-be block, or empty when no account-scope reject would have
  latched one; release it with `openpit_pretrade_destroy_account_block_list`.
  A real call records this block in the engine's blocked-accounts registry; a
  dry-run reports it here without recording it.

Lifetime contract:

- the returned list is detached from the report state.

```c
OpenPitPretradeAccountBlockList *
openpit_pretrade_pre_trade_dry_run_report_get_account_block(
    const OpenPitPretradePreTradeDryRunReport * report
);
```

## `openpit_destroy_pretrade_pre_trade_dry_run_report`

Releases a dry-run report pointer owned by the caller.

Contract:

- passing null is allowed;
- after this call the pointer is invalid;
- this function always succeeds.

```c
void openpit_destroy_pretrade_pre_trade_dry_run_report(
    OpenPitPretradePreTradeDryRunReport * report
);
```

## `openpit_engine_apply_execution_report`

Applies an execution report to engine state.

Returns `true` on success, `false` on error.

Success:

- returns `true`;
- if `out_blocks` is not null and at least one policy entered a blocked state,
  writes a caller-owned `OpenPitPretradeAccountBlockList` pointer; release it
  with `openpit_pretrade_destroy_account_block_list`;
- when no policy blocked, `out_blocks` is left untouched;
- if `out_adjustments` is not null and at least one policy produced an
  account-adjustment outcome, writes a caller-owned
  `OpenPitAccountAdjustmentOutcomeList` pointer; release it with
  `openpit_destroy_account_adjustment_outcome_list`;
- when no outcome was produced, `out_adjustments` is left untouched.

Error:

- returns `false` when input pointers are invalid or the report payload cannot
  be decoded;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Lifetime contract:

- `report` is read as a borrowed view during this call only;
- the operation does not retain any pointer into source memory after this
  function returns.

```c
bool openpit_engine_apply_execution_report(
    OpenPitEngine * engine,
    const OpenPitExecutionReport * report,
    OpenPitPretradeAccountBlockList ** out_blocks,
    OpenPitAccountAdjustmentOutcomeList ** out_adjustments,
    OpenPitOutError out_error
);
```

## `openpit_destroy_account_adjustment_batch_error`

Releases a batch-error object returned by account-adjustment apply.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void openpit_destroy_account_adjustment_batch_error(
    OpenPitAccountAdjustmentBatchError * batch_error
);
```

## `openpit_account_adjustment_batch_error_get_failed_adjustment_index`

Returns the failing adjustment index from a batch error.

Contract:

- `batch_error` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
size_t openpit_account_adjustment_batch_error_get_failed_adjustment_index(
    const OpenPitAccountAdjustmentBatchError * batch_error
);
```

## `openpit_account_adjustment_batch_error_get_rejects`

Returns a non-owning reject-list view from a batch error.

Contract:

- `batch_error` must be a valid non-null pointer;
- the returned pointer is valid while `batch_error` is alive;
- this function never fails;
- violating the pointer contract aborts the call.

```c
const OpenPitPretradeRejectList *
openpit_account_adjustment_batch_error_get_rejects(
    const OpenPitAccountAdjustmentBatchError * batch_error
);
```

## `openpit_engine_apply_account_adjustment`

Applies a batch of account adjustments to one account.

Success:

- returns `OpenPitAccountAdjustmentApplyStatus::Applied` when the batch was
  accepted and applied;
- returns `OpenPitAccountAdjustmentApplyStatus::Rejected` when the call itself
  completed normally but a policy rejected the batch; read `out_reject`.

Error:

- returns `OpenPitAccountAdjustmentApplyStatus::Error` when input pointers are
  invalid or some adjustment payload cannot be decoded;
- on `Error`, if `out_error` is not null, it is filled with a caller-owned
  `OpenPitSharedString` that MUST be destroyed by the caller.

Result handling:

- `Applied` means there is no reject object to clean up;
- on `Applied`, if `out_outcomes` is not null and at least one policy produced
  an account-adjustment outcome, writes a caller-owned
  `OpenPitAccountAdjustmentOutcomeList` pointer; release it with
  `openpit_destroy_account_adjustment_outcome_list`; if no outcome was
  produced, `out_outcomes` is left untouched;
- `Rejected` stores batch error details in `out_reject`, the caller must
  release a returned object with
  `openpit_destroy_account_adjustment_batch_error`;
- rejects returned by `openpit_account_adjustment_batch_error_get_rejects`
  contain string views borrowed from the batch error and must not be used
  after the batch error is destroyed;
- when `Error` is returned, do not use any pointer from a previous unrelated
  call as if it belonged to this failure.

Lifetime contract:

- every `adjustment` entry from the contiguous input array is read as a
  borrowed view during this call only;
- release a returned batch error with
  `openpit_destroy_account_adjustment_batch_error`.

```c
OpenPitAccountAdjustmentApplyStatus openpit_engine_apply_account_adjustment(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id,
    const OpenPitAccountAdjustment * adjustments,
    size_t adjustments_len,
    OpenPitAccountAdjustmentBatchError ** out_reject,
    OpenPitAccountAdjustmentOutcomeList ** out_outcomes,
    OpenPitOutError out_error
);
```

## `OpenPitAccountGroupError`

Structured error returned by account-group registry operations.

Ownership:

- created by `openpit_engine_register_account_group` and
  `openpit_engine_unregister_account_group` on failure;
- owned by the caller;
- released with `openpit_destroy_account_group_error`.

```c
typedef struct OpenPitAccountGroupError OpenPitAccountGroupError;
```

## `openpit_destroy_account_group_error`

Releases a caller-owned account-group error.

Contract:

- call exactly once per pointer returned by a registry function;
- passing null is allowed and has no effect.

```c
void openpit_destroy_account_group_error(
    OpenPitAccountGroupError * err
);
```

## `openpit_account_group_error_get_message`

Returns the human-readable error message from an account-group error.

Contract:

- `err` must be a valid non-null pointer;
- the returned view borrows from the error object and is valid while the error
  is alive;
- violating the pointer contract aborts the call.

```c
OpenPitStringView openpit_account_group_error_get_message(
    const OpenPitAccountGroupError * err
);
```

## `openpit_account_group_error_get_account`

Returns the offending account identifier from an account-group error.

Contract:

- `err` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
OpenPitParamAccountId openpit_account_group_error_get_account(
    const OpenPitAccountGroupError * err
);
```

## `openpit_account_group_error_get_current_group`

Returns the current group of the offending account from an account-group error,
or returns `false` and leaves `out_group` untouched when no group is set.

Contract:

- `err` must be a valid non-null pointer;
- `out_group` must be a valid non-null pointer;
- returns `true` when the account belongs to a group and writes that group to
  `out_group`;
- returns `false` when the account belongs to no group; `out_group` is written
  to only when the return value is `true`;
- violating the pointer contract aborts the call.

```c
bool openpit_account_group_error_get_current_group(
    const OpenPitAccountGroupError * err,
    OpenPitParamAccountGroupId * out_group
);
```

## `openpit_engine_register_account_group`

Atomically registers every account in `accounts` into `group`.

The operation is all-or-nothing: if any listed account is already a member of
any group (including `group`), no account is registered.

Contract:

- `engine` must be a valid non-null engine pointer;
- `accounts` must point to an array of at least `accounts_len` account
  identifiers, or may be null when `accounts_len` is zero;
- `group` is the target group and must not be the reserved
  `OPENPIT_DEFAULT_ACCOUNT_GROUP`.

Success:

- returns `true`; all listed accounts are now members of `group`.

Error:

- returns `false` when `engine` is null, `accounts` is null with non-zero
  length, `group` is the reserved default group, or any listed account is
  already registered;
- for pointer/argument errors, if `out_error` is not null, writes a
  caller-owned `OpenPitSharedString` error handle that MUST be released with
  `openpit_destroy_shared_string`;
- for domain errors (reserved target group, or account already registered), if
  `out_group_error` is not null, writes a caller-owned
  `OpenPitAccountGroupError` pointer that MUST be released with
  `openpit_destroy_account_group_error`; `out_error` is left untouched for
  domain failures.

```c
bool openpit_engine_register_account_group(
    OpenPitEngine * engine,
    const OpenPitParamAccountId * accounts,
    size_t accounts_len,
    OpenPitParamAccountGroupId group,
    OpenPitAccountGroupError ** out_group_error,
    OpenPitOutError out_error
);
```

## `openpit_engine_unregister_account_group`

Atomically removes every account in `accounts` from `group`.

The operation is all-or-nothing: if any listed account is not currently a member
of `group`, no account is removed.

Contract:

- `engine` must be a valid non-null engine pointer;
- `accounts` must point to an array of at least `accounts_len` account
  identifiers, or may be null when `accounts_len` is zero;
- `group` is the group to remove accounts from and must not be the reserved
  `OPENPIT_DEFAULT_ACCOUNT_GROUP`.

Success:

- returns `true`; all listed accounts are now removed from `group`.

Error:

- returns `false` when `engine` is null, `accounts` is null with non-zero
  length, `group` is the reserved default group, or any listed account is not
  in `group`;
- for pointer/argument errors, if `out_error` is not null, writes a
  caller-owned `OpenPitSharedString` error handle that MUST be released with
  `openpit_destroy_shared_string`;
- for domain errors (reserved target group, or account not in group), if
  `out_group_error` is not null, writes a caller-owned
  `OpenPitAccountGroupError` pointer that MUST be released with
  `openpit_destroy_account_group_error`; `out_error` is left untouched for
  domain failures.

```c
bool openpit_engine_unregister_account_group(
    OpenPitEngine * engine,
    const OpenPitParamAccountId * accounts,
    size_t accounts_len,
    OpenPitParamAccountGroupId group,
    OpenPitAccountGroupError ** out_group_error,
    OpenPitOutError out_error
);
```

## `openpit_engine_account_group`

Returns the account-group membership of a single account.

Contract:

- `engine` must be a valid non-null engine pointer;
- `account` is the account identifier to look up;
- `out_group` must be a valid non-null pointer.

Success:

- returns `true` when the account belongs to a group and writes that group
  identifier to `out_group`;
- returns `false` when the account belongs to no group; `out_group` is not
  written to when the return value is `false`.

Error:

- aborts the call when `engine` or `out_group` is null.

```c
bool openpit_engine_account_group(
    const OpenPitEngine * engine,
    OpenPitParamAccountId account,
    OpenPitParamAccountGroupId * out_group
);
```

## `openpit_engine_set_account_currency`

Sets the explicit currency of `account`.

Setting or changing the account currency does not validate existing holdings and
does not recompute stored average entry price or realized PnL. The caller owns
the risk of changing currency on live state; a control or recompute API may be
added later.

Contract:

- passing null for `engine` returns `false` and writes `out_error`;
- `asset` is parsed as an asset code and must be present;
- on parse failure, if `out_error` is not null, writes a caller-owned
  `OpenPitSharedString` error handle that MUST be released with
  `openpit_destroy_shared_string`.

```c
bool openpit_engine_set_account_currency(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id,
    OpenPitStringView asset,
    OpenPitOutError out_error
);
```

## `openpit_engine_clear_account_currency`

Clears the explicit currency of `account`.

Clearing the account currency does not validate existing holdings and does not
recompute stored average entry price or realized PnL. The caller owns the risk
of changing currency on live state; a control or recompute API may be added
later.

Contract:

- `engine` must be a valid non-null engine pointer.

```c
void openpit_engine_clear_account_currency(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id
);
```

## `openpit_engine_set_account_group_currency`

Sets the currency inherited by accounts in `group`.

`OPENPIT_DEFAULT_ACCOUNT_GROUP` (value `0`) is allowed and represents the global
default tier.

Setting or changing the group currency does not validate existing holdings and
does not recompute stored average entry price or realized PnL. The caller owns
the risk of changing currency on live state; a control or recompute API may be
added later.

Contract:

- passing null for `engine` returns `false` and writes `out_error`;
- `group` must be `OPENPIT_DEFAULT_ACCOUNT_GROUP` or a valid non-reserved
  group id; an invalid (reserved) id returns `false` and writes `out_error`;
- `asset` is parsed as an asset code and must be present;
- on any failure, if `out_error` is not null, writes a caller-owned
  `OpenPitSharedString` error handle that MUST be released with
  `openpit_destroy_shared_string`.

```c
bool openpit_engine_set_account_group_currency(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group,
    OpenPitStringView asset,
    OpenPitOutError out_error
);
```

## `openpit_engine_clear_account_group_currency`

Clears the currency inherited by accounts in `group`.

`OPENPIT_DEFAULT_ACCOUNT_GROUP` (value `0`) is allowed and represents the global
default tier.

Clearing the group currency does not validate existing holdings and does not
recompute stored average entry price or realized PnL. The caller owns the risk
of changing currency on live state; a control or recompute API may be added
later.

Contract:

- `engine` must be a valid non-null engine pointer;
- `group` must be `OPENPIT_DEFAULT_ACCOUNT_GROUP` or a valid non-reserved
  group id; an invalid (reserved) id is silently ignored (no-op) because this
  function has no error-output channel; prefer
  `openpit_engine_set_account_group_currency` when error reporting is needed.

```c
void openpit_engine_clear_account_group_currency(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group
);
```

## `OpenPitAccountBlockError`

Structured error returned by account block operations.

Ownership:

- created by `openpit_engine_replace_account_block_reason`,
  `openpit_engine_block_account_group`,
  `openpit_engine_unblock_account_group`, and
  `openpit_engine_replace_account_group_block_reason` on failure;
- owned by the caller;
- released with `openpit_destroy_account_block_error`.

```c
typedef struct OpenPitAccountBlockError OpenPitAccountBlockError;
```

## `OpenPitAccountBlockErrorKind`

Discriminant for the variant carried by an [`OpenPitAccountBlockError`].

```c
typedef uint32_t OpenPitAccountBlockErrorKind;
/**
 * The target group is the reserved default account group.
 */
#define OpenPitAccountBlockErrorKind_ReservedGroup \
    ((OpenPitAccountBlockErrorKind) 0)
/**
 * The target account is not currently blocked.
 */
#define OpenPitAccountBlockErrorKind_AccountNotBlocked \
    ((OpenPitAccountBlockErrorKind) 1)
/**
 * The target account group is not currently blocked.
 */
#define OpenPitAccountBlockErrorKind_GroupNotBlocked \
    ((OpenPitAccountBlockErrorKind) 2)
```

## `openpit_destroy_account_block_error`

Releases a caller-owned account-block error.

Contract:

- call exactly once per pointer returned by a block function;
- passing null is allowed and has no effect.

```c
void openpit_destroy_account_block_error(
    OpenPitAccountBlockError * err
);
```

## `openpit_account_block_error_get_message`

Returns the human-readable error message from an account-block error.

Contract:

- `err` must be a valid non-null pointer;
- the returned view borrows from the error object and is valid while the error
  is alive;
- violating the pointer contract aborts the call.

```c
OpenPitStringView openpit_account_block_error_get_message(
    const OpenPitAccountBlockError * err
);
```

## `openpit_account_block_error_get_kind`

Returns the variant kind of an account-block error.

Contract:

- `err` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
OpenPitAccountBlockErrorKind openpit_account_block_error_get_kind(
    const OpenPitAccountBlockError * err
);
```

## `openpit_account_block_error_get_account`

Returns the offending account identifier from an account-block error.

Contract:

- `err` must be a valid non-null pointer;
- `out_account` must be a valid non-null pointer;
- returns `true` when the error variant carries an account and writes it to
  `out_account`;
- returns `false` when no account is present; `out_account` is left untouched
  when the return value is `false`;
- violating the pointer contract aborts the call.

```c
bool openpit_account_block_error_get_account(
    const OpenPitAccountBlockError * err,
    OpenPitParamAccountId * out_account
);
```

## `openpit_account_block_error_get_group`

Returns the offending account-group identifier from an account-block error.

Contract:

- `err` must be a valid non-null pointer;
- `out_group` must be a valid non-null pointer;
- returns `true` when the error variant carries a group and writes it to
  `out_group`;
- returns `false` when no group is present; `out_group` is left untouched when
  the return value is `false`;
- violating the pointer contract aborts the call.

```c
bool openpit_account_block_error_get_group(
    const OpenPitAccountBlockError * err,
    OpenPitParamAccountGroupId * out_group
);
```

## `OpenPitConfigureErrorKind`

Discriminant for the variant carried by an [`OpenPitConfigureError`].

```c
typedef uint32_t OpenPitConfigureErrorKind;
/**
 * No configurable policy carries the requested name.
 */
#define OpenPitConfigureErrorKind_Unknown ((OpenPitConfigureErrorKind) 0)
/**
 * A policy is registered under the name, but its settings type differs from
 * the one the called configure function targets.
 */
#define OpenPitConfigureErrorKind_TypeMismatch ((OpenPitConfigureErrorKind) 1)
/**
 * The applied update was rejected by the policy's settings validation; the
 * prior configuration still applies.
 */
#define OpenPitConfigureErrorKind_Validation ((OpenPitConfigureErrorKind) 2)
```

## `OpenPitConfigureError`

Structured error returned by runtime policy reconfiguration.

Ownership:

- created by the `openpit_engine_configure_*` functions on failure;
- owned by the caller;
- released with `openpit_destroy_configure_error`.

```c
typedef struct OpenPitConfigureError OpenPitConfigureError;
```

## `openpit_destroy_configure_error`

Releases a caller-owned configure error.

Contract:

- call exactly once per pointer returned by an `openpit_engine_configure_*`
  function;
- passing null is allowed and has no effect.

```c
void openpit_destroy_configure_error(
    OpenPitConfigureError * err
);
```

## `openpit_configure_error_get_message`

Returns the human-readable error message from a configure error.

Contract:

- `err` must be a valid non-null pointer;
- the returned view borrows from the error object and is valid while the error
  is alive;
- violating the pointer contract aborts the call.

```c
OpenPitStringView openpit_configure_error_get_message(
    const OpenPitConfigureError * err
);
```

## `openpit_configure_error_get_kind`

Returns the variant kind of a configure error.

Contract:

- `err` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
OpenPitConfigureErrorKind openpit_configure_error_get_kind(
    const OpenPitConfigureError * err
);
```

## `openpit_engine_block_account`

Blocks `account` with `reason`.

The first cause for an account wins: if the account is already blocked (by an
admin call or a prior kill-switch), this call is a no-op and does not overwrite
the stored reason. Use `openpit_engine_replace_account_block_reason` to change
the stored reason.

Contract:

- `engine` must be a valid non-null engine pointer;
- `reason` is interpreted as UTF-8; an empty string is used when `reason.ptr`
  is null OR `reason.len` is zero; passing a null `ptr` with a non-zero `len`
  is caller misuse and is treated as empty (not read); an empty reason is
  explicitly allowed.

```c
void openpit_engine_block_account(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id,
    OpenPitStringView reason
);
```

## `openpit_engine_unblock_account`

Unblocks `account`, clearing any block on it.

Idempotent: a no-op when `account` is not blocked. Both admin blocks and
kill-switch blocks are cleared.

Contract:

- `engine` must be a valid non-null engine pointer.

```c
void openpit_engine_unblock_account(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id
);
```

## `openpit_engine_replace_account_block_reason`

Replaces the stored reason of an already-blocked account.

Unlike `openpit_engine_block_account`, which preserves the first cause, this
overwrites the stored cause with `reason`, leaving the account blocked.

Contract:

- `engine` must be a valid non-null engine pointer;
- `reason` is interpreted as UTF-8; an empty string is used when `reason.ptr`
  is null OR `reason.len` is zero; passing a null `ptr` with a non-zero `len`
  is caller misuse and is treated as empty (not read); an empty reason is
  explicitly allowed;
- on failure, if `out_error` is not null, writes a caller-owned
  `OpenPitAccountBlockError` pointer that MUST be released with
  `openpit_destroy_account_block_error`;
- returns `false` and writes `out_error` when `engine` is null.

Success:

- returns `true`; the stored reason has been replaced.

Error:

- returns `false` with `OpenPitAccountBlockErrorKind_AccountNotBlocked` when
  `account` is not currently blocked.

```c
bool openpit_engine_replace_account_block_reason(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id,
    OpenPitStringView reason,
    OpenPitAccountBlockError ** out_error
);
```

## `openpit_engine_block_account_group`

Blocks the account group `group` with `reason`.

The first cause for a group wins: re-blocking an already-blocked group is a
no-op. Use `openpit_engine_replace_account_group_block_reason` to change the
stored reason.

Contract:

- `engine` must be a valid non-null engine pointer;
- `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
- `reason` is interpreted as UTF-8; an empty string is used when `reason.ptr`
  is null OR `reason.len` is zero; passing a null `ptr` with a non-zero `len`
  is caller misuse and is treated as empty (not read); an empty reason is
  explicitly allowed;
- on failure, if `out_error` is not null, writes a caller-owned
  `OpenPitAccountBlockError` pointer that MUST be released with
  `openpit_destroy_account_block_error`;
- returns `false` and writes `out_error` when `engine` is null.

Success:

- returns `true`; the group is now blocked.

Error:

- returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
  `group` is the reserved default group.

```c
bool openpit_engine_block_account_group(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group,
    OpenPitStringView reason,
    OpenPitAccountBlockError ** out_error
);
```

## `openpit_engine_unblock_account_group`

Unblocks the account group `group`, clearing the group block.

Idempotent: a no-op when `group` is not blocked. Accounts blocked individually
remain blocked.

Contract:

- `engine` must be a valid non-null engine pointer;
- `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
- on failure, if `out_error` is not null, writes a caller-owned
  `OpenPitAccountBlockError` pointer that MUST be released with
  `openpit_destroy_account_block_error`;
- returns `false` and writes `out_error` when `engine` is null.

Success:

- returns `true`; the group is now unblocked.

Error:

- returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
  `group` is the reserved default group.

```c
bool openpit_engine_unblock_account_group(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group,
    OpenPitAccountBlockError ** out_error
);
```

## `openpit_engine_replace_account_group_block_reason`

Replaces the stored reason of an already-blocked account group.

Unlike `openpit_engine_block_account_group`, which preserves the first cause,
this overwrites the stored cause with `reason`, leaving the group blocked.

Contract:

- `engine` must be a valid non-null engine pointer;
- `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
- `reason` is interpreted as UTF-8; an empty string is used when `reason.ptr`
  is null OR `reason.len` is zero; passing a null `ptr` with a non-zero `len`
  is caller misuse and is treated as empty (not read); an empty reason is
  explicitly allowed;
- on failure, if `out_error` is not null, writes a caller-owned
  `OpenPitAccountBlockError` pointer that MUST be released with
  `openpit_destroy_account_block_error`;
- returns `false` and writes `out_error` when `engine` is null.

Success:

- returns `true`; the stored group-block reason has been replaced.

Error:

- returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
  `group` is the reserved default group;
- returns `false` with `OpenPitAccountBlockErrorKind_GroupNotBlocked` when
  `group` is not currently blocked.

```c
bool openpit_engine_replace_account_group_block_reason(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group,
    OpenPitStringView reason,
    OpenPitAccountBlockError ** out_error
);
```
