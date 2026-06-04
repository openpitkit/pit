# Parameter Types

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitParamLeverage`

Leverage multiplier for FFI payloads.

Uses fixed-point scale `10` in integer units:

- `10` means `1.0x`
- `11` means `1.1x`
- `1005` means `100.5x`

Valid range: `10..=30000`.

A value of `OPENPIT_PARAM_LEVERAGE_NOT_SET` (`0`) means leverage is not
specified.

```c
typedef uint16_t OpenPitParamLeverage;
```

## `OPENPIT_PARAM_LEVERAGE_NOT_SET`

Sentinel value indicating leverage is not set.

```c
#define OPENPIT_PARAM_LEVERAGE_NOT_SET ((OpenPitParamLeverage) 0)
```

## `OPENPIT_PARAM_LEVERAGE_SCALE`

Fixed-point scale used by leverage payloads.

```c
#define OPENPIT_PARAM_LEVERAGE_SCALE ((OpenPitParamLeverage) 10)
```

## `OPENPIT_PARAM_LEVERAGE_MIN`

Minimum leverage in whole units.

```c
#define OPENPIT_PARAM_LEVERAGE_MIN ((OpenPitParamLeverage) 1)
```

## `OPENPIT_PARAM_LEVERAGE_MAX`

Maximum leverage in whole units.

```c
#define OPENPIT_PARAM_LEVERAGE_MAX ((OpenPitParamLeverage) 3000)
```

## `OPENPIT_PARAM_LEVERAGE_STEP`

Supported leverage increment step.

```c
#define OPENPIT_PARAM_LEVERAGE_STEP ((float) 0.1)
```

## `OpenPitParamAccountId`

Stable account identifier type for FFI payloads.

WARNING: Use exactly one account-id source model per runtime:

- either purely numeric IDs (`openpit_create_param_account_id_from_uint64`),
- or purely string-derived IDs
  (`openpit_create_param_account_id_from_string`).

Do not mix both models in the same runtime state. A hashed string value can
coincide with a direct numeric ID, and then two distinct accounts become one
logical key in maps and engine state.

```c
typedef uint64_t OpenPitParamAccountId;
```

## `OpenPitParamSide`

Order side.

```c
typedef uint8_t OpenPitParamSide;
/**
 * Value is absent.
 */
#define OpenPitParamSide_NotSet ((OpenPitParamSide) 0)
/**
 * Buy side.
 */
#define OpenPitParamSide_Buy ((OpenPitParamSide) 1)
/**
 * Sell side.
 */
#define OpenPitParamSide_Sell ((OpenPitParamSide) 2)
```

## `OpenPitParamPositionSide`

Position direction.

```c
typedef uint8_t OpenPitParamPositionSide;
/**
 * Value is absent.
 */
#define OpenPitParamPositionSide_NotSet ((OpenPitParamPositionSide) 0)
/**
 * Long exposure.
 */
#define OpenPitParamPositionSide_Long ((OpenPitParamPositionSide) 1)
/**
 * Short exposure.
 */
#define OpenPitParamPositionSide_Short ((OpenPitParamPositionSide) 2)
```

## `OpenPitParamPositionMode`

Position accounting mode.

```c
typedef uint8_t OpenPitParamPositionMode;
/**
 * Value is absent.
 */
#define OpenPitParamPositionMode_NotSet ((OpenPitParamPositionMode) 0)
/**
 * Opposite trades net into one position.
 */
#define OpenPitParamPositionMode_Netting ((OpenPitParamPositionMode) 1)
/**
 * Long and short positions are tracked separately.
 */
#define OpenPitParamPositionMode_Hedged ((OpenPitParamPositionMode) 2)
```

## `OpenPitParamPositionEffect`

Whether a trade opens or closes exposure.

```c
typedef uint8_t OpenPitParamPositionEffect;
/**
 * Value is absent.
 */
#define OpenPitParamPositionEffect_NotSet ((OpenPitParamPositionEffect) 0)
/**
 * The trade opens or increases exposure.
 */
#define OpenPitParamPositionEffect_Open ((OpenPitParamPositionEffect) 1)
/**
 * The trade closes or reduces exposure.
 */
#define OpenPitParamPositionEffect_Close ((OpenPitParamPositionEffect) 2)
```

## `OpenPitParamTradeAmountKind`

Selects how one trade-amount numeric value should be interpreted.

```c
typedef uint8_t OpenPitParamTradeAmountKind;
/**
 * No amount field is selected.
 */
#define OpenPitParamTradeAmountKind_NotSet ((OpenPitParamTradeAmountKind) 0)
/**
 * The value is instrument quantity.
 */
#define OpenPitParamTradeAmountKind_Quantity ((OpenPitParamTradeAmountKind) 1)
/**
 * The value is settlement volume.
 */
#define OpenPitParamTradeAmountKind_Volume ((OpenPitParamTradeAmountKind) 2)
```

## `OpenPitParamRoundingStrategy`

Decimal rounding strategy for typed parameter constructors.

```c
typedef uint8_t OpenPitParamRoundingStrategy;
/**
 * Round half to nearest even number.
 */
#define OpenPitParamRoundingStrategy_MidpointNearestEven \
    ((OpenPitParamRoundingStrategy) 0)
/**
 * Round half away from zero.
 */
#define OpenPitParamRoundingStrategy_MidpointAwayFromZero \
    ((OpenPitParamRoundingStrategy) 1)
/**
 * Round towards positive infinity.
 */
#define OpenPitParamRoundingStrategy_Up ((OpenPitParamRoundingStrategy) 2)
/**
 * Round towards negative infinity.
 */
