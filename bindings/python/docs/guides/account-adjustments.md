# Account adjustments

Account adjustments model non-trading changes to account state. Typical uses are
balance corrections, direct position imports, or administrative state sync.

## Balance adjustment

```python
import openpit

adjustment = openpit.AccountAdjustment(
    operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
    amount=openpit.AccountAdjustmentAmount(
        total=openpit.param.AdjustmentAmount.absolute(
            openpit.param.PositionSize("10000"),
        ),
    ),
)

engine = openpit.Engine.builder().build()
result = engine.apply_account_adjustment(
    account_id=openpit.param.AccountId.from_u64(99224416),
    adjustments=[adjustment],
)
assert result.ok
```

## Position adjustment

```python
adjustment = openpit.AccountAdjustment(
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
)
```

## Policy result contract

An `AccountAdjustmentPolicy` can return:

- `None` for success without mutations.
- An iterable of `PolicyReject` objects to reject the batch.
- A tuple of `Mutation` objects to register rollback work.

The engine stops on the first rejected adjustment and exposes that index through
`AccountAdjustmentBatchResult.failed_index`.
