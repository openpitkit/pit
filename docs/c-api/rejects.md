# Rejects

[Back to index](index.md)

## `PitRejectScope`

Broad area to which a reject applies.

Valid values: `Order` (1), `Account` (2). Zero is not a valid scope value;
the caller must always set this field explicitly.

```c
typedef uint8_t PitRejectScope;
/**
 * The reject applies to one order or order-like request.
 */
#define PitRejectScope_Order ((PitRejectScope) 1)
/**
 * The reject applies to account state rather than to one order only.
 */
#define PitRejectScope_Account ((PitRejectScope) 2)
```

## `PitRejectCode`

Stable classification code for a reject.

Read this first when you need machine-readable handling. The textual fields
in [`PitReject`] provide operator-facing explanation and extra context.

Valid codes are `1..=39` and `255` (`Other`). Unknown incoming codes are
mapped to `Other` (`255`).

```c
typedef uint16_t PitRejectCode;
/**
 * A required field is absent.
 */
#define PitRejectCode_MissingRequiredField ((PitRejectCode) 1)
/**
 * A field cannot be parsed from the supplied wire value.
 */
#define PitRejectCode_InvalidFieldFormat ((PitRejectCode) 2)
/**
 * A field is syntactically valid but semantically unacceptable.
 */
#define PitRejectCode_InvalidFieldValue ((PitRejectCode) 3)
/**
 * The requested order type is not supported.
 */
#define PitRejectCode_UnsupportedOrderType ((PitRejectCode) 4)
/**
 * The requested time-in-force is not supported.
 */
#define PitRejectCode_UnsupportedTimeInForce ((PitRejectCode) 5)
/**
 * Another order attribute is unsupported.
 */
#define PitRejectCode_UnsupportedOrderAttribute ((PitRejectCode) 6)
/**
 * The client order identifier duplicates an active order.
 */
#define PitRejectCode_DuplicateClientOrderId ((PitRejectCode) 7)
/**
 * The order arrived after the allowed entry deadline.
 */
#define PitRejectCode_TooLateToEnter ((PitRejectCode) 8)
/**
 * Trading is closed for the relevant venue or session.
 */
#define PitRejectCode_ExchangeClosed ((PitRejectCode) 9)
/**
 * The instrument cannot be resolved.
 */
#define PitRejectCode_UnknownInstrument ((PitRejectCode) 10)
/**
 * The account cannot be resolved.
 */
#define PitRejectCode_UnknownAccount ((PitRejectCode) 11)
/**
 * The venue cannot be resolved.
 */
#define PitRejectCode_UnknownVenue ((PitRejectCode) 12)
/**
 * The clearing account cannot be resolved.
 */
#define PitRejectCode_UnknownClearingAccount ((PitRejectCode) 13)
/**
 * The collateral asset cannot be resolved.
 */
#define PitRejectCode_UnknownCollateralAsset ((PitRejectCode) 14)
/**
 * Available balance is insufficient.
 */
#define PitRejectCode_InsufficientFunds ((PitRejectCode) 15)
/**
 * Available margin is insufficient.
 */
#define PitRejectCode_InsufficientMargin ((PitRejectCode) 16)
/**
 * Available position is insufficient.
 */
#define PitRejectCode_InsufficientPosition ((PitRejectCode) 17)
/**
 * A credit limit was exceeded.
 */
#define PitRejectCode_CreditLimitExceeded ((PitRejectCode) 18)
/**
 * A risk limit was exceeded.
 */
#define PitRejectCode_RiskLimitExceeded ((PitRejectCode) 19)
/**
 * The order exceeds a generic configured limit.
 */
#define PitRejectCode_OrderExceedsLimit ((PitRejectCode) 20)
/**
 * The order quantity exceeds a configured limit.
 */
#define PitRejectCode_OrderQtyExceedsLimit ((PitRejectCode) 21)
/**
 * The order notional exceeds a configured limit.
 */
#define PitRejectCode_OrderNotionalExceedsLimit ((PitRejectCode) 22)
/**
 * The resulting position exceeds a configured limit.
 */
#define PitRejectCode_PositionLimitExceeded ((PitRejectCode) 23)
/**
 * Concentration constraints were violated.
 */
#define PitRejectCode_ConcentrationLimitExceeded ((PitRejectCode) 24)
/**
 * Leverage constraints were violated.
 */
#define PitRejectCode_LeverageLimitExceeded ((PitRejectCode) 25)
/**
 * The request rate exceeded a configured limit.
 */
#define PitRejectCode_RateLimitExceeded ((PitRejectCode) 26)
/**
 * A loss barrier has blocked further risk-taking.
 */
#define PitRejectCode_PnlKillSwitchTriggered ((PitRejectCode) 27)
/**
 * The account is blocked.
 */
#define PitRejectCode_AccountBlocked ((PitRejectCode) 28)
/**
 * The account is not authorized for this action.
 */
#define PitRejectCode_AccountNotAuthorized ((PitRejectCode) 29)
/**
 * A compliance restriction blocked the action.
 */
#define PitRejectCode_ComplianceRestriction ((PitRejectCode) 30)
/**
 * The instrument is restricted.
 */
#define PitRejectCode_InstrumentRestricted ((PitRejectCode) 31)
/**
 * A jurisdiction restriction blocked the action.
 */
#define PitRejectCode_JurisdictionRestriction ((PitRejectCode) 32)
/**
 * The action would violate wash-trade prevention.
 */
#define PitRejectCode_WashTradePrevention ((PitRejectCode) 33)
/**
 * The action would violate self-match prevention.
 */
#define PitRejectCode_SelfMatchPrevention ((PitRejectCode) 34)
/**
 * Short-sale restriction blocked the action.
 */
#define PitRejectCode_ShortSaleRestriction ((PitRejectCode) 35)
/**
 * Required risk configuration is missing.
 */
#define PitRejectCode_RiskConfigurationMissing ((PitRejectCode) 36)
/**
 * Required reference data is unavailable.
 */
#define PitRejectCode_ReferenceDataUnavailable ((PitRejectCode) 37)
/**
 * The system could not compute an order value needed for validation.
 */
#define PitRejectCode_OrderValueCalculationFailed ((PitRejectCode) 38)
/**
 * A required service or subsystem is unavailable.
 */
#define PitRejectCode_SystemUnavailable ((PitRejectCode) 39)
/**
 * Reserved discriminant for caller-defined reject classes.
 *
 * Use together with `Reject::with_user_data` to attach a caller-defined
 * payload that the receiving code can decode. The SDK does not interpret
 * this code beyond mapping it to FFI value 254.
 */
#define PitRejectCode_Custom ((PitRejectCode) 254)
/**
 * A catch-all code for rejects that do not fit a more specific class.
 */
#define PitRejectCode_Other ((PitRejectCode) 255)
```

