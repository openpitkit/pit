# Policies

[Back to index](index.md)

## `PitPretradeCheckPreTradeStartPolicy`

Opaque pointer for a policy that runs at the start-stage pre-trade check.

Contract:

- Returned by start-stage policy create functions.
- May be passed to

`pit_engine_builder_add_check_pre_trade_start_policy`.

- Must be released by the caller with

`pit_destroy_pretrade_check_pre_trade_start_policy` when no longer needed.

```c
typedef struct PitPretradeCheckPreTradeStartPolicy
    PitPretradeCheckPreTradeStartPolicy;
```

## `PitPretradePreTradePolicy`

Opaque pointer for a policy that runs during the main pre-trade check stage.

Contract:

- Returned by main-stage policy create functions.
- May be passed to `pit_engine_builder_add_pre_trade_policy`.
- Must be released by the caller with

`pit_destroy_pretrade_pre_trade_policy` when no longer needed.

```c
typedef struct PitPretradePreTradePolicy PitPretradePreTradePolicy;
```

## `PitAccountAdjustmentPolicy`

Opaque pointer for a policy that validates account adjustments.

Contract:

- Returned by account-adjustment policy create functions.
- May be passed to

`pit_engine_builder_add_account_adjustment_policy`.

- Must be released by the caller with

`pit_destroy_account_adjustment_policy` when no longer needed.

```c
typedef struct PitAccountAdjustmentPolicy PitAccountAdjustmentPolicy;
```

## `pit_create_pretrade_policies_order_validation_policy`

Creates a built-in start-stage policy that validates order input shape.

Why it exists:

- Use it to reject structurally invalid orders before deeper checks run.

Success:

- returns a new caller-owned pointer.
- this function always succeeds.

Lifetime contract:

- The returned pointer belongs to the caller.
- If the pointer is added to the engine builder, the engine keeps its own

reference to the same policy object.

- The caller must still release its own pointer with

`pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
longer needed locally.

```c
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_order_validation_policy(
    void
);
```

## `pit_create_pretrade_policies_rate_limit_policy`

Creates a built-in start-stage policy that limits how many orders may be
accepted within a time window.

Arguments:

- `max_orders`: maximum number of accepted orders allowed in one window.
- `window_seconds`: size of the rolling window in seconds.

Success:

- returns a new caller-owned pointer.
- this function always succeeds.

Lifetime contract:

- The returned pointer belongs to the caller.
- If the pointer is added to the engine builder, the engine keeps its own

reference to the same policy object.

- The caller must still release its own pointer with

`pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
longer needed locally.

```c
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_rate_limit_policy(
    size_t max_orders,
    uint64_t window_seconds
);
```

## `PitPretradePoliciesPnlKillSwitchParam`

One barrier definition for `pit_create_pretrade_policies_pnl_killswitch_policy`.

What it describes:

- A settlement asset and the loss threshold attached to it.

Contract:

- `settlement_asset` must point to a valid, null-terminated string for the

duration of the call.

- `barrier` must contain a valid PnL threshold value.
- The array passed to the create function may contain multiple entries.

```c
typedef struct PitPretradePoliciesPnlKillSwitchParam {
    PitStringView settlement_asset;
    PitParamPnl barrier;
} PitPretradePoliciesPnlKillSwitchParam;
```

## `pit_create_pretrade_policies_pnl_killswitch_policy`

Creates a built-in start-stage policy that rejects new orders once a loss
threshold is reached.

Why it exists:

- Use it as a kill switch per settlement asset.

Arguments:

- `params`: pointer to an array of barrier definitions.
- `params_len`: number of elements in `params`.

Contract:

- `params` must point to `params_len` readable entries.
- `params_len` must be greater than zero.
- Each `settlement_asset` pointer inside `params` must be a valid

null-terminated string for the duration of the call.

Success:

- returns a new caller-owned policy object.

Error:

- returns null when arguments are invalid or the policy cannot be created;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The returned pointer belongs to the caller.
- If the pointer is added to the engine builder, the engine keeps its own

reference to the same policy object.

- The caller must still release its own pointer with

`pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
longer needed locally.

```c
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_pnl_killswitch_policy(
    const PitPretradePoliciesPnlKillSwitchParam * params,
    size_t params_len,
    PitOutError out_error
);
```

## `PitPretradePoliciesOrderSizeLimitParam`

One limit definition for `pit_create_pretrade_policies_order_size_limit_policy`.

What it describes:

- Per-settlement maximum quantity and maximum notional allowed for one order.

Contract:

- `settlement_asset` must point to a valid, null-terminated string for the

duration of the call.

- `max_quantity` and `max_notional` must contain valid limit values.

```c
typedef struct PitPretradePoliciesOrderSizeLimitParam {
    PitStringView settlement_asset;
    PitParamQuantity max_quantity;
    PitParamVolume max_notional;
} PitPretradePoliciesOrderSizeLimitParam;
```

## `pit_create_pretrade_policies_order_size_limit_policy`

Creates a built-in start-stage policy that rejects orders above configured
size limits.

Why it exists:

- Use it to cap order quantity and notional per settlement asset.

Arguments:

- `params`: pointer to an array of size-limit definitions.
- `params_len`: number of elements in `params`.

Contract:

- `params` must point to `params_len` readable entries.
- `params_len` must be greater than zero.
- Each `settlement_asset` pointer inside `params` must be a valid

null-terminated string for the duration of the call.

Success:

- returns a new caller-owned policy object.

Error:

- returns null when arguments are invalid;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The returned pointer belongs to the caller.
- If the pointer is added to the engine builder, the engine keeps its own

reference to the same policy object.

- The caller must still release its own pointer with

`pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
longer needed locally.

```c
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_order_size_limit_policy(
    const PitPretradePoliciesOrderSizeLimitParam * params,
    size_t params_len,
    PitOutError out_error
);
```

## `pit_destroy_pretrade_check_pre_trade_start_policy`

```c
void pit_destroy_pretrade_check_pre_trade_start_policy(
    PitPretradeCheckPreTradeStartPolicy * policy
);
```

## `pit_destroy_pretrade_pre_trade_policy`

```c
void pit_destroy_pretrade_pre_trade_policy(
    PitPretradePreTradePolicy * policy
);
```

## `pit_destroy_account_adjustment_policy`

```c
void pit_destroy_account_adjustment_policy(
    PitAccountAdjustmentPolicy * policy
);
```

## `pit_pretrade_check_pre_trade_start_policy_get_name`

```c
PitStringView pit_pretrade_check_pre_trade_start_policy_get_name(
    const PitPretradeCheckPreTradeStartPolicy * policy
);
```

## `pit_pretrade_pre_trade_policy_get_name`

```c
PitStringView pit_pretrade_pre_trade_policy_get_name(
    const PitPretradePreTradePolicy * policy
);
```

## `pit_account_adjustment_policy_get_name`

```c
PitStringView pit_account_adjustment_policy_get_name(
    const PitAccountAdjustmentPolicy * policy
);
```

## `pit_engine_builder_add_check_pre_trade_start_policy`

Adds a start-stage policy to the engine builder.

Why it exists:

- Registers a policy that runs before the main pre-trade stage.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy` must be a valid non-null start-stage policy pointer.

Success:

- returns `true` and the builder retains its own reference to the policy.

Error:

- returns `false` when the builder or policy cannot be used;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The engine builder retains its own reference to the policy object.
- The caller still owns the passed pointer and must release that local pointer

separately with `pit_destroy_pretrade_check_pre_trade_start_policy` when
it is no longer needed.

```c
bool pit_engine_builder_add_check_pre_trade_start_policy(
    PitEngineBuilder * builder,
    PitPretradeCheckPreTradeStartPolicy * policy,
    PitOutError out_error
);
```

## `pit_engine_builder_add_pre_trade_policy`

Adds a main-stage pre-trade policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy` must be a valid non-null main-stage policy pointer.

Success:

- returns `true` and the builder retains its own reference to the policy.

Error:

- returns `false` when the builder or policy cannot be used;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The engine builder retains its own reference to the policy object.
- The caller still owns the passed pointer and must release that local pointer

separately with `pit_destroy_pretrade_pre_trade_policy` when it is no
longer needed.

```c
bool pit_engine_builder_add_pre_trade_policy(
    PitEngineBuilder * builder,
    PitPretradePreTradePolicy * policy,
    PitOutError out_error
);
```

## `pit_engine_builder_add_account_adjustment_policy`

