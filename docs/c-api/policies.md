# Policies

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitPretradePreTradePolicy`

Opaque pointer for a pre-trade policy.

Contract:

- Returned by custom policy create functions.
- May be passed to `openpit_engine_builder_add_pre_trade_policy`.
- Must be released by the caller with
  `openpit_destroy_pretrade_pre_trade_policy` when no longer needed.
- A policy can implement any combination of start-stage, main-stage,
  post-trade, and account-adjustment hooks.

```c
typedef struct OpenPitPretradePreTradePolicy OpenPitPretradePreTradePolicy;
```

## `OpenPitPretradePoliciesPnlBoundsBarrier`

One broker barrier definition for
`openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`.

What it describes:

- A settlement asset and its lower/upper P&L bounds applied as a broker
  barrier across all accounts.

Contract:

- `settlement_asset` must point to a valid string for the duration of the
  call.
- The array passed to the add function may contain multiple entries.

```c
typedef struct OpenPitPretradePoliciesPnlBoundsBarrier {
    OpenPitStringView settlement_asset;
    OpenPitParamPnlOptional lower_bound;
    OpenPitParamPnlOptional upper_bound;
} OpenPitPretradePoliciesPnlBoundsBarrier;
```

## `OpenPitPretradePoliciesPnlBoundsAccountBarrier`

Per-(account, settlement-asset) P&L bounds barrier with an initial P&L seed.

What it describes:

- Refines P&L bounds for a specific account and settlement asset.
- `initial_pnl` is pre-loaded into storage at construction; accumulation
  starts from this value.
- Both the broker barrier (if any) and this account+asset barrier are
  evaluated on every check; the order passes only if neither is breached.

Passed to `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy` in
the `account` array.

```c
typedef struct OpenPitPretradePoliciesPnlBoundsAccountBarrier {
    OpenPitParamAccountId account_id;
    OpenPitStringView settlement_asset;
    OpenPitParamPnlOptional lower_bound;
    OpenPitParamPnlOptional upper_bound;
    OpenPitParamPnl initial_pnl;
} OpenPitPretradePoliciesPnlBoundsAccountBarrier;
```

## `OpenPitPretradePoliciesRateLimitBrokerBarrier`

Broker-wide rate-limit barrier for
`openpit_engine_builder_add_builtin_rate_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesRateLimitBrokerBarrier {
    size_t max_orders;
    uint64_t window_nanoseconds;
} OpenPitPretradePoliciesRateLimitBrokerBarrier;
```

## `OpenPitPretradePoliciesRateLimitAssetBarrier`

Per-settlement-asset rate-limit barrier for
`openpit_engine_builder_add_builtin_rate_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesRateLimitAssetBarrier {
    OpenPitStringView settlement_asset;
    size_t max_orders;
    uint64_t window_nanoseconds;
} OpenPitPretradePoliciesRateLimitAssetBarrier;
```

## `OpenPitPretradePoliciesRateLimitAccountBarrier`

Per-account rate-limit barrier for
`openpit_engine_builder_add_builtin_rate_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesRateLimitAccountBarrier {
    OpenPitParamAccountId account_id;
    size_t max_orders;
    uint64_t window_nanoseconds;
} OpenPitPretradePoliciesRateLimitAccountBarrier;
```

## `OpenPitPretradePoliciesRateLimitAccountAssetBarrier`

Per-(account, settlement-asset) rate-limit barrier for
`openpit_engine_builder_add_builtin_rate_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
    OpenPitParamAccountId account_id;
    OpenPitStringView settlement_asset;
    size_t max_orders;
    uint64_t window_nanoseconds;
} OpenPitPretradePoliciesRateLimitAccountAssetBarrier;
```

## `OpenPitPretradePoliciesOrderSizeLimit`

Shared order-size limits for
`openpit_engine_builder_add_builtin_order_size_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesOrderSizeLimit {
    OpenPitParamQuantity max_quantity;
    OpenPitParamVolume max_notional;
} OpenPitPretradePoliciesOrderSizeLimit;
```

## `OpenPitPretradePoliciesOrderSizeBrokerBarrier`

Broker-wide order-size barrier for
`openpit_engine_builder_add_builtin_order_size_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesOrderSizeBrokerBarrier {
    OpenPitPretradePoliciesOrderSizeLimit limit;
} OpenPitPretradePoliciesOrderSizeBrokerBarrier;
```

## `OpenPitPretradePoliciesOrderSizeAssetBarrier`

Per-settlement-asset order-size barrier for
`openpit_engine_builder_add_builtin_order_size_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesOrderSizeAssetBarrier {
    OpenPitPretradePoliciesOrderSizeLimit limit;
    OpenPitStringView settlement_asset;
} OpenPitPretradePoliciesOrderSizeAssetBarrier;
```

## `OpenPitPretradePoliciesOrderSizeAccountAssetBarrier`

Per-(account, settlement-asset) order-size barrier for
`openpit_engine_builder_add_builtin_order_size_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesOrderSizeAccountAssetBarrier {
    OpenPitPretradePoliciesOrderSizeLimit limit;
    OpenPitParamAccountId account_id;
    OpenPitStringView settlement_asset;
} OpenPitPretradePoliciesOrderSizeAccountAssetBarrier;
```

## `openpit_engine_builder_add_builtin_order_validation_policy`

Adds the built-in order-validation policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.

Success:

- returns `true`; the builder retains the policy.

Error:

- returns `false` when the builder is null or already consumed;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_engine_builder_add_builtin_order_validation_policy(
    OpenPitEngineBuilder * builder,
    OpenPitOutError out_error
);
```

