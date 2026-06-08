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
	"fmt"
	"sync"

	"github.com/shopspring/decimal"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/reject"

	"openpit-loadtest-spot-funds-go/internal/generator"
)

// orderRejectCodeFor maps a generator order-check reject reason to the engine
// reject code the oracle requires. The generator keeps its own reason taxonomy
// (it never imports the engine); this is the single point that ties the two
// together on the order path.
//
// On the v1 order path the only reason the generator predicts is
// InsufficientFunds: it always seeds the charge asset first, so the contract's
// not-configured case never fires on a positive charge, and the real
// SpotFundsPolicy reports InsufficientFunds (not a distinct not-configured
// reject) on the order path anyway. Any other reason is therefore unexpected and
// returns false so checkOrder fails loudly rather than inventing a mapping. (The
// not-configured reason can appear on a FUNDING reject, but that path checks the
// accept/reject decision via the batch error, not a reject code, so it does not
// go through here.)
func orderRejectCodeFor(reason generator.RejectReason) (reject.Code, bool) {
	switch reason {
	case generator.RejectInsufficientFunds:
		return reject.CodeInsufficientFunds, true
	default:
		return 0, false
	}
}

// orderObservation is the engine's response to one order-check, handed to the
// oracle by the collector. accepted is true when ExecutePreTrade resolved with a
// reservation; otherwise rejects carries the engine reject list.
type orderObservation struct {
	accepted bool
	rejects  []reject.Reject
}

// settleObservation is the engine's response to one settlement. blocked is true
// when the report produced any account block; outcomes are the per-asset
// post-trade adjustment outcomes the policy emitted (carrying absolute post-op
// balances).
type settleObservation struct {
	blocked  bool
	outcomes []accountadjustment.Outcome
}

// fundingObservation is the engine's response to one funding adjustment.
// rejected is true when the batch was rejected; outcomes carry the post-op
// balance of the funded asset on accept.
type fundingObservation struct {
	rejected bool
	outcomes []accountadjustment.Outcome
}

// oracle checks every engine response against the generator's prediction and
// (at end of run) the aggregate fund-conservation / no-oversell invariants. It
// is safe for concurrent use by the collector pool. The first divergence is
// recorded and returned to the caller; checking continues so a run reports a
// coherent total, but the run is considered failed once err is set.
//
// The PER-OP checks (checkOrder/checkSettlement/checkFunding) are strict and
// ORDER-INDEPENDENT: each compares one event's precomputed prediction against
// the engine's response, with no shared mutable aggregate state, so the
// open-loop collector pool may run them in any completion order. The AGGREGATE
// invariants are computed once at the end (checkInvariants) by replaying the
// predictions in deterministic emission (Seq) order, so they too are independent
// of the concurrent processing order. This is what lets the strict oracle
// survive true open-loop while staying race-free.
type oracle struct {
	mu sync.Mutex

	firstErr error
	checked  uint64
}

type assetKey struct {
	account string
	asset   string
}

type holdings struct {
	available decimal.Decimal
	held      decimal.Decimal
}

func newOracle() *oracle {
	return &oracle{}
}

// Err returns the first divergence seen, or nil if the engine agreed with every
// prediction.
func (o *oracle) Err() error {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.firstErr
}

// Checked returns how many operations the oracle inspected.
func (o *oracle) Checked() uint64 {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.checked
}

// fail records the first divergence (idempotent: later failures are kept only
// if none was recorded yet). Caller holds o.mu.
func (o *oracle) fail(err error) {
	if o.firstErr == nil {
		o.firstErr = err
	}
}

// failExternal records a non-prediction failure (a build, transport, or
// cancellation error surfaced by the driver) as the run's first error. Unlike
// fail it takes the lock itself, for callers outside a check method.
func (o *oracle) failExternal(err error) {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.fail(err)
}

// checkOrder verifies an order-check outcome against the event's prediction.
// Balances are not directly observable on the order path (the async reservation
// wrapper does not surface AccountAdjustments), so this asserts the decision and
// the reject code only (see package doc). It is order-independent: it compares
// the fixed prediction against the engine response and mutates no shared state
// beyond the locked counter and first-error slot. The event's global Seq names
// the op in any error.
func (o *oracle) checkOrder(ev *generator.Event, obs orderObservation) {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.checked++

	if ev.Accept != obs.accepted {
		o.fail(fmt.Errorf(
			"oracle: order-check mismatch: account %s seq %d corr %d: predicted accept=%t, engine accept=%t (engine rejects: %s)",
			ev.Account, ev.Seq, ev.CorrelationID, ev.Accept, obs.accepted, describeRejects(obs.rejects)))
		return
	}

	if !ev.Accept {
		// Reject: the engine reject code must map to the predicted reason.
		wantCode, ok := orderRejectCodeFor(ev.Reason)
		if !ok {
			o.fail(fmt.Errorf(
				"oracle: order-check reject with unmappable predicted reason %q: account %s seq %d corr %d",
				ev.Reason, ev.Account, ev.Seq, ev.CorrelationID))
			return
		}
		if !containsCode(obs.rejects, wantCode) {
			o.fail(fmt.Errorf(
				"oracle: order-check reject-code mismatch: account %s seq %d corr %d: predicted %q (%v), engine rejects: %s",
				ev.Account, ev.Seq, ev.CorrelationID, ev.Reason, wantCode, describeRejects(obs.rejects)))
			return
		}
	}
}

