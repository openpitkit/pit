# Account Outcome

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitOutcomeAmount`

A delta/absolute pair for one position field.

```c
typedef struct OpenPitOutcomeAmount {
    OpenPitParamPositionSize delta;
    OpenPitParamPositionSize absolute;
} OpenPitOutcomeAmount;
```

## `OpenPitOutcomeAmountOptional`

```c
typedef struct OpenPitOutcomeAmountOptional {
    OpenPitOutcomeAmount value;
    bool is_set;
} OpenPitOutcomeAmountOptional;
```

## `OpenPitPnlOutcomeAmount`

An account-currency delta/absolute pair for realized PnL.

```c
typedef struct OpenPitPnlOutcomeAmount {
    OpenPitParamPnl delta;
    OpenPitParamPnl absolute;
} OpenPitPnlOutcomeAmount;
```

## `OpenPitPnlOutcomeAmountOptional`

```c
typedef struct OpenPitPnlOutcomeAmountOptional {
    OpenPitPnlOutcomeAmount value;
    bool is_set;
} OpenPitPnlOutcomeAmountOptional;
```

## `OpenPitPnlHaltReason`

Raw reason code for a realized-PnL calculation halt.

This is a primitive rather than a Rust enum so callers can pass arbitrary bytes
without creating an invalid Rust enum discriminant at the FFI boundary. Inbound
values are validated before conversion to `OpenPitPnlHaltReason` values.

```c
typedef uint8_t OpenPitPnlHaltReason;
```

## `OPENPIT_PNL_HALT_REASON_NONE`

The realized-PnL amount is available.

```c
#define OPENPIT_PNL_HALT_REASON_NONE ((OpenPitPnlHaltReason) 0)
```

## `OPENPIT_PNL_HALT_REASON_MISSING_FX`

A required FX quote was unavailable.

```c
#define OPENPIT_PNL_HALT_REASON_MISSING_FX ((OpenPitPnlHaltReason) 1)
```

## `OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY`

The current account currency was unavailable.

```c
#define OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY \
    ((OpenPitPnlHaltReason) 2)
```

## `OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL`

The initial realized PnL needed to continue the ledger was unavailable.

```c
#define OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL ((OpenPitPnlHaltReason) 3)
```

## `OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS`

The position cost basis needed to calculate realized PnL was unavailable.

```c
#define OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS ((OpenPitPnlHaltReason) 4)
```

## `OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW`

Exact realized-PnL arithmetic overflowed.

```c
#define OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW ((OpenPitPnlHaltReason) 5)
```

## `OpenPitPnlOutcome`

Realized-PnL result: either the amount or a halt reason.

```c
typedef struct OpenPitPnlOutcome {
    OpenPitPnlHaltReason halt_reason;
    OpenPitPnlOutcomeAmountOptional amount;
} OpenPitPnlOutcome;
```

## `OpenPitPnlStateKind`

Raw discriminator for [`OpenPitPnlState`].

```c
typedef uint8_t OpenPitPnlStateKind;
```

## `OPENPIT_PNL_STATE_VALUE`

`OpenPitPnlState::value` contains the authoritative accumulated PnL.

```c
#define OPENPIT_PNL_STATE_VALUE ((OpenPitPnlStateKind) 0)
```

## `OPENPIT_PNL_STATE_HALTED`

`OpenPitPnlState::halt_reason` contains the reason calculation stopped.

```c
#define OPENPIT_PNL_STATE_HALTED ((OpenPitPnlStateKind) 1)
```

## `OpenPitPnlState`

Explicit realized-PnL accumulator state.

```c
typedef struct OpenPitPnlState {
    OpenPitPnlStateKind kind;
    OpenPitParamPnl value;
    OpenPitPnlHaltReason halt_reason;
} OpenPitPnlState;
```

## `OpenPitPnlOutcomeOptional`

```c
typedef struct OpenPitPnlOutcomeOptional {
    OpenPitPnlOutcome value;
    bool is_set;
} OpenPitPnlOutcomeOptional;
```

## `OpenPitAccountPnlOutcome`

Account-level realized-PnL result for one account.

When `halt_reason` is `OPENPIT_PNL_HALT_REASON_NONE`, `amount` is authoritative.
Otherwise `halt_reason` explains why `amount` is not authoritative; do not
interpret it as zero or read any stored PnL value as current. Position
accumulators are independent. SpotFunds emits a halted account outcome only for
the operation that transitions the accumulator to halted; later operations omit
the unchanged halt.

```c
typedef struct OpenPitAccountPnlOutcome {
    OpenPitParamAccountId account_id;
    uint16_t policy_group_id;
    OpenPitPnlHaltReason halt_reason;
    OpenPitPnlOutcomeAmountOptional amount;
} OpenPitAccountPnlOutcome;
```

## `OpenPitAccountOutcomeEntry`

Raw outcome data produced by a policy for one asset.

```c
typedef struct OpenPitAccountOutcomeEntry {
    OpenPitStringView asset;
    OpenPitOutcomeAmountOptional balance;
    OpenPitOutcomeAmountOptional held;
    OpenPitOutcomeAmountOptional incoming;
    OpenPitPnlOutcomeOptional realized_pnl;
    OpenPitParamPriceOptional average_entry_price;
} OpenPitAccountOutcomeEntry;
```

## `OpenPitAccountAdjustmentOutcome`

Account position outcome with the group tag of the business entity that produced
it.

```c
typedef struct OpenPitAccountAdjustmentOutcome {
    uint16_t policy_group_id;
    OpenPitAccountOutcomeEntry entry;
} OpenPitAccountAdjustmentOutcome;
```

## `OpenPitAccountAdjustmentOutcomeList`

Caller-owned list of account-adjustment outcomes.

```c
typedef struct OpenPitAccountAdjustmentOutcomeList
    OpenPitAccountAdjustmentOutcomeList;