## `openpit_engine_builder_add_builtin_rate_limit_policy`

Adds the built-in rate-limit policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- At least one barrier axis must be configured: `broker` non-null, `asset_len > 0`, `account_len > 0`, or `account_asset_len > 0`.
- When a length is greater than zero the corresponding pointer must point to
  that many readable entries.
- Each `settlement_asset` string view inside an array entry must be valid for
  the duration of the call.

Success:

- returns `true`; the builder retains the policy.

Error:

- returns `false` when the builder is null or already consumed, when no
  barrier axis is configured, or when argument parsing fails;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_engine_builder_add_builtin_rate_limit_policy(
    OpenPitEngineBuilder * builder,
    const OpenPitPretradePoliciesRateLimitBrokerBarrier * broker,
    const OpenPitPretradePoliciesRateLimitAssetBarrier * asset,
    size_t asset_len,
    const OpenPitPretradePoliciesRateLimitAccountBarrier * account,
    size_t account_len,
    const OpenPitPretradePoliciesRateLimitAccountAssetBarrier * account_asset,
    size_t account_asset_len,
    OpenPitOutError out_error
);
```

## `openpit_engine_builder_add_builtin_order_size_limit_policy`

Adds the built-in order-size limit policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- At least one barrier axis must be configured: `broker` non-null, `asset_len > 0`, or `account_asset_len > 0`.
- When a length is greater than zero the corresponding pointer must point to
  that many readable entries.
- Each `settlement_asset` string view inside an array entry must be valid for
  the duration of the call.
- `max_quantity` and `max_notional` inside each limit must be valid.

Success:

- returns `true`; the builder retains the policy.

Error:

- returns `false` when the builder is null or already consumed, when no
  barrier axis is configured, or when argument parsing fails;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_engine_builder_add_builtin_order_size_limit_policy(
    OpenPitEngineBuilder * builder,
    const OpenPitPretradePoliciesOrderSizeBrokerBarrier * broker,
    const OpenPitPretradePoliciesOrderSizeAssetBarrier * asset,
    size_t asset_len,
    const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier * account_asset,
    size_t account_asset_len,
    OpenPitOutError out_error
);
```

## `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`

Adds the built-in P&L bounds kill-switch policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- At least one barrier must be provided: `broker_len > 0` or `account_len > 0`.
- When a length is greater than zero the corresponding pointer must point to
  that many readable entries.
