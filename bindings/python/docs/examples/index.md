# Examples

The examples on this page cover common OpenPit integration flows. Each snippet
is intended to be copied into a Python file after installing `openpit`.

<!-- markdownlint-disable MD013 -->

| Example | Scenario |
| --- | --- |
| [Full order lifecycle](#full-order-lifecycle) | Configure policies, reserve, commit, and apply a report |
| [Domain value types](#domain-value-types) | Build validated financial values |
| [Directional helpers](#directional-helpers) | Use side and position-side enums |
| [Leverage values](#leverage-values) | Construct leverage multipliers |
| [Start-stage reject handling](#start-stage-reject-handling) | Inspect start-stage rejects |
| [Main-stage finalization](#main-stage-finalization) | Execute a request and commit or inspect rejects |
| [Shortcut flow](#shortcut-flow) | Run start and main stages together |
| [Account-adjustment batch](#account-adjustment-batch) | Apply balance and position adjustments |
| [Account-adjustment check](#account-adjustment-check) | Validate account adjustments with a custom policy |
| [Rollback-safe main policy](#rollback-safe-main-policy) | Register a mutation and rely on rollback |
| [Custom notional-cap policy](#custom-notional-cap-policy) | Reject orders above a strategy cap |
| [Custom order models](#custom-order-models) | Preserve project metadata through callbacks |

<!-- markdownlint-enable MD013 -->

## Full order lifecycle

Use this when integrating OpenPit around a real order-submission path. The
reservation is committed only after the downstream send succeeds.

```python
import datetime
import openpit
import openpit.pretrade.policies


def send_order_to_venue(order: openpit.Order) -> None:
    _ = order


pnl_policy = (
    openpit.pretrade.policies.build_pnl_bounds_killswitch()
    .broker_barriers(
        openpit.pretrade.policies.PnlBoundsBrokerBarrier(
            settlement_asset="USD",
            lower_bound=openpit.param.Pnl("-1000"),
        ),
    )
)
rate_limit_policy = (
    openpit.pretrade.policies.build_rate_limit()
    .broker_barrier(
        openpit.pretrade.policies.RateLimitBrokerBarrier(
            limit=openpit.pretrade.policies.RateLimit(
                max_orders=100,
                window=datetime.timedelta(seconds=1),
            ),
        ),
    )
)
size_policy = (
    openpit.pretrade.policies.build_order_size_limit()
    .asset_barriers(
        openpit.pretrade.policies.OrderSizeAssetBarrier(
            limit=openpit.pretrade.policies.OrderSizeLimit(
                max_quantity=openpit.param.Quantity("500"),
                max_notional=openpit.param.Volume("100000"),
            ),
            settlement_asset="USD",
        ),
    )
)

engine = (
    openpit.Engine.builder()
    .no_sync()
    .builtin(openpit.pretrade.policies.build_order_validation())
    .builtin(pnl_policy)
    .builtin(rate_limit_policy)
    .builtin(size_policy)
    .build()
)

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_int(99224416),
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
        account_id=openpit.param.AccountId.from_int(99224416),
        side=openpit.param.Side.BUY,
    ),
    financial_impact=openpit.FinancialImpact(
        pnl=openpit.param.Pnl("-50"),
        fee=openpit.param.Fee("3.4"),
    ),
)
assert not engine.apply_execution_report(report=report).account_blocks
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
import openpit.pretrade.policies

engine = (
    openpit.Engine.builder()
    .no_sync()
    .builtin(
        openpit.pretrade.policies.build_order_validation(),
    )
    .build()
)

order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_int(99224416),
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
        account_id=openpit.param.AccountId.from_int(99224416),
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
        account_id=openpit.param.AccountId.from_int(99224416),
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

account_id = openpit.param.AccountId.from_int(99224416)
adjustments = [
    openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
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
            balance=openpit.param.AdjustmentAmount.absolute(
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

## Account-adjustment check

Use this pattern when administrative account changes must obey custom limits.

<!-- Test mirror: pit/bindings/python/tests/integration/test_examples_wiki.py -->
```python
import openpit


class CumulativeLimitPolicy(openpit.pretrade.Policy):
    """Tracks cumulative totals per asset, rejects batch on limit breach."""

    def __init__(self, max_cumulative: openpit.param.PositionSize) -> None:
        self._max = max_cumulative
        self._totals: dict[str, openpit.param.PositionSize] = {}

    @property
    def name(self) -> str:
        return "CumulativeLimitPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyAccountAdjustmentResult:
        del ctx, account_id
        operation = adjustment.operation
        if not isinstance(operation, openpit.AccountAdjustmentBalanceOperation):
            return openpit.pretrade.PolicyAccountAdjustmentResult()

        asset_id = operation.asset
        amount = adjustment.amount
        balance = amount.balance if amount is not None else None
        if asset_id is None or balance is None:
            return openpit.pretrade.PolicyAccountAdjustmentResult()

        previous = self._totals.get(asset_id)
        current = previous if previous is not None else openpit.param.PositionSize("0")
        absolute = balance.as_absolute
        if absolute is not None:
            new_total = absolute
        else:
            delta = balance.as_delta
            if delta is None:
                return openpit.pretrade.PolicyAccountAdjustmentResult()
            new_total = current + delta

        # Reject if the limit is breached.
        if new_total > self._max:
            return openpit.pretrade.PolicyAccountAdjustmentResult(
                rejects=(
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="cumulative limit exceeded",
                        details=f"{asset_id}: {new_total} > {self._max}",
                        scope=openpit.pretrade.RejectScope.ACCOUNT,
                    ),
                ),
            )

        # Apply immediately so later adjustments in the same batch see it.
        self._totals[asset_id] = new_total

        def rollback() -> None:
            if previous is None:
                self._totals.pop(asset_id, None)
            else:
                self._totals[asset_id] = previous

        return openpit.pretrade.PolicyAccountAdjustmentResult(
            mutations=(
                openpit.Mutation(
                    commit=lambda: None,
                    rollback=rollback,
                ),
            ),
        )


def balance_adjustment(
    amount: openpit.param.AdjustmentAmount,
) -> openpit.AccountAdjustment:
    return openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(balance=amount),
    )


policy = CumulativeLimitPolicy(openpit.param.PositionSize("100"))
engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()
account_id = openpit.param.AccountId.from_int(99224416)

seed = engine.apply_account_adjustment(
    account_id=account_id,
    adjustments=[
        balance_adjustment(
            openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize("40")
            )
        )
    ],
)
rejected = engine.apply_account_adjustment(
    account_id=account_id,
    adjustments=[
        balance_adjustment(
            openpit.param.AdjustmentAmount.delta(openpit.param.PositionSize("30"))
        ),
        balance_adjustment(
            openpit.param.AdjustmentAmount.delta(openpit.param.PositionSize("40"))
        ),
    ],
)
after_rollback = engine.apply_account_adjustment(
    account_id=account_id,
    adjustments=[
        balance_adjustment(
            openpit.param.AdjustmentAmount.delta(openpit.param.PositionSize("60"))
        )
    ],
)

assert seed.ok
assert not rejected.ok
assert rejected.failed_index == 1
assert rejected.rejects[0].code == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
assert after_rollback.ok
```

## Rollback-safe main policy

Use mutations when a policy updates state before the final outcome is known.

```python
import openpit


class ReserveThenValidatePolicy(openpit.pretrade.Policy):
    def __init__(self) -> None:
        self._reserved = openpit.param.Volume("0")
        self._limit = openpit.param.Volume("50")

    @property
    def name(self) -> str:
        return "ReserveThenValidatePolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
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
engine = openpit.Engine.builder().pre_trade(policy=policy).build()
order = openpit.Order(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_int(99224416),
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

Use a main-stage check when the check needs complete order context and can
return structured rejects.

```python
import openpit


class NotionalCapPolicy(openpit.pretrade.Policy):
    def __init__(self, max_abs_notional: openpit.param.Volume) -> None:
        self._max_abs_notional = max_abs_notional

    @property
    def name(self) -> str:
        return "NotionalCapPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
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
            account_id=openpit.param.AccountId.from_int(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(quantity),
            price=openpit.param.Price("25"),
        ),
    )


engine = (
    openpit.Engine.builder()
    .pre_trade(
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


class StrategyTagPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "StrategyTagPolicy"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
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
    .no_sync()
    .pre_trade(policy=StrategyTagPolicy())
    .build()
)
order = StrategyOrder(
    operation=openpit.OrderOperation(
        instrument=openpit.Instrument("AAPL", "USD"),
        account_id=openpit.param.AccountId.from_int(99224416),
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
