# Execution Reports

[Back to index](index.md)

## `PitExecutionReportOperation`

Populated operation-identification group for an execution report.

```c
typedef struct PitExecutionReportOperation {
    PitInstrument instrument;
    PitParamAccountIdOptional account_id;
    PitParamSide side;
} PitExecutionReportOperation;
```

## `PitFinancialImpact`

Populated financial-impact group for an execution report.

```c
typedef struct PitFinancialImpact {
    PitParamPnlOptional pnl;
    PitParamFeeOptional fee;
} PitFinancialImpact;
```

## `PitExecutionReportTrade`

Fill trade payload (`price + quantity`) for execution reports.

```c
typedef struct PitExecutionReportTrade {
    PitParamPrice price;
    PitParamQuantity quantity;
} PitExecutionReportTrade;
```

## `PitExecutionReportFill`

Populated fill-details group for an execution report.

```c
typedef struct PitExecutionReportFill {
    PitExecutionReportTradeOptional last_trade;
    PitParamQuantityOptional leaves_quantity;
    PitParamPriceOptional lock_price;
    PitExecutionReportIsFinalOptional is_final;
} PitExecutionReportFill;
```

## `PitExecutionReportPositionImpact`

Populated position-impact group for an execution report.

```c
typedef struct PitExecutionReportPositionImpact {
    PitParamPositionEffect position_effect;
    PitParamPositionSide position_side;
} PitExecutionReportPositionImpact;
```

## `PitExecutionReportOperationOptional`

```c
typedef struct PitExecutionReportOperationOptional {
    PitExecutionReportOperation value;
    bool is_set;
} PitExecutionReportOperationOptional;
```

## `PitFinancialImpactOptional`

```c
typedef struct PitFinancialImpactOptional {
    PitFinancialImpact value;
    bool is_set;
} PitFinancialImpactOptional;
```

## `PitExecutionReportTradeOptional`

```c
typedef struct PitExecutionReportTradeOptional {
    PitExecutionReportTrade value;
    bool is_set;
} PitExecutionReportTradeOptional;
```

## `PitExecutionReportIsFinalOptional`

```c
typedef struct PitExecutionReportIsFinalOptional {
    bool value;
    bool is_set;
} PitExecutionReportIsFinalOptional;
```

## `PitExecutionReportFillOptional`

```c
typedef struct PitExecutionReportFillOptional {
    PitExecutionReportFill value;
    bool is_set;
} PitExecutionReportFillOptional;
```

## `PitExecutionReportPositionImpactOptional`

```c
typedef struct PitExecutionReportPositionImpactOptional {
    PitExecutionReportPositionImpact value;
    bool is_set;
} PitExecutionReportPositionImpactOptional;
```

## `PitExecutionReport`

Full caller-owned execution-report payload.

```c
typedef struct PitExecutionReport {
    PitExecutionReportOperationOptional operation;
    PitFinancialImpactOptional financial_impact;
    PitExecutionReportFillOptional fill;
    PitExecutionReportPositionImpactOptional position_impact;
    void * user_data;
} PitExecutionReport;
```

## `PitPretradePostTradeResult`

Aggregated post-trade processing result.

```c
typedef struct PitPretradePostTradeResult {
    bool kill_switch_triggered;
} PitPretradePostTradeResult;
```