- Each `settlement_asset` string view inside an array entry must be valid for
  the duration of the call.

Success:

- returns `true`; the builder retains the policy.

Error:

- returns `false` when the builder is null or already consumed, when no
  barrier is configured, or when argument parsing fails;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
    OpenPitEngineBuilder * builder,
    const OpenPitPretradePoliciesPnlBoundsBarrier * broker,
    size_t broker_len,
    const OpenPitPretradePoliciesPnlBoundsAccountBarrier * account,
    size_t account_len,
    OpenPitOutError out_error
);
```

## `openpit_destroy_pretrade_pre_trade_policy`

Destroys the caller-owned pointer for a pre-trade policy.

Lifetime contract:

- Call this exactly once for each pointer that was returned to the caller by a
  custom policy create function.
- After this call the pointer is no longer valid.
- Passing a null pointer is allowed and has no effect.
- This function always succeeds.
- If the policy was previously added to the engine builder, the engine keeps
  its own reference and may continue using the policy.
- Destroying this caller-owned pointer does not remove the policy from the
  engine.

```c
void openpit_destroy_pretrade_pre_trade_policy(
    OpenPitPretradePreTradePolicy * policy
);
```

## `openpit_pretrade_pre_trade_policy_get_name`

Returns the stable policy name for a pre-trade policy pointer.

Contract:

- This function never fails.
- `policy` must be a valid non-null pointer.
- The returned view does not own memory.
- The view remains valid while the policy object is alive and its name is not
  changed.
- Passing an invalid pointer aborts the call.

```c
OpenPitStringView openpit_pretrade_pre_trade_policy_get_name(
    const OpenPitPretradePreTradePolicy * policy
);
```

## `openpit_engine_builder_add_pre_trade_policy`

Adds a pre-trade policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy` must be a valid non-null pre-trade policy pointer.

Success:

- returns `true` and the builder retains its own reference to the policy.

Error:

- returns `false` when the builder or policy cannot be used;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Lifetime contract:

- The engine builder retains its own reference to the policy object.
- The caller still owns the passed pointer and must release that local pointer
  separately with `openpit_destroy_pretrade_pre_trade_policy` when it is no
  longer needed.

```c
bool openpit_engine_builder_add_pre_trade_policy(
    OpenPitEngineBuilder * builder,
    OpenPitPretradePreTradePolicy * policy,
    OpenPitOutError out_error
);
```

## `OpenPitPretradeContext`

Opaque context passed to main-stage C policy callbacks.

Valid only for the duration of the callback. Cannot be constructed by caller
code.

Future extension: this type is the designated seam for engine storage-cell
access. A read accessor will be added here when the engine store is introduced.

```c
typedef struct OpenPitPretradeContext OpenPitPretradeContext;
```

## `OpenPitAccountAdjustmentContext`

Opaque context passed to account-adjustment C policy callbacks.

Valid only for the duration of the callback. Cannot be constructed by caller
code.

Future extension: this type is the designated seam for engine storage-cell
access. A read accessor will be added here when the engine store is introduced.

```c
typedef struct OpenPitAccountAdjustmentContext OpenPitAccountAdjustmentContext;
```

## `OpenPitMutations`

Opaque, non-owning pointer to the mutation collector.

Valid only during the policy callback that received it. The caller must not
store or use this pointer after the callback returns.

```c
typedef struct OpenPitMutations OpenPitMutations;
```

## `OpenPitMutationFn`

Callback invoked for either commit or rollback of a registered mutation.

```c
typedef void (*OpenPitMutationFn)(
    void * user_data
);
```

## `OpenPitMutationFreeFn`

Optional callback to release mutation user_data after execution.

Called exactly once per `openpit_mutations_push`:

- after `commit_fn` when commit runs;
- after `rollback_fn` when rollback runs;
- or on drop if neither action ran.

```c
typedef void (*OpenPitMutationFreeFn)(
    void * user_data
);
```

## `openpit_mutations_push`

Registers one commit/rollback mutation in the provided collector.

