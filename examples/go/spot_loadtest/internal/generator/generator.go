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

// Package generator builds a seeded, deterministic, pre-materialised stream of
// abstract load-test events for the spot-limit harness. It is an INDEPENDENT
// reimplementation of the spot-funds arithmetic - it never imports the engine
// binding - which is what makes the Phase-3 oracle non-circular.
//
// The generator maintains a shadow ledger (funds) and a position lifecycle, and
// for every order it PREDICTS the engine's accept/reject decision and resulting
// balances. Those predictions, serialised with the stream, are the oracle the
// driver later checks the live engine against.
package generator

import (
	"fmt"

	"github.com/shopspring/decimal"

	"openpit-loadtest-spot-funds-go/internal/config"
)

// priceGrid is the deterministic per-instrument limit-price table, in integer
// cents so prices are always exact at priceScale. Phase 3 maps these to
// param.Price via NewPriceFromString on the rendered decimal.
//
// Prices are assigned per symbol from a fixed grid so the same config yields
// the same prices; values are classic two-decimal equity ticks.
var priceCentsGrid = []uint64{
	5000, 7500, 10000, 12500, 15000, 20000, 25000, 30000, 42500, 99900,
}

// generator owns all mutable simulation state and the RNG. Everything it does
// is a pure function of (config, seed), so the emitted stream is deterministic.
type generator struct {
	cfg  *config.Config
	pop  *population
	led  *ledger
	life *lifecycle
	g    *rng

	// prices is the per-symbol limit price (exact decimal at priceScale).
	prices map[string]decimal.Decimal

	// ctrl is the offline reject-rate controller.
	ctrl *rejectController

	// active is the bounded active working set (one entry per live slot);
	// activeIdx is the set of account indices currently in it, for O(1) "is this
	// account active?" lookups during admission.
	active    []activeSlot
	activeIdx map[int]struct{}

	events []Event
	stats  StreamStats
	seq    uint64
	corr   uint64
}

// Generate builds the full deterministic event stream for cfg. The same cfg
// (including Run.Seed) always yields a byte-identical serialised stream and
// identical predictions.
func Generate(cfg *config.Config) (*Stream, error) {
	pop, err := buildPopulation(cfg)
	if err != nil {
		return nil, fmt.Errorf("generator: %w", err)
	}

	gen := &generator{
		cfg:    cfg,
		pop:    pop,
		led:    newLedger(),
		life:   newLifecycle(cfg.Lifecycle),
		g:      newRNG(cfg.Run.Seed),
		prices: assignPrices(pop.symbols),
		ctrl:   newRejectController(cfg.Reject.TargetRate),
	}

	gen.seedAccounts()

	if err := gen.run(); err != nil {
		return nil, fmt.Errorf("generator: %w", err)
	}

	// Assign each event its virtual arrival time on the offline causal timeline.
	// This is a separate pass over the already-emitted events using a dedicated
	// schedule RNG, so the content stays byte-identical and only VirtualT0 is
	// added (see assignVirtualTimes).
	gen.assignVirtualTimes()

	return &Stream{Events: gen.events, Stats: gen.stats}, nil
}

// assignPrices binds each symbol to a deterministic limit price from the grid.
func assignPrices(symbols []string) map[string]decimal.Decimal {
	prices := make(map[string]decimal.Decimal, len(symbols))
	for i, sym := range symbols {
		cents := priceCentsGrid[i%len(priceCentsGrid)]
		prices[sym] = priceFromCents(cents)
	}
	return prices
}

// seedAccounts emits an Absolute funding event seeding every account's
// settlement balance, so no account starves and the charge asset is always
// configured before its first order (contract §2.4 seeding). These events are
// flagged as seeds: the driver applies them synchronously on the underlying
// engine before the async run (seeding is setup, not measured load), while the
// shadow oracle still verifies their predicted post-seed balances.
func (gen *generator) seedAccounts() {
	settle := gen.cfg.Instruments.Settlement
	seed := gen.cfg.Funding.Seed
	for i := range gen.pop.accounts {
		acc := gen.pop.accounts[i].id
		res := gen.led.applyFunding(acc, settle, fundingAbsolute, seed)
		gen.emitFundingSeed(acc, settle, fundingAbsolute, seed, res, true)
	}
}

