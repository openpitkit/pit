# Rejects

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitPretradeRejectScope`

Raw reject-scope code accepted from C callers.

Zero is not valid; callers must set this field explicitly.

```c
typedef uint8_t OpenPitPretradeRejectScope;
```

## `OPENPIT_PRETRADE_REJECT_SCOPE_ORDER`

The reject applies to one order or order-like request.

```c
#define OPENPIT_PRETRADE_REJECT_SCOPE_ORDER ((OpenPitPretradeRejectScope) 1)
```

## `OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT`

The reject applies to account state rather than to one order only.

```c
#define OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT ((OpenPitPretradeRejectScope) 2)
```

## `OpenPitPretradeRejectCode`

Raw stable classification code for a reject.

Read this first when you need machine-readable handling. The textual fields in
[`OpenPitPretradeReject`] provide operator-facing explanation and extra context.

Valid codes are `1..=42`, `254` (`Custom`), and `255` (`Other`). Unknown
incoming codes are mapped to `Other` (`255`).

```c
typedef uint16_t OpenPitPretradeRejectCode;
```

## `OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD`

A required field is absent.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD \
    ((OpenPitPretradeRejectCode) 1)
```

## `OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT`

A field cannot be parsed from the supplied wire value.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT \
    ((OpenPitPretradeRejectCode) 2)
```

## `OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE`

A field is syntactically valid but semantically unacceptable.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE \
    ((OpenPitPretradeRejectCode) 3)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE`

The requested order type is not supported.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE \
    ((OpenPitPretradeRejectCode) 4)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE`

The requested time-in-force is not supported.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE \
    ((OpenPitPretradeRejectCode) 5)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE`

Another order attribute is unsupported.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE \
    ((OpenPitPretradeRejectCode) 6)
```

## `OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID`

The client order identifier duplicates an active order.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID \
    ((OpenPitPretradeRejectCode) 7)
```

## `OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER`

The order arrived after the allowed entry deadline.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER \
    ((OpenPitPretradeRejectCode) 8)
```

## `OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED`

Trading is closed for the relevant venue or session.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED \
    ((OpenPitPretradeRejectCode) 9)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT`

The instrument cannot be resolved.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT \
    ((OpenPitPretradeRejectCode) 10)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT`

The account cannot be resolved.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT \
    ((OpenPitPretradeRejectCode) 11)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE`

The venue cannot be resolved.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE \
    ((OpenPitPretradeRejectCode) 12)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT`

The clearing account cannot be resolved.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT \
    ((OpenPitPretradeRejectCode) 13)
```

## `OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET`

The collateral asset cannot be resolved.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET \
    ((OpenPitPretradeRejectCode) 14)
```

## `OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS`

Available balance is insufficient.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS \
    ((OpenPitPretradeRejectCode) 15)
```

## `OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN`

Available margin is insufficient.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN \
    ((OpenPitPretradeRejectCode) 16)
```

## `OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION`

Available position is insufficient.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION \
    ((OpenPitPretradeRejectCode) 17)
```

## `OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED`

A credit limit was exceeded.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED \
    ((OpenPitPretradeRejectCode) 18)
```

## `OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED`

A risk limit was exceeded.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED \
    ((OpenPitPretradeRejectCode) 19)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT`

The order exceeds a generic configured limit.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT \
    ((OpenPitPretradeRejectCode) 20)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT`

The order quantity exceeds a configured limit.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT \
    ((OpenPitPretradeRejectCode) 21)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT`

The order notional exceeds a configured limit.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT \
    ((OpenPitPretradeRejectCode) 22)
```

## `OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED`

The resulting position exceeds a configured limit.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED \
    ((OpenPitPretradeRejectCode) 23)
```

## `OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED`

Concentration constraints were violated.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED \
    ((OpenPitPretradeRejectCode) 24)
```

## `OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED`

Leverage constraints were violated.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED \
    ((OpenPitPretradeRejectCode) 25)
```

## `OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED`

The request rate exceeded a configured limit.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED \
    ((OpenPitPretradeRejectCode) 26)
```

## `OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED`

A loss barrier has blocked further risk-taking.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED \
    ((OpenPitPretradeRejectCode) 27)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED`

The account is blocked.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED \
    ((OpenPitPretradeRejectCode) 28)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED`

The account is not authorized for this action.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED \
    ((OpenPitPretradeRejectCode) 29)
```

## `OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION`

A compliance restriction blocked the action.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION \
    ((OpenPitPretradeRejectCode) 30)
```

## `OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED`

The instrument is restricted.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED \
    ((OpenPitPretradeRejectCode) 31)
```

## `OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION`

A jurisdiction restriction blocked the action.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION \
    ((OpenPitPretradeRejectCode) 32)
```

## `OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION`

The action would violate wash-trade prevention.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION \
    ((OpenPitPretradeRejectCode) 33)
```

## `OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION`

The action would violate self-match prevention.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION \
    ((OpenPitPretradeRejectCode) 34)
```

## `OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION`

Short-sale restriction blocked the action.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION \
    ((OpenPitPretradeRejectCode) 35)
```

## `OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING`

Required risk configuration is missing.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING \
    ((OpenPitPretradeRejectCode) 36)
```

## `OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE`

Required reference data is unavailable.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE \
    ((OpenPitPretradeRejectCode) 37)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED`

The system could not compute an order value needed for validation.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED \
    ((OpenPitPretradeRejectCode) 38)
```

## `OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE`

A required service or subsystem is unavailable.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE \
    ((OpenPitPretradeRejectCode) 39)
```

## `OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE`