```

## `openpit_destroy_account_adjustment_outcome_list`

Releases a caller-owned account-adjustment outcome list.

Contract:

- passing null is allowed;
- this function always succeeds.

### Safety

`outcomes` must be either null or a pointer returned by this library. The list
must be destroyed at most once.

```c
void openpit_destroy_account_adjustment_outcome_list(
    OpenPitAccountAdjustmentOutcomeList * outcomes
);
```

## `openpit_account_adjustment_outcome_list_len`

Returns the number of outcomes in the list.

Contract:

- `list` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

### Safety

`list` must be a valid non-null pointer returned by this library and must remain
alive for the duration of this call.

```c
size_t openpit_account_adjustment_outcome_list_len(
    const OpenPitAccountAdjustmentOutcomeList * list
);
```

## `openpit_account_adjustment_outcome_list_get`

Copies a non-owning outcome view at `index` into `out_outcome`.

The copied view borrows string memory from `list`.

Contract:

- `list` must be a valid non-null pointer;
- `out_outcome` must be a valid non-null pointer;
- returns `true` when a value exists and was copied;
- returns `false` when `index` is out of bounds and does not write
  `out_outcome`;
- the copied view remains valid while `list` is alive and unchanged;
- this function never fails;
- violating the pointer contract aborts the call.

### Safety

`list` must be returned by this library. `out_outcome` must be valid and
writable for the duration of this call.

```c
bool openpit_account_adjustment_outcome_list_get(
    const OpenPitAccountAdjustmentOutcomeList * list,
    size_t index,
    OpenPitAccountAdjustmentOutcome * out_outcome
);
```

## `OpenPitAccountPnlOutcomeList`

Borrowed list of account-level PnL outcomes owned by a post-trade result.

```c
typedef struct OpenPitAccountPnlOutcomeList OpenPitAccountPnlOutcomeList;
```

## `openpit_account_pnl_outcome_list_len`

Returns the number of account-level PnL outcomes in the list.

Contract:

- `list` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

### Safety

`list` must be borrowed from a live `OpenPitPostTradeResult` and remain alive
for the duration of this call.

```c
size_t openpit_account_pnl_outcome_list_len(
    const OpenPitAccountPnlOutcomeList * list
);
```

## `openpit_account_pnl_outcome_list_get`

Copies the self-contained account-level PnL outcome at `index` into
`out_outcome`.

Contract:

- `list` must be a valid non-null pointer;
- `out_outcome` must be a valid non-null pointer;
- returns `true` when a value exists and was copied;
- returns `false` when `index` is out of bounds and does not write
  `out_outcome`;
- the copied value remains valid independently of the owning result;
- this function never fails;
- violating the pointer contract aborts the call.

### Safety

`list` must be borrowed from a live `OpenPitPostTradeResult`. `out_outcome` must
be valid and writable for the duration of this call.

```c
bool openpit_account_pnl_outcome_list_get(
    const OpenPitAccountPnlOutcomeList * list,
    size_t index,
    OpenPitAccountPnlOutcome * out_outcome
);
```

## `OpenPitPretradePreTradeResult`

Callback-scoped collector for the per-policy main-stage pre-trade result.

Holds the two result channels a policy may produce during the main-stage check:
lock prices and account adjustments. Neither channel carries a
`policy_group_id`; the engine assigns the policy's group when assembling the
final result.

```c
typedef struct OpenPitPretradePreTradeResult OpenPitPretradePreTradeResult;
```

## `openpit_pretrade_pre_trade_result_push_lock_price`

Appends one lock price to the main-stage pre-trade result.

### Safety

If `result` is non-null it must be a valid, properly aligned pointer to an
`OpenPitPretradePreTradeResult` that is exclusively accessible for the duration
of this call.

Contract:

- `result` must be a valid non-null callback-scoped pointer;
- `price` is validated with the same domain rules as
  `openpit_create_param_price`;
- no `policy_group_id` is accepted: the engine assigns the policy's group.

Success:

- returns `true`; the result now carries one extra lock price.

Error:

- returns `false` when `result` is null or `price` fails domain validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_pre_trade_result_push_lock_price(
    OpenPitPretradePreTradeResult * result,
    OpenPitParamPrice price,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_pre_trade_result_push_account_adjustment`

