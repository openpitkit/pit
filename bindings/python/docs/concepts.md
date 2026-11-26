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
  never concurrent. Exclusive operations, such as group membership, run
  with no other call in flight; see the threading guide.

## Start stage

Start-stage checks run during `engine.start_pre_trade(order=...)`. They are
for fast checks that must run before the main-stage request exists, such as
payload validation, rate limiting, or kill switches.

The engine aggregates entries from all registered instances and classifies
each as an effective reject or a review mark. Live start-stage checks do
not register rollback mutations.

## Main stage

The start stage returns `openpit.pretrade.Request` on success. Calling
`request.execute()` runs main-stage checks. These policies can return rejects
and register `openpit.Mutation` objects.

An effective reject rolls back the run. When only review marks remain, the
engine returns them with a `Reservation`. Any `ReviewWithoutHold` mark
discards the holds and policy results.

## Reservation

A reservation is explicit and single-use. Inspect `reviews` before sending.
When marks are present, commit the reservation and keep the order unsent.
After approval, rerun with `OrderReview(confirmed=True)` and set
`already_held=True` only if Spot Funds kept its own hold in the committed
review. A Spot Funds instance registered in a review mode that marked the order
holds nothing. After such a first review, rerun with `confirmed=True` only. A
review of an already-held rerun changes no Spot Funds hold: whatever its
marks, the hold the first review kept stays. On approval, rerun with
`confirmed=True` and `already_held=True` again; if the order ends without a
committed approval rerun, release that hold with a final cancel report
carrying the remaining reserved quantity and the first run's Spot Funds lock
prices. For ordinary orders, commit after successful submission or roll back
when submission fails.

## Post-trade feedback

After execution, pass `openpit.ExecutionReport` objects to
`engine.apply_execution_report(report=...)`. Policies use reports to update
state such as P&L accumulators and may report that a kill switch is active.
Review marks do not block accounts. For review-mode instances, only blocks
with evaluation-failure codes are effective in execution-report handling and
runtime configuration.

## Account adjustments

`engine.apply_account_adjustment(...)` validates non-trading account state
changes, such as balance corrections or direct position updates. The input is a
batch and the result reports either success or the first failing adjustment.
An accepted batch can also report account blocks that were recorded by the
engine before the call returned.
Any reject entry fails the batch, regardless of instance or record mode.