#define OpenPitParamRoundingStrategy_Down ((OpenPitParamRoundingStrategy) 3)
```

## `OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT`

Default rounding strategy alias.

```c
#define OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_MidpointNearestEven)
```

## `OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER`

Banker's rounding alias.

```c
#define OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_MidpointNearestEven)
```

## `OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT`

Conservative profit rounding alias.

```c
#define OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_Down)
```

## `OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS`

Conservative loss rounding alias.

```c
#define OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_Down)
```

## `OpenPitParamTradeAmount`

One trade-amount value plus its interpretation mode.

The numeric value is interpreted according to `kind`:

- `Quantity` means instrument quantity;
- `Volume` means settlement notional volume.

```c
typedef struct OpenPitParamTradeAmount {
    OpenPitParamDecimal value;
    OpenPitParamTradeAmountKind kind;
} OpenPitParamTradeAmount;
```

## `OpenPitTriBool`

Tri-state boolean value.

```c
typedef uint8_t OpenPitTriBool;
/**
 * Value is absent.
 */
#define OpenPitTriBool_NotSet ((OpenPitTriBool) 0)
/**
 * Boolean false.
 */
#define OpenPitTriBool_False ((OpenPitTriBool) 1)
/**
 * Boolean true.
 */
#define OpenPitTriBool_True ((OpenPitTriBool) 2)
```

## `OpenPitParamAdjustmentAmountKind`

Selects how an account-adjustment amount should be interpreted.

```c
typedef uint8_t OpenPitParamAdjustmentAmountKind;
/**
 * No amount is specified.
 */
#define OpenPitParamAdjustmentAmountKind_NotSet \
    ((OpenPitParamAdjustmentAmountKind) 0)
/**
 * Change current state by the supplied signed amount.
 */
#define OpenPitParamAdjustmentAmountKind_Delta \
    ((OpenPitParamAdjustmentAmountKind) 1)
/**
 * Set current state to the supplied signed amount.
 */
#define OpenPitParamAdjustmentAmountKind_Absolute \
    ((OpenPitParamAdjustmentAmountKind) 2)
```

## `OpenPitParamDecimal`

Decimal value represented as `mantissa * 10^-scale`.

```c
typedef struct OpenPitParamDecimal {
    int64_t mantissa_lo;
    int64_t mantissa_hi;
    int32_t scale;
} OpenPitParamDecimal;
```

## `OpenPitParamPnl`

Validated `Pnl` value wrapper.

```c
typedef struct OpenPitParamPnl {
    OpenPitParamDecimal _0;
} OpenPitParamPnl;
```

## `openpit_create_param_pnl`

Validates a decimal and returns a `Pnl` wrapper.

Meaning: Profit and loss value; positive means profit, negative means loss.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool openpit_create_param_pnl(
    OpenPitParamDecimal value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_get_decimal`

Returns the decimal stored in `Pnl`.

```c
OpenPitParamDecimal openpit_param_pnl_get_decimal(
    OpenPitParamPnl value
);
```

## `OpenPitParamPrice`

Validated `Price` value wrapper.

```c
typedef struct OpenPitParamPrice {
    OpenPitParamDecimal _0;
} OpenPitParamPrice;
```

