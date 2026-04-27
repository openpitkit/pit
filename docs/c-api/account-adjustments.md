# Account Adjustments

[Back to index](index.md)

## `PitParamAdjustmentAmount`

One amount component inside an account adjustment.

The numeric value is interpreted according to `kind`:

- `Delta` means "change current state by this signed amount";
- `Absolute` means "set current state to this signed amount".

```c
typedef struct PitParamAdjustmentAmount {
    PitParamPositionSize value;
    PitParamAdjustmentAmountKind kind;
} PitParamAdjustmentAmount;
```

## `PitAccountAdjustmentBalanceOperation`

Balance-operation payload for account adjustment.

```c
typedef struct PitAccountAdjustmentBalanceOperation {
    PitStringView asset;
    PitParamPriceOptional average_entry_price;
} PitAccountAdjustmentBalanceOperation;
```

## `PitAccountAdjustmentPositionOperation`

Position-operation payload for account adjustment.

```c
typedef struct PitAccountAdjustmentPositionOperation {
    PitInstrument instrument;
    PitStringView collateral_asset;
    PitParamPriceOptional average_entry_price;
    PitParamLeverage leverage;
    PitParamPositionMode mode;
} PitAccountAdjustmentPositionOperation;
```

## `PitAccountAdjustmentBalanceOperationOptional`

```c
typedef struct PitAccountAdjustmentBalanceOperationOptional {
    PitAccountAdjustmentBalanceOperation value;
    bool is_set;
} PitAccountAdjustmentBalanceOperationOptional;
```

## `PitAccountAdjustmentPositionOperationOptional`

```c
typedef struct PitAccountAdjustmentPositionOperationOptional {
    PitAccountAdjustmentPositionOperation value;
    bool is_set;
} PitAccountAdjustmentPositionOperationOptional;
```

## `PitAccountAdjustmentAmount`

Optional amount-change group for account adjustment.

The group is absent when every field is absent.

```c
typedef struct PitAccountAdjustmentAmount {
    PitParamAdjustmentAmount total;
    PitParamAdjustmentAmount reserved;
    PitParamAdjustmentAmount pending;
} PitAccountAdjustmentAmount;
```

## `PitAccountAdjustmentBounds`

Optional bounds group for account adjustment.

The group is absent when every bound is absent.

```c
typedef struct PitAccountAdjustmentBounds {
    PitParamPositionSizeOptional total_upper;
    PitParamPositionSizeOptional total_lower;
    PitParamPositionSizeOptional reserved_upper;
    PitParamPositionSizeOptional reserved_lower;
    PitParamPositionSizeOptional pending_upper;
    PitParamPositionSizeOptional pending_lower;
} PitAccountAdjustmentBounds;
```

## `PitAccountAdjustment`

Full caller-owned account-adjustment payload.

```c
typedef struct PitAccountAdjustment {
    PitAccountAdjustmentBalanceOperationOptional balance_operation;
    PitAccountAdjustmentPositionOperationOptional position_operation;
    PitAccountAdjustmentAmountOptional amount;
    PitAccountAdjustmentBoundsOptional bounds;
    void * user_data;
} PitAccountAdjustment;
```

## `PitAccountAdjustmentApplyStatus`

Result of `pit_engine_apply_account_adjustment`.

```c
typedef uint8_t PitAccountAdjustmentApplyStatus;
/**
 * The call failed before the batch could be evaluated.
 */
#define PitAccountAdjustmentApplyStatus_Error \
    ((PitAccountAdjustmentApplyStatus) 0)
/**
 * The batch was accepted and applied.
 */
#define PitAccountAdjustmentApplyStatus_Applied \
    ((PitAccountAdjustmentApplyStatus) 1)
/**
 * The batch was evaluated and rejected by policy or validation logic.
 */
#define PitAccountAdjustmentApplyStatus_Rejected \
    ((PitAccountAdjustmentApplyStatus) 2)
```

## `PitAccountAdjustmentAmountOptional`

```c
typedef struct PitAccountAdjustmentAmountOptional {
    PitAccountAdjustmentAmount value;
    bool is_set;
} PitAccountAdjustmentAmountOptional;
```

## `PitAccountAdjustmentBoundsOptional`

```c
typedef struct PitAccountAdjustmentBoundsOptional {
    PitAccountAdjustmentBounds value;
    bool is_set;
} PitAccountAdjustmentBoundsOptional;
```
