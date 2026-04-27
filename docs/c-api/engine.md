# Engine

[Back to index](index.md)

## `PitEngineBuilder`

Opaque builder pointer used to assemble an engine instance.

Ownership:

- returned by `pit_create_engine_builder`;
- owned by the caller until passed to `pit_destroy_engine_builder`;
- consumed by `pit_engine_builder_build`.

```c
typedef struct PitEngineBuilder PitEngineBuilder;
```

## `PitEngine`

Opaque engine pointer.

The engine stores policies and mutable risk state. The caller owns the
pointer until `pit_destroy_engine`.

```c
typedef struct PitEngine PitEngine;
```

## `PitPretradePreTradeRequest`

Opaque pointer for a deferred pre-trade request.

This is returned by `pit_engine_start_pre_trade`. It can be executed once
with `pit_pretrade_pre_trade_request_execute` or discarded with
`pit_destroy_pretrade_pre_trade_request`.

```c
typedef struct PitPretradePreTradeRequest PitPretradePreTradeRequest;
```

## `PitPretradePreTradeReservation`

Opaque reservation pointer returned by a successful pre-trade check.

A reservation represents resources that have been tentatively locked. The
caller must resolve it exactly once by calling
`pit_pretrade_pre_trade_reservation_commit`,
`pit_pretrade_pre_trade_reservation_rollback`, or
`pit_destroy_pretrade_pre_trade_reservation`.

```c
typedef struct PitPretradePreTradeReservation PitPretradePreTradeReservation;
```

## `PitPretradePreTradeLock`

Price-lock snapshot returned from a reservation.

```c
typedef struct PitPretradePreTradeLock {
    PitParamPriceOptional price;
} PitPretradePreTradeLock;
```

## `PitPretradeStatus`

Result status for pre-trade operations.

```c
typedef uint8_t PitPretradeStatus;
/**
 * Order/request passed this stage; read the success out-pointer.
 */
#define PitPretradeStatus_Passed ((PitPretradeStatus) 0)
/**
 * Order/request was rejected; read the reject out-pointer.
 */
#define PitPretradeStatus_Rejected ((PitPretradeStatus) 1)
/**
 * Call failed due to invalid input; read the error out-pointer.
 */
#define PitPretradeStatus_Error ((PitPretradeStatus) 2)
```

## `PitAccountAdjustmentBatchError`

Batch rejection details returned by account-adjustment apply API.

Ownership:

- created by `pit_engine_apply_account_adjustment` on `Rejected`;
- owned by the caller;
- released with `pit_destroy_account_adjustment_batch_error`.

```c
typedef struct PitAccountAdjustmentBatchError PitAccountAdjustmentBatchError;
```

## `pit_create_engine_builder`

Creates a new engine builder.

Contract:

- returns a new caller-owned builder object;
- this function always succeeds.

Cleanup:

- release the pointer with `pit_destroy_engine_builder` if you stop before

building;

- after a successful build the builder is consumed and must still be

released with `pit_destroy_engine_builder`.

```c
PitEngineBuilder * pit_create_engine_builder(void);
```

## `pit_destroy_engine_builder`

Releases a builder pointer owned by the caller.

Contract:

- passing null is allowed;
- after this call the pointer is invalid;
- this function always succeeds.

```c
void pit_destroy_engine_builder(
    PitEngineBuilder * builder
);
```

## `pit_engine_builder_build`

Finalizes a builder and creates an engine.

Success:

- returns a non-null engine pointer.

Error:

- returns null when `builder` is null, the builder was already consumed, or

configuration is invalid;

- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Ownership:

- on success the returned engine pointer is owned by the caller and must be

released with `pit_destroy_engine`;

- the builder becomes consumed regardless of success and must not be reused.

```c
PitEngine * pit_engine_builder_build(
    PitEngineBuilder * builder,
    PitOutError out_error
);
```

## `pit_destroy_engine`

Releases an engine pointer owned by the caller.

Contract:

- passing null is allowed;
- destroying the engine also releases any state and policies retained by

that engine instance;

- this function always succeeds.

```c
void pit_destroy_engine(
    PitEngine * engine
);
```

## `pit_engine_start_pre_trade`

Starts pre-trade processing and returns a deferred request pointer.

This stage validates whether the order can enter the full pre-trade flow.

Success:

- returns `Passed` when the order passed this stage; read `out_request`;
- returns `Rejected` when the order was rejected; read `out_rejects` if not

null.

Error:

- returns `Error` when input pointers are invalid or the order payload

cannot be decoded;

- on `Error`, if `out_error` is not null, it is filled with a

caller-owned `PitSharedString` that MUST be destroyed by the caller.

Cleanup:

- release a successful request with `pit_pretrade_pre_trade_request_execute`
  or