// checkSettlement verifies a settlement outcome against the event's prediction:
// the decision (a full fill of a previously accepted order must NOT block) and
// the engine-volunteered post-op balances of the two affected legs against the
// predicted Post. Order-independent (compares fixed predictions to the engine
// response).
func (o *oracle) checkSettlement(ev *generator.Event, obs settleObservation) {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.checked++

	if obs.blocked {
		o.fail(fmt.Errorf(
			"oracle: settlement produced an account block: account %s seq %d corr %d (predicted clean settle)",
			ev.Account, ev.Seq, ev.CorrelationID))
		return
	}

	// Index the engine outcomes by asset for an exact per-leg balance compare.
	got := outcomesByAsset(obs.outcomes)
	for i := range ev.Post {
		want := ev.Post[i]
		gotOutcome, ok := got[want.Asset]
		if !ok {
			o.fail(fmt.Errorf(
				"oracle: settlement missing engine outcome for asset %s: account %s seq %d corr %d (predicted available=%s held=%s)",
				want.Asset, ev.Account, ev.Seq, ev.CorrelationID, want.Available, want.Held))
			return
		}
		if err := compareLeg("settlement", ev, want, gotOutcome); err != nil {
			o.fail(err)
			return
		}
	}
}

// checkFunding verifies a funding adjustment against the event's prediction: the
// accept/reject decision and, on accept, the engine-volunteered post-op balance
// of the funded asset against the predicted Post. Order-independent.
func (o *oracle) checkFunding(ev *generator.Event, obs fundingObservation) {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.checked++

	predictedReject := !ev.Accept
	if predictedReject != obs.rejected {
		o.fail(fmt.Errorf(
			"oracle: funding decision mismatch: account %s seq %d asset %s: predicted reject=%t, engine reject=%t",
			ev.Account, ev.Seq, ev.FundingAsset, predictedReject, obs.rejected))
		return
	}

	if obs.rejected {
		// A rejected funding leaves balances untouched: no post-op balance to
		// compare.
		return
	}

	got := outcomesByAsset(obs.outcomes)
	for i := range ev.Post {
		want := ev.Post[i]
		gotOutcome, ok := got[want.Asset]
		if !ok {
			o.fail(fmt.Errorf(
				"oracle: funding missing engine outcome for asset %s: account %s seq %d (predicted available=%s held=%s)",
				want.Asset, ev.Account, ev.Seq, want.Available, want.Held))
			return
		}
		if err := compareLeg("funding", ev, want, gotOutcome); err != nil {
			o.fail(err)
			return
		}
	}
}

// checkInvariants asserts the aggregate fund-conservation and no-oversell
// invariants over the predicted end state. It is called ONCE after the run has
// drained, and it reconstructs the end state by replaying the predictions in
// deterministic emission (Seq) order - NOT from any concurrently-mutated state -
// so it is independent of the order in which the open-loop collectors checked
// the live ops. The predictions are the same authoritative Post values the
// per-op checks asserted the engine reproduced, so a clean per-op run plus a
// clean replay here pins the engine's end state end-to-end.
//
//   - No-oversell: no predicted available or held balance is negative anywhere.
//   - Conservation: for every asset, the sum over all accounts of
//     (available + held) equals the net value that entered the system for that
//     asset, where net value = external funding injections + signed trade flow
//     (a buy spends q*p of settlement cash and gains q of the underlying; a sell
//     does the reverse). The order-check hold is internal to one asset
//     (available->held) so it nets to zero in the per-asset total.
func (o *oracle) checkInvariants(events []generator.Event) error {
	// If a per-op divergence already failed the run, surface it first.
	if err := o.Err(); err != nil {
		return err
	}

	final := make(map[assetKey]holdings)
	expected := make(map[string]decimal.Decimal)

	apply := func(key assetKey, h holdings) { final[key] = h }
	addExpected := func(asset string, delta decimal.Decimal) {
		expected[asset] = expected[asset].Add(delta)
	}

	for i := range events {
		ev := &events[i]
		switch ev.Kind {
		case generator.EventOrderCheck:
			// A hold moves available->held within one asset (nets to zero in the
			// per-asset total); a reject leaves the slot unchanged. Either way Post is
			// the post-op holdings of the charge asset, so just record it.
			for j := range ev.Post {
				b := ev.Post[j]
				apply(assetKey{ev.Account, b.Asset}, holdings{available: b.Available, held: b.Held})
			}
		case generator.EventSettlement:
			// A full fill is an internal value transform between the two trade legs:
			// account the settlement-cash spend and the underlying inventory gain
			// (q*p and q), signed by side.
			notional := ev.Quantity.Mul(ev.Price) // q*p, exact at the pinned scales
			switch ev.Side {
			case generator.SideBuy:
				addExpected(ev.Settlement, notional.Neg())
				addExpected(ev.Underlying, ev.Quantity)
			case generator.SideSell:
				addExpected(ev.Underlying, ev.Quantity.Neg())
				addExpected(ev.Settlement, notional)
			}
			for j := range ev.Post {
				b := ev.Post[j]
				apply(assetKey{ev.Account, b.Asset}, holdings{available: b.Available, held: b.Held})
			}
		case generator.EventFunding:
			// Funding injects external value into the funded asset's available leg
			// (held is never touched). The injection is the change in available
			// relative to the prior holdings; a rejected funding leaves Post == prior
			// so this nets to zero. This captures both Absolute (set) and Delta (add)
			// semantics uniformly. Seeds are funding events too and are included here.
			for j := range ev.Post {
				b := ev.Post[j]
				key := assetKey{ev.Account, b.Asset}
				prior := final[key]
				if ev.Accept {
					addExpected(b.Asset, b.Available.Sub(prior.available))
				}
				apply(key, holdings{available: b.Available, held: b.Held})
			}
		}
	}

	return checkHoldingsConserved(final, expected)
}