Adds an account-adjustment policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy` must be a valid non-null account-adjustment policy pointer.

Success:

- returns `true` and the builder retains its own reference to the policy.

Error:

- returns `false` when the builder or policy cannot be used;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The engine builder retains its own reference to the policy object.
- The caller still owns the passed pointer and must release that local pointer

separately with `pit_destroy_account_adjustment_policy` when it
is no longer needed.

```c
bool pit_engine_builder_add_account_adjustment_policy(
    PitEngineBuilder * builder,
    PitAccountAdjustmentPolicy * policy,
    PitOutError out_error
);
```

## `PitPretradeContext`

Opaque context passed to main-stage C policy callbacks.

Valid only for the duration of the callback. Cannot be constructed by
caller code.

Future extension: this type is the designated seam for engine
storage-cell access. A read accessor will be added here when the engine
store is introduced.

```c
typedef struct PitPretradeContext PitPretradeContext;
```

## `PitAccountAdjustmentContext`

Opaque context passed to account-adjustment C policy callbacks.

Valid only for the duration of the callback. Cannot be constructed by
caller code.

Future extension: this type is the designated seam for engine
storage-cell access. A read accessor will be added here when the engine
store is introduced.

```c
typedef struct PitAccountAdjustmentContext PitAccountAdjustmentContext;
```

## `PitMutations`

Opaque, non-owning pointer to the mutation collector.

Valid only during the policy callback that received it.
The caller must not store or use this pointer after the callback returns.

```c
typedef struct PitMutations PitMutations;
```

## `PitMutationFn`

Callback invoked for either commit or rollback of a registered mutation.

```c
typedef void (*PitMutationFn)(
    void * user_data
);
```

## `PitMutationFreeFn`

Optional callback to release mutation user_data after execution.

Called exactly once per `pit_mutations_push`:

- after `commit_fn` when commit runs;
- after `rollback_fn` when rollback runs;
- or on drop if neither action ran.

```c
typedef void (*PitMutationFreeFn)(
    void * user_data
);
```

## `pit_mutations_push`

Registers one commit/rollback mutation in the provided collector.

Contract:

- `mutations` must be a valid non-null callback-scoped pointer.
- `commit_fn` and `rollback_fn` must remain callable until one of them is

executed.

- `user_data` is passed to both callbacks.
- Exactly one of `commit_fn` or `rollback_fn` runs for each successful push.
- After the executed callback returns, `free_fn` is called exactly once when

provided.

- If neither callback runs (for example collector drop), only `free_fn`

runs exactly once when provided.

Error:

- returns `false` when `mutations` is null or invalid;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

```c
bool pit_mutations_push(
    PitMutations * mutations,
    PitMutationFn commit_fn,
    PitMutationFn rollback_fn,
    void * user_data,
    PitMutationFreeFn free_fn,
    PitOutError out_error
);
```

## `PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn`

Callback used by a custom start-stage policy to validate one order.

Contract:

- `ctx` is a read-only context valid only for the duration of the callback.
- `order` points to a read-only order view valid only for the duration of

the callback.

- `order` is passed as a borrowed view and is not copied before the

callback runs.

- If the callback wants to keep any data from `order`, it must copy that

data before returning.

- Return null or an empty list to accept the order.
- Return a non-empty reject list to reject the order.
- A rejected order must set explicit `code` and `scope` values in every

list item.

- The returned list ownership is transferred to the engine; create it with

`pit_create_reject_list`.

- Every reject payload is copied into internal storage before the callback

returns.

- `user_data` is passed through unchanged from policy creation.

```c
typedef PitRejectList *
(*PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn)(
    const PitPretradeContext * ctx,
    const PitOrder * order,
    void * user_data
);
```

## `PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn`

Callback used by a custom start-stage policy to observe an execution report.

Contract:

- `report` points to a read-only report view valid only for the duration of

the callback.

- `report` is passed as a borrowed view and is not copied before the

callback runs.

- If the callback wants to keep any data from `report`, it must copy that

data before returning.

- Return `true` if the policy state changed and the engine should keep the

update.

- Return `false` when nothing changed.
- `user_data` is passed through unchanged from policy creation.

```c
typedef bool (*PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn)(
    const PitExecutionReport * report,
    void * user_data
);
```

## `PitPretradeCheckPreTradeStartPolicyFreeUserDataFn`

Callback invoked when the last reference to a custom start-stage policy is
released and the policy object is about to be destroyed.

Contract:

- Called exactly once, on the thread that drops the last policy reference.
- After this callback returns, no further callbacks will be invoked for

this policy instance.

- `user_data` is the same value that was passed at policy creation.
- The callback must release any resources associated with `user_data`.

```c
typedef void (*PitPretradeCheckPreTradeStartPolicyFreeUserDataFn)(
    void * user_data
);
```

## `PitPretradePreTradePolicyCheckFn`

Callback used by a custom main-stage policy to perform a pre-trade check.

Contract:

- `ctx` is a read-only context valid only for the duration of the callback.
- `order` points to a read-only order view valid only for the duration of

the callback.

- `order` is passed as a borrowed view and is not copied before the

callback runs.

- If the callback wants to keep any data from `order`, it must copy that

data before returning.

- `mutations` is a callback-scoped non-owning pointer that allows the

callback to register commit/rollback mutations.

- The callback must not store or use `mutations` after return.
- Return null or an empty list to accept the order.
- Return a non-empty reject list to reject the order.
- Every returned reject must contain explicit `code` and `scope` values.
- The returned list ownership is transferred to the engine; create it with

`pit_create_reject_list`.

- Every reject payload is copied into internal storage before this callback

returns.

- `user_data` is passed through unchanged from policy creation.

```c
typedef PitRejectList * (*PitPretradePreTradePolicyCheckFn)(
    const PitPretradeContext * ctx,
    const PitOrder * order,
    PitMutations * mutations,
    void * user_data
);
```

## `PitPretradePreTradePolicyApplyExecutionReportFn`

Callback used by a custom main-stage policy to observe an execution report.

Contract:

- `report` points to a read-only report view valid only for the duration of

the callback.

- `report` is passed as a borrowed view and is not copied before the

callback runs.

- If the callback wants to keep any data from `report`, it must copy that

data before returning.

- Return `true` if the policy state changed and the engine should keep the

update.

- Return `false` when nothing changed.
- `user_data` is passed through unchanged from policy creation.

```c
typedef bool (*PitPretradePreTradePolicyApplyExecutionReportFn)(
    const PitExecutionReport * report,
    void * user_data
);
```

## `PitPretradePreTradePolicyFreeUserDataFn`

Callback invoked when the last reference to a custom main-stage policy is
released and the policy object is about to be destroyed.

Contract:

- Called exactly once, on the thread that drops the last policy reference.
- After this callback returns, no further callbacks will be invoked for

this policy instance.

- `user_data` is the same value that was passed at policy creation.
- The callback must release any resources associated with `user_data`.

```c
typedef void (*PitPretradePreTradePolicyFreeUserDataFn)(
    void * user_data
);
```

## `PitAccountAdjustmentPolicyApplyFn`

Callback used by a custom account-adjustment policy to validate one
adjustment.

Contract:

- `ctx` is a read-only context valid only for the duration of the callback.
- `adjustment` points to a read-only adjustment view valid only for the

duration of the callback.

- `adjustment` is passed as a borrowed view and is not copied before the

callback runs.

- If the callback wants to keep any data from `adjustment`, it must copy

that data before returning.

- `account_id` must follow the same source model as the rest of the

runtime state (numeric-only or string-derived-only).

- `mutations` is a callback-scoped non-owning pointer that allows the

callback to register commit/rollback mutations.

- The callback must not store or use `mutations` after return.
- Return null to accept the adjustment.
- Return a non-empty reject list to reject the adjustment.
- Returned reject list ownership is transferred to the callee.
- `user_data` is passed through unchanged from policy creation.

```c
typedef PitRejectList * (*PitAccountAdjustmentPolicyApplyFn)(
    const PitAccountAdjustmentContext * ctx,
    PitParamAccountId account_id,
    const PitAccountAdjustment * adjustment,
    PitMutations * mutations,
    void * user_data
);
```

## `PitAccountAdjustmentPolicyFreeUserDataFn`

Callback invoked when the last reference to a custom account-adjustment
policy is released and the policy object is about to be destroyed.

Contract:

- Called exactly once, on the thread that drops the last policy reference.
- After this callback returns, no further callbacks will be invoked for

this policy instance.

- `user_data` is the same value that was passed at policy creation.
- The callback must release any resources associated with `user_data`.

```c
typedef void (*PitAccountAdjustmentPolicyFreeUserDataFn)(
    void * user_data
);
```

## `pit_create_pretrade_custom_check_pre_trade_start_policy`

Creates a custom start-stage policy from caller-provided callbacks.

Why it exists:

- Lets the caller implement policy logic outside the engine and plug it into

the same builder flow as built-in policies.

Contract:

- `name` must point to a valid, null-terminated string for the duration of

the call.

- `check_fn`, `apply_fn`, and `free_user_data_fn` must remain callable for

as long as the policy may still be used by either the caller pointer or
the engine.

- `free_user_data_fn` will be called exactly once, when the last reference

to the policy is released.

- `user_data` is stored as-is and passed back to every callback invocation.

Success:

- returns a new caller-owned policy object.

Error:

- returns null when `name` is invalid;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The policy stores its own copy of `name`; the caller may release the input

string after this function returns.

- The returned pointer is owned by the caller and must be released with

`pit_destroy_pretrade_check_pre_trade_start_policy` when no longer needed.

- If the policy is added to the engine builder, the engine keeps its own

reference, but the caller must still release the caller-owned pointer.

- `free_user_data_fn` runs once the last reference to the policy is

released; when the engine is the final holder, it runs as part of engine
destruction.

```c
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_custom_check_pre_trade_start_policy(
    PitStringView name,
    PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn check_fn,
    PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn apply_execution_report_fn,
    PitPretradeCheckPreTradeStartPolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    PitOutError out_error
);
```

## `pit_create_pretrade_custom_pre_trade_policy`

Creates a custom main-stage pre-trade policy from caller-provided callbacks.

Contract:

- `name` must point to a valid, null-terminated string for the duration of

the call.

- `check_fn`, `apply_fn`, and `free_user_data_fn` must

remain callable for as long as the policy may still be used by either the
caller pointer or the engine.

- Custom policy callbacks can register commit/rollback mutations through the

mutations pointer passed to `check_fn`.

- `free_user_data_fn` will be called exactly once, when the last reference

to the policy is released.

- `user_data` is stored as-is and passed back to every callback invocation.

Success:

- returns a new caller-owned policy object.

Error:

- returns null when `name` is invalid;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The policy stores its own copy of `name`; the caller may release the input

string after this function returns.

- The returned pointer is owned by the caller and must be released with

`pit_destroy_pretrade_pre_trade_policy` when no longer needed.

- If the policy is added to the engine builder, the engine keeps its own

reference, but the caller must still release the caller-owned pointer.

- `free_user_data_fn` runs once the last reference to the policy is

released; when the engine is the final holder, it runs as part of engine
destruction.

```c
PitPretradePreTradePolicy * pit_create_pretrade_custom_pre_trade_policy(
    PitStringView name,
    PitPretradePreTradePolicyCheckFn check_fn,
    PitPretradePreTradePolicyApplyExecutionReportFn apply_fn,
    PitPretradePreTradePolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    PitOutError out_error
);
```

## `pit_create_custom_account_adjustment_policy`

Creates a custom account-adjustment policy from caller-provided callbacks.

Contract:

- `name` must point to a valid, null-terminated string for the duration of

the call.

- `apply_fn` and `free_user_data_fn` must remain callable for as long as

the policy may still be used by either the caller pointer or the engine.

- Custom policy callbacks can register commit/rollback mutations through the

mutations pointer passed to `apply_fn`.

- `free_user_data_fn` will be called exactly once, when the last reference

to the policy is released.

- `user_data` is stored as-is and passed back to every callback invocation.

Success:

- returns a new caller-owned policy object.

Error:

- returns null when `name` is invalid;
- if `out_error` is not null, writes a caller-owned `PitSharedString`

error handle that MUST be released with `pit_destroy_shared_string`.

Lifetime contract:

- The policy stores its own copy of `name`; the caller may release the input

string after this function returns.

- The returned pointer is owned by the caller and must be released with

`pit_destroy_account_adjustment_policy` when no longer needed.

- If the policy is added to the engine builder, the engine keeps its own

reference, but the caller must still release the caller-owned pointer.

- `free_user_data_fn` runs once the last reference to the policy is

released; when the engine is the final holder, it runs as part of engine
destruction.

```c
PitAccountAdjustmentPolicy * pit_create_custom_account_adjustment_policy(
    PitStringView name,
    PitAccountAdjustmentPolicyApplyFn apply_fn,
    PitAccountAdjustmentPolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    PitOutError out_error
);
```
