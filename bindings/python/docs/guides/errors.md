# Errors and rejects

OpenPit separates business rejects from API errors.

## Business rejects

Policy rejects are expected outcomes. They are returned in result objects:

- `StartResult.rejects`
- `ExecuteResult.rejects`
- `AccountAdjustmentBatchResult.rejects`

Each reject contains a stable code, reason, details, policy name, scope, and an
optional `user_data` token.

```python
result = engine.execute_pre_trade(order=order)
if not result:
    for reject in result.rejects:
        print(reject.policy, reject.code, reject.reason)
```

## Exceptions

Exceptions are reserved for invalid API usage or unexpected callback failures.
Common cases include:

- `TypeError` for wrong wrapper types, such as raw `int` account IDs.
- `ValueError` for invalid domain values, such as empty assets.
- `RuntimeError` for lifecycle misuse, such as executing a request twice.
- `RejectError` for callback-level failures surfaced by the binding.

Do not raise exceptions for normal risk decisions in custom policies. Return
`PolicyReject` or `PolicyDecision.reject(...)` instead.

## `AccountBlockError`

Raised by the fallible admin account-blocking methods on `engine.accounts()`:
`replace_block_reason`, `block_group`, `unblock_group`, and
`replace_group_block_reason`.

`AccountBlockError` carries a human-readable error message indicating the
failure reason (e.g., "account N is not blocked", "account group N is not
blocked", "the reserved default account group is not a valid target").

```python
try:
    engine.accounts().replace_block_reason(account_id, "new reason")
except AccountBlockError as e:
    print(str(e))
```

### `AccountBlocked` reject code

When a pre-trade order is submitted for an account that is currently blocked
(directly or through its group), the engine rejects it with reject code
`RejectCode.ACCOUNT_BLOCKED`. This reject is returned through the normal
`StartResult.rejects` / `ExecuteResult.rejects` channel - it is not raised as
an exception.
