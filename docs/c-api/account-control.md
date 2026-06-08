# Account Control

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitAccountControl`

Opaque handle to the per-account blocking facility bound to one account.

What it is:

- A caller-owned handle that records a block against a single, already-bound
  account on the engine's shared blocked-accounts facility.

Why it exists:

- It lets a policy callback both block the bound account immediately and
  retain the ability to block it later within the same pre-trade transaction,
  for example from a deferred commit or rollback callback that has no other
  channel to surface a block.

Lifetime contract:

- Every handle returned to the caller is owned by the caller and MUST be
  released with `openpit_destroy_account_control` exactly once.
- A handle is valid to use ONLY within the pre-trade processing of the request
  it belongs to — from the callback that produced it through the commit or
  rollback of that request's reservation. Recording a block through it after
  that pre-trade transaction has completed is undefined behaviour.

```c
typedef struct OpenPitAccountControl OpenPitAccountControl;
```

## `openpit_account_control_block`

Records a block against the account bound to an account-control handle.

Records `block` against the bound account on the engine's shared
blocked-accounts facility. The first cause recorded for an account wins; later
calls for the same account are no-ops.

Contract:

- `control` must be a valid non-null account-control handle, or null.
- `block` payload fields are copied into internal storage before this call
  returns.
- Passing a null `control` records nothing and has no effect.

### Safety

`control` must be either null or a valid account-control handle provided by this
library.

```c
void openpit_account_control_block(
    const OpenPitAccountControl * control,
    OpenPitPretradeAccountBlock block
);
```

## `openpit_account_control_clone`

Returns a new handle referring to the same account-control facility.

Use this to retain the ability to block the bound account from a later callback
within the same pre-trade transaction. The returned handle records blocks
against the same account as the source handle and shares its validity window: it
is valid to use only within that pre-trade transaction, and is undefined
afterwards.

Success:

- returns a non-null caller-owned handle to the same facility.

Error:

- returns null when `control` is null.

Cleanup:

- the returned handle MUST be released with `openpit_destroy_account_control`
  exactly once.

### Safety

`control` must be either null or a valid account-control handle provided by this
library.

```c
OpenPitAccountControl * openpit_account_control_clone(
    const OpenPitAccountControl * control
);
```

## `openpit_destroy_account_control`

Releases a caller-owned account-control handle.

Lifetime contract:

- Call this exactly once for each handle that was returned to the caller.
- After this call the handle is no longer valid.
- Passing a null pointer is allowed and has no effect.
- This function always succeeds.

```c
void openpit_destroy_account_control(
    OpenPitAccountControl * control
);
```

## `openpit_pretrade_context_get_account_control`

Returns an account-control handle for a main-stage pre-trade context.

A main-stage pre-trade context carries account control only when an account
could be bound to the request.

Contract:

- `ctx` must be the callback-scoped context pointer passed to a custom
  main-stage pre-trade callback; it is valid only for the duration of that
  callback.

Success:

- returns a non-null caller-owned handle when the context carries account
  control.

Error:

- returns null when `ctx` is null or the context carries no account control
  (no account could be bound).

Cleanup:

- the returned handle MUST be released with `openpit_destroy_account_control`
  exactly once. It may be retained for deferred blocking, but it is valid to
  use only within the pre-trade transaction of this request — through the
  commit or rollback of its reservation; recording a block through it
  afterwards is undefined.

### Safety

`ctx` must be either null or a valid callback-scoped pre-trade context pointer
provided to this library.

```c
OpenPitAccountControl * openpit_pretrade_context_get_account_control(
    const OpenPitPretradeContext * ctx
);
```

## `openpit_account_adjustment_context_get_account_control`

Returns an account-control handle for an account-adjustment context.

An account-adjustment context always carries account control, so this call
returns a non-null handle for any valid context.

Contract:

- `ctx` must be the callback-scoped context pointer passed to a custom
  account-adjustment callback; it is valid only for the duration of that
  callback.

Success:

- returns a non-null caller-owned handle.

Error:

- returns null when `ctx` is null.

Cleanup:

- the returned handle MUST be released with `openpit_destroy_account_control`
  exactly once. It may be retained for deferred blocking, but it is valid to
  use only within the account adjustment processing of this request — through
  the commit or rollback of that request; recording a block through it
  afterwards is undefined.

### Safety

`ctx` must be either null or a valid callback-scoped account-adjustment context
pointer provided to this library.

```c
OpenPitAccountControl * openpit_account_adjustment_context_get_account_control(
    const OpenPitAccountAdjustmentContext * ctx
);
```

## `openpit_pretrade_context_get_account_group`

Returns the account-group for a main-stage pre-trade context.

Looks up the group registered for the bound order account. The result is cached
on first call and reused for subsequent calls within the same context lifetime.

Contract:

- `ctx` must be the callback-scoped context pointer passed to a custom
  main-stage pre-trade callback; it is valid only for the duration of that
  callback.
- `out_group` must be a valid non-null pointer.

Success:

- returns `true` and writes the group to `out_group` when the account is
  registered in a group;
- returns `false` when `ctx` is null, no account was bound to the request, or
  the account belongs to no group; `out_group` is not written to.

### Safety

`ctx` must be either null or a valid callback-scoped pre-trade context pointer
provided to this library.

```c
bool openpit_pretrade_context_get_account_group(
    const OpenPitPretradeContext * ctx,
    OpenPitParamAccountGroupId * out_group
);
```

## `openpit_account_adjustment_context_get_account_group`

Returns the account-group for an account-adjustment context.

Looks up the group registered for the adjusted account. The result is cached on
first call and reused for subsequent calls within the same context lifetime.

Contract:

- `ctx` must be the callback-scoped context pointer passed to a custom
  account-adjustment callback; it is valid only for the duration of that
  callback.
- `out_group` must be a valid non-null pointer.

Success:

- returns `true` and writes the group to `out_group` when the account is
  registered in a group;
- returns `false` when `ctx` is null or the account belongs to no group;
  `out_group` is not written to.

### Safety

`ctx` must be either null or a valid callback-scoped account-adjustment context
pointer provided to this library.

```c
bool openpit_account_adjustment_context_get_account_group(
    const OpenPitAccountAdjustmentContext * ctx,
    OpenPitParamAccountGroupId * out_group
);
```

## `openpit_post_trade_context_get_account_group`

Returns the account-group for a post-trade context.

Looks up the group registered for the report's account. The result is cached on
first call and reused for subsequent calls within the same context lifetime.

Contract:

- `ctx` must be the callback-scoped context pointer passed to a custom
  `apply_execution_report` callback; it is valid only for the duration of that
  callback.
- `out_group` must be a valid non-null pointer.

Success:

- returns `true` and writes the group to `out_group` when the account is
  registered in a group;
- returns `false` when `ctx` is null or the account belongs to no group;
  `out_group` is not written to.

### Safety

`ctx` must be either null or a valid callback-scoped post-trade context pointer
provided to this library.

```c
bool openpit_post_trade_context_get_account_group(
    const OpenPitPostTradeContext * ctx,
    OpenPitParamAccountGroupId * out_group
);
```