`pit_destroy_pretrade_pre_trade_request`.

Reject ownership contract:

- on `Rejected`, a non-null `PitRejectList` pointer is written to
  `out_rejects`

if it is not null;

- the caller takes ownership and MUST release it with

`pit_destroy_reject_list`; failing to do so leaks the heap allocation made
inside this call;

- no thread-local state is involved, and the returned pointer is safe to

read on any thread;

- on `Passed` and `Error`, null is written to `out_rejects`, and the caller

must not call destroy in those cases.

Order lifetime contract:

- `order` is read as a borrowed view during this call;
- the operation snapshots that payload before returning, because the

deferred request may outlive the source buffers.

```c
PitPretradeStatus pit_engine_start_pre_trade(
    PitEngine * engine,
    const PitOrder * order,
    PitPretradePreTradeRequest ** out_request,
    PitRejectList ** out_rejects,
    PitOutError out_error
);
```

## `pit_engine_execute_pre_trade`

Runs the complete pre-trade check in one call.

Success:

- returns `Passed` when the order passed this stage; read `out_reservation`;
- returns `Rejected` when the order was rejected is not null; read

`out_rejects`.

Error:

- returns `Error` when input pointers are invalid or the order payload

cannot be decoded;

- on `Error`, if `out_error` is not null, it is filled with a

caller-owned `PitSharedString` that MUST be destroyed by the caller.

Cleanup:

- release a successful reservation with
  `pit_pretrade_pre_trade_reservation_commit`,

`pit_pretrade_pre_trade_reservation_rollback`, or
`pit_destroy_pretrade_pre_trade_reservation`.

Reject ownership contract:

- on `Rejected`, a non-null `PitRejectList` pointer is written to

`out_rejects` if it is not null;

- the caller takes ownership and MUST release it with

`pit_destroy_reject_list`; failing to do so leaks the heap allocation made
inside this call;

- no thread-local state is involved, and the returned pointer is safe to

read on any thread;

- on `Passed` and `Error`, null is written to `out_rejects`, and the caller

must not call destroy in those cases.

Order lifetime contract:

- `order` is read as a borrowed view during this call only;
- the operation does not retain any pointer into source memory after this

function returns.

```c
PitPretradeStatus pit_engine_execute_pre_trade(
    PitEngine * engine,
    const PitOrder * order,
    PitPretradePreTradeReservation ** out_reservation,
    PitRejectList ** out_rejects,
    PitOutError out_error
);
```

## `pit_pretrade_pre_trade_request_execute`

Executes a deferred request returned by `pit_engine_start_pre_trade`.

Success:

- returns `Passed` when the order passed this stage; read `out_reservation`;
- returns `Rejected` when the order was rejected and `out_rejects` is not

null; read `out_rejects`.

Error:

- returns `Error` when input pointers are invalid or the order payload

cannot be decoded;

- on `Error`, if `out_error` is not null, it is filled with a

caller-owned `PitSharedString` that MUST be destroyed by the caller.

Ownership:

- this call consumes the request object's content exactly once;
- after a successful or failed execute, the object itself may still

be released with `pit_destroy_pretrade_pre_trade_request`, but it cannot be
executed again.

Reject ownership contract:

- on `Rejected`, a non-null `PitRejectList` pointer is written to

`out_rejects` if it is not null;

- the caller takes ownership and MUST release it with

`pit_destroy_reject_list`; failing to do so leaks the heap allocation made
inside this call;

- no thread-local state is involved, and the returned pointer is safe to

read on any thread;

- on `Passed` and `Error`, null is written to `out_rejects`, and the caller

must not call destroy in those cases.

```c
PitPretradeStatus pit_pretrade_pre_trade_request_execute(
    PitPretradePreTradeRequest * request,
    PitPretradePreTradeReservation ** out_reservation,
    PitRejectList ** out_rejects,
    PitOutError out_error
);
```

## `pit_destroy_pretrade_pre_trade_request`

Releases a deferred request pointer owned by the caller.

Contract:

- passing null is allowed;
- destroying an unexecuted request abandons it without creating a

reservation;

- this function always succeeds.

```c
void pit_destroy_pretrade_pre_trade_request(
    PitPretradePreTradeRequest * request
);
```

## `pit_pretrade_pre_trade_reservation_commit`

Finalizes a reservation and applies the reserved state permanently.

This call is idempotent at the pointer level: if the reservation was already
consumed, nothing happens. Passing null is allowed.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void pit_pretrade_pre_trade_reservation_commit(
    PitPretradePreTradeReservation * reservation
);
```

## `pit_pretrade_pre_trade_reservation_rollback`

Cancels a reservation and releases the reserved state.

This call is idempotent at the pointer level: if the reservation was already
consumed, nothing happens. Passing null is allowed.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void pit_pretrade_pre_trade_reservation_rollback(
    PitPretradePreTradeReservation * reservation
);
```