// checkHoldingsConserved is the order-independent invariant core: it asserts no
// predicted holding is negative and the per-asset totals equal the expected
// funding + trade flow.
func checkHoldingsConserved(final map[assetKey]holdings, expected map[string]decimal.Decimal) error {
	totals := make(map[string]decimal.Decimal)
	for key, h := range final {
		if h.available.IsNegative() {
			return fmt.Errorf(
				"oracle: oversell invariant: account %s asset %s available=%s is negative",
				key.account, key.asset, h.available)
		}
		if h.held.IsNegative() {
			return fmt.Errorf(
				"oracle: oversell invariant: account %s asset %s held=%s is negative",
				key.account, key.asset, h.held)
		}
		totals[key.asset] = totals[key.asset].Add(h.available).Add(h.held)
	}

	// Compare every asset that appears in either map so a non-zero total with a
	// zero expectation (or vice versa) is caught.
	assets := make(map[string]struct{}, len(totals)+len(expected))
	for a := range totals {
		assets[a] = struct{}{}
	}
	for a := range expected {
		assets[a] = struct{}{}
	}
	for asset := range assets {
		total := totals[asset]
		exp := expected[asset]
		if !total.Equal(exp) {
			return fmt.Errorf(
				"oracle: conservation invariant: asset %s total available+held=%s != expected (funding+trade flow)=%s",
				asset, total, exp)
		}
	}
	return nil
}

// compareLeg asserts the engine's volunteered post-op available/held for one
// affected leg equal the predicted balance exactly. The SpotFundsPolicy sets the
// outcome Absolute fields to the true post-op available()/held(), so an exact
// decimal compare is correct. A field the policy omits (zero delta) means that
// field did not change; the oracle then trusts the predicted value for the
// unchanged field only when the changed field matched, which is sufficient
// because the changed field is what the op moved.
func compareLeg(kind string, ev *generator.Event, want generator.Balance, got accountadjustment.AccountOutcomeEntry) error {
	if av, ok := got.Balance.Get(); ok {
		if !av.Absolute.Decimal().Equal(want.Available) {
			return fmt.Errorf(
				"oracle: %s available mismatch: account %s seq %d corr %d asset %s: predicted %s, engine %s",
				kind, ev.Account, ev.Seq, ev.CorrelationID, want.Asset, want.Available, av.Absolute.String())
		}
	}
	if hd, ok := got.Held.Get(); ok {
		if !hd.Absolute.Decimal().Equal(want.Held) {
			return fmt.Errorf(
				"oracle: %s held mismatch: account %s seq %d corr %d asset %s: predicted %s, engine %s",
				kind, ev.Account, ev.Seq, ev.CorrelationID, want.Asset, want.Held, hd.Absolute.String())
		}
	}
	return nil
}

// outcomesByAsset indexes a policy outcome list by asset code. The spot funds
// policy emits at most one entry per asset, so a later entry for the same asset
// would be a policy contract violation; the last one wins here and the compare
// still detects any inconsistency.
func outcomesByAsset(outcomes []accountadjustment.Outcome) map[string]accountadjustment.AccountOutcomeEntry {
	m := make(map[string]accountadjustment.AccountOutcomeEntry, len(outcomes))
	for i := range outcomes {
		m[outcomes[i].Entry.Asset.String()] = outcomes[i].Entry
	}
	return m
}

func containsCode(rejects []reject.Reject, want reject.Code) bool {
	for i := range rejects {
		if rejects[i].Code == want {
			return true
		}
	}
	return false
}

func describeRejects(rejects []reject.Reject) string {
	if len(rejects) == 0 {
		return "<none>"
	}
	out := ""
	for i := range rejects {
		if i > 0 {
			out += ","
		}
		out += fmt.Sprintf("%v(%s)", rejects[i].Code, rejects[i].Reason)
	}
	return out
}
