# Engine lifecycle

The engine owns the policy instances registered by the builder. Build one engine
per independent risk-control state and choose the synchronization policy that
matches the host's call pattern.

## Build an engine

```python
import openpit
import openpit.pretrade.policies

engine = (
    openpit.Engine.builder()
    .no_sync()
    .builtin(
        openpit.pretrade.policies.build_order_validation(),
    )
    .pre_trade(policy=MyMainStagePolicy())
    .pre_trade(policy=MyAdjustmentCheck())
    .build()
)
```

Policy names must be unique within one engine configuration.

Use `full_sync()` when the same engine is called concurrently from multiple
OS threads. Use `account_sync()` only when calls are serialized on one
engine handle and each account is pinned to one processing chain.

## Run the explicit two-stage flow

```python
start_result = engine.start_pre_trade(order=order)
if not start_result:
    messages = ", ".join(
        f"{reject.policy} [{reject.code}]: {reject.reason}"
        for reject in start_result.rejects
    )
    raise RuntimeError(messages)
else:
    execute_result = start_result.request.execute()
```

Use the explicit flow when there is useful work between the lightweight start
stage and the heavier main stage.

## Run the shortcut flow

```python
execute_result = engine.execute_pre_trade(order=order)
```

The shortcut returns the same `ExecuteResult` shape as `request.execute()`. It
can contain start-stage rejects or main-stage rejects.

## Apply drop copy

`engine.apply_drop_copy(order=order)` evaluates a historical order through the
same policy pipeline without enforcing ordinary pre-trade rejects. It returns
a `DropCopyResult` in the same shape as `ExecuteResult`: on success it carries
a single-use `DropCopyOperation`, otherwise it carries fatal evaluation
rejects. The operation exposes the applied `lock`, `account_adjustments`,
`account_block`, and `is_account_blocked`, and is finalized exactly like a
reservation - `commit()` keeps the applied bookkeeping, `rollback()` reverts
it, and garbage collection rolls back an unfinalized operation.

Rollback does not reverse account-control operations or refund rate-limit
attempts. Existing account and group blocks do not prevent drop-copy
evaluation, and `DropCopyOperation.is_account_blocked` reports the apply-time
blocked-state snapshot independently from the block requested by this call.
That snapshot is captured before `apply_drop_copy` returns and does not track
later registry changes. The engine must read the order account before
evaluation; an order without a readable account returns a fatal
`MissingRequiredField` reject before any policy runs or state changes.

## Finalize reservations

```python
def send_order(order: openpit.Order) -> None:
    _ = order


if execute_result:
    reservation = execute_result.reservation
    try:
        send_order(order)
    except Exception:
        reservation.rollback()
        raise
    else:
        reservation.commit()
```

A reservation is single-use. Calling `commit()` or `rollback()` after it has
already been finalized raises `RuntimeError`.

Finalization must not fail. A mutation callable that raises is re-raised to the
caller once the rest of the batch has run, and independently arms the engine
kill switch: a mutation registered by a custom policy - every `Mutation`
registered from Python - blocks every account, so the next pre-trade call is
rejected with `SystemUnavailable` until an operator calls
`engine.accounts().unblock_all()`.

## Apply post-trade reports

```python
post_trade = engine.apply_execution_report(report=report)
if post_trade.account_blocks:
    print("halt new orders until the blocked state is cleared")
```

Reports are how stateful policies receive realized trading outcomes.