## `pit_pretrade_pre_trade_reservation_get_lock`

Returns a snapshot of the lock attached to a reservation.

Contract:

- `reservation` must be a valid non-null pointer;
- violating the pointer contract aborts the call;
- this function never fails.

Lifetime contract:

- the returned snapshot is detached from the reservation state.

```c
PitPretradePreTradeLock pit_pretrade_pre_trade_reservation_get_lock(
    const PitPretradePreTradeReservation * reservation
);
```

## `pit_destroy_pretrade_pre_trade_reservation`

Releases a reservation pointer owned by the caller.

Contract:

- passing null is allowed;
- destroying an unresolved reservation triggers rollback of any pending

mutations;

- callers that need explicit resolution should call commit or rollback

first;

- this function always succeeds.

```c
void pit_destroy_pretrade_pre_trade_reservation(
    PitPretradePreTradeReservation * reservation
);
```

## `PitEngineApplyExecutionReportResult`

Result of `pit_engine_apply_execution_report`.

```c
typedef struct PitEngineApplyExecutionReportResult {
    PitPretradePostTradeResult post_trade_result;
    bool is_error;
} PitEngineApplyExecutionReportResult;
```

## `pit_engine_apply_execution_report`

Applies an execution report to engine state.

Success:

- returns `PitEngineApplyExecutionReportResult { is_error = false, ... }`.

Error:

- returns `PitEngineApplyExecutionReportResult { is_error = true,
  post_trade_result = { kill_switch_triggered = false } }`

when input pointers are invalid or the report payload cannot be decoded;

- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`;

- when `is_error` is `true`, do not trust any other fields beyond the fact

that the call failed.

Lifetime contract:

- `report` is read as a borrowed view during this call only;
- the operation does not retain any pointer into source memory after this

function returns.

```c
PitEngineApplyExecutionReportResult pit_engine_apply_execution_report(
    PitEngine * engine,
    const PitExecutionReport * report,
    PitOutError out_error
);
```

## `pit_destroy_account_adjustment_batch_error`

Releases a batch-error object returned by account-adjustment apply.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void pit_destroy_account_adjustment_batch_error(
    PitAccountAdjustmentBatchError * batch_error
);
```

## `pit_account_adjustment_batch_error_get_failed_adjustment_index`

Returns the failing adjustment index from a batch error.

Contract:

- `batch_error` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
size_t pit_account_adjustment_batch_error_get_failed_adjustment_index(
    const PitAccountAdjustmentBatchError * batch_error
);
```

## `pit_account_adjustment_batch_error_get_rejects`

Returns a non-owning reject-list view from a batch error.

Contract:

- `batch_error` must be a valid non-null pointer;
- the returned pointer is valid while `batch_error` is alive;
- this function never fails;
- violating the pointer contract aborts the call.

```c
const PitRejectList * pit_account_adjustment_batch_error_get_rejects(
    const PitAccountAdjustmentBatchError * batch_error
);
```

## `pit_engine_apply_account_adjustment`

Applies a batch of account adjustments to one account.

Success:

- returns `PitAccountAdjustmentApplyStatus::Applied` when the batch was

accepted and applied;

- returns `PitAccountAdjustmentApplyStatus::Rejected` when the call itself

completed normally but a policy rejected the batch; read `out_reject`.

Error:

- returns `PitAccountAdjustmentApplyStatus::Error` when input pointers are

invalid or some adjustment payload cannot be decoded;

- on `Error`, if `out_error` is not null, it is filled with a

caller-owned `PitSharedString` that MUST be destroyed by the caller.

Result handling:

- `Applied` means there is no reject object to clean up;
- `Rejected` stores batch error details in `out_reject`, the caller must

release a returned object with `pit_destroy_account_adjustment_batch_error`;

- rejects returned by `pit_account_adjustment_batch_error_get_rejects`

contain string views borrowed from the batch error and must not be used
after the batch error is destroyed;

- when `Error` is returned, do not use any pointer from a previous

unrelated call as if it belonged to this failure.

Lifetime contract:

- every `adjustment` entry from the contiguous input array is read as a

borrowed view during this call only;

- release a returned batch error with

`pit_destroy_account_adjustment_batch_error`.

```c
PitAccountAdjustmentApplyStatus pit_engine_apply_account_adjustment(
    PitEngine * engine,
    PitParamAccountId account_id,
    const PitAccountAdjustment * adjustments,
    size_t adjustments_len,
    PitAccountAdjustmentBatchError ** out_reject,
    PitOutError out_error
);
```
