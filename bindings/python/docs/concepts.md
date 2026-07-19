# Concepts

OpenPit separates pre-trade validation from order submission. The engine
does not send orders to venues; it evaluates an order, returns business
rejects or a reservation, and leaves external I/O to the caller.

## Engine

`openpit.Engine` is the runtime object that owns policy instances. Build
it once with `openpit.Engine.builder()`, choose a synchronization policy,
and reuse it under the matching call pattern.

- `full_sync()` allows concurrent calls on the same engine handle.
- `no_sync()` keeps the handle on the OS thread that created it.
- `account_sync()` allows concurrent calls when the caller pins each
  account to one processing chain, so calls for the same account are
  never concurrent.

## Start stage

Start-stage checks run during `engine.start_pre_trade(order=...)`. They are
for fast checks that must run before the main-stage request exists, such as
payload validation, rate limiting, or kill switches.

Start-stage checks return normal business rejects aggregated from all
registered policies. They do not register rollback mutations.

## Main stage

The start stage returns `openpit.pretrade.Request` on success. Calling
`request.execute()` runs main-stage checks. These policies can return rejects
and register `openpit.Mutation` objects.

If any main-stage check rejects, the engine rolls registered mutations back.
If all policies pass, the engine returns a `Reservation`.

## Reservation

A reservation is explicit and single-use. Call `commit()` only after the caller
knows the order should become durable, for example after it is accepted by the
next downstream component. Call `rollback()` when the caller decides not to send
or cannot send the order.

## Post-trade feedback

After execution, pass `openpit.ExecutionReport` objects to
`engine.apply_execution_report(report=...)`. Policies use reports to update
state such as P&L accumulators and may report that a kill switch is active.

## Account adjustments

`engine.apply_account_adjustment(...)` validates non-trading account state
changes, such as balance corrections or direct position updates. The input is a
batch and the result reports either success or the first failing adjustment.
An accepted batch can also report account blocks that were recorded by the
engine before the call returned.
