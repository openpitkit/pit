# Examples

The examples on this page are adapted from the runnable integration tests and
wiki-backed examples in the repository. Each snippet is intended to be copied
into a Python file after installing `openpit`.

<!-- markdownlint-disable MD013 -->

| Example | Scenario | Source |
| --- | --- | --- |
| [Full order lifecycle](#full-order-lifecycle) | Configure policies, reserve, commit, and apply a report | [`test_examples_readme.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_readme.py) |
| [Domain value types](#domain-value-types) | Build validated financial values | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Directional helpers](#directional-helpers) | Use side and position-side enums | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Leverage values](#leverage-values) | Construct leverage multipliers | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Start-stage reject handling](#start-stage-reject-handling) | Inspect start-stage rejects | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Main-stage finalization](#main-stage-finalization) | Execute a request and commit or inspect rejects | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Shortcut flow](#shortcut-flow) | Run start and main stages together | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Account-adjustment batch](#account-adjustment-batch) | Apply balance and position adjustments | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Account-adjustment policy](#account-adjustment-policy) | Validate account adjustments with a custom policy | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Rollback-safe main policy](#rollback-safe-main-policy) | Register a mutation and rely on rollback | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Custom notional-cap policy](#custom-notional-cap-policy) | Reject orders above a strategy cap | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |
| [Custom order models](#custom-order-models) | Preserve project metadata through callbacks | [`test_examples_wiki.py`](https://github.com/openpitkit/pit/blob/main/bindings/python/tests/integration/test_examples_wiki.py) |

<!-- markdownlint-enable MD013 -->

## Full order lifecycle

Use this when integrating OpenPit around a real order-submission path. The
reservation is committed only after the downstream send succeeds.

```python
import openpit


def send_order_to_venue(order: openpit.Order) -> None:
    _ = order


pnl_policy = openpit.pretrade.policies.PnlBoundsKillSwitchPolicy(
    settlement_asset="USD",
    lower_bound=openpit.param.Pnl("-1000"),
    initial_pnl=openpit.param.Pnl("0"),
)
rate_limit_policy = openpit.pretrade.policies.RateLimitPolicy(
    max_orders=100,
    window_seconds=1,
)
size_policy = openpit.pretrade.policies.OrderSizeLimitPolicy(
    limit=openpit.pretrade.policies.OrderSizeLimit(
        settlement_asset="USD",
        max_quantity=openpit.param.Quantity("500"),
        max_notional=openpit.param.Volume("100000"),
    ),
)

engine = (
    openpit.Engine.builder()
    .check_pre_trade_start_policy(
        policy=openpit.pretrade.policies.OrderValidationPolicy(),
    )
    .check_pre_trade_start_policy(policy=pnl_policy)
    .check_pre_trade_start_policy(policy=rate_limit_policy)
    .check_pre_trade_start_policy(policy=size_policy)
    .build()
)

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity("100"),
        price=openpit.param.Price("185"),
    ),
)

result = engine.execute_pre_trade(order=order)
if not result:
    raise RuntimeError(result.rejects[0].reason)

try:
    send_order_to_venue(order)
except Exception:
    result.reservation.rollback()
    raise
else:
    result.reservation.commit()

report = openpit.ExecutionReport(
    operation=openpit.ExecutionReportOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
    ),
    financial_impact=openpit.FinancialImpact(
        pnl=openpit.param.Pnl("-50"),
        fee=openpit.param.Fee("3.4"),
    ),
)
assert engine.apply_execution_report(report=report).kill_switch_triggered is False
```

## Domain value types

Use domain types at integration boundaries so policies receive validated values.

```python
from decimal import Decimal

import openpit

asset = openpit.param.Asset("AAPL")
quantity = openpit.param.Quantity("10.5")
price = openpit.param.Price(185)
pnl = openpit.param.Pnl("-12.5")

assert asset == "AAPL"
assert quantity.decimal == Decimal("10.5")
assert str(price.calculate_volume(quantity)) == "1942.5"
assert str(pnl) == "-12.5"
```

## Directional helpers

Use enum helpers instead of comparing raw strings throughout policy code.

```python
import openpit

side = openpit.param.Side.BUY
position_side = openpit.param.PositionSide.LONG

assert side.opposite() is openpit.param.Side.SELL
assert side.sign() == 1
assert position_side.opposite() is openpit.param.PositionSide.SHORT
```

## Leverage values

Use `Leverage` when a margin order or position snapshot needs an explicit
multiplier.

```python
import openpit

from_multiplier = openpit.param.Leverage(100)
from_float = openpit.param.Leverage(100.5)

assert from_multiplier.value == 100.0
assert from_float.value == 100.5
```

## Start-stage reject handling

Use this pattern when the caller wants to separate cheap start checks from the
main stage.

```python
import openpit

engine = (
    openpit.Engine.builder()
    .check_pre_trade_start_policy(
        policy=openpit.pretrade.policies.OrderValidationPolicy(),
    )
    .build()
)

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity("100"),
        price=openpit.param.Price("185"),
    ),
)

start_result = engine.start_pre_trade(order=order)
if not start_result:
    for reject in start_result.rejects:
        print(reject.policy, reject.code, reject.reason)
else:
    request = start_result.request
```

## Main-stage finalization

Use this pattern when the caller wants explicit control over main-stage timing.

```python
import openpit

engine = openpit.Engine.builder().build()
order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity("100"),
        price=openpit.param.Price("185"),
    ),
)

start_result = engine.start_pre_trade(order=order)
execute_result = start_result.request.execute()
if execute_result:
    execute_result.reservation.commit()
else:
    for reject in execute_result.rejects:
        print(reject.policy, reject.code, reject.reason)
```

## Shortcut flow

Use `execute_pre_trade` when no work is needed between start and main stages.

```python
import openpit

engine = openpit.Engine.builder().build()
order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity("100"),
        price=openpit.param.Price("185"),
    ),
)

execute_result = engine.execute_pre_trade(order=order)
if execute_result:
    execute_result.reservation.commit()
```

## Account-adjustment batch

Use account adjustments for non-trading balance or position state changes.

```python
import openpit

account_id = openpit.param.AccountId.from_u64(99224416)
adjustments = [
    openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            total=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize("10000"),
            ),
        ),
    ),
    openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentPositionOperation(
            instrument=openpit.Instrument("SPX", "USD"),
            collateral_asset="USD",
            average_entry_price=openpit.param.Price("95000"),
            mode=openpit.param.PositionMode.HEDGED,
        ),
        amount=openpit.AccountAdjustmentAmount(
            total=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize("-3"),
            ),
        ),
    ),
]

engine = openpit.Engine.builder().build()
assert engine.apply_account_adjustment(
    account_id=account_id,
    adjustments=adjustments,
).ok
```

## Account-adjustment policy

Use this pattern when administrative account changes must obey custom limits.

```python
import openpit


class CumulativeLimitPolicy(openpit.AccountAdjustmentPolicy):
    def __init__(self, max_cumulative: openpit.param.Volume) -> None:
        self._max = max_cumulative
        self._totals: dict[str, openpit.param.Volume] = {}

    @property
    def name(self) -> str:
        return "CumulativeLimitPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> tuple[openpit.Mutation, ...] | list[openpit.pretrade.PolicyReject]:
        del ctx, account_id
        assert adjustment.operation is not None
        asset_id = adjustment.operation.asset
        previous = self._totals.get(asset_id, openpit.param.Volume("0"))
        next_total = previous
        if next_total > self._max:
            return [
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="cumulative limit exceeded",
                    details=f"{asset_id}: {next_total} > {self._max}",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                )
            ]
        self._totals[asset_id] = next_total
        return (
            openpit.Mutation(
                commit=lambda: None,
                rollback=lambda: self._totals.__setitem__(asset_id, previous),
            ),
        )


policy = CumulativeLimitPolicy(max_cumulative=openpit.param.Volume("1000000"))
engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()
adjustment = openpit.AccountAdjustment(
    operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
)
result = engine.apply_account_adjustment(
    account_id=openpit.param.AccountId.from_u64(99224416),
    adjustments=[adjustment],
)
assert result.ok
```

## Rollback-safe main policy

Use mutations when a policy updates state before the final outcome is known.

```python
import openpit


class ReserveThenValidatePolicy(openpit.pretrade.PreTradePolicy):
    def __init__(self) -> None:
        self._reserved = openpit.param.Volume("0")
        self._limit = openpit.param.Volume("50")

    @property
    def name(self) -> str:
        return "ReserveThenValidatePolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        previous = self._reserved
        self._reserved = openpit.param.Volume("100")
        rollback = openpit.Mutation(
            commit=lambda: None,
            rollback=lambda: setattr(self, "_reserved", previous),
        )
        if self._reserved > self._limit:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="reservation exceeds limit",
                        details=f"reserved {self._reserved}, limit {self._limit}",
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ],
                mutations=[rollback],
            )
        return openpit.pretrade.PolicyDecision.accept(mutations=[rollback])

    def apply_execution_report(self, report: openpit.ExecutionReport) -> bool:
        del report
        return False


policy = ReserveThenValidatePolicy()
engine = openpit.Engine.builder().pre_trade_policy(policy=policy).build()
order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity("10"),
        price=openpit.param.Price("25"),
    ),
)
result = engine.execute_pre_trade(order=order)
assert not result.ok
assert policy._reserved == openpit.param.Volume("0")
```

## Custom notional-cap policy

Use a main-stage policy when the check needs complete order context and can
return structured rejects.

```python
import openpit


class NotionalCapPolicy(openpit.pretrade.PreTradePolicy):
    def __init__(self, max_abs_notional: openpit.param.Volume) -> None:
        self._max_abs_notional = max_abs_notional

    @property
    def name(self) -> str:
        return "NotionalCapPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx
        assert order.operation is not None
        amount = order.operation.trade_amount
        if amount.is_volume:
            requested = amount.as_volume
        else:
            assert order.operation.price is not None
            requested = order.operation.price.calculate_volume(amount.as_quantity)
        if requested > self._max_abs_notional:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="strategy cap exceeded",
                        details=f"requested {requested}",
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )
        return openpit.pretrade.PolicyDecision.accept()

    def apply_execution_report(self, report: openpit.ExecutionReport) -> bool:
        del report
        return False


def make_order(quantity: str) -> openpit.Order:
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(quantity),
            price=openpit.param.Price("25"),
        ),
    )


engine = (
    openpit.Engine.builder()
    .pre_trade_policy(
        policy=NotionalCapPolicy(openpit.param.Volume("1000")),
    )
    .build()
)
result = engine.execute_pre_trade(order=make_order("10"))
assert result.ok
result.reservation.commit()
```

## Custom order models

Subclass `Order` or `ExecutionReport` when policy callbacks need integration
metadata in addition to the engine-facing contract.

```python
import openpit


class StrategyOrder(openpit.Order):
    def __init__(
        self,
        *,
        operation: openpit.OrderOperation,
        strategy_tag: str,
    ) -> None:
        super().__init__(operation=operation)
        self.strategy_tag = strategy_tag


class StrategyTagPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    @property
    def name(self) -> str:
        return "StrategyTagPolicy"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> list[openpit.pretrade.PolicyReject]:
        del ctx
        strategy_order = order
        if not isinstance(strategy_order, StrategyOrder):
            return []
        if strategy_order.strategy_tag == "blocked":
            return [
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
                    reason="strategy blocked",
                    details="strategy tag is not allowed",
                    scope=openpit.pretrade.RejectScope.ORDER,
                )
            ]
        return []

    def apply_execution_report(self, report: openpit.ExecutionReport) -> bool:
        del report
        return False


engine = (
    openpit.Engine.builder()
    .check_pre_trade_start_policy(policy=StrategyTagPolicy())
    .build()
)
order = StrategyOrder(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_u64(99224416),
        side=openpit.param.Side.BUY,
        trade_amount=openpit.param.TradeAmount.quantity("10"),
        price=openpit.param.Price("25"),
    ),
    strategy_tag="alpha",
)
result = engine.execute_pre_trade(order=order)
assert result.ok
result.reservation.commit()
```
