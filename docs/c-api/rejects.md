# Rejects

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitPretradeRejectScope`

Broad area to which a reject applies.

Valid values: `Order` (1), `Account` (2). Zero is not a valid scope value; the
caller must always set this field explicitly.

```c
typedef uint8_t OpenPitPretradeRejectScope;
/**
 * The reject applies to one order or order-like request.
 */
#define OpenPitPretradeRejectScope_Order ((OpenPitPretradeRejectScope) 1)
/**
 * The reject applies to account state rather than to one order only.
 */
#define OpenPitPretradeRejectScope_Account ((OpenPitPretradeRejectScope) 2)
```

## `OpenPitPretradeRejectCode`

Stable classification code for a reject.

Read this first when you need machine-readable handling. The textual fields in
[`OpenPitPretradeReject`] provide operator-facing explanation and extra context.

Valid codes are `1..=42` and `255` (`Other`). Unknown incoming codes are mapped
to `Other` (`255`).

```c
typedef uint16_t OpenPitPretradeRejectCode;
/**
 * A required field is absent.
 */
#define OpenPitPretradeRejectCode_MissingRequiredField \
    ((OpenPitPretradeRejectCode) 1)
/**
 * A field cannot be parsed from the supplied wire value.
 */
#define OpenPitPretradeRejectCode_InvalidFieldFormat \
    ((OpenPitPretradeRejectCode) 2)
/**
 * A field is syntactically valid but semantically unacceptable.
 */
#define OpenPitPretradeRejectCode_InvalidFieldValue \
    ((OpenPitPretradeRejectCode) 3)
/**
 * The requested order type is not supported.
 */
#define OpenPitPretradeRejectCode_UnsupportedOrderType \
    ((OpenPitPretradeRejectCode) 4)
/**
 * The requested time-in-force is not supported.
 */
#define OpenPitPretradeRejectCode_UnsupportedTimeInForce \
    ((OpenPitPretradeRejectCode) 5)
/**
 * Another order attribute is unsupported.
 */
#define OpenPitPretradeRejectCode_UnsupportedOrderAttribute \
    ((OpenPitPretradeRejectCode) 6)
/**
 * The client order identifier duplicates an active order.
 */
#define OpenPitPretradeRejectCode_DuplicateClientOrderId \
    ((OpenPitPretradeRejectCode) 7)
/**
 * The order arrived after the allowed entry deadline.
 */
#define OpenPitPretradeRejectCode_TooLateToEnter ((OpenPitPretradeRejectCode) 8)
/**
 * Trading is closed for the relevant venue or session.
 */
#define OpenPitPretradeRejectCode_ExchangeClosed ((OpenPitPretradeRejectCode) 9)
/**
 * The instrument cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownInstrument \
    ((OpenPitPretradeRejectCode) 10)
/**
 * The account cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownAccount \
    ((OpenPitPretradeRejectCode) 11)
/**
 * The venue cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownVenue ((OpenPitPretradeRejectCode) 12)
/**
 * The clearing account cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownClearingAccount \
    ((OpenPitPretradeRejectCode) 13)
/**
 * The collateral asset cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownCollateralAsset \
    ((OpenPitPretradeRejectCode) 14)
/**
 * Available balance is insufficient.
 */
#define OpenPitPretradeRejectCode_InsufficientFunds \
    ((OpenPitPretradeRejectCode) 15)
/**
 * Available margin is insufficient.
 */
#define OpenPitPretradeRejectCode_InsufficientMargin \
    ((OpenPitPretradeRejectCode) 16)
/**
 * Available position is insufficient.
 */
#define OpenPitPretradeRejectCode_InsufficientPosition \
    ((OpenPitPretradeRejectCode) 17)
/**
 * A credit limit was exceeded.
 */
#define OpenPitPretradeRejectCode_CreditLimitExceeded \
    ((OpenPitPretradeRejectCode) 18)
/**
 * A risk limit was exceeded.
 */
#define OpenPitPretradeRejectCode_RiskLimitExceeded \
    ((OpenPitPretradeRejectCode) 19)
/**
 * The order exceeds a generic configured limit.
 */
#define OpenPitPretradeRejectCode_OrderExceedsLimit \
    ((OpenPitPretradeRejectCode) 20)
/**
 * The order quantity exceeds a configured limit.
 */
#define OpenPitPretradeRejectCode_OrderQtyExceedsLimit \
    ((OpenPitPretradeRejectCode) 21)
/**
 * The order notional exceeds a configured limit.
 */
