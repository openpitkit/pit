# Policy contracts

Policies are regular Python objects that implement
`openpit.pretrade.Policy`. They return business decisions; they should
raise exceptions only for programming errors or unexpected runtime failures.

## Start-stage checks

Implement `check_pre_trade_start` for fast admission checks.

```python
import openpit


class BlockedAccountPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "BlockedAccountPolicy"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx
        assert order.operation is not None
        if order.operation.account_id == openpit.param.AccountId.from_int(1):
            return (
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.ACCOUNT_BLOCKED,
                    reason="account is blocked",
                    details="account 1 cannot send new orders",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                ),
            )
        return ()

    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        del report
        return False
```

Start-stage checks return an iterable of `PolicyReject` objects. An empty
iterable means success.

## Main-stage checks

Implement `openpit.pretrade.Policy` when a policy may reserve state or
needs to run in the main stage.

```python
import openpit


class NotionalCapPolicy(openpit.pretrade.Policy):
    def __init__(self, max_notional: openpit.param.Volume) -> None:
        self._max_notional = max_notional

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

        if requested > self._max_notional:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="notional cap exceeded",
                        details=f"requested {requested}, max {self._max_notional}",
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )
        return openpit.pretrade.PolicyDecision.accept()

    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        del report
        return False
```

Use `PolicyDecision.accept(mutations=[...])` when the policy has reserved state
that must be finalized or rolled back by the engine.

## Mutations

A `Mutation` is a pair of callables:

- `commit`: called when the reservation or drop-copy operation is committed.
- `rollback`: called when the main stage rejects, when the reservation or
  drop-copy operation rolls back, and as compensation when the engine unwinds
  a fatal drop-copy evaluation failure - even for a mutation whose own commit
  was never reached.

Apply tentative state before registering a mutation. A drop-copy operation
carries the registered mutations to its owner and finalizes them exactly like
a reservation: `commit()` runs every commit callback in registration order,
while explicit or implicit `rollback()` runs every rollback callback in
reverse registration order.

Register mutations when policy state changes before the final order-submission
outcome is known.

### Finalizers must not fail

Neither callable has the right to fail. By the time a finalizer runs the
decision is already made and the state it finalizes was applied eagerly, so
there is nothing left to compensate. A callable that raises anyway never stops
the batch - every remaining callback still runs - and the failure is reported
on two independent channels:

- the exception is re-raised to the caller of `commit()` / `rollback()` with
  its original type and message;
- the engine arms its kill switch, because its own bookkeeping is now in an
  unknown state. Every `Mutation` registered from Python belongs to a custom
  policy whose state reach the engine cannot bound, so **every** account is
  blocked, not only the order's own.

The block is not reported to the finalizing caller. It surfaces when the next
pre-trade call is rejected with `SystemUnavailable`, and an operator clears it
with `engine.accounts().unblock_all()`. That call leaves accounts and groups
blocked individually in place.

## Built-in policies

- `build_order_validation()`: validates required order fields and basic shape.
- `build_rate_limit()`: rejects requests after a configured limit is reached.
- `build_pnl_bounds_killswitch()`: blocks accounts when accumulated P&L is
  outside configured bounds.
- `build_order_size_limit()`: enforces optional quantity limits by underlying
  asset and notional limits by settlement asset, with additive broker caps.
- `build_spot_funds()`: enforces available-funds checks by default and tracks
  reservations and settlement state.
- `build_spot_funds_pnl_bounds_killswitch()`: enables account P&L bounds with
  Spot Funds in `TRACK_ONLY` mode. This preset records reservations but disables
  the insufficient-funds gate, so available funds may go negative instead of
  producing an `InsufficientFunds` reject. Arithmetic overflow is still
  surfaced.

### Spot Funds P&L barriers and currency

`SpotFundsPnlBoundsBarrier` is a frozen dataclass whose `currency` field is
required; `lower_bound` and `upper_bound` default to `None`, and at least one
of them must be given. The same dataclass is used at construction time, through
`global_barrier()`, `account_group_barriers()` and `account_barriers()` on
`build_spot_funds_pnl_bounds_killswitch()`, and at runtime, through
`engine.configure().spot_funds_pnl_bounds_killswitch(...)`.

`currency` validates account-specific barriers and filters fallback barriers:

- An account barrier always belongs to that account. If its `currency` differs
  from the account's effective currency, the mismatch blocks the account with
  `RejectCode.PNL_KILL_SWITCH_TRIGGERED`; the cascade does not continue.
- Account-group and global barriers apply only when their `currency` equals the
  account's effective currency. A mismatching group barrier is skipped so a
  matching global barrier can become effective.
- If there is no account barrier and neither the group nor global fallback
  matches, the account has no P&L control. Bounds are not evaluated and neither
  a breach nor a halt blocks it, while `openpit.param.Pnl` values keep
  accumulating and being published.
- If the account has no effective currency, nothing can mismatch, so no level
  is filtered by currency and the first barrier in the account -> group ->
  global cascade applies. Today's missing-currency behaviour is unchanged: the
  P&L line halts with
  `openpit.pretrade.PnlHaltReason.MISSING_ACCOUNT_CURRENCY` where it owes a
  denominated value, and that halt blocks the account.

`lower_bound` and `upper_bound` are plain `openpit.param.Pnl` values in the
barrier's own currency and are never converted by an FX rate. The barrier stays
a numeric control with no quote dependency: a missing FX quote can halt the
accumulator, but it can never move a bound.

Currency writes through `engine.accounts()` - `set_currency()`,
`clear_currency()`, `set_group_currency()`, and `clear_group_currency()` - are
unchecked and re-evaluate nothing. Stored P&L and cost basis accumulated under
a different effective currency lose their meaning; the SDK guarantees nothing
about them and does not convert, detect, or report the change. Barrier
selection itself stays deterministic: the cascade above is re-resolved with the
new effective currency on the next policy access.
`register_group()` and `unregister_group()` keep their existing
contract: they resolve the effective currency on each side of the transition
and evaluate the account against its new effective barrier before returning.