// run is the main loop. It emits order-checks until the configured order-check
// budget is reached, interleaving settlements for accepted orders and
// self-funding top-ups.
//
// Bounded-concurrency arrival model. Modelling every account as hot at once is
// unrealistic, so the scheduler keeps a bounded ACTIVE WORKING SET of at most
// cfg.Concurrency.ActiveAccounts accounts. Only accounts in that set wake and
// fire bursts; an account stays active for a short random dwell (a handful of
// wakes), then goes idle and is evicted, and a replacement is admitted from the
// dormant pool (weighted so chatty cohorts cycle back often and the dormant
// majority almost never). Consequently the number of DISTINCT accounts active
// within any short window stays bounded near ActiveAccounts, which bounds the
// engine's live per-account dispatch queues. The whole schedule is driven by the
// single RNG consumed in a fixed order, so it stays a pure function of
// (seed, config) and the stream remains byte-identical for a given seed.
func (gen *generator) run() error {
	target := gen.cfg.Run.TotalOps
	if target == 0 {
		// Duration-based runs are a Phase-6 concern; size a sensible default so
		// the generator still produces a usable stream when only duration is set.
		target = defaultOrderCheckBudget
	}

	gen.initActiveSet()

	// Each step wakes one currently-active account. When the active set cannot be
	// populated (no admissible accounts) the loop would spin; that cannot happen
	// because ActiveAccounts >= 1 and the population is non-empty (both validated
	// upstream), so at least one slot always holds a live account.
	for gen.stats.OrderChecks < target {
		slot := gen.g.intn(len(gen.active))
		acc := &gen.pop.accounts[gen.active[slot].account]
		if err := gen.wake(acc, target); err != nil {
			return err
		}
		gen.advanceSlot(slot)
	}
	return nil
}

// defaultOrderCheckBudget bounds duration-only configs (Phase 6 wires real
// wall-clock budgeting; here we just need a finite, deterministic stream).
const defaultOrderCheckBudget = 100000

// dwellWakesMin / dwellWakesSpread bound how many wakes an admitted account
// stays in the active set before it goes idle and is evicted. A short, bounded
// dwell with a random spread models the wake -> burst -> idle cycle and keeps
// accounts churning through the bounded active set so distinct accounts touched
// over the whole run still greatly exceed the active-set size.
const (
	dwellWakesMin    = 2
	dwellWakesSpread = 6
)

// activeSlot is one slot of the bounded active working set: the account index
// currently occupying it and the remaining number of wakes before it idles.
type activeSlot struct {
	account int
	dwell   int
}

// initActiveSet fills every active slot with a freshly admitted account. The
// active-set size is min(ActiveAccounts, population), clamped to at least one.
// ActiveAccounts == 0 means "no bound": the active set is the whole population
// (used by code-built test configs that omit the knob; real configs always set
// it). Even then the wake/dwell churn keeps the schedule a pure function of
// (seed, config).
func (gen *generator) initActiveSet() {
	n := len(gen.pop.accounts)
	size := int(gen.cfg.Concurrency.ActiveAccounts) //nolint:gosec // active set is a configured bound well below int max
	if size <= 0 || size > n {
		size = n
	}
	gen.active = make([]activeSlot, size)
	gen.activeIdx = make(map[int]struct{}, size)
	for i := range gen.active {
		acc := gen.pop.admitAccount(gen.g, gen.isActive)
		gen.active[i] = activeSlot{account: acc, dwell: gen.nextDwell()}
		gen.activeIdx[acc] = struct{}{}
	}
}

// advanceSlot decrements the slot's dwell and, when it reaches zero, evicts the
// account (it goes idle) and admits a replacement drawn from the dormant pool.
// This is what keeps the active set bounded while letting accounts cycle in and
// out over the run.
func (gen *generator) advanceSlot(slot int) {
	gen.active[slot].dwell--
	if gen.active[slot].dwell > 0 {
		return
	}
	delete(gen.activeIdx, gen.active[slot].account)
	acc := gen.pop.admitAccount(gen.g, gen.isActive)
	if acc < 0 {
		// Every account is active (active set == population). Keep the slot's
		// current occupant active by re-seeding its dwell rather than emptying
		// the slot; the bound still holds because no new distinct account enters.
		acc = gen.active[slot].account
	}
	gen.active[slot] = activeSlot{account: acc, dwell: gen.nextDwell()}
	gen.activeIdx[acc] = struct{}{}
}

// isActive reports whether account index i currently occupies an active slot.
func (gen *generator) isActive(i int) bool {
	_, ok := gen.activeIdx[i]
	return ok
}

// nextDwell draws a bounded dwell length (in wakes) for a newly admitted
// account. One RNG draw, consumed in stream order, so determinism holds.
func (gen *generator) nextDwell() int {
	return dwellWakesMin + gen.g.intn(dwellWakesSpread)
}