#define OpenPitPretradeRejectCode_OrderNotionalExceedsLimit \
    ((OpenPitPretradeRejectCode) 22)
/**
 * The resulting position exceeds a configured limit.
 */
#define OpenPitPretradeRejectCode_PositionLimitExceeded \
    ((OpenPitPretradeRejectCode) 23)
/**
 * Concentration constraints were violated.
 */
#define OpenPitPretradeRejectCode_ConcentrationLimitExceeded \
    ((OpenPitPretradeRejectCode) 24)
/**
 * Leverage constraints were violated.
 */
#define OpenPitPretradeRejectCode_LeverageLimitExceeded \
    ((OpenPitPretradeRejectCode) 25)
/**
 * The request rate exceeded a configured limit.
 */
#define OpenPitPretradeRejectCode_RateLimitExceeded \
    ((OpenPitPretradeRejectCode) 26)
/**
 * A loss barrier has blocked further risk-taking.
 */
#define OpenPitPretradeRejectCode_PnlKillSwitchTriggered \
    ((OpenPitPretradeRejectCode) 27)
/**
 * The account is blocked.
 */
#define OpenPitPretradeRejectCode_AccountBlocked \
    ((OpenPitPretradeRejectCode) 28)
/**
 * The account is not authorized for this action.
 */
#define OpenPitPretradeRejectCode_AccountNotAuthorized \
    ((OpenPitPretradeRejectCode) 29)
/**
 * A compliance restriction blocked the action.
 */
#define OpenPitPretradeRejectCode_ComplianceRestriction \
    ((OpenPitPretradeRejectCode) 30)
/**
 * The instrument is restricted.
 */
#define OpenPitPretradeRejectCode_InstrumentRestricted \
    ((OpenPitPretradeRejectCode) 31)
/**
 * A jurisdiction restriction blocked the action.
 */
#define OpenPitPretradeRejectCode_JurisdictionRestriction \
    ((OpenPitPretradeRejectCode) 32)
/**
 * The action would violate wash-trade prevention.
 */
#define OpenPitPretradeRejectCode_WashTradePrevention \
    ((OpenPitPretradeRejectCode) 33)
/**
 * The action would violate self-match prevention.
 */
#define OpenPitPretradeRejectCode_SelfMatchPrevention \
    ((OpenPitPretradeRejectCode) 34)
/**
 * Short-sale restriction blocked the action.
 */
#define OpenPitPretradeRejectCode_ShortSaleRestriction \
    ((OpenPitPretradeRejectCode) 35)
/**
 * Required risk configuration is missing.
 */
#define OpenPitPretradeRejectCode_RiskConfigurationMissing \
    ((OpenPitPretradeRejectCode) 36)
/**
 * Required reference data is unavailable.
 */
#define OpenPitPretradeRejectCode_ReferenceDataUnavailable \
    ((OpenPitPretradeRejectCode) 37)
/**
 * The system could not compute an order value needed for validation.
 */
#define OpenPitPretradeRejectCode_OrderValueCalculationFailed \
    ((OpenPitPretradeRejectCode) 38)
/**
 * A required service or subsystem is unavailable.
 */
#define OpenPitPretradeRejectCode_SystemUnavailable \
    ((OpenPitPretradeRejectCode) 39)
/**
 * Required mark price is unavailable.
 */
#define OpenPitPretradeRejectCode_MarkPriceUnavailable \
    ((OpenPitPretradeRejectCode) 40)
/**
 * Account adjustment would violate configured bounds.
 */
#define OpenPitPretradeRejectCode_AccountAdjustmentBoundsExceeded \
    ((OpenPitPretradeRejectCode) 41)
/**
 * Underlying decimal arithmetic overflowed during evaluation.
 */
#define OpenPitPretradeRejectCode_ArithmeticOverflow \
    ((OpenPitPretradeRejectCode) 42)
/**
 * Reserved discriminant for caller-defined reject classes.
 *
 * Use together with `Reject::with_user_data` to attach a caller-defined
 * payload that the receiving code can decode. The SDK does not interpret this
 * code beyond mapping it to FFI value 254.
 */
#define OpenPitPretradeRejectCode_Custom ((OpenPitPretradeRejectCode) 254)
/**
 * A catch-all code for rejects that do not fit a more specific class.
 */
#define OpenPitPretradeRejectCode_Other ((OpenPitPretradeRejectCode) 255)
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
- this function never fails;
- violating the pointer contract aborts the call.

```c
void openpit_pretrade_reject_list_push(
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
