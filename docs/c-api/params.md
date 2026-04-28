# Parameter Types

[Back to index](index.md)

## `PitParamLeverage`

Leverage multiplier for FFI payloads.

Uses fixed-point scale `10` in integer units:

- `10` means `1.0x`
- `11` means `1.1x`
- `1005` means `100.5x`

Valid range: `10..=30000`.

A value of `PIT_PARAM_LEVERAGE_NOT_SET` (`0`) means leverage is not
specified.

```c
typedef uint16_t PitParamLeverage;
```

## `PIT_PARAM_LEVERAGE_NOT_SET`

Sentinel value indicating leverage is not set.

```c
#define PIT_PARAM_LEVERAGE_NOT_SET ((PitParamLeverage) 0)
```

## `PIT_PARAM_LEVERAGE_SCALE`

Fixed-point scale used by leverage payloads.

```c
#define PIT_PARAM_LEVERAGE_SCALE ((PitParamLeverage) 10)
```

## `PIT_PARAM_LEVERAGE_MIN`

Minimum leverage in whole units.

```c
#define PIT_PARAM_LEVERAGE_MIN ((PitParamLeverage) 1)
```

## `PIT_PARAM_LEVERAGE_MAX`

Maximum leverage in whole units.

```c
#define PIT_PARAM_LEVERAGE_MAX ((PitParamLeverage) 3000)
```

## `PIT_PARAM_LEVERAGE_STEP`

Supported leverage increment step.

```c
#define PIT_PARAM_LEVERAGE_STEP ((float) 0.1)
```

## `PitParamAccountId`

Stable account identifier type for FFI payloads.

WARNING:
Use exactly one account-id source model per runtime:

- either purely numeric IDs (`pit_create_param_account_id_from_u64`),
- or purely string-derived IDs (`pit_create_param_account_id_from_str`).

Do not mix both models in the same runtime state. A hashed string value can
coincide with a direct numeric ID, and then two distinct accounts become one
logical key in maps and engine state.

```c
typedef uint64_t PitParamAccountId;
```

## `PitParamSide`

Order side.

```c
typedef uint8_t PitParamSide;
/**
 * Value is absent.
 */
#define PitParamSide_NotSet ((PitParamSide) 0)
/**
 * Buy side.
 */
#define PitParamSide_Buy ((PitParamSide) 1)
/**
 * Sell side.
 */
#define PitParamSide_Sell ((PitParamSide) 2)
```

## `PitParamPositionSide`

Position direction.

```c
typedef uint8_t PitParamPositionSide;
/**
 * Value is absent.
 */
#define PitParamPositionSide_NotSet ((PitParamPositionSide) 0)
/**
 * Long exposure.
 */
#define PitParamPositionSide_Long ((PitParamPositionSide) 1)
/**
 * Short exposure.
 */
#define PitParamPositionSide_Short ((PitParamPositionSide) 2)
```

## `PitParamPositionMode`

Position accounting mode.

```c
typedef uint8_t PitParamPositionMode;
/**
 * Value is absent.
 */
#define PitParamPositionMode_NotSet ((PitParamPositionMode) 0)
/**
 * Opposite trades net into one position.
 */
#define PitParamPositionMode_Netting ((PitParamPositionMode) 1)
/**
 * Long and short positions are tracked separately.
 */
#define PitParamPositionMode_Hedged ((PitParamPositionMode) 2)
```

## `PitParamPositionEffect`

Whether a trade opens or closes exposure.

```c
typedef uint8_t PitParamPositionEffect;
/**
 * Value is absent.
 */
#define PitParamPositionEffect_NotSet ((PitParamPositionEffect) 0)
/**
 * The trade opens or increases exposure.
 */
#define PitParamPositionEffect_Open ((PitParamPositionEffect) 1)
/**
 * The trade closes or reduces exposure.
 */
#define PitParamPositionEffect_Close ((PitParamPositionEffect) 2)
```

## `PitParamTradeAmountKind`

Selects how one trade-amount numeric value should be interpreted.

```c
typedef uint8_t PitParamTradeAmountKind;
/**
 * No amount field is selected.
 */
#define PitParamTradeAmountKind_NotSet ((PitParamTradeAmountKind) 0)
/**
 * The value is instrument quantity.
 */
#define PitParamTradeAmountKind_Quantity ((PitParamTradeAmountKind) 1)
/**
 * The value is settlement volume.
 */
#define PitParamTradeAmountKind_Volume ((PitParamTradeAmountKind) 2)
```

## `PitParamRoundingStrategy`

Decimal rounding strategy for typed parameter constructors.

```c
typedef uint8_t PitParamRoundingStrategy;
/**
 * Round half to nearest even number.
 */
#define PitParamRoundingStrategy_MidpointNearestEven \
    ((PitParamRoundingStrategy) 0)
/**
 * Round half away from zero.
 */
#define PitParamRoundingStrategy_MidpointAwayFromZero \
    ((PitParamRoundingStrategy) 1)
/**
 * Round towards positive infinity.
 */
#define PitParamRoundingStrategy_Up ((PitParamRoundingStrategy) 2)
/**
 * Round towards negative infinity.
 */
#define PitParamRoundingStrategy_Down ((PitParamRoundingStrategy) 3)
```

## `PIT_PARAM_ROUNDING_STRATEGY_DEFAULT`