Required mark price is unavailable.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE \
    ((OpenPitPretradeRejectCode) 40)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED`

Account adjustment would violate configured bounds.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED \
    ((OpenPitPretradeRejectCode) 41)
```

## `OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW`

Underlying decimal arithmetic overflowed during evaluation.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW \
    ((OpenPitPretradeRejectCode) 42)
```

## `OPENPIT_PRETRADE_REJECT_CODE_CUSTOM`

Reserved code for caller-defined reject classes.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_CUSTOM ((OpenPitPretradeRejectCode) 254)
```

## `OPENPIT_PRETRADE_REJECT_CODE_OTHER`

A catch-all code for rejects that do not fit a more specific class.

```c
#define OPENPIT_PRETRADE_REJECT_CODE_OTHER ((OpenPitPretradeRejectCode) 255)
```

## `OpenPitPretradeReject`

Single rejection record returned by checks.

```c
typedef struct OpenPitPretradeReject {
    OpenPitStringView policy;
    OpenPitStringView reason;
    OpenPitStringView details;
    void * user_data;
    OpenPitPretradeRejectCode code;
    OpenPitPretradeRejectScope scope;
} OpenPitPretradeReject;
```

## `OpenPitPretradeRejectList`

Caller-owned list of rejects.

```c
typedef struct OpenPitPretradeRejectList OpenPitPretradeRejectList;
```

## `openpit_pretrade_create_reject_list`

Creates a caller-owned reject list with preallocated capacity.

`reserve` is the requested number of elements to preallocate.

Contract:

- returns a new caller-owned list;
- release it with `openpit_pretrade_destroy_reject_list`;
- this function always succeeds.

```c
OpenPitPretradeRejectList * openpit_pretrade_create_reject_list(
    size_t reserve
);
```

## `openpit_pretrade_destroy_reject_list`

Releases a caller-owned reject list.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void openpit_pretrade_destroy_reject_list(
    OpenPitPretradeRejectList * rejects
);
```

## `openpit_pretrade_reject_list_push`

Appends one reject to the list by copying its payload.

Contract:

- `list` must be a valid non-null pointer;
- string views in `reject` are copied before this function returns;
- returns `true` after appending a reject with a valid scope;
- returns `false` for an unknown scope and leaves the list unchanged;
- violating the pointer contract aborts the call.

```c
bool openpit_pretrade_reject_list_push(
    OpenPitPretradeRejectList * list,
    OpenPitPretradeReject reject
);
```

## `openpit_pretrade_reject_list_len`

Returns the number of rejects in the list.

Contract:

- `list` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
size_t openpit_pretrade_reject_list_len(
    const OpenPitPretradeRejectList * list
);
```

## `openpit_pretrade_reject_list_get`

Copies a non-owning reject view at `index` into `out_reject`.

The copied view borrows string memory from `list`.

Contract:

- `list` must be a valid non-null pointer;
- `out_reject` must be a valid non-null pointer;
- returns `true` when a value exists and was copied;
- returns `false` when `index` is out of bounds and does not write
  `out_reject`;
- the copied view remains valid while `list` is alive and unchanged;
- this function never fails;
- violating the pointer contract aborts the call.

```c
bool openpit_pretrade_reject_list_get(
    const OpenPitPretradeRejectList * list,
    size_t index,
    OpenPitPretradeReject * out_reject
);
```

## `OpenPitPretradeAccountBlock`

Single account-block record returned by kill-switch checks.

```c
typedef struct OpenPitPretradeAccountBlock {
    OpenPitStringView policy;
    OpenPitStringView reason;
    OpenPitStringView details;
    void * user_data;
    OpenPitPretradeRejectCode code;
} OpenPitPretradeAccountBlock;
```

## `OpenPitPretradeAccountBlockList`

Caller-owned list of account blocks.

```c
typedef struct OpenPitPretradeAccountBlockList OpenPitPretradeAccountBlockList;
```

## `openpit_pretrade_create_account_block_list`

Creates a caller-owned account-block list with preallocated capacity.

`reserve` is the requested number of elements to preallocate.

Contract:

- returns a new caller-owned list;
- release it with `openpit_pretrade_destroy_account_block_list`;
- this function always succeeds.

```c
OpenPitPretradeAccountBlockList * openpit_pretrade_create_account_block_list(
    size_t reserve
);
```

## `openpit_pretrade_destroy_account_block_list`

Releases a caller-owned account-block list.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void openpit_pretrade_destroy_account_block_list(
    OpenPitPretradeAccountBlockList * blocks
);
```

## `openpit_pretrade_account_block_list_push`

Appends one account block to the list by copying its payload.

Contract:

- `list` must be a valid non-null pointer;
- string views in `block` are copied before this function returns;
- this function never fails;
- violating the pointer contract aborts the call.

```c
void openpit_pretrade_account_block_list_push(
    OpenPitPretradeAccountBlockList * list,
    OpenPitPretradeAccountBlock block
);
```

## `openpit_pretrade_account_block_list_len`

Returns the number of account blocks in the list.

Contract:

- `list` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
size_t openpit_pretrade_account_block_list_len(
    const OpenPitPretradeAccountBlockList * list
);
```

## `openpit_pretrade_account_block_list_get`

Copies a non-owning account-block view at `index` into `out_block`.

The copied view borrows string memory from `list`.

Contract:

- `list` must be a valid non-null pointer;
- `out_block` must be a valid non-null pointer;
- returns `true` when a value exists and was copied;
- returns `false` when `index` is out of bounds and does not write
  `out_block`;
- the copied view remains valid while `list` is alive and unchanged;
- this function never fails;
- violating the pointer contract aborts the call.

```c
bool openpit_pretrade_account_block_list_get(
    const OpenPitPretradeAccountBlockList * list,
    size_t index,
    OpenPitPretradeAccountBlock * out_block
);
```