// wake runs one account's burst. The cohort's activity gates the whole wake;
// each order in the burst runs the lifecycle, sizing, self-funding check, the
// reject controller, the shadow pre-trade, and (on accept) a settlement.
func (gen *generator) wake(acc *account, target uint64) error {
	co := &gen.pop.cohorts[acc.cohort]

	// Activity gate: an idle wake still advances the loop (it consumes an RNG
	// draw) but emits no order, modelling dormancy deterministically.
	if !gen.g.bernoulli(co.cfg.Activity) {
		return nil
	}

	for b := uint64(0); b < co.cfg.BurstLen && gen.stats.OrderChecks < target; b++ {
		if err := gen.oneOrder(acc); err != nil {
			return err
		}
	}
	return nil
}

// oneOrder generates a single order-check (and its settlement on accept).
func (gen *generator) oneOrder(acc *account) error {
	co := &gen.pop.cohorts[acc.cohort]

	// Pick instrument and price.
	symIdx := gen.pop.pickSymbol(gen.g, acc.cohort)
	underlying := gen.pop.symbols[symIdx]
	settle := gen.cfg.Instruments.Settlement
	price := gen.prices[underlying]

	// Self-funding: before deciding, top up if the account's settlement
	// available is at/below the configured threshold (contract §2.4 top-up).
	gen.maybeTopUp(acc.id, settle)

	// Lifecycle picks the action from the current position state.
	act := gen.life.decide(gen.g, acc.id, underlying)
	if act == actionIdle {
		return nil
	}

	side, lots := gen.sizeOrder(acc, underlying, settle, price, act)
	if lots == 0 {
		return nil
	}
	quantity := quantityDecimal(lots)

	// Reject controller decides whether to force this order into a predicted
	// reject by oversizing it just past available.
	forced := false
	if side == SideBuy && gen.ctrl.shouldForce(gen.g, co.cfg.RejectPropensity) {
		lots2, ok := gen.oversizeBuy(acc.id, settle, price)
		if ok {
			lots = lots2
			quantity = quantityDecimal(lots)
			forced = true
		}
	}

	corr := gen.nextCorr()
	res := gen.led.preTrade(acc.id, side, underlying, settle, quantity, price)
	gen.emitOrderCheck(acc.id, underlying, settle, side, quantity, price, corr, res)
	gen.ctrl.observe(res.accepted())

	if forced {
		gen.stats.ForcedRejects++
	} else if !res.accepted() {
		gen.stats.NaturalRejects++
	}

	if !res.accepted() {
		// Rejects leave funds and position untouched (the engine rolls back).
		return nil
	}

	// Accepted: advance the lifecycle and emit the matching full-fill
	// settlement, so the shadow position and funds track the engine.
	if side == SideBuy {
		gen.life.applyOpenOrAdd(acc.id, underlying, lots)
	} else {
		gen.life.applyClose(acc.id, underlying, lots)
	}

	settleRes, err := gen.led.settleFullFill(acc.id, side, underlying, settle, quantity, price)
	if err != nil {
		return err
	}
	gen.emitSettlement(acc.id, underlying, settle, side, quantity, price, corr, settleRes)
	return nil
}

// sizeOrder chooses the side and lot count for an action. Opens/adds are Buys
// sized from the cohort's size distribution but capped so the charge fits
// available (keeping natural rejects near zero - the controller owns reject
// rate). Closes are Sells sized from the current position by the lifecycle.
func (gen *generator) sizeOrder(acc *account, underlying, settle string, price decimal.Decimal, act action) (Side, uint64) {
	switch act {
	case actionOpen, actionAdd:
		lots := gen.pop.pickSize(gen.g, acc.cohort)
		return SideBuy, gen.capBuyToAvailable(acc.id, settle, price, lots)
	case actionPartialClose:
		return SideSell, gen.life.closeLots(gen.g, acc.id, underlying, false)
	case actionFullClose:
		return SideSell, gen.life.closeLots(gen.g, acc.id, underlying, true)
	default:
		return SideBuy, 0
	}
}

// capBuyToAvailable reduces a desired Buy lot count so that q*p <= available on
// the settlement asset. Returns 0 when even one lot does not fit (the caller
// treats 0 lots as "skip this order"). This keeps unforced Buys acceptable.
func (gen *generator) capBuyToAvailable(account, settle string, price decimal.Decimal, lots uint64) uint64 {
	avail := gen.led.available(account, settle)
	if price.IsZero() {
		return lots // a zero price charges nothing; any size fits
	}
	// Max lots that fit: floor(available / price).
	maxLots := avail.Div(price).Floor()
	if !maxLots.IsPositive() {
		return 0
	}
	maxU := maxLots.BigInt().Uint64()
	if lots > maxU {
		return maxU
	}
	return lots
}