Default rounding strategy alias.

```c
#define PIT_PARAM_ROUNDING_STRATEGY_DEFAULT \
    ((PitParamRoundingStrategy) export_rounding_strategy_const(RoundingStrategy_DEFAULT))
```

## `PIT_PARAM_ROUNDING_STRATEGY_BANKER`

Banker's rounding alias.

```c
#define PIT_PARAM_ROUNDING_STRATEGY_BANKER \
    ((PitParamRoundingStrategy) export_rounding_strategy_const(RoundingStrategy_BANKER))
```

## `PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT`

Conservative profit rounding alias.

```c
#define PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT \
    ((PitParamRoundingStrategy) export_rounding_strategy_const(RoundingStrategy_CONSERVATIVE_PROFIT))
```

## `PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS`

Conservative loss rounding alias.

```c
#define PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS \
    ((PitParamRoundingStrategy) export_rounding_strategy_const(RoundingStrategy_CONSERVATIVE_LOSS))
```

## `PitParamTradeAmount`

One trade-amount value plus its interpretation mode.

The numeric value is interpreted according to `kind`:

- `Quantity` means instrument quantity;
- `Volume` means settlement notional volume.

```c
typedef struct PitParamTradeAmount {
    PitParamDecimal value;
    PitParamTradeAmountKind kind;
} PitParamTradeAmount;
```

## `PitTriBool`

Tri-state boolean value.

```c
typedef uint8_t PitTriBool;
/**
 * Value is absent.
 */
#define PitTriBool_NotSet ((PitTriBool) 0)
/**
 * Boolean false.
 */
#define PitTriBool_False ((PitTriBool) 1)
/**
 * Boolean true.
 */
#define PitTriBool_True ((PitTriBool) 2)
```

## `PitParamAdjustmentAmountKind`

Selects how an account-adjustment amount should be interpreted.

```c
typedef uint8_t PitParamAdjustmentAmountKind;
/**
 * No amount is specified.
 */
#define PitParamAdjustmentAmountKind_NotSet ((PitParamAdjustmentAmountKind) 0)
/**
 * Change current state by the supplied signed amount.
 */
#define PitParamAdjustmentAmountKind_Delta ((PitParamAdjustmentAmountKind) 1)
/**
 * Set current state to the supplied signed amount.
 */
#define PitParamAdjustmentAmountKind_Absolute ((PitParamAdjustmentAmountKind) 2)
```

## `PitParamDecimal`

Decimal value represented as `mantissa * 10^-scale`.

```c
typedef struct PitParamDecimal {
    int64_t mantissa_lo;
    int64_t mantissa_hi;
    int32_t scale;
} PitParamDecimal;
```

## `PitParamPnl`

Validated `Pnl` value wrapper.

```c
typedef struct PitParamPnl {
    PitParamDecimal _0;
} PitParamPnl;
```

## `pit_create_param_pnl`

Validates a decimal and returns a `Pnl` wrapper.

