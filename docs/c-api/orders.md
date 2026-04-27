# Orders

[Back to index](index.md)

## `PitOrderOperation`

Optional operation group for an order.

The group is absent when all fields are absent.

```c
typedef struct PitOrderOperation {
    PitParamTradeAmount trade_amount;
    PitInstrument instrument;
    PitParamPriceOptional price;
    PitParamAccountIdOptional account_id;
    PitParamSide side;
} PitOrderOperation;
```

## `PitOrderPosition`

Optional position-management group for an order.

The group is absent when every field is `NotSet`.

```c
typedef struct PitOrderPosition {
    PitParamPositionSide position_side;
    PitTriBool reduce_only;
    PitTriBool close_position;
} PitOrderPosition;
```

## `PitOrderMargin`

Optional margin group for an order.

The group is absent when every field is `NotSet`.

```c
typedef struct PitOrderMargin {
    PitStringView collateral_asset;
    PitTriBool auto_borrow;
    PitParamLeverage leverage;
} PitOrderMargin;
```

## `PitOrder`

Full caller-owned order payload.

```c
typedef struct PitOrder {
    PitOrderOperationOptional operation;
    PitOrderMarginOptional margin;
    PitOrderPositionOptional position;
    void * user_data;
} PitOrder;
```

## `PitOrderOperationOptional`

```c
typedef struct PitOrderOperationOptional {
    PitOrderOperation value;
    bool is_set;
} PitOrderOperationOptional;
```

## `PitOrderMarginOptional`

```c
typedef struct PitOrderMarginOptional {
    PitOrderMargin value;
    bool is_set;
} PitOrderMarginOptional;
```

## `PitOrderPositionOptional`

```c
typedef struct PitOrderPositionOptional {
    PitOrderPosition value;
    bool is_set;
} PitOrderPositionOptional;
```
