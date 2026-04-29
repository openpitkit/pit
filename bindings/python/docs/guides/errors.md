# Errors and rejects

OpenPit separates business rejects from API errors.

## Business rejects

Policy rejects are expected outcomes. They are returned in result objects:

- `StartPreTradeResult.rejects`
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