// oversizeBuy returns a Buy lot count whose charge strictly exceeds available
// on the settlement asset, so the shadow pre-trade predicts InsufficientFunds.
// Returns ok=false when no integer oversize is representable (e.g. zero price).
func (gen *generator) oversizeBuy(account, settle string, price decimal.Decimal) (uint64, bool) {
	if price.IsZero() {
		return 0, false // a zero charge can never exceed available
	}
	avail := gen.led.available(account, settle)
	// Smallest lots with lots*price > available is floor(available/price)+1.
	lots := avail.Div(price).Floor().Add(decimal.NewFromInt(1))
	if !lots.IsPositive() {
		return 0, false
	}
	return lots.BigInt().Uint64(), true
}

// maybeTopUp injects a self-funding top-up when the account's settlement
// available is at or below the configured threshold (contract §2.4).
//
// It picks the adjustment kind by record presence, mirroring how a real funding
// system seeds-then-tops-up. When the settlement slot is absent (e.g. it was
// pruned after a reservation fully drained and settled), the top-up is an
// Absolute(seed) that restores the full seed. A Delta on a missing slot would
// create a zero slot and add only top_up; it would not reject. Otherwise the
// live balance receives Delta(top_up). Either way the prediction matches what
// the engine's AccountAdjustment pipeline would do, and the account never
// starves.
func (gen *generator) maybeTopUp(account, settle string) {
	if gen.cfg.Funding.Trigger != config.FundingBalanceBelow {
		return
	}
	cur, exists := gen.led.get(account, settle)
	if exists && cur.available.GreaterThan(gen.cfg.Funding.Threshold) {
		return
	}
	if !exists {
		// Re-seed a missing record with Absolute to restore the full seed.
		amount := gen.cfg.Funding.Seed
		res := gen.led.applyFunding(account, settle, fundingAbsolute, amount)
		gen.emitFunding(account, settle, fundingAbsolute, amount, res)
		return
	}
	amount := gen.cfg.Funding.TopUp
	res := gen.led.applyFunding(account, settle, fundingDelta, amount)
	gen.emitFunding(account, settle, fundingDelta, amount, res)
}

// --- event emission helpers (each records stats + appends one Event) ---

func (gen *generator) emitOrderCheck(account, underlying, settle string, side Side, quantity, price decimal.Decimal, corr uint64, res preTradeResult) {
	gen.stats.OrderChecks++
	if res.accepted() {
		gen.stats.Accepts++
	} else {
		gen.stats.Rejects++
	}
	gen.events = append(gen.events, Event{
		Seq:           gen.nextSeq(),
		Kind:          EventOrderCheck,
		Account:       account,
		Underlying:    underlying,
		Settlement:    settle,
		Side:          side,
		Quantity:      quantity,
		Price:         price,
		CorrelationID: corr,
		Accept:        res.accepted(),
		Reason:        res.reason,
		Post: []Balance{
			{Asset: res.chargeAsset, Available: res.postAvailable, Held: res.postHeld},
		},
	})
}

func (gen *generator) emitSettlement(account, underlying, settle string, side Side, quantity, price decimal.Decimal, corr uint64, res settlementResult) {
	gen.stats.Settlements++
	gen.events = append(gen.events, Event{
		Seq:           gen.nextSeq(),
		Kind:          EventSettlement,
		Account:       account,
		Underlying:    underlying,
		Settlement:    settle,
		Side:          side,
		Quantity:      quantity,
		Price:         price,
		CorrelationID: corr,
		Post: []Balance{
			{Asset: res.heldAsset, Available: res.heldPost.available, Held: res.heldPost.held},
			{Asset: res.creditAsset, Available: res.creditPost.available, Held: res.creditPost.held},
		},
	})
}

// emitFunding records a runtime (non-seed) top-up funding event.
func (gen *generator) emitFunding(account, asset string, kind fundingKind, amount decimal.Decimal, res fundingResult) {
	gen.emitFundingSeed(account, asset, kind, amount, res, false)
}

// emitFundingSeed records a funding event, flagging whether it is an initial
// seed (applied synchronously by the driver) or a runtime top-up (applied via
// the measured async pipeline).
func (gen *generator) emitFundingSeed(account, asset string, kind fundingKind, amount decimal.Decimal, res fundingResult, seed bool) {
	gen.stats.Fundings++
	if seed {
		gen.stats.Seeds++
	}
	gen.events = append(gen.events, Event{
		Seq:           gen.nextSeq(),
		Kind:          EventFunding,
		Account:       account,
		FundingKind:   kind,
		FundingAsset:  asset,
		FundingAmount: amount,
		FundingIsSeed: seed,
		Accept:        !res.rejected,
		Reason:        res.reason,
		Post: []Balance{
			{Asset: asset, Available: res.post.available, Held: res.post.held},
		},
	})
}

func (gen *generator) nextSeq() uint64 {
	s := gen.seq
	gen.seq++
	return s
}

func (gen *generator) nextCorr() uint64 {
	gen.corr++
	return gen.corr
}