## `PitReject`

Single rejection record returned by checks.

```c
typedef struct PitReject {
    PitStringView policy;
    PitStringView reason;
    PitStringView details;
    void * user_data;
    PitRejectCode code;
    PitRejectScope scope;
} PitReject;
```

## `PitRejectList`

Caller-owned list of rejects.

```c
typedef struct PitRejectList PitRejectList;
```

## `pit_create_reject_list`

Creates a caller-owned reject list with preallocated capacity.

`reserve` is the requested number of elements to preallocate.

Contract:

- returns a new caller-owned list;
- release it with `pit_destroy_reject_list`;
- this function always succeeds.

```c
PitRejectList * pit_create_reject_list(
    size_t reserve
);
```

## `pit_destroy_reject_list`

Releases a caller-owned reject list.

Contract:

- passing null is allowed;
- this function always succeeds.

```c
void pit_destroy_reject_list(
    PitRejectList * rejects
);
```

## `pit_reject_list_push`

Appends one reject to the list by copying its payload.

Contract:

- `list` must be a valid non-null pointer;
- string views in `reject` are copied before this function returns;
- this function never fails;
- violating the pointer contract aborts the call.

```c
void pit_reject_list_push(
    PitRejectList * list,
    PitReject reject
);
```

## `pit_reject_list_len`

Returns the number of rejects in the list.

Contract:

- `list` must be a valid non-null pointer;
- this function never fails;
- violating the pointer contract aborts the call.

```c
size_t pit_reject_list_len(
    const PitRejectList * list
);
```

## `pit_reject_list_get`

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
bool pit_reject_list_get(
    const PitRejectList * list,
    size_t index,
    PitReject * out_reject
);
```
