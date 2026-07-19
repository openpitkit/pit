// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

// Package driver wires the offline generator stream into the real openpit
// asyncengine and measures the Go FFI latency TRUE OPEN-LOOP against the
// generator's virtual causal timeline.
//
// # Pipeline
//
// The driver realises the virtual causal timeline from the design doc on a
// single AccountSync engine wrapped in an asyncengine.AsyncEngine (Dynamic
// strategy, dispatch sized to the offered concurrency):
//
//   - one submitter goroutine per account owns that account's ordered
//     sub-stream. It paces each event by sleeping until the event's VirtualT0
//     (relative to a shared run start), stamps that virtual arrival as the
//     measured t0, submits non-blocking, hands {future, t0, actual-submit, event}
//     to the collector via a buffered channel, and IMMEDIATELY advances to the
//     next event. It NEVER awaits a future and never waits for a decision before
//     the next submit, so many of one account's ops may be in flight at once
//     (true pipelining);
//   - a collector goroutine pool drains the channel, Awaits each future, records
//     the open-loop latency = resolveTime - VirtualT0 (and, for order-checks, the
//     service-time diagnostic = resolveTime - actualSubmit), checks the per-op
//     oracle, and hands accepted reservations to the finalizer pool;
//   - a finalizer goroutine pool CommitAndCloses accepted reservations off the
//     measured path;
//   - settlements and runtime funding are submitted by the per-account scheduler
//     at their own virtual arrival times (settlements are NO LONGER triggered by
//     the collector).
//
// # Open-loop latency definition (coordinated-omission defence)
//
// For every measured operation the latency is `resolveTime - VirtualT0`, where
// VirtualT0 is the event's intended arrival on the offline virtual causal
// timeline (mapped to an absolute instant as run-start + VirtualT0), stamped
// independently of when the submit actually happened. Because the submitter
// never waits on a decision before issuing the next event, any queueing or
// stall under load is captured inside `resolveTime - VirtualT0` rather than
// silently omitted. This is the HEADLINE. Two latencies are measured this way:
// the order check (stage 1->2) and the report settlement (stage 3->4).
//
// A separate SERVICE-TIME diagnostic (`resolveTime - actualSubmit`) is captured
// for order-checks and exposed ONLY in the diagnostics section. It discounts the
// queue wait that accrued before the actual submit, so it hides the saturation
// tail by construction and is never the headline.
//
// # Why the strict per-op oracle survives true open-loop
//
// The engine is FIFO-per-account (one channel-backed queue and one worker per
// account; reservation Commit/Close routed through the same queue) and the
// spot-funds hold is applied IN-PLACE at ExecutePreTrade (Commit is a no-op;
// only Close-without-commit rolls back). Each account's causal sub-stream is
// submitted from a single goroutine in emission order, and the generator assigns
// VirtualT0 so that order is non-decreasing per account (a top-up before its
// order, an order-check before its settlement, a settlement before a dependent
// later order). FIFO-per-account therefore replays the shadow's offline-ordered
// decisions exactly, so the precomputed per-event predictions can be checked as
// each op resolves (order-independently) without any per-op blocking.
//
// # Commit/close is outside every measured interval
//
// An accepted order-check resolves with an *asyncengine.AsyncReservation. The
// collector computes and records the order-check latency and runs the oracle,
// then hands the reservation to the finalizer pool, which CommitAndCloses it
// (commit = no-op, close releases the native handle while KEEPING the in-place
// hold). It uses CommitAndClose (keep), NEVER Close (rollback), for an accepted
// order. The commit happens strictly after the measured span closes, never
// inside it, and on a separate pool so the collector's Await loop stays
// responsive. A rejected or transport-failed order reserves nothing, so there is
// no reservation to finalize.
//
// # Oracle (strict, per account)
//
// The generator predicts, per account in account event order, the accept/reject
// decision and the resulting balances for every operation. The driver checks
// the live engine against those predictions:
//
//   - accept/reject MUST match for every order-check and settlement; on a
//     predicted reject the engine reject code MUST map to the predicted reason
//     (RejectInsufficientFunds -> reject.CodeInsufficientFunds);
//   - per-account operations are serialised by the engine (AccountSync) and the
//     generator predicts per account in order, so outcomes are correlated by
//     (account, per-account sequence index).
//
// ## Balance checking and its limitation (read this)
//
// The public Go binding exposes NO independent "read this account's holdings"
// query: openpit.Engine offers only StartPreTrade / ExecutePreTrade /
// ApplyExecutionReport / ApplyAccountAdjustment / Accounts (group management),
// and asyncengine.AsyncEngine mirrors that surface. There is therefore no
// holdings/available/held query to assert post-op balances against directly.
//
// The engine does, however, *volunteer* post-op balances as a side effect of
// some operations, and the driver uses them where available:
//
//   - ApplyAccountAdjustment (funding) returns an accountadjustment.BatchResult
//     whose Outcomes hold the adjustment outcomes, and ApplyExecutionReport
//     (settlement) returns those outcomes inside its PostTradeResult. For the
//     SpotFundsPolicy each outcome's
//     OutcomeAmount.Absolute is the true post-op available/held of the affected
//     (account, asset) leg (verified against the core: spot_funds/adjustment.rs
//     and spot_funds/execution.rs set absolute = new.available()/new.held()).
//     The driver asserts those volunteered post-op balances equal the shadow's
//     predicted Post exactly, per affected asset, for FUNDING and SETTLEMENT
//     operations.
//   - ExecutePreTrade (order-check) resolves with a reservation, not an outcome
//     list. The inner pretrade.Reservation does expose AccountAdjustments(), but
//     the asyncengine.AsyncReservation wrapper does not surface it (and reaching
//     into binding internals is forbidden). So an accepted ORDER-CHECK's post-op
//     balances cannot be read through the public async surface; only its
//     accept/reject decision is observable.
//
// This means the oracle is balance-exact for funding and settlement, and
// decision-only for the order-check itself. That is not a silent weakening: a
// per-account balance divergence introduced at an order-check necessarily
// changes that account's available balance, which surfaces as a later
// accept/reject mismatch on the same account (the generator drives long
// per-account sequences with self-funding and rejects calibrated near the
// available boundary), and the settlement of that very order re-reads the two
// affected legs' absolute balances. As a secondary net the driver also checks
// an aggregate fund-conservation invariant over the whole run (total
// available+held per asset moves only by the net of funding adjustments) and a
// no-oversell invariant (no balance ever goes negative). Any divergence is an
// explicit error naming the account, the per-account op index, and expected vs
// actual; there are no silent failures or lossy fallbacks.
package driver
