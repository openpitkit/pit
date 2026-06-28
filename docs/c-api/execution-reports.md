# Execution Reports

<!-- markdownlint-disable MD013 MD024 -->

[Back to index](index.md)

## `OpenPitExecutionReportOperation`

Populated operation-identification group for an execution report.

```c
typedef struct OpenPitExecutionReportOperation {
    OpenPitInstrument instrument;
    OpenPitParamAccountIdOptional account_id;
    OpenPitParamSide side;
} OpenPitExecutionReportOperation;
```

## `OpenPitFinancialImpact`

Populated financial-impact group for an execution report.

```c
typedef struct OpenPitFinancialImpact {
    OpenPitParamPnlOptional pnl;
    OpenPitParamFeeOptional fee;
} OpenPitFinancialImpact;
```

## `OpenPitExecutionReportTrade`

Fill trade payload (`price + quantity`) for execution reports.

```c
typedef struct OpenPitExecutionReportTrade {
    OpenPitParamPrice price;
    OpenPitParamQuantity quantity;
} OpenPitExecutionReportTrade;
```

## `OpenPitExecutionReportFill`

Populated fill-details group for an execution report.

```c
typedef struct OpenPitExecutionReportFill {
    OpenPitExecutionReportTradeOptional last_trade;
    OpenPitParamMonetaryAmountOptional fee;
    OpenPitParamQuantityOptional leaves_quantity;
    const OpenPitPretradePreTradeLock * lock;
    OpenPitExecutionReportIsFinalOptional is_final;
} OpenPitExecutionReportFill;
```

## `OpenPitExecutionReportPositionImpact`

Populated position-impact group for an execution report.

```c
typedef struct OpenPitExecutionReportPositionImpact {
    OpenPitParamPositionEffect position_effect;
    OpenPitParamPositionSide position_side;
} OpenPitExecutionReportPositionImpact;
```

## `OpenPitExecutionReportOperationOptional`

```c
typedef struct OpenPitExecutionReportOperationOptional {
    OpenPitExecutionReportOperation value;
    bool is_set;
} OpenPitExecutionReportOperationOptional;
```

## `OpenPitFinancialImpactOptional`

```c
typedef struct OpenPitFinancialImpactOptional {
    OpenPitFinancialImpact value;
    bool is_set;
} OpenPitFinancialImpactOptional;
```

## `OpenPitExecutionReportTradeOptional`

```c
typedef struct OpenPitExecutionReportTradeOptional {
    OpenPitExecutionReportTrade value;
    bool is_set;
} OpenPitExecutionReportTradeOptional;
```

## `OpenPitExecutionReportIsFinalOptional`

```c
typedef struct OpenPitExecutionReportIsFinalOptional {
    bool value;
    bool is_set;
} OpenPitExecutionReportIsFinalOptional;
```

## `OpenPitExecutionReportFillOptional`

```c
typedef struct OpenPitExecutionReportFillOptional {
    OpenPitExecutionReportFill value;
    bool is_set;
} OpenPitExecutionReportFillOptional;
```

## `OpenPitExecutionReportPositionImpactOptional`

```c
typedef struct OpenPitExecutionReportPositionImpactOptional {
    OpenPitExecutionReportPositionImpact value;
    bool is_set;
} OpenPitExecutionReportPositionImpactOptional;
```

## `OpenPitExecutionReport`

Full caller-owned execution-report payload.

```c
typedef struct OpenPitExecutionReport {
    OpenPitExecutionReportOperationOptional operation;
    OpenPitFinancialImpactOptional financial_impact;
    OpenPitExecutionReportFillOptional fill;
    OpenPitExecutionReportPositionImpactOptional position_impact;
    void * user_data;
} OpenPitExecutionReport;
```