Meaning: Profit and loss value; positive means profit, negative means loss.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_pnl(
    PitParamDecimal value,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_get_decimal`

Returns the decimal stored in `Pnl`.

```c
PitParamDecimal pit_param_pnl_get_decimal(
    PitParamPnl value
);
```

## `PitParamPrice`

Validated `Price` value wrapper.

```c
typedef struct PitParamPrice {
    PitParamDecimal _0;
} PitParamPrice;
```

## `pit_create_param_price`

Validates a decimal and returns a `Price` wrapper.

Meaning: Price per one instrument unit; may be negative in some derivative
markets.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_price(
    PitParamDecimal value,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_get_decimal`

Returns the decimal stored in `Price`.

```c
PitParamDecimal pit_param_price_get_decimal(
    PitParamPrice value
);
```

## `PitParamQuantity`

Validated `Quantity` value wrapper.

```c
typedef struct PitParamQuantity {
    PitParamDecimal _0;
} PitParamQuantity;
```

## `pit_create_param_quantity`

Validates a decimal and returns a `Quantity` wrapper.

Meaning: Instrument quantity; non-negative amount in instrument units.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_quantity(
    PitParamDecimal value,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_get_decimal`

Returns the decimal stored in `Quantity`.

```c
PitParamDecimal pit_param_quantity_get_decimal(
    PitParamQuantity value
);
```

## `PitParamVolume`

Validated `Volume` value wrapper.

```c
typedef struct PitParamVolume {
    PitParamDecimal _0;
} PitParamVolume;
```

## `pit_create_param_volume`

Validates a decimal and returns a `Volume` wrapper.

Meaning: Settlement notional volume; non-negative amount in settlement units.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_volume(
    PitParamDecimal value,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_get_decimal`

Returns the decimal stored in `Volume`.

```c
PitParamDecimal pit_param_volume_get_decimal(
    PitParamVolume value
);
```

## `PitParamCashFlow`

Validated `CashFlow` value wrapper.

```c
typedef struct PitParamCashFlow {
    PitParamDecimal _0;
} PitParamCashFlow;
```

## `pit_create_param_cash_flow`

Validates a decimal and returns a `CashFlow` wrapper.

Meaning: Cash flow contribution; positive is inflow, negative is outflow.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_cash_flow(
    PitParamDecimal value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_get_decimal`

Returns the decimal stored in `CashFlow`.

```c
PitParamDecimal pit_param_cash_flow_get_decimal(
    PitParamCashFlow value
);
```

## `PitParamPositionSize`

Validated `PositionSize` value wrapper.

```c
typedef struct PitParamPositionSize {
    PitParamDecimal _0;
} PitParamPositionSize;
```

## `pit_create_param_position_size`

Validates a decimal and returns a `PositionSize` wrapper.

Meaning: Signed position size; long is positive, short is negative.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_position_size(
    PitParamDecimal value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_get_decimal`

Returns the decimal stored in `PositionSize`.

```c
PitParamDecimal pit_param_position_size_get_decimal(
    PitParamPositionSize value
);
```

## `PitParamFee`

Validated `Fee` value wrapper.

```c
typedef struct PitParamFee {
    PitParamDecimal _0;
} PitParamFee;
```

## `pit_create_param_fee`

Validates a decimal and returns a `Fee` wrapper.

Meaning: Fee amount; can be negative for rebates or reconciliation adjustments.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_fee(
    PitParamDecimal value,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_get_decimal`

Returns the decimal stored in `Fee`.

```c
PitParamDecimal pit_param_fee_get_decimal(
    PitParamFee value
);
```

## `PitParamNotional`

Validated `Notional` value wrapper.

```c
typedef struct PitParamNotional {
    PitParamDecimal _0;
} PitParamNotional;
```

## `pit_create_param_notional`

Validates a decimal and returns a `Notional` wrapper.

Meaning: Monetary position exposure for margin and risk calculation; always
non-negative.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool pit_create_param_notional(
    PitParamDecimal value,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_get_decimal`

Returns the decimal stored in `Notional`.

```c
PitParamDecimal pit_param_notional_get_decimal(
    PitParamNotional value
);
```

## `pit_create_param_pnl_from_str`

```c
bool pit_create_param_pnl_from_str(
    PitStringView value,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_create_param_pnl_from_f64`

```c
bool pit_create_param_pnl_from_f64(
    double value,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_create_param_pnl_from_i64`

```c
bool pit_create_param_pnl_from_i64(
    int64_t value,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_create_param_pnl_from_u64`

```c
bool pit_create_param_pnl_from_u64(
    uint64_t value,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_create_param_pnl_from_str_rounded`

```c
bool pit_create_param_pnl_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_create_param_pnl_from_f64_rounded`

```c
bool pit_create_param_pnl_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_create_param_pnl_from_decimal_rounded`

```c
bool pit_create_param_pnl_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_to_f64`

```c
bool pit_param_pnl_to_f64(
    PitParamPnl value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_is_zero`

```c
bool pit_param_pnl_is_zero(
    PitParamPnl value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_compare`

```c
bool pit_param_pnl_compare(
    PitParamPnl lhs,
    PitParamPnl rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_to_string`

```c
PitSharedString * pit_param_pnl_to_string(
    PitParamPnl value,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_add`

```c
bool pit_param_pnl_checked_add(
    PitParamPnl lhs,
    PitParamPnl rhs,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_sub`

```c
bool pit_param_pnl_checked_sub(
    PitParamPnl lhs,
    PitParamPnl rhs,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_mul_i64`

```c
bool pit_param_pnl_checked_mul_i64(
    PitParamPnl value,
    int64_t multiplier,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_mul_u64`

```c
bool pit_param_pnl_checked_mul_u64(
    PitParamPnl value,
    uint64_t multiplier,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_mul_f64`

```c
bool pit_param_pnl_checked_mul_f64(
    PitParamPnl value,
    double multiplier,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_div_i64`

```c
bool pit_param_pnl_checked_div_i64(
    PitParamPnl value,
    int64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_div_u64`

```c
bool pit_param_pnl_checked_div_u64(
    PitParamPnl value,
    uint64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_div_f64`

```c
bool pit_param_pnl_checked_div_f64(
    PitParamPnl value,
    double divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_rem_i64`

```c
bool pit_param_pnl_checked_rem_i64(
    PitParamPnl value,
    int64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_rem_u64`

```c
bool pit_param_pnl_checked_rem_u64(
    PitParamPnl value,
    uint64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_rem_f64`

```c
bool pit_param_pnl_checked_rem_f64(
    PitParamPnl value,
    double divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_checked_neg`

```c
bool pit_param_pnl_checked_neg(
    PitParamPnl value,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_create_param_price_from_str`

```c
bool pit_create_param_price_from_str(
    PitStringView value,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_create_param_price_from_f64`

```c
bool pit_create_param_price_from_f64(
    double value,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_create_param_price_from_i64`

```c
bool pit_create_param_price_from_i64(
    int64_t value,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_create_param_price_from_u64`

```c
bool pit_create_param_price_from_u64(
    uint64_t value,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_create_param_price_from_str_rounded`

```c
bool pit_create_param_price_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_create_param_price_from_f64_rounded`

```c
bool pit_create_param_price_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_create_param_price_from_decimal_rounded`

```c
bool pit_create_param_price_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_to_f64`

```c
bool pit_param_price_to_f64(
    PitParamPrice value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_price_is_zero`

```c
bool pit_param_price_is_zero(
    PitParamPrice value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_price_compare`

```c
bool pit_param_price_compare(
    PitParamPrice lhs,
    PitParamPrice rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_price_to_string`

```c
PitSharedString * pit_param_price_to_string(
    PitParamPrice value,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_add`

```c
bool pit_param_price_checked_add(
    PitParamPrice lhs,
    PitParamPrice rhs,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_sub`

```c
bool pit_param_price_checked_sub(
    PitParamPrice lhs,
    PitParamPrice rhs,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_mul_i64`

```c
bool pit_param_price_checked_mul_i64(
    PitParamPrice value,
    int64_t multiplier,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_mul_u64`

```c
bool pit_param_price_checked_mul_u64(
    PitParamPrice value,
    uint64_t multiplier,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_mul_f64`

```c
bool pit_param_price_checked_mul_f64(
    PitParamPrice value,
    double multiplier,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_div_i64`

```c
bool pit_param_price_checked_div_i64(
    PitParamPrice value,
    int64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_div_u64`

```c
bool pit_param_price_checked_div_u64(
    PitParamPrice value,
    uint64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_div_f64`

```c
bool pit_param_price_checked_div_f64(
    PitParamPrice value,
    double divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_rem_i64`

```c
bool pit_param_price_checked_rem_i64(
    PitParamPrice value,
    int64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_rem_u64`

```c
bool pit_param_price_checked_rem_u64(
    PitParamPrice value,
    uint64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_rem_f64`

```c
bool pit_param_price_checked_rem_f64(
    PitParamPrice value,
    double divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_param_price_checked_neg`

```c
bool pit_param_price_checked_neg(
    PitParamPrice value,
    PitParamPrice * out,
    PitOutParamError out_error
);
```

## `pit_create_param_quantity_from_str`

```c
bool pit_create_param_quantity_from_str(
    PitStringView value,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_create_param_quantity_from_f64`

```c
bool pit_create_param_quantity_from_f64(
    double value,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_create_param_quantity_from_i64`

```c
bool pit_create_param_quantity_from_i64(
    int64_t value,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_create_param_quantity_from_u64`

```c
bool pit_create_param_quantity_from_u64(
    uint64_t value,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_create_param_quantity_from_str_rounded`

```c
bool pit_create_param_quantity_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_create_param_quantity_from_f64_rounded`

```c
bool pit_create_param_quantity_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_create_param_quantity_from_decimal_rounded`

```c
bool pit_create_param_quantity_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_to_f64`

```c
bool pit_param_quantity_to_f64(
    PitParamQuantity value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_is_zero`

```c
bool pit_param_quantity_is_zero(
    PitParamQuantity value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_compare`

```c
bool pit_param_quantity_compare(
    PitParamQuantity lhs,
    PitParamQuantity rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_to_string`

```c
PitSharedString * pit_param_quantity_to_string(
    PitParamQuantity value,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_add`

```c
bool pit_param_quantity_checked_add(
    PitParamQuantity lhs,
    PitParamQuantity rhs,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_sub`

```c
bool pit_param_quantity_checked_sub(
    PitParamQuantity lhs,
    PitParamQuantity rhs,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_mul_i64`

```c
bool pit_param_quantity_checked_mul_i64(
    PitParamQuantity value,
    int64_t multiplier,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_mul_u64`

```c
bool pit_param_quantity_checked_mul_u64(
    PitParamQuantity value,
    uint64_t multiplier,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_mul_f64`

```c
bool pit_param_quantity_checked_mul_f64(
    PitParamQuantity value,
    double multiplier,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_div_i64`

```c
bool pit_param_quantity_checked_div_i64(
    PitParamQuantity value,
    int64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_div_u64`

```c
bool pit_param_quantity_checked_div_u64(
    PitParamQuantity value,
    uint64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_div_f64`

```c
bool pit_param_quantity_checked_div_f64(
    PitParamQuantity value,
    double divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_rem_i64`

```c
bool pit_param_quantity_checked_rem_i64(
    PitParamQuantity value,
    int64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_rem_u64`

```c
bool pit_param_quantity_checked_rem_u64(
    PitParamQuantity value,
    uint64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_checked_rem_f64`

```c
bool pit_param_quantity_checked_rem_f64(
    PitParamQuantity value,
    double divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_create_param_volume_from_str`

```c
bool pit_create_param_volume_from_str(
    PitStringView value,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_volume_from_f64`

```c
bool pit_create_param_volume_from_f64(
    double value,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_volume_from_i64`

```c
bool pit_create_param_volume_from_i64(
    int64_t value,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_volume_from_u64`

```c
bool pit_create_param_volume_from_u64(
    uint64_t value,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_volume_from_str_rounded`

```c
bool pit_create_param_volume_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_volume_from_f64_rounded`

```c
bool pit_create_param_volume_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_volume_from_decimal_rounded`

```c
bool pit_create_param_volume_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_to_f64`

```c
bool pit_param_volume_to_f64(
    PitParamVolume value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_is_zero`

```c
bool pit_param_volume_is_zero(
    PitParamVolume value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_compare`

```c
bool pit_param_volume_compare(
    PitParamVolume lhs,
    PitParamVolume rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_to_string`

```c
PitSharedString * pit_param_volume_to_string(
    PitParamVolume value,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_add`

```c
bool pit_param_volume_checked_add(
    PitParamVolume lhs,
    PitParamVolume rhs,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_sub`

```c
bool pit_param_volume_checked_sub(
    PitParamVolume lhs,
    PitParamVolume rhs,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_mul_i64`

```c
bool pit_param_volume_checked_mul_i64(
    PitParamVolume value,
    int64_t multiplier,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_mul_u64`

```c
bool pit_param_volume_checked_mul_u64(
    PitParamVolume value,
    uint64_t multiplier,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_mul_f64`

```c
bool pit_param_volume_checked_mul_f64(
    PitParamVolume value,
    double multiplier,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_div_i64`

```c
bool pit_param_volume_checked_div_i64(
    PitParamVolume value,
    int64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_div_u64`

```c
bool pit_param_volume_checked_div_u64(
    PitParamVolume value,
    uint64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_div_f64`

```c
bool pit_param_volume_checked_div_f64(
    PitParamVolume value,
    double divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_rem_i64`

```c
bool pit_param_volume_checked_rem_i64(
    PitParamVolume value,
    int64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_rem_u64`

```c
bool pit_param_volume_checked_rem_u64(
    PitParamVolume value,
    uint64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_checked_rem_f64`

```c
bool pit_param_volume_checked_rem_f64(
    PitParamVolume value,
    double divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_cash_flow_from_str`

```c
bool pit_create_param_cash_flow_from_str(
    PitStringView value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_create_param_cash_flow_from_f64`

```c
bool pit_create_param_cash_flow_from_f64(
    double value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_create_param_cash_flow_from_i64`

```c
bool pit_create_param_cash_flow_from_i64(
    int64_t value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_create_param_cash_flow_from_u64`

```c
bool pit_create_param_cash_flow_from_u64(
    uint64_t value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_create_param_cash_flow_from_str_rounded`

```c
bool pit_create_param_cash_flow_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_create_param_cash_flow_from_f64_rounded`

```c
bool pit_create_param_cash_flow_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_create_param_cash_flow_from_decimal_rounded`

```c
bool pit_create_param_cash_flow_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_to_f64`

```c
bool pit_param_cash_flow_to_f64(
    PitParamCashFlow value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_is_zero`

```c
bool pit_param_cash_flow_is_zero(
    PitParamCashFlow value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_compare`

```c
bool pit_param_cash_flow_compare(
    PitParamCashFlow lhs,
    PitParamCashFlow rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_to_string`

```c
PitSharedString * pit_param_cash_flow_to_string(
    PitParamCashFlow value,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_add`

```c
bool pit_param_cash_flow_checked_add(
    PitParamCashFlow lhs,
    PitParamCashFlow rhs,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_sub`

```c
bool pit_param_cash_flow_checked_sub(
    PitParamCashFlow lhs,
    PitParamCashFlow rhs,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_mul_i64`

```c
bool pit_param_cash_flow_checked_mul_i64(
    PitParamCashFlow value,
    int64_t multiplier,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_mul_u64`

```c
bool pit_param_cash_flow_checked_mul_u64(
    PitParamCashFlow value,
    uint64_t multiplier,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_mul_f64`

```c
bool pit_param_cash_flow_checked_mul_f64(
    PitParamCashFlow value,
    double multiplier,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_div_i64`

```c
bool pit_param_cash_flow_checked_div_i64(
    PitParamCashFlow value,
    int64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_div_u64`

```c
bool pit_param_cash_flow_checked_div_u64(
    PitParamCashFlow value,
    uint64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_div_f64`

```c
bool pit_param_cash_flow_checked_div_f64(
    PitParamCashFlow value,
    double divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_rem_i64`

```c
bool pit_param_cash_flow_checked_rem_i64(
    PitParamCashFlow value,
    int64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_rem_u64`

```c
bool pit_param_cash_flow_checked_rem_u64(
    PitParamCashFlow value,
    uint64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_rem_f64`

```c
bool pit_param_cash_flow_checked_rem_f64(
    PitParamCashFlow value,
    double divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_checked_neg`

```c
bool pit_param_cash_flow_checked_neg(
    PitParamCashFlow value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_create_param_position_size_from_str`

```c
bool pit_create_param_position_size_from_str(
    PitStringView value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_create_param_position_size_from_f64`

```c
bool pit_create_param_position_size_from_f64(
    double value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_create_param_position_size_from_i64`

```c
bool pit_create_param_position_size_from_i64(
    int64_t value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_create_param_position_size_from_u64`

```c
bool pit_create_param_position_size_from_u64(
    uint64_t value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_create_param_position_size_from_str_rounded`

```c
bool pit_create_param_position_size_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_create_param_position_size_from_f64_rounded`

```c
bool pit_create_param_position_size_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_create_param_position_size_from_decimal_rounded`

```c
bool pit_create_param_position_size_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_to_f64`

```c
bool pit_param_position_size_to_f64(
    PitParamPositionSize value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_is_zero`

```c
bool pit_param_position_size_is_zero(
    PitParamPositionSize value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_compare`

```c
bool pit_param_position_size_compare(
    PitParamPositionSize lhs,
    PitParamPositionSize rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_to_string`

```c
PitSharedString * pit_param_position_size_to_string(
    PitParamPositionSize value,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_add`

```c
bool pit_param_position_size_checked_add(
    PitParamPositionSize lhs,
    PitParamPositionSize rhs,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_sub`

```c
bool pit_param_position_size_checked_sub(
    PitParamPositionSize lhs,
    PitParamPositionSize rhs,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_mul_i64`

```c
bool pit_param_position_size_checked_mul_i64(
    PitParamPositionSize value,
    int64_t multiplier,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_mul_u64`

```c
bool pit_param_position_size_checked_mul_u64(
    PitParamPositionSize value,
    uint64_t multiplier,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_mul_f64`

```c
bool pit_param_position_size_checked_mul_f64(
    PitParamPositionSize value,
    double multiplier,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_div_i64`

```c
bool pit_param_position_size_checked_div_i64(
    PitParamPositionSize value,
    int64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_div_u64`

```c
bool pit_param_position_size_checked_div_u64(
    PitParamPositionSize value,
    uint64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_div_f64`

```c
bool pit_param_position_size_checked_div_f64(
    PitParamPositionSize value,
    double divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_rem_i64`

```c
bool pit_param_position_size_checked_rem_i64(
    PitParamPositionSize value,
    int64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_rem_u64`

```c
bool pit_param_position_size_checked_rem_u64(
    PitParamPositionSize value,
    uint64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_rem_f64`

```c
bool pit_param_position_size_checked_rem_f64(
    PitParamPositionSize value,
    double divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_neg`

```c
bool pit_param_position_size_checked_neg(
    PitParamPositionSize value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_create_param_fee_from_str`

```c
bool pit_create_param_fee_from_str(
    PitStringView value,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_create_param_fee_from_f64`

```c
bool pit_create_param_fee_from_f64(
    double value,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_create_param_fee_from_i64`

```c
bool pit_create_param_fee_from_i64(
    int64_t value,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_create_param_fee_from_u64`

```c
bool pit_create_param_fee_from_u64(
    uint64_t value,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_create_param_fee_from_str_rounded`

```c
bool pit_create_param_fee_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_create_param_fee_from_f64_rounded`

```c
bool pit_create_param_fee_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_create_param_fee_from_decimal_rounded`

```c
bool pit_create_param_fee_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_to_f64`

```c
bool pit_param_fee_to_f64(
    PitParamFee value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_is_zero`

```c
bool pit_param_fee_is_zero(
    PitParamFee value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_compare`

```c
bool pit_param_fee_compare(
    PitParamFee lhs,
    PitParamFee rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_to_string`

```c
PitSharedString * pit_param_fee_to_string(
    PitParamFee value,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_add`

```c
bool pit_param_fee_checked_add(
    PitParamFee lhs,
    PitParamFee rhs,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_sub`

```c
bool pit_param_fee_checked_sub(
    PitParamFee lhs,
    PitParamFee rhs,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_mul_i64`

```c
bool pit_param_fee_checked_mul_i64(
    PitParamFee value,
    int64_t multiplier,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_mul_u64`

```c
bool pit_param_fee_checked_mul_u64(
    PitParamFee value,
    uint64_t multiplier,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_mul_f64`

```c
bool pit_param_fee_checked_mul_f64(
    PitParamFee value,
    double multiplier,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_div_i64`

```c
bool pit_param_fee_checked_div_i64(
    PitParamFee value,
    int64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_div_u64`

```c
bool pit_param_fee_checked_div_u64(
    PitParamFee value,
    uint64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_div_f64`

```c
bool pit_param_fee_checked_div_f64(
    PitParamFee value,
    double divisor,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_rem_i64`

```c
bool pit_param_fee_checked_rem_i64(
    PitParamFee value,
    int64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_rem_u64`

```c
bool pit_param_fee_checked_rem_u64(
    PitParamFee value,
    uint64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_rem_f64`

```c
bool pit_param_fee_checked_rem_f64(
    PitParamFee value,
    double divisor,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_checked_neg`

```c
bool pit_param_fee_checked_neg(
    PitParamFee value,
    PitParamFee * out,
    PitOutParamError out_error
);
```

## `pit_create_param_notional_from_str`

```c
bool pit_create_param_notional_from_str(
    PitStringView value,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_create_param_notional_from_f64`

```c
bool pit_create_param_notional_from_f64(
    double value,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_create_param_notional_from_i64`

```c
bool pit_create_param_notional_from_i64(
    int64_t value,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_create_param_notional_from_u64`

```c
bool pit_create_param_notional_from_u64(
    uint64_t value,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_create_param_notional_from_str_rounded`

```c
bool pit_create_param_notional_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_create_param_notional_from_f64_rounded`

```c
bool pit_create_param_notional_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_create_param_notional_from_decimal_rounded`

```c
bool pit_create_param_notional_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_to_f64`

```c
bool pit_param_notional_to_f64(
    PitParamNotional value,
    double * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_is_zero`

```c
bool pit_param_notional_is_zero(
    PitParamNotional value,
    bool * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_compare`

```c
bool pit_param_notional_compare(
    PitParamNotional lhs,
    PitParamNotional rhs,
    int8_t * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_to_string`

```c
PitSharedString * pit_param_notional_to_string(
    PitParamNotional value,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_add`

```c
bool pit_param_notional_checked_add(
    PitParamNotional lhs,
    PitParamNotional rhs,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_sub`

```c
bool pit_param_notional_checked_sub(
    PitParamNotional lhs,
    PitParamNotional rhs,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_mul_i64`

```c
bool pit_param_notional_checked_mul_i64(
    PitParamNotional value,
    int64_t multiplier,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_mul_u64`

```c
bool pit_param_notional_checked_mul_u64(
    PitParamNotional value,
    uint64_t multiplier,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_mul_f64`

```c
bool pit_param_notional_checked_mul_f64(
    PitParamNotional value,
    double multiplier,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_div_i64`

```c
bool pit_param_notional_checked_div_i64(
    PitParamNotional value,
    int64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_div_u64`

```c
bool pit_param_notional_checked_div_u64(
    PitParamNotional value,
    uint64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_div_f64`

```c
bool pit_param_notional_checked_div_f64(
    PitParamNotional value,
    double divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_rem_i64`

```c
bool pit_param_notional_checked_rem_i64(
    PitParamNotional value,
    int64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_rem_u64`

```c
bool pit_param_notional_checked_rem_u64(
    PitParamNotional value,
    uint64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_checked_rem_f64`

```c
bool pit_param_notional_checked_rem_f64(
    PitParamNotional value,
    double divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `PitParamNotionalOptional`

```c
typedef struct PitParamNotionalOptional {
    PitParamNotional value;
    bool is_set;
} PitParamNotionalOptional;
```

## `PitParamPnlOptional`

```c
typedef struct PitParamPnlOptional {
    PitParamPnl value;
    bool is_set;
} PitParamPnlOptional;
```

## `PitParamPriceOptional`

```c
typedef struct PitParamPriceOptional {
    PitParamPrice value;
    bool is_set;
} PitParamPriceOptional;
```

## `PitParamQuantityOptional`

```c
typedef struct PitParamQuantityOptional {
    PitParamQuantity value;
    bool is_set;
} PitParamQuantityOptional;
```

## `PitParamVolumeOptional`

```c
typedef struct PitParamVolumeOptional {
    PitParamVolume value;
    bool is_set;
} PitParamVolumeOptional;
```

## `PitParamCashFlowOptional`

```c
typedef struct PitParamCashFlowOptional {
    PitParamCashFlow value;
    bool is_set;
} PitParamCashFlowOptional;
```

## `PitParamPositionSizeOptional`

```c
typedef struct PitParamPositionSizeOptional {
    PitParamPositionSize value;
    bool is_set;
} PitParamPositionSizeOptional;
```

## `PitParamFeeOptional`

```c
typedef struct PitParamFeeOptional {
    PitParamFee value;
    bool is_set;
} PitParamFeeOptional;
```

## `PitParamAccountIdOptional`

```c
typedef struct PitParamAccountIdOptional {
    PitParamAccountId value;
    bool is_set;
} PitParamAccountIdOptional;
```

## `pit_param_leverage_calculate_margin_required`

```c
bool pit_param_leverage_calculate_margin_required(
    PitParamLeverage leverage,
    PitParamNotional notional,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_price_calculate_volume`

```c
bool pit_param_price_calculate_volume(
    PitParamPrice price,
    PitParamQuantity quantity,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_calculate_volume`

```c
bool pit_param_quantity_calculate_volume(
    PitParamQuantity quantity,
    PitParamPrice price,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_calculate_quantity`

```c
bool pit_param_volume_calculate_quantity(
    PitParamVolume volume,
    PitParamPrice price,
    PitParamQuantity * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_to_cash_flow`

```c
bool pit_param_pnl_to_cash_flow(
    PitParamPnl value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_to_position_size`

```c
bool pit_param_pnl_to_position_size(
    PitParamPnl value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_pnl_from_fee`

```c
bool pit_param_pnl_from_fee(
    PitParamFee fee,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_from_pnl`

```c
bool pit_param_cash_flow_from_pnl(
    PitParamPnl pnl,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_from_fee`

```c
bool pit_param_cash_flow_from_fee(
    PitParamFee fee,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_from_volume_inflow`

```c
bool pit_param_cash_flow_from_volume_inflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_cash_flow_from_volume_outflow`

```c
bool pit_param_cash_flow_from_volume_outflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_to_pnl`

```c
bool pit_param_fee_to_pnl(
    PitParamFee fee,
    PitParamPnl * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_to_position_size`

```c
bool pit_param_fee_to_position_size(
    PitParamFee fee,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_fee_to_cash_flow`

```c
bool pit_param_fee_to_cash_flow(
    PitParamFee fee,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_to_cash_flow_inflow`

```c
bool pit_param_volume_to_cash_flow_inflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_to_cash_flow_outflow`

```c
bool pit_param_volume_to_cash_flow_outflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_from_pnl`

```c
bool pit_param_position_size_from_pnl(
    PitParamPnl pnl,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_from_fee`

```c
bool pit_param_position_size_from_fee(
    PitParamFee fee,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_from_quantity_and_side`

```c
bool pit_param_position_size_from_quantity_and_side(
    PitParamQuantity quantity,
    PitParamSide side,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_position_size_to_open_quantity`

```c
bool pit_param_position_size_to_open_quantity(
    PitParamPositionSize value,
    PitParamQuantity * out_quantity,
    PitParamSide * out_side,
    PitOutParamError out_error
);
```

## `pit_param_position_size_to_close_quantity`

```c
bool pit_param_position_size_to_close_quantity(
    PitParamPositionSize value,
    PitParamQuantity * out_quantity,
    PitParamSide * out_side,
    PitOutParamError out_error
);
```

## `pit_param_position_size_checked_add_quantity`

```c
bool pit_param_position_size_checked_add_quantity(
    PitParamPositionSize value,
    PitParamQuantity quantity,
    PitParamSide side,
    PitParamPositionSize * out,
    PitOutParamError out_error
);
```

## `pit_param_price_calculate_notional`

```c
bool pit_param_price_calculate_notional(
    PitParamPrice price,
    PitParamQuantity quantity,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_quantity_calculate_notional`

```c
bool pit_param_quantity_calculate_notional(
    PitParamQuantity quantity,
    PitParamPrice price,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_from_volume`

```c
bool pit_param_notional_from_volume(
    PitParamVolume volume,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_to_volume`

```c
bool pit_param_notional_to_volume(
    PitParamNotional notional,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_param_notional_calculate_margin_required`

```c
bool pit_param_notional_calculate_margin_required(
    PitParamNotional notional,
    PitParamLeverage leverage,
    PitParamNotional * out,
    PitOutParamError out_error
);
```

## `pit_param_volume_from_notional`

```c
bool pit_param_volume_from_notional(
    PitParamNotional notional,
    PitParamVolume * out,
    PitOutParamError out_error
);
```

## `pit_create_param_account_id_from_u64`

Constructs an account identifier from a 64-bit integer.

This is a direct numeric mapping with no collision risk.

WARNING:
Do not mix IDs produced by this function with IDs produced by
`pit_create_param_account_id_from_str` in the same runtime state.

Contract:

- returns a stable account identifier value;
- this function always succeeds.

```c
PitParamAccountId pit_create_param_account_id_from_u64(
    uint64_t value
);
```

## `pit_create_param_account_id_from_str`

Constructs an account identifier from a UTF-8 byte sequence.

The bytes are read only for the duration of the call. No trailing zero byte
is required.

Collision note:

- different account strings can map to the same identifier;
- for `n` distinct account strings the probability of at least one collision

is approximately `n^2 / 2^65`.

- if collision risk is unacceptable, keep your own collision-free

string-to-integer mapping and use `pit_create_param_account_id_from_u64`.

The previous sentence is why this helper is suitable for stable adapter-side
mapping, but not for workflows that require guaranteed uniqueness.

WARNING:
Do not mix IDs produced by this function with IDs produced by
`pit_create_param_account_id_from_u64` in the same runtime state.

Contract:

- returns `true` and writes a stable account identifier to `out` on success;
- returns `false` on invalid input and optionally writes `PitParamError`.

### Safety

`value.ptr` must be non-null and point to at least `value.len` readable
UTF-8 bytes.

```c
bool pit_create_param_account_id_from_str(
    PitStringView value,
    PitParamAccountId * out,
    PitOutParamError out_error
);
```

## `pit_create_param_asset_from_str`

Validates and copies an asset identifier into a caller-owned shared-string
handle.

The returned handle must be destroyed with `pit_destroy_param_asset`.

```c
PitSharedString * pit_create_param_asset_from_str(
    PitStringView value,
    PitOutParamError out_error
);
```

## `pit_destroy_param_asset`

Destroys a caller-owned asset handle created by
`pit_create_param_asset_from_str`.

```c
void pit_destroy_param_asset(
    PitSharedString * handle
);
```

## `PitParamErrorCode`

Parameter error code transported through FFI.

```c
typedef uint32_t PitParamErrorCode;
/**
 * Error code is not specified.
 */
#define PitParamErrorCode_Unspecified ((PitParamErrorCode) 0)
/**
 * Value must be non-negative.
 */
#define PitParamErrorCode_Negative ((PitParamErrorCode) 1)
/**
 * Division by zero.
 */
#define PitParamErrorCode_DivisionByZero ((PitParamErrorCode) 2)
/**
 * Arithmetic overflow.
 */
#define PitParamErrorCode_Overflow ((PitParamErrorCode) 3)
/**
 * Arithmetic underflow.
 */
#define PitParamErrorCode_Underflow ((PitParamErrorCode) 4)
/**
 * Invalid float value.
 */
#define PitParamErrorCode_InvalidFloat ((PitParamErrorCode) 5)
/**
 * Invalid textual format.
 */
#define PitParamErrorCode_InvalidFormat ((PitParamErrorCode) 6)
/**
 * Invalid price value.
 */
#define PitParamErrorCode_InvalidPrice ((PitParamErrorCode) 7)
/**
 * Invalid leverage value.
 */
#define PitParamErrorCode_InvalidLeverage ((PitParamErrorCode) 8)
/**
 * Catch-all code for unknown cases.
 */
#define PitParamErrorCode_Other ((PitParamErrorCode) 4294967295)
```

## `PitParamError`

Caller-owned parameter error container.

```c
typedef struct PitParamError {
    PitParamErrorCode code;
    PitSharedString * message;
} PitParamError;
```

## `PitOutParamError`

Parameter error out-pointer used by fallible param FFI calls.

```c
typedef PitParamError ** PitOutParamError;
```

## `pit_destroy_param_error`

Releases a caller-owned parameter error container.

### Safety

`handle` must be either null or a pointer returned by this library through
`PitOutParamError`. The handle must be destroyed at most once.

```c
void pit_destroy_param_error(
    PitParamError * handle
);
```
