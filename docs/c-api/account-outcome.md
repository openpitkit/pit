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

## `OpenPitAccountOutcomeEntry`

Raw outcome data produced by a policy for one asset.

```c
typedef struct OpenPitAccountOutcomeEntry {
    OpenPitStringView asset;
    OpenPitOutcomeAmountOptional balance;
    OpenPitOutcomeAmountOptional held;
    OpenPitOutcomeAmountOptional incoming;
    OpenPitPnlOutcomeAmountOptional realized_pnl;
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

`list` and `out_outcome` must be valid non-null pointers returned by or provided
to this library and must remain alive for the duration of this call.

```c
bool openpit_account_adjustment_outcome_list_get(
    const OpenPitAccountAdjustmentOutcomeList * list,
    size_t index,
    OpenPitAccountAdjustmentOutcome * out_outcome
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

## `OpenPitAccountOutcomeEntryList`

Callback-scoped collector for account-adjustment outcome entries.

Holds the account-adjustment outcome entries a policy produces. No
`policy_group_id` is carried; the engine assigns the policy's group.

```c
typedef struct OpenPitAccountOutcomeEntryList OpenPitAccountOutcomeEntryList;
```

## `openpit_account_outcome_entry_list_push`

Appends one account-outcome entry to the account-adjustment outcome list.

### Safety

If `list` is non-null it must be a valid, properly aligned pointer to an
`OpenPitAccountOutcomeEntryList` that is exclusively accessible for the duration
of this call.

Contract:

- `list` must be a valid non-null callback-scoped pointer;
- `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
- no `policy_group_id` is accepted: the engine assigns the policy's group.

Success:

- returns `true`; the list now carries one extra entry.

Error:

- returns `false` when `list` is null or `entry` fails validation;
- if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
  error handle that MUST be released with `openpit_destroy_shared_string`.

```c
bool openpit_account_outcome_entry_list_push(
    OpenPitAccountOutcomeEntryList * list,
    OpenPitAccountOutcomeEntry entry,
    OpenPitOutError out_error
);
```

## `openpit_destroy_pretrade_pre_trade_result`

Releases a main-stage pre-trade result collector. Passing null is allowed.

### Safety

`result` must be either null or a pointer returned by this library, and must be
destroyed at most once.

```c
void openpit_destroy_pretrade_pre_trade_result(
    OpenPitPretradePreTradeResult * result
);
```

## `openpit_destroy_post_trade_adjustment_list`

Releases a post-trade adjustment list collector. Passing null is allowed.

### Safety

`list` must be either null or a pointer returned by this library, and must be
destroyed at most once.

```c
void openpit_destroy_post_trade_adjustment_list(
    OpenPitPostTradeAdjustmentList * list
);
```

## `openpit_destroy_account_outcome_entry_list`

Releases an account-outcome entry list collector. Passing null is allowed.

### Safety

`list` must be either null or a pointer returned by this library, and must be
destroyed at most once.

```c
void openpit_destroy_account_outcome_entry_list(
    OpenPitAccountOutcomeEntryList * list
);
```