Appends one account-adjustment outcome to the main-stage pre-trade result.

### Safety

If `result` is non-null it must be a valid, properly aligned pointer to an
`OpenPitPretradePreTradeResult` that is exclusively accessible for the duration
of this call.

Contract:

- `result` must be a valid non-null callback-scoped pointer;
- `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
- no `policy_group_id` is accepted: the engine assigns the policy's group.

Success:

- returns `true`; the result now carries one extra account-adjustment entry.

Error:

- returns `false` when `result` is null or `entry` fails validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_pre_trade_result_push_account_adjustment(
    OpenPitPretradePreTradeResult * result,
    OpenPitAccountOutcomeEntry entry,
    OpenPitOutError out_error
);
```

## `OpenPitPostTradeAdjustmentList`

Callback-scoped collector for post-trade account-adjustment outcomes.

Holds the group-tagged account-adjustment outcomes a policy produces after an
execution report. Each push carries the producing policy's `policy_group_id`.

```c
typedef struct OpenPitPostTradeAdjustmentList OpenPitPostTradeAdjustmentList;
```

## `openpit_pretrade_post_trade_adjustment_list_push`

Appends one group-tagged account-adjustment outcome to the post-trade list.

### Safety

If `list` is non-null it must be a valid, properly aligned pointer to an
`OpenPitPostTradeAdjustmentList` that is exclusively accessible for the duration
of this call.

Contract:

- `list` must be a valid non-null callback-scoped pointer;
- `policy_group_id` tags the produced outcome;
- `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`.

Success:

- returns `true`; the list now carries one extra outcome.

Error:

- returns `false` when `list` is null or `entry` fails validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_post_trade_adjustment_list_push(
    OpenPitPostTradeAdjustmentList * list,
    uint16_t policy_group_id,
    OpenPitAccountOutcomeEntry entry,
    OpenPitOutError out_error
);
```

## `OpenPitPostTradeAccountPnlList`

Callback-scoped collector for post-trade account-level PnL outcomes.

Holds the group-tagged account-level PnL outcomes a policy produces after an
execution report.

```c
typedef struct OpenPitPostTradeAccountPnlList OpenPitPostTradeAccountPnlList;
```

## `openpit_pretrade_post_trade_account_pnl_list_push`

Appends one group-tagged account-level PnL outcome to the post-trade list.

### Safety

If `list` is non-null it must be a valid, properly aligned pointer to an
`OpenPitPostTradeAccountPnlList` that is exclusively accessible for the duration
of this call.

Contract:

- `list` must be a valid non-null callback-scoped pointer;
- `outcome` carries the producing policy's `policy_group_id` and is fully
  validated before it is appended.

Success:

- returns `true`; the list now carries one extra outcome.

Error:

- returns `false` when `list` is null or `outcome` fails validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_post_trade_account_pnl_list_push(
    OpenPitPostTradeAccountPnlList * list,
    OpenPitAccountPnlOutcome outcome,
    OpenPitOutError out_error
);
```

## `OpenPitPretradeAccountAdjustmentResult`

Callback-scoped collector for one account-adjustment policy result.

Holds the outcome entries and account blocks produced by the callback. The
engine keeps both channels only when the callback accepts the adjustment.

```c
typedef struct OpenPitPretradeAccountAdjustmentResult
    OpenPitPretradeAccountAdjustmentResult;
```

## `openpit_pretrade_account_adjustment_result_push_account_outcome`

Appends one account-outcome entry to an account-adjustment policy result.

### Safety

If `result` is non-null it must be a valid, properly aligned pointer to an
`OpenPitPretradeAccountAdjustmentResult` that is exclusively accessible for the
duration of this call.

Contract:

- `result` must be a valid non-null callback-scoped pointer;
- `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
- no `policy_group_id` is accepted: the engine assigns the policy's group.

Success:

- returns `true`; the result now carries one extra outcome entry.

Error:

- returns `false` when `result` is null or `entry` fails validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_pretrade_account_adjustment_result_push_account_outcome(
    OpenPitPretradeAccountAdjustmentResult * result,
    OpenPitAccountOutcomeEntry entry,
    OpenPitOutError out_error
);
```

## `openpit_pretrade_account_adjustment_result_push_account_block`

Appends one account block to an account-adjustment policy result.

### Safety

If `result` is non-null it must be a valid, properly aligned pointer to an
`OpenPitPretradeAccountAdjustmentResult` that is exclusively accessible for the
duration of this call.

Contract:

- `result` must be a valid non-null callback-scoped pointer;
- string views in `block` are copied before this function returns;
- this function never fails;
- violating the pointer contract aborts the call.

```c
void openpit_pretrade_account_adjustment_result_push_account_block(
    OpenPitPretradeAccountAdjustmentResult * result,
    OpenPitPretradeAccountBlock block
);
```
