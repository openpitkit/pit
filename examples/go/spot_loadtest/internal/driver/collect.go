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

package driver

import (
	"context"
	"errors"
	"fmt"
	"time"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// orderFuture wraps the ExecutePreTrade future so the collector can await it.
type orderFuture struct {
	fut *future.Future2[*asyncengine.AsyncReservation, []reject.Reject]
}

// settleFuture wraps the ApplyExecutionReport future so the collector can await
// it. In the open-loop driver the settlement is SUBMITTED by the per-account
// scheduler at the settlement's virtual arrival time (not triggered by the
// collector), and only AWAITED here.
type settleFuture struct {
	fut *future.Future[pretrade.PostTradeResult]
}

// fundingFuture wraps the ApplyAccountAdjustment future.
type fundingFuture struct {
	fut *future.Future2[
		optional.Option[reject.AccountAdjustmentBatchError],
		[]accountadjustment.Outcome,
	]
}

// collectOrder awaits an order-check future, records its open-loop latency
// (resolve - VirtualT0, the HEADLINE) and its service-time diagnostic (resolve -
// actual submit), and checks the per-op oracle. For an accepted order it hands
// the reservation to the finalizer pool to CommitAndClose off the measured path
// (commit is a no-op; close releases the native handle, KEEPING the in-place
// hold). It NEVER closes/rolls back an accepted order.
//
// The latency is computed from the resolve instant captured immediately after
// Await returns, so any engine queueing or stall the order experienced is
// included in resolve - VirtualT0 rather than omitted (open-loop CO defence).
func (r *run) collectOrder(item inflight) {
	reservation, rejects, err := item.orderFut.fut.Await(r.ctx)
	resolve := time.Now()
	latency := resolve.Sub(item.intendedT0)
	if err != nil {
		// Dispatch-capacity backpressure (ErrQueueLimit) is an EXPLICIT MEASURED
		// outcome, not an oracle failure: the engine refused the submit because the
		// live-queue cap was reached. Count it and surface it (never panic, never
		// drop silently), and do NOT run the oracle for this op since no decision
		// was produced. A healthy run reports zero backpressure, so the oracle still
		// checks every op; backpressure only appears when the dispatch is
		// undersized, which the report flags as a degraded run.
		if errors.Is(err, asyncengine.ErrQueueLimit) {
			r.sink.recordBackpressure(latency)
			return
		}
		r.sink.recordResolve(opOrderCheck, latency, false)
		r.oracle.failExternal(fmt.Errorf(
			"driver: ExecutePreTrade transport error (account %s corr %d): %w",
			item.event.Account, item.event.CorrelationID, err))
		return
	}
	accepted := reservation != nil
	r.sink.recordResolve(opOrderCheck, latency, accepted)
	// Service-time diagnostic (resolve - actual submit). DIAGNOSTIC only.
	r.sink.recordServiceTime(resolve.Sub(item.actualSubmit))
	r.oracle.checkOrder(item.event, orderObservation{accepted: accepted, rejects: rejects})

	if !accepted {
		// A rejected order reserved nothing: no reservation to finalize.
		return
	}

	// Finalize OUTSIDE the measured interval: CommitAndClose (commit = no-op,
	// close releases the native handle, keeping the in-place hold). Hand it to the
	// finalizer pool so the collector stays responsive (its Await loop must keep
	// up so resolve instants stay accurate); the finalizer awaits the close and
	// surfaces any error loudly.
	r.handOffFinalize(reservation)
}

// collectSettlement awaits a settlement (ApplyExecutionReport) future, records
// its open-loop latency (resolve - VirtualT0), and checks the per-op oracle
// (clean settle + engine-volunteered post-op balances). The settlement was
// submitted by the per-account scheduler at its virtual arrival time.
func (r *run) collectSettlement(item inflight) {
	result, err := item.settleFut.fut.Await(r.ctx)
	resolve := time.Now()
	latency := resolve.Sub(item.intendedT0)
	if err != nil {
		if errors.Is(err, asyncengine.ErrQueueLimit) {
			r.sink.recordBackpressure(latency)
			return
		}
		r.sink.recordResolve(opSettlement, latency, false)
		r.oracle.failExternal(fmt.Errorf(
			"driver: ApplyExecutionReport transport error (account %s corr %d): %w",
			item.event.Account, item.event.CorrelationID, err))
		return
	}
	blocked := len(result.AccountBlocks) > 0
	r.sink.recordResolve(opSettlement, latency, !blocked)
	r.oracle.checkSettlement(item.event, settleObservation{
		blocked:  blocked,
		outcomes: result.AccountAdjustmentOutcomes,
	})
}

// collectFunding awaits a funding adjustment, records its decision in the
// funding-only counters, and checks the per-op oracle. Only runtime top-ups
// flow through this path; seeds are applied synchronously before the async run
// and never hit it.
//
// Funding is NOT an order-check: its latency is recorded into NO headline
// histogram and it does NOT count toward the order-check totals or the
// achieved-reject-rate denominator (which would otherwise pad the denominator
// with funding accepts and pollute the order-check latency distribution). The
// resolve instant is still captured so a backpressure latency can be recorded
// for the checksum on the ErrQueueLimit path.
func (r *run) collectFunding(item inflight) {
	batchErr, outcomes, err := item.fundingFut.fut.Await(r.ctx)
	resolve := time.Now()
	if err != nil {
		// Dispatch-capacity backpressure is a measured outcome, not an oracle
		// failure (see collectOrder).
		if errors.Is(err, asyncengine.ErrQueueLimit) {
			r.sink.recordBackpressure(resolve.Sub(item.intendedT0))
			return
		}
		// A funding transport error is surfaced via the oracle; no funding
		// decision was produced, so nothing is recorded into the funding tally.
		r.oracle.failExternal(fmt.Errorf(
			"driver: ApplyAccountAdjustment transport error (account %s asset %s): %w",
			item.event.Account, item.event.FundingAsset, err))
		return
	}
	rejected := batchErr.IsSet()
	r.sink.recordFunding(!rejected)
	r.oracle.checkFunding(item.event, fundingObservation{rejected: rejected, outcomes: outcomes})
}

// handOffFinalize hands an accepted reservation to the finalizer pool WITHOUT
// EVER BLOCKING the collector (a blocked collector would stop draining the work
// handoff and so throttle the open-loop submit schedule with harness-internal
// backlog). It tries the fast buffered finalize channel first; if that buffer is
// momentarily full it spills to the unbounded finalizeOverflow, which finalizers
// also drain.
//
// Spilling here is counted as a HARNESS handoff stall DIAGNOSTIC. Unlike the work
// handoff (whose fast path fills mainly because the ENGINE is slow — real
// latency), the finalize backlog is purely harness-internal: it forms only when
// the finalizer pool cannot keep up with CommitAndClose throughput, never because
// of engine order-check latency. Because the handoff is non-blocking and
// CommitAndClose is fully off the measured path (the latency was already recorded
// at resolve), a full finalize fast path does NOT throttle the submit schedule
// and does NOT invalidate the run — it is reported as a diagnostic only. The
// reservation is still finalized (from the overflow) — the stall is counted,
// never a dropped reservation.
//
// On context cancellation it closes the reservation directly so the native
// handle never leaks.
func (r *run) handOffFinalize(reservation *asyncengine.AsyncReservation) {
	select {
	case r.finalize <- reservation:
		return
	default:
	}
	// Fast path full: the harness fell behind. Count the stall diagnostic (never a
	// silent absorb) and spill so the collector does not block. On cancellation,
	// close directly to release the native handle.
	select {
	case <-r.ctx.Done():
		_, _ = reservation.Close(context.Background()).Await(context.Background())
	default:
		r.sink.recordHandoffStall()
		r.finalizeOverflow.push(reservation)
	}
}

// finalizeLoop is one finalizer goroutine. It drains accepted reservations from
// BOTH the fast finalize channel AND the unbounded finalizeOverflow that the
// collector spills into when the fast buffer is full, and finalizes each with
// CommitAndClose (commit = no-op, close releases the native handle while KEEPING
// the in-place hold). This is strictly off the measured path. A CommitAndClose
// error is surfaced loudly via the oracle.
//
// The CommitAndClose is submitted on the reservation's per-account queue and may
// sit behind that account's later order-checks (true open-loop pipelining no
// longer serialises an account), so awaiting it here keeps that wait off the
// collector and out of every measured interval. The overflow is drained first on
// every iteration so spilled reservations cannot be stranded; when the collectors
// close the finalize channel, the closed branch drains any remaining overflow
// before returning, so no accepted reservation is ever leaked.
func (r *run) finalizeLoop() {
	for {
		if reservation, ok := r.finalizeOverflow.pop(); ok {
			r.finalizeOne(reservation)
			continue
		}
		reservation, ok := <-r.finalize
		if !ok {
			for {
				reservation, ok := r.finalizeOverflow.pop()
				if !ok {
					return
				}
				r.finalizeOne(reservation)
			}
		}
		r.finalizeOne(reservation)
	}
}

// finalizeOne CommitAndCloses one accepted reservation off the measured path,
// surfacing any error loudly via the oracle.
func (r *run) finalizeOne(reservation *asyncengine.AsyncReservation) {
	if _, err := reservation.CommitAndClose(r.ctx).Await(r.ctx); err != nil {
		r.oracle.failExternal(fmt.Errorf("driver: CommitAndClose: %w", err))
	}
}
