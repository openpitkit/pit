# Account Adjustments

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitParamAdjustmentAmount`

One amount component inside an account adjustment.

The numeric value is interpreted according to `kind`:

- `Delta` means "change current state by this signed amount";
- `Absolute` means "set current state to this signed amount".

```c
typedef struct OpenPitParamAdjustmentAmount {
    OpenPitParamPositionSize value;
    OpenPitParamAdjustmentAmountKind kind;
} OpenPitParamAdjustmentAmount;
```

## `OpenPitAccountAdjustmentBalanceOperation`

Balance-operation payload for account adjustment.

```c
typedef struct OpenPitAccountAdjustmentBalanceOperation {
    OpenPitStringView asset;
    OpenPitParamPriceOptional average_entry_price;
    OpenPitPnlStateOptional realized_pnl;
} OpenPitAccountAdjustmentBalanceOperation;
```

## `OpenPitAccountAdjustmentPositionOperation`

Position-operation payload for account adjustment.

```c
typedef struct OpenPitAccountAdjustmentPositionOperation {
    OpenPitInstrument instrument;
    OpenPitStringView collateral_asset;
    OpenPitParamPriceOptional average_entry_price;
    OpenPitParamLeverage leverage;
    OpenPitParamPositionMode mode;
} OpenPitAccountAdjustmentPositionOperation;
```

## `OpenPitAccountAdjustmentAccountPnlOperation`

Account-wide PnL adjustment payload.

```c
typedef struct OpenPitAccountAdjustmentAccountPnlOperation {
    OpenPitPnlState state;
} OpenPitAccountAdjustmentAccountPnlOperation;
```

## `OpenPitAccountAdjustmentOperationKind`

Raw selector for the meaningful account-adjustment operation payload.

Use the `OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_*` constants. Unknown values
are rejected before any operation payload is imported.

```c
typedef uint8_t OpenPitAccountAdjustmentOperationKind;
```

## `OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ABSENT`

No operation is supplied.

```c
#define OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ABSENT \
    ((OpenPitAccountAdjustmentOperationKind) 0)
```

## `OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_BALANCE`

The balance-operation payload is selected.

```c
#define OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_BALANCE \
    ((OpenPitAccountAdjustmentOperationKind) 1)
```

## `OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_POSITION`

The position-operation payload is selected.

```c
#define OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_POSITION \
    ((OpenPitAccountAdjustmentOperationKind) 2)
```

## `OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ACCOUNT_PNL`

The account-wide PnL payload is selected.

```c
#define OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ACCOUNT_PNL \
    ((OpenPitAccountAdjustmentOperationKind) 3)
```

## `OpenPitAccountAdjustmentOperation`

Account-adjustment operation as a single discriminated value.

`kind` selects which payload is meaningful; the payload not selected by `kind`
is ignored. Because a single discriminant chooses the payload, supplying both a
balance and a position operation at once is not representable.

```c
typedef struct OpenPitAccountAdjustmentOperation {
    OpenPitAccountAdjustmentOperationKind kind;
    OpenPitAccountAdjustmentBalanceOperation balance;
    OpenPitAccountAdjustmentPositionOperation position;
    OpenPitAccountAdjustmentAccountPnlOperation account_pnl;
} OpenPitAccountAdjustmentOperation;
```

## `OpenPitAccountAdjustmentAmount`

Optional amount-change group for account adjustment.

The group is absent when every field is absent.

```c
typedef struct OpenPitAccountAdjustmentAmount {
    OpenPitParamAdjustmentAmount balance;
    OpenPitParamAdjustmentAmount held;
    OpenPitParamAdjustmentAmount incoming;
} OpenPitAccountAdjustmentAmount;
```

## `OpenPitAccountAdjustmentBounds`

Optional bounds group for account adjustment.

The group is absent when every bound is absent.

```c
typedef struct OpenPitAccountAdjustmentBounds {
    OpenPitParamPositionSizeOptional balance_upper;
    OpenPitParamPositionSizeOptional balance_lower;
    OpenPitParamPositionSizeOptional held_upper;
    OpenPitParamPositionSizeOptional held_lower;
    OpenPitParamPositionSizeOptional incoming_upper;
    OpenPitParamPositionSizeOptional incoming_lower;
} OpenPitAccountAdjustmentBounds;
```

## `OpenPitAccountAdjustment`

Full caller-owned account-adjustment payload.

```c
typedef struct OpenPitAccountAdjustment {
    OpenPitAccountAdjustmentOperation operation;
    OpenPitAccountAdjustmentAmountOptional amount;
    OpenPitAccountAdjustmentBoundsOptional bounds;
    void * user_data;
} OpenPitAccountAdjustment;
```

## `OpenPitAccountAdjustmentApplyStatus`

Result of `openpit_engine_apply_account_adjustment`.

```c
typedef uint8_t OpenPitAccountAdjustmentApplyStatus;
/**
 * The call failed before the batch could be evaluated.
 */
#define OpenPitAccountAdjustmentApplyStatus_Error \
    ((OpenPitAccountAdjustmentApplyStatus) 0)
/**
 * The batch was accepted and applied.
 */
#define OpenPitAccountAdjustmentApplyStatus_Applied \
    ((OpenPitAccountAdjustmentApplyStatus) 1)
/**
 * The batch was evaluated and rejected by policy or validation logic.
 */
#define OpenPitAccountAdjustmentApplyStatus_Rejected \
    ((OpenPitAccountAdjustmentApplyStatus) 2)
```

## `OpenPitAccountAdjustmentAmountOptional`

```c
typedef struct OpenPitAccountAdjustmentAmountOptional {
    OpenPitAccountAdjustmentAmount value;
    bool is_set;
} OpenPitAccountAdjustmentAmountOptional;
```

## `OpenPitAccountAdjustmentBoundsOptional`

```c
typedef struct OpenPitAccountAdjustmentBoundsOptional {
    OpenPitAccountAdjustmentBounds value;
    bool is_set;
} OpenPitAccountAdjustmentBoundsOptional;
```

## `OpenPitPnlStateOptional`

```c
typedef struct OpenPitPnlStateOptional {
    OpenPitPnlState value;
    bool is_set;
} OpenPitPnlStateOptional;
```

## `openpit_param_adjustment_amount_to_string`

Renders an adjustment amount into a caller-owned shared string.

Returns null and writes `out_error` when the amount is not set or its numeric
value cannot be decoded.

```c
OpenPitSharedString * openpit_param_adjustment_amount_to_string(
    OpenPitParamAdjustmentAmount value,
    OpenPitOutParamError out_error
);
```