## `openpit_create_param_price`

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
bool openpit_create_param_price(
    OpenPitParamDecimal value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_get_decimal`

Returns the decimal stored in `Price`.

```c
OpenPitParamDecimal openpit_param_price_get_decimal(
    OpenPitParamPrice value
);
```

## `OpenPitParamQuantity`

Validated `Quantity` value wrapper.

```c
typedef struct OpenPitParamQuantity {
    OpenPitParamDecimal _0;
} OpenPitParamQuantity;
```

## `openpit_create_param_quantity`

Validates a decimal and returns a `Quantity` wrapper.

Meaning: Instrument quantity; non-negative amount in instrument units.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool openpit_create_param_quantity(
    OpenPitParamDecimal value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_get_decimal`

Returns the decimal stored in `Quantity`.

```c
OpenPitParamDecimal openpit_param_quantity_get_decimal(
    OpenPitParamQuantity value
);
```

## `OpenPitParamVolume`

Validated `Volume` value wrapper.

```c
typedef struct OpenPitParamVolume {
    OpenPitParamDecimal _0;
} OpenPitParamVolume;
```

## `openpit_create_param_volume`

Validates a decimal and returns a `Volume` wrapper.

Meaning: Settlement notional volume; non-negative amount in settlement units.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool openpit_create_param_volume(
    OpenPitParamDecimal value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_get_decimal`

Returns the decimal stored in `Volume`.

```c
OpenPitParamDecimal openpit_param_volume_get_decimal(
    OpenPitParamVolume value
);
```

## `OpenPitParamCashFlow`

Validated `CashFlow` value wrapper.

```c
typedef struct OpenPitParamCashFlow {
    OpenPitParamDecimal _0;
} OpenPitParamCashFlow;
```

## `openpit_create_param_cash_flow`

Validates a decimal and returns a `CashFlow` wrapper.

Meaning: Cash flow contribution; positive is inflow, negative is outflow.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool openpit_create_param_cash_flow(
    OpenPitParamDecimal value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_get_decimal`

Returns the decimal stored in `CashFlow`.

```c
OpenPitParamDecimal openpit_param_cash_flow_get_decimal(
    OpenPitParamCashFlow value
);
```

## `OpenPitParamPositionSize`

Validated `PositionSize` value wrapper.

```c
typedef struct OpenPitParamPositionSize {
    OpenPitParamDecimal _0;
} OpenPitParamPositionSize;
```

## `openpit_create_param_position_size`

Validates a decimal and returns a `PositionSize` wrapper.

Meaning: Signed position size; long is positive, short is negative.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool openpit_create_param_position_size(
    OpenPitParamDecimal value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_get_decimal`

Returns the decimal stored in `PositionSize`.

```c
OpenPitParamDecimal openpit_param_position_size_get_decimal(
    OpenPitParamPositionSize value
);
```

## `OpenPitParamFee`

Validated `Fee` value wrapper.

```c
typedef struct OpenPitParamFee {
    OpenPitParamDecimal _0;
} OpenPitParamFee;
```

## `openpit_create_param_fee`

Validates a decimal and returns a `Fee` wrapper.

Meaning: Fee amount; can be negative for rebates or reconciliation adjustments.

Success:

- returns `true` and writes a validated wrapper to `out`.

Error:

- returns `false` when `out` is null or when the decimal does not satisfy the
  rules of this type;
- on error read `out_error` for the message.

```c
bool openpit_create_param_fee(
    OpenPitParamDecimal value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_get_decimal`

Returns the decimal stored in `Fee`.

```c
OpenPitParamDecimal openpit_param_fee_get_decimal(
    OpenPitParamFee value
);
```

## `OpenPitParamNotional`

Validated `Notional` value wrapper.

```c
typedef struct OpenPitParamNotional {
    OpenPitParamDecimal _0;
} OpenPitParamNotional;
```

## `openpit_create_param_notional`

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
bool openpit_create_param_notional(
    OpenPitParamDecimal value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_get_decimal`

Returns the decimal stored in `Notional`.

```c
OpenPitParamDecimal openpit_param_notional_get_decimal(
    OpenPitParamNotional value
);
```

## `openpit_create_param_pnl_from_string`

```c
bool openpit_create_param_pnl_from_string(
    OpenPitStringView value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_pnl_from_f64`

```c
bool openpit_create_param_pnl_from_f64(
    double value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_pnl_from_int64`

```c
bool openpit_create_param_pnl_from_int64(
    int64_t value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_pnl_from_uint64`

```c
bool openpit_create_param_pnl_from_uint64(
    uint64_t value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_pnl_from_string_rounded`

```c
bool openpit_create_param_pnl_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_pnl_from_f64_rounded`

```c
bool openpit_create_param_pnl_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_pnl_from_decimal_rounded`

```c
bool openpit_create_param_pnl_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_to_f64`

```c
bool openpit_param_pnl_to_f64(
    OpenPitParamPnl value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_is_zero`

```c
bool openpit_param_pnl_is_zero(
    OpenPitParamPnl value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_compare`

```c
bool openpit_param_pnl_compare(
    OpenPitParamPnl lhs,
    OpenPitParamPnl rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_to_string`

```c
OpenPitSharedString * openpit_param_pnl_to_string(
    OpenPitParamPnl value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_add`

```c
bool openpit_param_pnl_checked_add(
    OpenPitParamPnl lhs,
    OpenPitParamPnl rhs,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_sub`

```c
bool openpit_param_pnl_checked_sub(
    OpenPitParamPnl lhs,
    OpenPitParamPnl rhs,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_mul_i64`

```c
bool openpit_param_pnl_checked_mul_i64(
    OpenPitParamPnl value,
    int64_t multiplier,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_mul_u64`

```c
bool openpit_param_pnl_checked_mul_u64(
    OpenPitParamPnl value,
    uint64_t multiplier,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_mul_f64`

```c
bool openpit_param_pnl_checked_mul_f64(
    OpenPitParamPnl value,
    double multiplier,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_div_i64`

```c
bool openpit_param_pnl_checked_div_i64(
    OpenPitParamPnl value,
    int64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_div_u64`

```c
bool openpit_param_pnl_checked_div_u64(
    OpenPitParamPnl value,
    uint64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_div_f64`

```c
bool openpit_param_pnl_checked_div_f64(
    OpenPitParamPnl value,
    double divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_rem_i64`

```c
bool openpit_param_pnl_checked_rem_i64(
    OpenPitParamPnl value,
    int64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_rem_u64`

```c
bool openpit_param_pnl_checked_rem_u64(
    OpenPitParamPnl value,
    uint64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_rem_f64`

```c
bool openpit_param_pnl_checked_rem_f64(
    OpenPitParamPnl value,
    double divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_checked_neg`

```c
bool openpit_param_pnl_checked_neg(
    OpenPitParamPnl value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_price_from_string`

```c
bool openpit_create_param_price_from_string(
    OpenPitStringView value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_price_from_f64`

```c
bool openpit_create_param_price_from_f64(
    double value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_price_from_int64`

```c
bool openpit_create_param_price_from_int64(
    int64_t value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_price_from_uint64`

```c
bool openpit_create_param_price_from_uint64(
    uint64_t value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_price_from_string_rounded`

```c
bool openpit_create_param_price_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_price_from_f64_rounded`

```c
bool openpit_create_param_price_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_price_from_decimal_rounded`

```c
bool openpit_create_param_price_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_to_f64`

```c
bool openpit_param_price_to_f64(
    OpenPitParamPrice value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_is_zero`

```c
bool openpit_param_price_is_zero(
    OpenPitParamPrice value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_compare`

```c
bool openpit_param_price_compare(
    OpenPitParamPrice lhs,
    OpenPitParamPrice rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_to_string`

```c
OpenPitSharedString * openpit_param_price_to_string(
    OpenPitParamPrice value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_add`

```c
bool openpit_param_price_checked_add(
    OpenPitParamPrice lhs,
    OpenPitParamPrice rhs,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_sub`

```c
bool openpit_param_price_checked_sub(
    OpenPitParamPrice lhs,
    OpenPitParamPrice rhs,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_mul_i64`

```c
bool openpit_param_price_checked_mul_i64(
    OpenPitParamPrice value,
    int64_t multiplier,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_mul_u64`

```c
bool openpit_param_price_checked_mul_u64(
    OpenPitParamPrice value,
    uint64_t multiplier,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_mul_f64`

```c
bool openpit_param_price_checked_mul_f64(
    OpenPitParamPrice value,
    double multiplier,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_div_i64`

```c
bool openpit_param_price_checked_div_i64(
    OpenPitParamPrice value,
    int64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_div_u64`

```c
bool openpit_param_price_checked_div_u64(
    OpenPitParamPrice value,
    uint64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_div_f64`

```c
bool openpit_param_price_checked_div_f64(
    OpenPitParamPrice value,
    double divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_rem_i64`

```c
bool openpit_param_price_checked_rem_i64(
    OpenPitParamPrice value,
    int64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_rem_u64`

```c
bool openpit_param_price_checked_rem_u64(
    OpenPitParamPrice value,
    uint64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_rem_f64`

```c
bool openpit_param_price_checked_rem_f64(
    OpenPitParamPrice value,
    double divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_checked_neg`

```c
bool openpit_param_price_checked_neg(
    OpenPitParamPrice value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_quantity_from_string`

```c
bool openpit_create_param_quantity_from_string(
    OpenPitStringView value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_quantity_from_f64`

```c
bool openpit_create_param_quantity_from_f64(
    double value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_quantity_from_int64`

```c
bool openpit_create_param_quantity_from_int64(
    int64_t value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_quantity_from_uint64`

```c
bool openpit_create_param_quantity_from_uint64(
    uint64_t value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_quantity_from_string_rounded`

```c
bool openpit_create_param_quantity_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_quantity_from_f64_rounded`

```c
bool openpit_create_param_quantity_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_quantity_from_decimal_rounded`

```c
bool openpit_create_param_quantity_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_to_f64`

```c
bool openpit_param_quantity_to_f64(
    OpenPitParamQuantity value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_is_zero`

```c
bool openpit_param_quantity_is_zero(
    OpenPitParamQuantity value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_compare`

```c
bool openpit_param_quantity_compare(
    OpenPitParamQuantity lhs,
    OpenPitParamQuantity rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_to_string`

```c
OpenPitSharedString * openpit_param_quantity_to_string(
    OpenPitParamQuantity value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_add`

```c
bool openpit_param_quantity_checked_add(
    OpenPitParamQuantity lhs,
    OpenPitParamQuantity rhs,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_sub`

```c
bool openpit_param_quantity_checked_sub(
    OpenPitParamQuantity lhs,
    OpenPitParamQuantity rhs,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_mul_i64`

```c
bool openpit_param_quantity_checked_mul_i64(
    OpenPitParamQuantity value,
    int64_t multiplier,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_mul_u64`

```c
bool openpit_param_quantity_checked_mul_u64(
    OpenPitParamQuantity value,
    uint64_t multiplier,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_mul_f64`

```c
bool openpit_param_quantity_checked_mul_f64(
    OpenPitParamQuantity value,
    double multiplier,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_div_i64`

```c
bool openpit_param_quantity_checked_div_i64(
    OpenPitParamQuantity value,
    int64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_div_u64`

```c
bool openpit_param_quantity_checked_div_u64(
    OpenPitParamQuantity value,
    uint64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_div_f64`

```c
bool openpit_param_quantity_checked_div_f64(
    OpenPitParamQuantity value,
    double divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_rem_i64`

```c
bool openpit_param_quantity_checked_rem_i64(
    OpenPitParamQuantity value,
    int64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_rem_u64`

```c
bool openpit_param_quantity_checked_rem_u64(
    OpenPitParamQuantity value,
    uint64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_checked_rem_f64`

```c
bool openpit_param_quantity_checked_rem_f64(
    OpenPitParamQuantity value,
    double divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_volume_from_string`

```c
bool openpit_create_param_volume_from_string(
    OpenPitStringView value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_volume_from_f64`

```c
bool openpit_create_param_volume_from_f64(
    double value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_volume_from_int64`

```c
bool openpit_create_param_volume_from_int64(
    int64_t value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_volume_from_uint64`

```c
bool openpit_create_param_volume_from_uint64(
    uint64_t value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_volume_from_string_rounded`

```c
bool openpit_create_param_volume_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_volume_from_f64_rounded`

```c
bool openpit_create_param_volume_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_volume_from_decimal_rounded`

```c
bool openpit_create_param_volume_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_to_f64`

```c
bool openpit_param_volume_to_f64(
    OpenPitParamVolume value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_is_zero`

```c
bool openpit_param_volume_is_zero(
    OpenPitParamVolume value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_compare`

```c
bool openpit_param_volume_compare(
    OpenPitParamVolume lhs,
    OpenPitParamVolume rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_to_string`

```c
OpenPitSharedString * openpit_param_volume_to_string(
    OpenPitParamVolume value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_add`

```c
bool openpit_param_volume_checked_add(
    OpenPitParamVolume lhs,
    OpenPitParamVolume rhs,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_sub`

```c
bool openpit_param_volume_checked_sub(
    OpenPitParamVolume lhs,
    OpenPitParamVolume rhs,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_mul_i64`

```c
bool openpit_param_volume_checked_mul_i64(
    OpenPitParamVolume value,
    int64_t multiplier,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_mul_u64`

```c
bool openpit_param_volume_checked_mul_u64(
    OpenPitParamVolume value,
    uint64_t multiplier,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_mul_f64`

```c
bool openpit_param_volume_checked_mul_f64(
    OpenPitParamVolume value,
    double multiplier,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_div_i64`

```c
bool openpit_param_volume_checked_div_i64(
    OpenPitParamVolume value,
    int64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_div_u64`

```c
bool openpit_param_volume_checked_div_u64(
    OpenPitParamVolume value,
    uint64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_div_f64`

```c
bool openpit_param_volume_checked_div_f64(
    OpenPitParamVolume value,
    double divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_rem_i64`

```c
bool openpit_param_volume_checked_rem_i64(
    OpenPitParamVolume value,
    int64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_rem_u64`

```c
bool openpit_param_volume_checked_rem_u64(
    OpenPitParamVolume value,
    uint64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_checked_rem_f64`

```c
bool openpit_param_volume_checked_rem_f64(
    OpenPitParamVolume value,
    double divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_cash_flow_from_string`

```c
bool openpit_create_param_cash_flow_from_string(
    OpenPitStringView value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_cash_flow_from_f64`

```c
bool openpit_create_param_cash_flow_from_f64(
    double value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_cash_flow_from_int64`

```c
bool openpit_create_param_cash_flow_from_int64(
    int64_t value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_cash_flow_from_uint64`

```c
bool openpit_create_param_cash_flow_from_uint64(
    uint64_t value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_cash_flow_from_string_rounded`

```c
bool openpit_create_param_cash_flow_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_cash_flow_from_f64_rounded`

```c
bool openpit_create_param_cash_flow_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_cash_flow_from_decimal_rounded`

```c
bool openpit_create_param_cash_flow_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_to_f64`

```c
bool openpit_param_cash_flow_to_f64(
    OpenPitParamCashFlow value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_is_zero`

```c
bool openpit_param_cash_flow_is_zero(
    OpenPitParamCashFlow value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_compare`

```c
bool openpit_param_cash_flow_compare(
    OpenPitParamCashFlow lhs,
    OpenPitParamCashFlow rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_to_string`

```c
OpenPitSharedString * openpit_param_cash_flow_to_string(
    OpenPitParamCashFlow value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_add`

```c
bool openpit_param_cash_flow_checked_add(
    OpenPitParamCashFlow lhs,
    OpenPitParamCashFlow rhs,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_sub`

```c
bool openpit_param_cash_flow_checked_sub(
    OpenPitParamCashFlow lhs,
    OpenPitParamCashFlow rhs,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_mul_i64`

```c
bool openpit_param_cash_flow_checked_mul_i64(
    OpenPitParamCashFlow value,
    int64_t multiplier,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_mul_u64`

```c
bool openpit_param_cash_flow_checked_mul_u64(
    OpenPitParamCashFlow value,
    uint64_t multiplier,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_mul_f64`

```c
bool openpit_param_cash_flow_checked_mul_f64(
    OpenPitParamCashFlow value,
    double multiplier,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_div_i64`

```c
bool openpit_param_cash_flow_checked_div_i64(
    OpenPitParamCashFlow value,
    int64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_div_u64`

```c
bool openpit_param_cash_flow_checked_div_u64(
    OpenPitParamCashFlow value,
    uint64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_div_f64`

```c
bool openpit_param_cash_flow_checked_div_f64(
    OpenPitParamCashFlow value,
    double divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_rem_i64`

```c
bool openpit_param_cash_flow_checked_rem_i64(
    OpenPitParamCashFlow value,
    int64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_rem_u64`

```c
bool openpit_param_cash_flow_checked_rem_u64(
    OpenPitParamCashFlow value,
    uint64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_rem_f64`

```c
bool openpit_param_cash_flow_checked_rem_f64(
    OpenPitParamCashFlow value,
    double divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_checked_neg`

```c
bool openpit_param_cash_flow_checked_neg(
    OpenPitParamCashFlow value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_position_size_from_string`

```c
bool openpit_create_param_position_size_from_string(
    OpenPitStringView value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_position_size_from_f64`

```c
bool openpit_create_param_position_size_from_f64(
    double value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_position_size_from_int64`

```c
bool openpit_create_param_position_size_from_int64(
    int64_t value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_position_size_from_uint64`

```c
bool openpit_create_param_position_size_from_uint64(
    uint64_t value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_position_size_from_string_rounded`

```c
bool openpit_create_param_position_size_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_position_size_from_f64_rounded`

```c
bool openpit_create_param_position_size_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_position_size_from_decimal_rounded`

```c
bool openpit_create_param_position_size_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_to_f64`

```c
bool openpit_param_position_size_to_f64(
    OpenPitParamPositionSize value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_is_zero`

```c
bool openpit_param_position_size_is_zero(
    OpenPitParamPositionSize value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_compare`

```c
bool openpit_param_position_size_compare(
    OpenPitParamPositionSize lhs,
    OpenPitParamPositionSize rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_to_string`

```c
OpenPitSharedString * openpit_param_position_size_to_string(
    OpenPitParamPositionSize value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_add`

```c
bool openpit_param_position_size_checked_add(
    OpenPitParamPositionSize lhs,
    OpenPitParamPositionSize rhs,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_sub`

```c
bool openpit_param_position_size_checked_sub(
    OpenPitParamPositionSize lhs,
    OpenPitParamPositionSize rhs,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_mul_i64`

```c
bool openpit_param_position_size_checked_mul_i64(
    OpenPitParamPositionSize value,
    int64_t multiplier,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_mul_u64`

```c
bool openpit_param_position_size_checked_mul_u64(
    OpenPitParamPositionSize value,
    uint64_t multiplier,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_mul_f64`

```c
bool openpit_param_position_size_checked_mul_f64(
    OpenPitParamPositionSize value,
    double multiplier,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_div_i64`

```c
bool openpit_param_position_size_checked_div_i64(
    OpenPitParamPositionSize value,
    int64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_div_u64`

```c
bool openpit_param_position_size_checked_div_u64(
    OpenPitParamPositionSize value,
    uint64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_div_f64`

```c
bool openpit_param_position_size_checked_div_f64(
    OpenPitParamPositionSize value,
    double divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_rem_i64`

```c
bool openpit_param_position_size_checked_rem_i64(
    OpenPitParamPositionSize value,
    int64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_rem_u64`

```c
bool openpit_param_position_size_checked_rem_u64(
    OpenPitParamPositionSize value,
    uint64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_rem_f64`

```c
bool openpit_param_position_size_checked_rem_f64(
    OpenPitParamPositionSize value,
    double divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_neg`

```c
bool openpit_param_position_size_checked_neg(
    OpenPitParamPositionSize value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_fee_from_string`

```c
bool openpit_create_param_fee_from_string(
    OpenPitStringView value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_fee_from_f64`

```c
bool openpit_create_param_fee_from_f64(
    double value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_fee_from_int64`

```c
bool openpit_create_param_fee_from_int64(
    int64_t value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_fee_from_uint64`

```c
bool openpit_create_param_fee_from_uint64(
    uint64_t value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_fee_from_string_rounded`

```c
bool openpit_create_param_fee_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_fee_from_f64_rounded`

```c
bool openpit_create_param_fee_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_fee_from_decimal_rounded`

```c
bool openpit_create_param_fee_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_to_f64`

```c
bool openpit_param_fee_to_f64(
    OpenPitParamFee value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_is_zero`

```c
bool openpit_param_fee_is_zero(
    OpenPitParamFee value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_compare`

```c
bool openpit_param_fee_compare(
    OpenPitParamFee lhs,
    OpenPitParamFee rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_to_string`

```c
OpenPitSharedString * openpit_param_fee_to_string(
    OpenPitParamFee value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_add`

```c
bool openpit_param_fee_checked_add(
    OpenPitParamFee lhs,
    OpenPitParamFee rhs,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_sub`

```c
bool openpit_param_fee_checked_sub(
    OpenPitParamFee lhs,
    OpenPitParamFee rhs,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_mul_i64`

```c
bool openpit_param_fee_checked_mul_i64(
    OpenPitParamFee value,
    int64_t multiplier,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_mul_u64`

```c
bool openpit_param_fee_checked_mul_u64(
    OpenPitParamFee value,
    uint64_t multiplier,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_mul_f64`

```c
bool openpit_param_fee_checked_mul_f64(
    OpenPitParamFee value,
    double multiplier,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_div_i64`

```c
bool openpit_param_fee_checked_div_i64(
    OpenPitParamFee value,
    int64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_div_u64`

```c
bool openpit_param_fee_checked_div_u64(
    OpenPitParamFee value,
    uint64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_div_f64`

```c
bool openpit_param_fee_checked_div_f64(
    OpenPitParamFee value,
    double divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_rem_i64`

```c
bool openpit_param_fee_checked_rem_i64(
    OpenPitParamFee value,
    int64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_rem_u64`

```c
bool openpit_param_fee_checked_rem_u64(
    OpenPitParamFee value,
    uint64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_rem_f64`

```c
bool openpit_param_fee_checked_rem_f64(
    OpenPitParamFee value,
    double divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_checked_neg`

```c
bool openpit_param_fee_checked_neg(
    OpenPitParamFee value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_notional_from_string`

```c
bool openpit_create_param_notional_from_string(
    OpenPitStringView value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_notional_from_f64`

```c
bool openpit_create_param_notional_from_f64(
    double value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_notional_from_int64`

```c
bool openpit_create_param_notional_from_int64(
    int64_t value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_notional_from_uint64`

```c
bool openpit_create_param_notional_from_uint64(
    uint64_t value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_notional_from_string_rounded`

```c
bool openpit_create_param_notional_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_notional_from_f64_rounded`

```c
bool openpit_create_param_notional_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_notional_from_decimal_rounded`

```c
bool openpit_create_param_notional_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_to_f64`

```c
bool openpit_param_notional_to_f64(
    OpenPitParamNotional value,
    double * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_is_zero`

```c
bool openpit_param_notional_is_zero(
    OpenPitParamNotional value,
    bool * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_compare`

```c
bool openpit_param_notional_compare(
    OpenPitParamNotional lhs,
    OpenPitParamNotional rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_to_string`

```c
OpenPitSharedString * openpit_param_notional_to_string(
    OpenPitParamNotional value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_add`

```c
bool openpit_param_notional_checked_add(
    OpenPitParamNotional lhs,
    OpenPitParamNotional rhs,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_sub`

```c
bool openpit_param_notional_checked_sub(
    OpenPitParamNotional lhs,
    OpenPitParamNotional rhs,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_mul_i64`

```c
bool openpit_param_notional_checked_mul_i64(
    OpenPitParamNotional value,
    int64_t multiplier,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_mul_u64`

```c
bool openpit_param_notional_checked_mul_u64(
    OpenPitParamNotional value,
    uint64_t multiplier,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_mul_f64`

```c
bool openpit_param_notional_checked_mul_f64(
    OpenPitParamNotional value,
    double multiplier,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_div_i64`

```c
bool openpit_param_notional_checked_div_i64(
    OpenPitParamNotional value,
    int64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_div_u64`

```c
bool openpit_param_notional_checked_div_u64(
    OpenPitParamNotional value,
    uint64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_div_f64`

```c
bool openpit_param_notional_checked_div_f64(
    OpenPitParamNotional value,
    double divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_rem_i64`

```c
bool openpit_param_notional_checked_rem_i64(
    OpenPitParamNotional value,
    int64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_rem_u64`

```c
bool openpit_param_notional_checked_rem_u64(
    OpenPitParamNotional value,
    uint64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_checked_rem_f64`

```c
bool openpit_param_notional_checked_rem_f64(
    OpenPitParamNotional value,
    double divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `OpenPitParamNotionalOptional`

```c
typedef struct OpenPitParamNotionalOptional {
    OpenPitParamNotional value;
    bool is_set;
} OpenPitParamNotionalOptional;
```

## `OpenPitParamPnlOptional`

```c
typedef struct OpenPitParamPnlOptional {
    OpenPitParamPnl value;
    bool is_set;
} OpenPitParamPnlOptional;
```

## `OpenPitParamPriceOptional`

```c
typedef struct OpenPitParamPriceOptional {
    OpenPitParamPrice value;
    bool is_set;
} OpenPitParamPriceOptional;
```

## `OpenPitParamQuantityOptional`

```c
typedef struct OpenPitParamQuantityOptional {
    OpenPitParamQuantity value;
    bool is_set;
} OpenPitParamQuantityOptional;
```

## `OpenPitParamVolumeOptional`

```c
typedef struct OpenPitParamVolumeOptional {
    OpenPitParamVolume value;
    bool is_set;
} OpenPitParamVolumeOptional;
```

## `OpenPitParamCashFlowOptional`

```c
typedef struct OpenPitParamCashFlowOptional {
    OpenPitParamCashFlow value;
    bool is_set;
} OpenPitParamCashFlowOptional;
```

## `OpenPitParamPositionSizeOptional`

```c
typedef struct OpenPitParamPositionSizeOptional {
    OpenPitParamPositionSize value;
    bool is_set;
} OpenPitParamPositionSizeOptional;
```

## `OpenPitParamFeeOptional`

```c
typedef struct OpenPitParamFeeOptional {
    OpenPitParamFee value;
    bool is_set;
} OpenPitParamFeeOptional;
```

## `OpenPitParamAccountIdOptional`

```c
typedef struct OpenPitParamAccountIdOptional {
    OpenPitParamAccountId value;
    bool is_set;
} OpenPitParamAccountIdOptional;
```

## `openpit_param_leverage_calculate_margin_required`

```c
bool openpit_param_leverage_calculate_margin_required(
    OpenPitParamLeverage leverage,
    OpenPitParamNotional notional,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_calculate_volume`

```c
bool openpit_param_price_calculate_volume(
    OpenPitParamPrice price,
    OpenPitParamQuantity quantity,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_calculate_position_size`

```c
bool openpit_param_price_calculate_position_size(
    OpenPitParamPrice price,
    OpenPitParamQuantity quantity,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_calculate_volume`

```c
bool openpit_param_quantity_calculate_volume(
    OpenPitParamQuantity quantity,
    OpenPitParamPrice price,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_calculate_quantity`

```c
bool openpit_param_volume_calculate_quantity(
    OpenPitParamVolume volume,
    OpenPitParamPrice price,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_to_cash_flow`

```c
bool openpit_param_pnl_to_cash_flow(
    OpenPitParamPnl value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_to_position_size`

```c
bool openpit_param_pnl_to_position_size(
    OpenPitParamPnl value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_to_position_size`

```c
bool openpit_param_quantity_to_position_size(
    OpenPitParamQuantity value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_to_position_size`

```c
bool openpit_param_volume_to_position_size(
    OpenPitParamVolume value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_pnl_from_fee`

```c
bool openpit_param_pnl_from_fee(
    OpenPitParamFee fee,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_from_pnl`

```c
bool openpit_param_cash_flow_from_pnl(
    OpenPitParamPnl pnl,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_from_fee`

```c
bool openpit_param_cash_flow_from_fee(
    OpenPitParamFee fee,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_from_volume_inflow`

```c
bool openpit_param_cash_flow_from_volume_inflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_cash_flow_from_volume_outflow`

```c
bool openpit_param_cash_flow_from_volume_outflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_to_pnl`

```c
bool openpit_param_fee_to_pnl(
    OpenPitParamFee fee,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_to_position_size`

```c
bool openpit_param_fee_to_position_size(
    OpenPitParamFee fee,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_fee_to_cash_flow`

```c
bool openpit_param_fee_to_cash_flow(
    OpenPitParamFee fee,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_to_cash_flow_inflow`

```c
bool openpit_param_volume_to_cash_flow_inflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_to_cash_flow_outflow`

```c
bool openpit_param_volume_to_cash_flow_outflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_from_pnl`

```c
bool openpit_param_position_size_from_pnl(
    OpenPitParamPnl pnl,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_from_fee`

```c
bool openpit_param_position_size_from_fee(
    OpenPitParamFee fee,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_from_quantity_and_side`

```c
bool openpit_param_position_size_from_quantity_and_side(
    OpenPitParamQuantity quantity,
    OpenPitParamSide side,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_to_open_quantity`

```c
bool openpit_param_position_size_to_open_quantity(
    OpenPitParamPositionSize value,
    OpenPitParamQuantity * out_quantity,
    OpenPitParamSide * out_side,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_to_close_quantity`

```c
bool openpit_param_position_size_to_close_quantity(
    OpenPitParamPositionSize value,
    OpenPitParamQuantity * out_quantity,
    OpenPitParamSide * out_side,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_size_checked_add_quantity`

```c
bool openpit_param_position_size_checked_add_quantity(
    OpenPitParamPositionSize value,
    OpenPitParamQuantity quantity,
    OpenPitParamSide side,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_price_calculate_notional`

```c
bool openpit_param_price_calculate_notional(
    OpenPitParamPrice price,
    OpenPitParamQuantity quantity,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_quantity_calculate_notional`

```c
bool openpit_param_quantity_calculate_notional(
    OpenPitParamQuantity quantity,
    OpenPitParamPrice price,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_from_volume`

```c
bool openpit_param_notional_from_volume(
    OpenPitParamVolume volume,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_to_volume`

```c
bool openpit_param_notional_to_volume(
    OpenPitParamNotional notional,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_notional_calculate_margin_required`

```c
bool openpit_param_notional_calculate_margin_required(
    OpenPitParamNotional notional,
    OpenPitParamLeverage leverage,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);
```

## `openpit_param_volume_from_notional`

```c
bool openpit_param_volume_from_notional(
    OpenPitParamNotional notional,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_account_id_from_uint64`

Constructs an account identifier from a 64-bit integer.

This is a direct numeric mapping with no collision risk.

WARNING: Do not mix IDs produced by this function with IDs produced by
`openpit_create_param_account_id_from_string` in the same runtime state.

Contract:

- returns a stable account identifier value;
- this function always succeeds.

```c
OpenPitParamAccountId openpit_create_param_account_id_from_uint64(
    uint64_t value
);
```

## `openpit_create_param_account_id_from_string`

Constructs an account identifier from a UTF-8 byte sequence.

The bytes are read only for the duration of the call. No trailing zero byte is
required.

Collision note:

- different account strings can map to the same identifier;
- for `n` distinct account strings the probability of at least one collision
  is approximately `n^2 / 2^65`.
- if collision risk is unacceptable, keep your own collision-free
  string-to-integer mapping and use
  `openpit_create_param_account_id_from_uint64`.

The previous sentence is why this helper is suitable for stable adapter-side
mapping, but not for workflows that require guaranteed uniqueness.

WARNING: Do not mix IDs produced by this function with IDs produced by
`openpit_create_param_account_id_from_uint64` in the same runtime state.

Contract:

- returns `true` and writes a stable account identifier to `out` on success;
- returns `false` on invalid input and optionally writes `OpenPitParamError`.

### Safety

`value.ptr` must be non-null and point to at least `value.len` readable UTF-8
bytes.

```c
bool openpit_create_param_account_id_from_string(
    OpenPitStringView value,
    OpenPitParamAccountId * out,
    OpenPitOutParamError out_error
);
```

## `openpit_create_param_asset_from_string`

Validates and copies an asset identifier into a caller-owned shared-string
handle.

The returned handle must be destroyed with `openpit_destroy_param_asset`.

```c
OpenPitSharedString * openpit_create_param_asset_from_string(
    OpenPitStringView value,
    OpenPitOutParamError out_error
);
```

## `openpit_destroy_param_asset`

Destroys a caller-owned asset handle created by
`openpit_create_param_asset_from_string`.

```c
void openpit_destroy_param_asset(
    OpenPitSharedString * handle
);
```

## `openpit_param_side_to_string`

Renders an order side into a caller-owned shared string.

Returns null and writes `out_error` when the side is not set.

```c
OpenPitSharedString * openpit_param_side_to_string(
    OpenPitParamSide value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_side_to_string`

Renders a position side into a caller-owned shared string.

Returns null and writes `out_error` when the position side is not set.

```c
OpenPitSharedString * openpit_param_position_side_to_string(
    OpenPitParamPositionSide value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_effect_to_string`

Renders a position effect into a caller-owned shared string.

Returns null and writes `out_error` when the position effect is not set.

```c
OpenPitSharedString * openpit_param_position_effect_to_string(
    OpenPitParamPositionEffect value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_position_mode_to_string`

Renders a position mode into a caller-owned shared string.

Returns null and writes `out_error` when the position mode is not set.

```c
OpenPitSharedString * openpit_param_position_mode_to_string(
    OpenPitParamPositionMode value,
    OpenPitOutParamError out_error
);
```

## `openpit_param_account_id_to_string`

Renders an account identifier into a caller-owned shared string.

This conversion always succeeds.

```c
OpenPitSharedString * openpit_param_account_id_to_string(
    OpenPitParamAccountId value
);
```

## `openpit_param_trade_amount_to_string`

Renders a trade amount into a caller-owned shared string.

Returns null and writes `out_error` when the trade amount is not set or its
numeric value cannot be decoded.

```c
OpenPitSharedString * openpit_param_trade_amount_to_string(
    OpenPitParamTradeAmount value,
    OpenPitOutParamError out_error
);
```

## `OpenPitParamErrorCode`

Parameter error code transported through FFI.

```c
typedef uint32_t OpenPitParamErrorCode;
/**
 * Error code is not specified.
 */
#define OpenPitParamErrorCode_Unspecified ((OpenPitParamErrorCode) 0)
/**
 * Value must be non-negative.
 */
#define OpenPitParamErrorCode_Negative ((OpenPitParamErrorCode) 1)
/**
 * Division by zero.
 */
#define OpenPitParamErrorCode_DivisionByZero ((OpenPitParamErrorCode) 2)
/**
 * Arithmetic overflow.
 */
#define OpenPitParamErrorCode_Overflow ((OpenPitParamErrorCode) 3)
/**
 * Arithmetic underflow.
 */
#define OpenPitParamErrorCode_Underflow ((OpenPitParamErrorCode) 4)
/**
 * Invalid float value.
 */
#define OpenPitParamErrorCode_InvalidFloat ((OpenPitParamErrorCode) 5)
/**
 * Invalid textual format.
 */
#define OpenPitParamErrorCode_InvalidFormat ((OpenPitParamErrorCode) 6)
/**
 * Invalid price value.
 */
#define OpenPitParamErrorCode_InvalidPrice ((OpenPitParamErrorCode) 7)
/**
 * Invalid leverage value.
 */
#define OpenPitParamErrorCode_InvalidLeverage ((OpenPitParamErrorCode) 8)
/**
 * Asset identifier is empty.
 */
#define OpenPitParamErrorCode_AssetEmpty ((OpenPitParamErrorCode) 9)
/**
 * Account identifier string is empty.
 */
#define OpenPitParamErrorCode_AccountIdEmpty ((OpenPitParamErrorCode) 10)
/**
 * Catch-all code for unknown cases.
 */
#define OpenPitParamErrorCode_Other ((OpenPitParamErrorCode) 4294967295)
```

## `OpenPitParamError`

Caller-owned parameter error container.

```c
typedef struct OpenPitParamError {
    OpenPitParamErrorCode code;
    OpenPitSharedString * message;
} OpenPitParamError;
```

## `OpenPitOutParamError`

Parameter error out-pointer used by fallible param FFI calls.

```c
typedef OpenPitParamError ** OpenPitOutParamError;
```

## `openpit_destroy_param_error`

Releases a caller-owned parameter error container.

### Safety

`handle` must be either null or a pointer returned by this library through
`OpenPitOutParamError`. The handle must be destroyed at most once.

```c
void openpit_destroy_param_error(
    OpenPitParamError * handle
);
```