Contract:

- `mutations` must be a valid non-null callback-scoped pointer.
- `commit_fn` and `rollback_fn` must remain callable until one of them is
  executed.
- `user_data` is passed to both callbacks.
- Exactly one of `commit_fn` or `rollback_fn` runs for each successful push.
- After the executed callback returns, `free_fn` is called exactly once when
  provided.
- If neither callback runs (for example collector drop), only `free_fn` runs
  exactly once when provided.

Error:

- returns `false` when `mutations` is null or invalid;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_mutations_push(
    OpenPitMutations * mutations,
    OpenPitMutationFn commit_fn,
    OpenPitMutationFn rollback_fn,
    void * user_data,
    OpenPitMutationFreeFn free_fn,
    OpenPitOutError out_error
);
```

## `OpenPitPretradePreTradePolicyCheckPreTradeStartFn`

Callback used by a custom pre-trade policy to validate one order before a
deferred pre-trade request is created.

Contract:

- `ctx` is a read-only context valid only for the duration of the callback.
- `order` points to a read-only order view valid only for the duration of the
  callback.
- `order` is passed as a borrowed view and is not copied before the callback
  runs.
- If the callback wants to keep any data from `order`, it must copy that data
  before returning.
- Return null or an empty list to accept the order.
- Return a non-empty reject list to reject the order.
- A rejected order must set explicit `code` and `scope` values in every list
  item.
- The returned list ownership is transferred to the engine; create it with
  `openpit_pretrade_create_reject_list`.
- Every reject payload is copied into internal storage before the callback
  returns.
- `user_data` is passed through unchanged from policy creation.

```c
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyCheckPreTradeStartFn)(
    const OpenPitPretradeContext * ctx,
    const OpenPitOrder * order,
    void * user_data
);
```

## `OpenPitPretradePreTradePolicyPerformPreTradeCheckFn`

Callback used by a custom pre-trade policy to perform a main-stage check.

Contract:

- `ctx` is a read-only context valid only for the duration of the callback.
- `order` points to a read-only order view valid only for the duration of the
  callback.
- `order` is passed as a borrowed view and is not copied before the callback
  runs.
- If the callback wants to keep any data from `order`, it must copy that data
  before returning.
- `mutations` is a callback-scoped non-owning pointer that allows the callback
  to register commit/rollback mutations.
- The callback must not store or use `mutations` after return.
- Return null or an empty list to accept the order.
- Return a non-empty reject list to reject the order.
- Every returned reject must contain explicit `code` and `scope` values.
- The returned list ownership is transferred to the engine; create it with
  `openpit_pretrade_create_reject_list`.
- Every reject payload is copied into internal storage before this callback
  returns.
- `user_data` is passed through unchanged from policy creation.

```c
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyPerformPreTradeCheckFn)(
    const OpenPitPretradeContext * ctx,
    const OpenPitOrder * order,
    OpenPitMutations * mutations,
    void * user_data
);
```

## `OpenPitPretradePreTradePolicyApplyExecutionReportFn`

Callback used by a custom pre-trade policy to observe an execution report.

Contract:

- `report` points to a read-only report view valid only for the duration of
  the callback.
- `report` is passed as a borrowed view and is not copied before the callback
  runs.
- If the callback wants to keep any data from `report`, it must copy that data
  before returning.
- Return a non-null account-block list when this policy reports a kill-switch
  trigger. The returned list ownership is transferred to the engine; create it
  with `openpit_pretrade_create_account_block_list`.
- Return null to indicate no kill-switch condition.
- A null `apply_execution_report_fn` means that hook returns an empty list (no
  kill switch).
- `user_data` is passed through unchanged from policy creation.

```c
typedef OpenPitPretradeAccountBlockList *
(*OpenPitPretradePreTradePolicyApplyExecutionReportFn)(
    const OpenPitExecutionReport * report,
    void * user_data
);
```

## `OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn`

Callback used by a custom pre-trade policy to validate one account adjustment.

Contract:

- `ctx` is a read-only context valid only for the duration of the callback.
- `adjustment` points to a read-only adjustment view valid only for the
  duration of the callback.
- `adjustment` is passed as a borrowed view and is not copied before the
  callback runs.
- If the callback wants to keep any data from `adjustment`, it must copy that
  data before returning.
- `account_id` must follow the same source model as the rest of the runtime
  state (numeric-only or string-derived-only).
- `mutations` is a callback-scoped non-owning pointer that allows the callback
  to register commit/rollback mutations.
- The callback must not store or use `mutations` after return.
- Return null to accept the adjustment.
- Return a non-empty reject list to reject the adjustment.
- Returned reject list ownership is transferred to the callee.
- `user_data` is passed through unchanged from policy creation.

```c
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn)(
    const OpenPitAccountAdjustmentContext * ctx,
    OpenPitParamAccountId account_id,
    const OpenPitAccountAdjustment * adjustment,
    OpenPitMutations * mutations,
    void * user_data
);
```

## `OpenPitPretradePreTradePolicyFreeUserDataFn`

Callback invoked when the last reference to a custom pre-trade policy is
released and the policy object is about to be destroyed.

Contract:

- Called exactly once, on the thread that drops the last policy reference.
- After this callback returns, no further callbacks will be invoked for this
  policy instance.
- `user_data` is the same value that was passed at policy creation.
- The callback must release any resources associated with `user_data`.

```c
typedef void (*OpenPitPretradePreTradePolicyFreeUserDataFn)(
    void * user_data
);
```

## `openpit_create_pretrade_custom_pre_trade_policy`

Creates a custom pre-trade policy from caller-provided callbacks.

Contract:

- `name` must point to a valid, null-terminated string for the duration of the
  call.
- `check_pre_trade_start_fn`, `perform_pre_trade_check_fn`,
  `apply_execution_report_fn`, and `apply_account_adjustment_fn` may be null.
- A null `check_pre_trade_start_fn`, `perform_pre_trade_check_fn`, or
  `apply_account_adjustment_fn` means that hook accepts by default.
- A null `apply_execution_report_fn` means that hook returns an empty list (no
  kill switch).
- Non-null callbacks and `free_user_data_fn` must remain callable for as long
  as the policy may still be used by either the caller pointer or the engine.
- Custom main-stage and account-adjustment callbacks can register
  commit/rollback mutations through their `mutations` pointer.
- `free_user_data_fn` will be called exactly once, when the last reference to
  the policy is released.
- `user_data` is opaque to the SDK: the engine never inspects, dereferences,
  or frees it; it is forwarded verbatim to the registered callbacks. Lifetime,
  thread-safety, and meaning of the pointed-at state are entirely the caller's
  responsibility. Under `OpenPitSyncPolicy_Local` or
  `OpenPitSyncPolicy_Account`, the caller serialises per-handle invocation per
  the SDK threading contract; under `OpenPitSyncPolicy_Full`, the caller is
  responsible for making any state reachable through `user_data` safe under
  concurrent invocation.

Success:

- returns a new caller-owned policy object.

Error:

- returns null when `name` is invalid;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

Lifetime contract:

- The policy stores its own copy of `name`; the caller may release the input
  string after this function returns.
- The returned pointer is owned by the caller and must be released with
  `openpit_destroy_pretrade_pre_trade_policy` when no longer needed.
- If the policy is added to the engine builder, the engine keeps its own
  reference, but the caller must still release the caller-owned pointer.
- `free_user_data_fn` runs once the last reference to the policy is released;
  when the engine is the final holder, it runs as part of engine destruction.

```c
OpenPitPretradePreTradePolicy * openpit_create_pretrade_custom_pre_trade_policy(
    OpenPitStringView name,
    OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_fn,
    OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_fn,
    OpenPitPretradePreTradePolicyApplyExecutionReportFn apply_execution_report_fn,
    OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn apply_account_adjustment_fn,
    OpenPitPretradePreTradePolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    OpenPitOutError out_error
);
```
