# Policies

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitPostTradeContext`

Opaque context passed to the `apply_execution_report` C policy callback.

Valid only for the duration of the callback. Cannot be constructed by caller
code.

```c
typedef struct OpenPitPostTradeContext OpenPitPostTradeContext;
```

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
- `out_result` is a callback-scoped non-owning collector the callback may fill
  with lock prices and account adjustments via
  `openpit_pretrade_pre_trade_result_push_lock_price` and
  `openpit_pretrade_pre_trade_result_push_account_adjustment`. Neither push
  carries a `policy_group_id`; the engine assigns the policy group. The
  callback must not store or use `out_result` after return.
- The reject channel and the `out_result` channel are independent: a callback
  may both reject and fill `out_result`, but the engine only keeps
  `out_result` when the callback accepts (returns null or an empty list).
- Return null or an empty list to accept the order.
- Return a non-empty reject list to reject the order.
- Every returned reject must contain explicit `code` and `scope` values.
- The returned list ownership is transferred to the engine; create it with
  `openpit_pretrade_create_reject_list`.
- Every reject payload is copied into internal storage before this callback
  returns.
- `user_data` is passed through unchanged from policy creation.

Parameter ordering convention: read-only inputs first (`ctx`, `order`), then
callback-scoped collectors in the order (`mutations`, `out_result`), then the
trailing opaque `user_data`.

```c
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyPerformPreTradeCheckFn)(
    const OpenPitPretradeContext * ctx,
    const OpenPitOrder * order,
    OpenPitMutations * mutations,
    OpenPitPretradePreTradeResult * out_result,
    void * user_data
);
```

## `OpenPitPretradePreTradePolicyApplyExecutionReportFn`

Callback used by a custom pre-trade policy to observe an execution report.

Contract:

- `ctx` is a read-only post-trade context valid only for the duration of the
  callback. Use `openpit_post_trade_context_get_account_group` to query the
  report account's group.
- `report` points to a read-only report view valid only for the duration of
  the callback.
- `report` is passed as a borrowed view and is not copied before the callback
  runs.
- If the callback wants to keep any data from `report`, it must copy that data
  before returning.
- `out_adjustments` is a callback-scoped non-owning collector the callback may
  fill with group-tagged account-adjustment outcomes via
  `openpit_pretrade_post_trade_adjustment_list_push`. This channel IS
  group-tagged. The callback must not store or use `out_adjustments` after
  return.
- The account-block return and the `out_adjustments` channel are independent:
  a callback may report blocks, adjustments, both, or neither.
- Return a non-null account-block list when this policy reports a kill-switch
  trigger. The returned list ownership is transferred to the engine; create it
  with `openpit_pretrade_create_account_block_list`.
- Return null to indicate no kill-switch condition.
- A null `apply_execution_report_fn` means that hook returns no blocks and no
  adjustments.
- `user_data` is passed through unchanged from policy creation.

Parameter ordering convention: read-only context first (`ctx`), then read-only
input (`report`), then the callback-scoped collector (`out_adjustments`), then
the trailing opaque `user_data`.

```c
typedef OpenPitPretradeAccountBlockList *
(*OpenPitPretradePreTradePolicyApplyExecutionReportFn)(
    const OpenPitPostTradeContext * ctx,
    const OpenPitExecutionReport * report,
    OpenPitPostTradeAdjustmentList * out_adjustments,
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
- `out_outcomes` is a callback-scoped non-owning collector the callback may
  fill with account-outcome entries via
  `openpit_account_outcome_entry_list_push`. No `policy_group_id` is carried;
  the engine assigns the policy group. The callback must not store or use
  `out_outcomes` after return.
- The reject channel and the `out_outcomes` channel are independent: the
  engine only keeps `out_outcomes` when the callback accepts (returns null or
  an empty list).
- Return null to accept the adjustment.
- Return a non-empty reject list to reject the adjustment.
- Returned reject list ownership is transferred to the callee.
- `user_data` is passed through unchanged from policy creation.

Parameter ordering convention: read-only inputs first (`ctx`, `account_id`,
`adjustment`), then callback-scoped collectors in the order (`mutations`,
`out_outcomes`), then the trailing opaque `user_data`.

```c
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn)(
    const OpenPitAccountAdjustmentContext * ctx,
    OpenPitParamAccountId account_id,
    const OpenPitAccountAdjustment * adjustment,
    OpenPitMutations * mutations,
    OpenPitAccountOutcomeEntryList * out_outcomes,
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
- `policy_group_id` is the policy-group tag the engine embeds in every account
  adjustment outcome this policy produces. Use `0` for the default group.
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
  responsibility. Under `OpenPitSyncPolicy_None` or
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
    uint16_t policy_group_id,
    OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_fn,
    OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_fn,
    OpenPitPretradePreTradePolicyApplyExecutionReportFn apply_execution_report_fn,
    OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn apply_account_adjustment_fn,
    OpenPitPretradePreTradePolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    OpenPitOutError out_error
);
```

## `openpit_create_pretrade_custom_pre_trade_policy_with_dry_run`

Creates a custom pre-trade policy with explicit dry-run hooks.

This is an additive companion to
`openpit_create_pretrade_custom_pre_trade_policy`: it takes the same callbacks
plus a dry-run variant for each pre-trade stage, placed right after its normal
counterpart. The dry-run callbacks reuse the SAME function-pointer types as
their normal counterparts - `check_pre_trade_start_dry_run_fn` has the same
shape as `check_pre_trade_start_fn`, and `perform_pre_trade_check_dry_run_fn`
the same shape as `perform_pre_trade_check_fn`.

Contract:

- `name` must point to a valid, null-terminated string for the duration of the
  call.
- `policy_group_id` is the policy-group tag the engine embeds in every account
  adjustment outcome this policy produces. Use `0` for the default group.
- Every callback except `free_user_data_fn` may be null; the null behavior of
  the normal callbacks matches
  `openpit_create_pretrade_custom_pre_trade_policy`.
- A null `check_pre_trade_start_dry_run_fn` or
  `perform_pre_trade_check_dry_run_fn` leaves that dry-run hook delegating to
  its normal counterpart (`check_pre_trade_start_fn` /
  `perform_pre_trade_check_fn` respectively), exactly matching the Rust trait
  default; pass non-null to install an explicit read-only dry-run variant.
- Non-null callbacks and `free_user_data_fn` must remain callable for as long
  as the policy may still be used by either the caller pointer or the engine.
- Custom main-stage and account-adjustment callbacks can register
  commit/rollback mutations through their `mutations` pointer.
- `free_user_data_fn` will be called exactly once, when the last reference to
  the policy is released.
- `user_data` is opaque to the SDK: the engine never inspects, dereferences,
  or frees it; it is forwarded verbatim to the registered callbacks. Lifetime,
  thread-safety, and meaning of the pointed-at state are entirely the caller's
  responsibility. Under `OpenPitSyncPolicy_None` or
  `OpenPitSyncPolicy_Account`, the caller serialises per-handle invocation per
  the SDK threading contract; under `OpenPitSyncPolicy_Full`, the caller is
  responsible for making any state reachable through `user_data` safe under
  concurrent invocation.

A dry-run reports the verdict, lock, and account adjustments the order *would*
produce without moving engine state. A policy whose normal hooks mutate
immediately (for example, a rate limiter that spends budget) MUST install
read-only dry-run hooks here so a dry-run leaves engine state untouched.

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
OpenPitPretradePreTradePolicy *
openpit_create_pretrade_custom_pre_trade_policy_with_dry_run(
    OpenPitStringView name,
    uint16_t policy_group_id,
    OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_fn,
    OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_dry_run_fn,
    OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_fn,
    OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_dry_run_fn,
    OpenPitPretradePreTradePolicyApplyExecutionReportFn apply_execution_report_fn,
    OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn apply_account_adjustment_fn,
    OpenPitPretradePreTradePolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
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

## `openpit_engine_builder_add_builtin_order_size_limit_policy`

Adds the built-in order-size limit policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy_group_id` assigns the policy to a policy group (pass `0` for
  default).
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
    uint16_t policy_group_id,
    const OpenPitPretradePoliciesOrderSizeBrokerBarrier * broker,
    const OpenPitPretradePoliciesOrderSizeAssetBarrier * asset,
    size_t asset_len,
    const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier * account_asset,
    size_t account_asset_len,
    OpenPitOutError out_error
);
```

## `openpit_engine_configure_order_size_limit`

Retunes the built-in order-size limit policy registered under `name`.

This is a partial update (PATCH) at the axis level: each axis is replaced
wholesale only when its `has_*` flag is `true`, mirroring the replace-shaped
settings setters.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; it is interpreted as UTF-8. A built-in policy
  added via `openpit_engine_builder_add_builtin_order_size_limit_policy`
  registers under its fixed name `"OrderSizeLimitPolicy"`, so pass that string
  here.
- When `has_broker` is `true`, the broker barrier is set to `*broker` when
  `broker` is non-null, or cleared when `broker` is null.
- When `has_asset` is `true`, the per-asset axis is replaced by the
  `asset_len` entries at `asset`.
- When `has_account_asset` is `true`, the per-(account, asset) axis is
  replaced by the `account_asset_len` entries at `account_asset`.
- Each `settlement_asset` view and every `max_quantity`/`max_notional` must be
  valid for the duration of the call.
- A `has_*` flag set to `false` leaves that axis untouched. The policy's "at
  least one barrier" rule still applies to the resulting configuration.

Success:

- returns `true`; the new limits apply from the next order onward.

Error:

- returns `false`; if `out_error` is non-null, writes a caller-owned
  `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
- a null `engine` returns `false` and, when `out_error` is non-null, writes a
  caller-owned `OpenPitConfigureError` (`Validation`) that must be released
  with `openpit_destroy_configure_error`.

```c
bool openpit_engine_configure_order_size_limit(
    OpenPitEngine * engine,
    OpenPitStringView name,
    const OpenPitPretradePoliciesOrderSizeBrokerBarrier * broker,
    bool has_broker,
    const OpenPitPretradePoliciesOrderSizeAssetBarrier * asset,
    size_t asset_len,
    bool has_asset,
    const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier * account_asset,
    size_t account_asset_len,
    bool has_account_asset,
    OpenPitConfigureError ** out_error
);
```

## `openpit_engine_builder_add_builtin_order_validation_policy`

Adds the built-in order-validation policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy_group_id` assigns the policy to a policy group (pass `0` for
  default).

Success:

- returns `true`; the builder retains the policy.

Error:

- returns `false` when the builder is null or already consumed;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_engine_builder_add_builtin_order_validation_policy(
    OpenPitEngineBuilder * builder,
    uint16_t policy_group_id,
    OpenPitOutError out_error
);
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

## `OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate`

Runtime replacement for a per-(account, settlement-asset) P&L barrier.

Passed to `openpit_engine_configure_pnl_bounds_killswitch`. It intentionally has
no `initial_pnl`: runtime replacement preserves and evaluates the live
accumulated P&L.

```c
typedef struct OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate {
    OpenPitParamAccountId account_id;
    OpenPitStringView settlement_asset;
    OpenPitParamPnlOptional lower_bound;
    OpenPitParamPnlOptional upper_bound;
} OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate;
```

## `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`

Adds the built-in P&L bounds kill-switch policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy_group_id` assigns the policy to a policy group (pass `0` for
  default).
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
    uint16_t policy_group_id,
    const OpenPitPretradePoliciesPnlBoundsBarrier * broker,
    size_t broker_len,
    const OpenPitPretradePoliciesPnlBoundsAccountBarrier * account,
    size_t account_len,
    OpenPitOutError out_error
);
```

## `openpit_engine_configure_pnl_bounds_killswitch`

Retunes the built-in P&L bounds kill-switch policy registered under `name`.

This is a partial update (PATCH) at the axis level: each axis is replaced
wholesale only when its `has_*` flag is `true`, mirroring the replace-shaped
settings setters. Runtime account barriers use a dedicated update DTO with no
`initial_pnl`; accumulated P&L is preserved.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; it is interpreted as UTF-8. A built-in policy
  added via `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`
  registers under its fixed name `"PnlBoundsKillSwitchPolicy"`, so pass that
  string here.
- When `has_broker` is `true`, the broker axis is replaced by the `broker_len`
  entries at `broker` (a length of zero clears it, subject to the policy's "at
  least one barrier" rule).
- When `has_account` is `true`, the account+asset axis is replaced by the
  `account_len` entries at `account`.
- Each `settlement_asset` view must be valid for the duration of the call.
- A `has_*` flag set to `false` leaves that axis untouched.

Success:

- returns `true`; the new barriers apply from the next check onward.

Error:

- returns `false`; if `out_error` is non-null, writes a caller-owned
  `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
- a null `engine` returns `false` and, when `out_error` is non-null, writes a
  caller-owned `OpenPitConfigureError` (`Validation`) that must be released
  with `openpit_destroy_configure_error`.

```c
bool openpit_engine_configure_pnl_bounds_killswitch(
    OpenPitEngine * engine,
    OpenPitStringView name,
    const OpenPitPretradePoliciesPnlBoundsBarrier * broker,
    size_t broker_len,
    bool has_broker,
    const OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate * account,
    size_t account_len,
    bool has_account,
    OpenPitConfigureError ** out_error
);
```

## `openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl`

Force-sets the live accumulated P&L for a `(account_id, settlement_asset)` entry
of the P&L bounds kill-switch policy registered under `name`.

This is an absolute assignment, deliberately distinct from
`openpit_engine_configure_pnl_bounds_killswitch`: that function retunes the
bounds and never touches accumulated P&L, whereas this overwrites the live
accumulator. The entry is created if it does not exist yet. The new value is
evaluated against the live bounds from the next check onward.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; it is interpreted as UTF-8. A built-in policy
  added via `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`
  registers under its fixed name `"PnlBoundsKillSwitchPolicy"`, so pass that
  string here.
- `settlement_asset` must be valid for the duration of the call.
- `pnl` is the absolute value the entry is set to.

Success:

- returns `true`; the new accumulated P&L applies from the next check onward.

Error:

- returns `false`; if `out_error` is non-null, writes a caller-owned
  `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
- a null `engine` returns `false` and, when `out_error` is non-null, writes a
  caller-owned `OpenPitConfigureError` (`Validation`) that must be released
  with `openpit_destroy_configure_error`.

```c
bool openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(
    OpenPitEngine * engine,
    OpenPitStringView name,
    OpenPitParamAccountId account_id,
    OpenPitStringView settlement_asset,
    OpenPitParamPnl pnl,
    OpenPitConfigureError ** out_error
);
```

## `OpenPitPretradePoliciesRateLimitBrokerBarrier`

Broker-wide rate-limit barrier for
`openpit_engine_builder_add_builtin_rate_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesRateLimitBrokerBarrier {
    size_t max_orders;
    int64_t window_nanoseconds;
} OpenPitPretradePoliciesRateLimitBrokerBarrier;
```

## `OpenPitPretradePoliciesRateLimitAssetBarrier`

Per-settlement-asset rate-limit barrier for
`openpit_engine_builder_add_builtin_rate_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesRateLimitAssetBarrier {
    OpenPitStringView settlement_asset;
    size_t max_orders;
    int64_t window_nanoseconds;
} OpenPitPretradePoliciesRateLimitAssetBarrier;
```

## `OpenPitPretradePoliciesRateLimitAccountBarrier`

Per-account rate-limit barrier for
`openpit_engine_builder_add_builtin_rate_limit_policy`.

```c
typedef struct OpenPitPretradePoliciesRateLimitAccountBarrier {
    OpenPitParamAccountId account_id;
    size_t max_orders;
    int64_t window_nanoseconds;
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
    int64_t window_nanoseconds;
} OpenPitPretradePoliciesRateLimitAccountAssetBarrier;
```

## `openpit_engine_builder_add_builtin_rate_limit_policy`

Adds the built-in rate-limit policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `policy_group_id` assigns the policy to a policy group (pass `0` for
  default).
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
    uint16_t policy_group_id,
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

## `openpit_engine_configure_rate_limit`

Retunes the built-in rate-limit policy registered under `name`.

This is a partial update (PATCH): each axis is touched only when its `has_*`
flag is `true`. A touched axis is replaced wholesale — barriers can be added and
removed at runtime. A barrier key that survives the replacement keeps its live
counter (no reset). An empty axis (`len` 0 with `has_*` true) clears it, subject
to the policy's at-least-one- barrier rule. Setting `has_broker` to `true` with
a null `broker` pointer clears the broker barrier.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; it is interpreted as UTF-8. A built-in policy
  added via `openpit_engine_builder_add_builtin_rate_limit_policy` registers
  under its fixed name `"RateLimitPolicy"`, so pass that string here.
- When `has_broker` is `true` and `broker` is non-null, it must point to one
  readable entry whose `max_orders`/`window_nanoseconds` replace the broker
  barrier; a null `broker` with `has_broker` true clears it.
- When `has_asset`/`has_account`/`has_account_asset` is `true`, the matching
  pointer must point to `*_len` readable entries (a length of zero clears that
  axis). Each `settlement_asset` view must be valid for the duration of the
  call.
- A `has_*` flag set to `false` leaves that axis untouched regardless of the
  pointer/length arguments.

Success:

- returns `true`; the new limits apply from the next order onward with no
  counter reset.

Error:

- returns `false`; if `out_error` is non-null, writes a caller-owned
  `OpenPitConfigureError` (release with `openpit_destroy_configure_error`)
  describing the unknown policy, settings-type mismatch, or rejected update.
- a null `engine` returns `false` and, when `out_error` is non-null, writes a
  caller-owned `OpenPitConfigureError` (`Validation`) that must be released
  with `openpit_destroy_configure_error`.

```c
bool openpit_engine_configure_rate_limit(
    OpenPitEngine * engine,
    OpenPitStringView name,
    const OpenPitPretradePoliciesRateLimitBrokerBarrier * broker,
    bool has_broker,
    const OpenPitPretradePoliciesRateLimitAssetBarrier * asset,
    size_t asset_len,
    bool has_asset,
    const OpenPitPretradePoliciesRateLimitAccountBarrier * account,
    size_t account_len,
    bool has_account,
    const OpenPitPretradePoliciesRateLimitAccountAssetBarrier * account_asset,
    size_t account_asset_len,
    bool has_account_asset,
    OpenPitConfigureError ** out_error
);
```

## `OpenPitPretradePoliciesSpotFundsLimitMode`

Selects how the spot-funds control reacts to insufficient available funds.

The default is `Enforce`, matching the core [`SpotFundsLimitMode`] default.

```c
typedef uint8_t OpenPitPretradePoliciesSpotFundsLimitMode;
/**
 * Reject a reservation when available funds are insufficient; the reservation
 * is not recorded.
 */
#define OpenPitPretradePoliciesSpotFundsLimitMode_Enforce \
    ((OpenPitPretradePoliciesSpotFundsLimitMode) 0)
/**
 * Always record the reservation; `available` may go negative and a shortfall
 * never rejects. Arithmetic overflow is still surfaced.
 */
#define OpenPitPretradePoliciesSpotFundsLimitMode_TrackOnly \
    ((OpenPitPretradePoliciesSpotFundsLimitMode) 1)
```

## `OpenPitPretradePoliciesSpotFundsOverrideTargetTag`

Tagged target variants for a spot-funds slippage override.

Spot funds overrides use an explicit tagged hierarchy matching the Rust
[`SpotFundsOverrideTarget`](openpit::SpotFundsOverrideTarget) variants:
`Instrument`, `InstrumentAccount`, and `InstrumentAccountGroup`.

```c
typedef uint8_t OpenPitPretradePoliciesSpotFundsOverrideTargetTag;
/**
 * Instrument-level override.
 */
#define OpenPitPretradePoliciesSpotFundsOverrideTargetTag_Instrument \
    ((OpenPitPretradePoliciesSpotFundsOverrideTargetTag) 0)
/**
 * Override for one instrument and account.
 */
#define OpenPitPretradePoliciesSpotFundsOverrideTargetTag_InstrumentAccount \
    ((OpenPitPretradePoliciesSpotFundsOverrideTargetTag) 1)
/**
 * Override for one instrument and account group.
 */
#define OpenPitPretradePoliciesSpotFundsOverrideTargetTag_InstrumentAccountGroup \
    ((OpenPitPretradePoliciesSpotFundsOverrideTargetTag) 2)
```

## `OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument`

Payload for an instrument-level spot-funds override target.

```c
typedef struct OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument {
    OpenPitMarketDataInstrumentId instrument_id;
} OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument;
```

## `OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccount`

Payload for an instrument-and-account spot-funds override target.

```c
typedef struct OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccount {
    OpenPitMarketDataInstrumentId instrument_id;
    OpenPitParamAccountId account_id;
} OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccount;
```

## `OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccountGroup`

Payload for an instrument-and-account-group spot-funds override target.

```c
typedef struct
    OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccountGroup {
    OpenPitMarketDataInstrumentId instrument_id;
    OpenPitParamAccountGroupId account_group_id;
} OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccountGroup;
```

## `OpenPitPretradePoliciesSpotFundsOverrideTargetPayload`

Variant payload for a tagged spot-funds override target.

```c
typedef union OpenPitPretradePoliciesSpotFundsOverrideTargetPayload {
    OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument instrument;
    OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccount
        instrument_account;
    OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccountGroup
        instrument_account_group;
} OpenPitPretradePoliciesSpotFundsOverrideTargetPayload;
```

## `OpenPitPretradePoliciesSpotFundsOverrideTarget`

Explicit tagged target for a spot-funds slippage override.

The `tag` selects exactly one union payload. Unknown tags are rejected through
the function's existing error channel before the payload is read.

```c
typedef struct OpenPitPretradePoliciesSpotFundsOverrideTarget {
    uint8_t tag;
    OpenPitPretradePoliciesSpotFundsOverrideTargetPayload payload;
} OpenPitPretradePoliciesSpotFundsOverrideTarget;
```

## `OpenPitPretradePoliciesSpotFundsOverride`

Slippage override entry for the spot funds policy.

`target` mirrors the three variants of
[`SpotFundsOverrideTarget`](openpit::SpotFundsOverrideTarget). When
`has_slippage_bps` is `true`, `slippage_bps` is used for the selected target.
When it is `false`, construction ignores the entry and runtime configuration
clears the selected override. Slippage resolves account -> account group ->
instrument -> global for each order.

```c
typedef struct OpenPitPretradePoliciesSpotFundsOverride {
    OpenPitPretradePoliciesSpotFundsOverrideTarget target;
    uint16_t slippage_bps;
    bool has_slippage_bps;
} OpenPitPretradePoliciesSpotFundsOverride;
```

## `openpit_engine_builder_add_builtin_spot_funds_policy`

Adds the built-in spot funds policy to the engine builder.

Contract:

- `builder` must be a valid engine builder pointer.
- `market_data` is a borrowed market-data service handle or null. Null
  disables market orders entirely (limit-only mode): they are rejected with
  `UnsupportedOrderType`. A non-null handle enables market orders; the policy
  reads live quotes from the supplied market-data service.
- `market_slippage_bps` is a pointer to a `u16` or null. When `market_data` is
  non-null it MUST be non-null too (otherwise this is a configuration error
  and the call fails). The value is the worst-case global slippage in basis
  points (1 bps = 0.01%). Range validation is performed by the core engine.
- `pricing_source` selects the base price: `0` = Mark, `1` = BookTop.
- `instrument_overrides` / `overrides_len` describe a contiguous array of
  slippage overrides; pass null + 0 for none. Each entry uses an explicit
  tagged target matching `Instrument`, `InstrumentAccount`, or
  `InstrumentAccountGroup`. An unknown tag fails the call. An entry with
  `has_slippage_bps == false` is ignored. Slippage resolves account -> account
  group -> instrument -> global per order.
- `policy_group_id` tags the policy instance.

Mismatch guard: when `market_data` is non-null and the engine is multi-threaded
(`Full` or `Account` sync mode) but the market-data service was built in no-sync
(`None`, no-op locks) mode, this call fails with a descriptive error. A no-sync
engine accepts both no-sync and full-sync MD services.

Success: returns `true`; the builder retains the policy.

Error: returns `false`. If `out_error` is non-null, writes a caller-owned
`OpenPitSharedString` error handle (release with
`openpit_destroy_shared_string`).

```c
bool openpit_engine_builder_add_builtin_spot_funds_policy(
    OpenPitEngineBuilder * builder,
    const OpenPitMarketDataService * market_data,
    const uint16_t * market_slippage_bps,
    uint8_t pricing_source,
    const OpenPitPretradePoliciesSpotFundsOverride * instrument_overrides,
    size_t overrides_len,
    uint16_t policy_group_id,
    OpenPitOutError out_error
);
```

## `openpit_engine_configure_spot_funds`

Retunes the built-in spot-funds policy registered under `name`.

This is a partial update (PATCH): the global slippage, pricing source, and each
supplied override are applied only when their corresponding `has_*` flag is
`true`. The market-data service handle is fixed at build time and cannot be
changed here; this function only tunes the slippage / pricing cascade that lives
in the settings cell.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; it is interpreted as UTF-8. A built-in policy
  added via `openpit_engine_builder_add_builtin_spot_funds_policy` registers
  under its fixed name `"SpotFundsPolicy"`, so pass that string here.
- When `has_global_slippage_bps` is `true`, the global slippage is set to
  `global_slippage_bps`.
- When `has_pricing_source` is `true`, the pricing source is set from
  `pricing_source` (`0` = Mark, `1` = BookTop).
- When `has_overrides` is `true`, each of the `overrides_len` entries at
  `instrument_overrides` is applied via insert-or-clear: an entry with
  `has_slippage_bps == false` clears any override at its explicit tagged
  target. Unknown target tags fail the call.
- A `has_*` flag set to `false` leaves that dimension untouched.

Success:

- returns `true`; the new cascade applies from the next market order onward.

Error:

- returns `false`; if `out_error` is non-null, writes a caller-owned
  `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
- a null `engine` returns `false` and, when `out_error` is non-null, writes a
  caller-owned `OpenPitConfigureError` (`Validation`) that must be released
  with `openpit_destroy_configure_error`.

```c
bool openpit_engine_configure_spot_funds(
    OpenPitEngine * engine,
    OpenPitStringView name,
    uint16_t global_slippage_bps,
    bool has_global_slippage_bps,
    uint8_t pricing_source,
    bool has_pricing_source,
    const OpenPitPretradePoliciesSpotFundsOverride * instrument_overrides,
    size_t overrides_len,
    bool has_overrides,
    OpenPitConfigureError ** out_error
);
```

## `openpit_engine_configure_spot_funds_global_limit_mode`

Sets the global spot-funds limit mode for the policy registered under `name`.

The global mode applies to every order that resolves to neither a per-account
nor a per-account-group override.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; it is interpreted as UTF-8. A built-in policy
  added via `openpit_engine_builder_add_builtin_spot_funds_policy` registers
  under its fixed name `"SpotFundsPolicy"`.
- `mode` selects `Enforce` (0; reject on insufficient funds) or `TrackOnly`
  (1; always record, allow negative available).

Success:

- returns `true`; the new global mode applies from the next order onward.

Error:

- returns `false`; if `out_error` is non-null, writes a caller-owned
  `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
- a null `engine` or null / invalid-UTF-8 `name` returns `false` and, when
  `out_error` is non-null, writes a caller-owned `OpenPitConfigureError`
  (`Validation`) that must be released with `openpit_destroy_configure_error`.
- an invalid `mode` returns `false` and writes `Validation`.

```c
bool openpit_engine_configure_spot_funds_global_limit_mode(
    OpenPitEngine * engine,
    OpenPitStringView name,
    uint8_t mode,
    OpenPitConfigureError ** out_error
);
```

## `openpit_engine_configure_spot_funds_account_limit_mode`

Pins or clears the spot-funds limit mode for one account on the policy
registered under `name`.

The per-account override wins over the account-group and global tiers.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; see
  `openpit_engine_configure_spot_funds_global_limit_mode`.
- `account_id` is the account the override applies to.
- When `has_mode` is `true`, the account is pinned to `mode`. When `has_mode`
  is `false`, any existing per-account override is cleared and the cascade
  falls through to the account-group and global tiers; `mode` is ignored. When
  `has_mode` is `true`, `mode` must select `Enforce` (0) or `TrackOnly` (1).

Success / error: as `openpit_engine_configure_spot_funds_global_limit_mode`.

```c
bool openpit_engine_configure_spot_funds_account_limit_mode(
    OpenPitEngine * engine,
    OpenPitStringView name,
    OpenPitParamAccountId account_id,
    uint8_t mode,
    bool has_mode,
    OpenPitConfigureError ** out_error
);
```

## `openpit_engine_configure_spot_funds_account_group_limit_mode`

Pins or clears the spot-funds limit mode for one account group on the policy
registered under `name`.

The override applies to every account in the group that has no per-account
override.

Contract:

- `engine` must be a valid non-null engine pointer.
- `name` selects the policy; see
  `openpit_engine_configure_spot_funds_global_limit_mode`.
- `account_group_id` is the account group the override applies to; an invalid
  id fails the call with `Validation`.
- When `has_mode` is `true`, the group is pinned to `mode`. When `has_mode` is
  `false`, any existing per-account-group override is cleared and the cascade
  falls through to the global tier; `mode` is ignored. When `has_mode` is
  `true`, `mode` must select `Enforce` (0) or `TrackOnly` (1).

Success / error: as `openpit_engine_configure_spot_funds_global_limit_mode`.

```c
bool openpit_engine_configure_spot_funds_account_group_limit_mode(
    OpenPitEngine * engine,
    OpenPitStringView name,
    OpenPitParamAccountGroupId account_group_id,
    uint8_t mode,
    bool has_mode,
    OpenPitConfigureError ** out_error
);
```
