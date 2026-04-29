# Concepts

OpenPit separates pre-trade validation from order submission. The engine does
not send orders to venues. It evaluates the order, returns business rejects or a
reservation, and leaves external I/O to the caller.

## Engine

`openpit.Engine` is the runtime object that owns policy instances. Build it once
with `openpit.Engine.builder()` and reuse it for sequential calls.

The engine is single-threaded by contract. Do not call public methods on the
same engine object concurrently.

## Start stage

Start-stage policies run during `engine.start_pre_trade(order=...)`. They are
for fast checks that must run before the main-stage request exists, such as
payload validation, rate limiting, or kill switches.

Start-stage policies return normal business rejects. They do not register
rollback mutations.

## Main stage

The start stage returns `openpit.pretrade.PreTradeRequest` on success. Calling
`request.execute()` runs main-stage policies. These policies can return rejects
and register `openpit.Mutation` objects.

If any main-stage policy rejects, the engine rolls registered mutations back.
If all policies pass, the engine returns a `PreTradeReservation`.

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
