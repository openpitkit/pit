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

package generator

import (
	"fmt"

	"github.com/shopspring/decimal"
)

// Side is the order side. Defined here (not imported from the binding) so the
// generator stays engine-independent.
type Side uint8

const (
	// SideBuy charges the settlement asset (q*p).
	SideBuy Side = iota
	// SideSell charges the underlying asset (q).
	SideSell
)

// String renders the side as the contract vocabulary token.
func (s Side) String() string {
	switch s {
	case SideBuy:
		return "BUY"
	case SideSell:
		return "SELL"
	default:
		return "UNKNOWN"
	}
}

// RejectReason is the generator's own reject taxonomy. It is intentionally a
// local enum (not reject.Code from the binding) so the generator never imports
// the engine; Phase 3 maps these reasons to engine reject codes when it checks
// the oracle.
type RejectReason string

const (
	// RejectNone marks an accepted order.
	RejectNone RejectReason = ""
	// RejectInsufficientFunds mirrors the engine's InsufficientFunds: the
	// charge exceeds available on the charge asset (a missing record counts as
	// available = 0).
	RejectInsufficientFunds RejectReason = "InsufficientFunds"
	// RejectAccountAssetNotConfigured is the contract's "no Holdings record"
	// reject. NOTE: the real SpotFundsPolicy never emits a distinct
	// not-configured reject on the order path - it creates a zero slot and
	// reports InsufficientFunds instead. The generator therefore never predicts
	// this on a positive charge (it always seeds the charge asset first); the
	// constant exists only to document the contract clause.
	RejectAccountAssetNotConfigured RejectReason = "SpotAccountAssetNotConfigured"
)

// assetKey identifies one (account, asset) holdings slot. account and asset are
// the engine-facing string keys (FNV-hashed by the binding later); the shadow
// ledger keys on the strings directly.
type assetKey struct {
	account string
	asset   string
}

// Holdings is one (account, asset) balance, IBKR-style: total = available +
// held, both >= 0.
type Holdings struct {
	available decimal.Decimal
	held      decimal.Decimal
}

// ledger is the shadow fund model. It reimplements policy-contract §2 exactly,
// including the engine's prune-when-zero behaviour: a slot whose available and
// held both reach zero is removed, so a missing slot and an all-zero slot are
// indistinguishable (this matters for byte-identical serialisation and for the
// Phase-3 oracle comparing against engine state).
type ledger struct {
	holdings map[assetKey]Holdings
}

func newLedger() *ledger {
	return &ledger{holdings: make(map[assetKey]Holdings)}
}

// get returns the slot and whether it exists. A missing slot reads as zero.
func (l *ledger) get(account, asset string) (Holdings, bool) {
	h, ok := l.holdings[assetKey{account, asset}]
	return h, ok
}

// available returns the available balance for (account, asset); 0 if absent.
func (l *ledger) available(account, asset string) decimal.Decimal {
	if h, ok := l.get(account, asset); ok {
		return h.available
	}
	return decimal.Zero
}

// set stores a slot, pruning it when both legs are zero so the map mirrors the
// engine's remove_if_zero semantics.
func (l *ledger) set(account, asset string, h Holdings) {
	key := assetKey{account, asset}
	if h.available.IsZero() && h.held.IsZero() {
		delete(l.holdings, key)
		return
	}
	l.holdings[key] = h
}

// chargeAsset returns the asset charged for an order and the charge amount,
// per contract §2.1 (v1 subset: limit + quantity).
//
//   - Buy:  charge settlement, amount = q*p.
//   - Sell: charge underlying,  amount = q.
func chargeAsset(side Side, underlying, settlement string, quantity, price decimal.Decimal) (asset string, amount decimal.Decimal) {
	if side == SideBuy {
		return settlement, chargeForBuy(quantity, price)
	}
	return underlying, chargeForSell(quantity)
}

// preTradeResult is the predicted outcome of an order check plus the predicted
// post-op balance of the charge asset.
type preTradeResult struct {
	reason       RejectReason
	chargeAsset  string
	chargeAmount decimal.Decimal
	// postAvailable / postHeld are the charge asset's balance AFTER the hold on
	// accept. On reject they are the unchanged current balance.
	postAvailable decimal.Decimal
	postHeld      decimal.Decimal
}

func (r preTradeResult) accepted() bool { return r.reason == RejectNone }

// preTrade mirrors SpotFundsPolicy::execute_pre_trade for one order
// (contract §2.2): compute the charge, then try_hold on the charge asset. A
// zero charge is a clean no-op accept (the engine reserves nothing). On accept
// it moves available -= charge, held += charge and commits the slot.
//
// The reject test below matches the authoritative Holdings::try_hold v1 path
// because held is always >= 0: funding touches available only, and exact full
// fills consume exactly the reserved held amount. The core's full rule is
// amount > available + min(held, 0); if a future variant allows negative held,
// this shadow rule must be extended to match.
func (l *ledger) preTrade(account string, side Side, underlying, settlement string, quantity, price decimal.Decimal) preTradeResult {
	asset, charge := chargeAsset(side, underlying, settlement, quantity, price)
	cur, _ := l.get(account, asset)

	// Zero charge: accept without touching the slot (matches reserve_leg's
	// is_zero early return).
	if charge.IsZero() {
		return preTradeResult{
			reason:        RejectNone,
			chargeAsset:   asset,
			chargeAmount:  charge,
			postAvailable: cur.available,
			postHeld:      cur.held,
		}
	}

	// try_hold: reject when charge > available (missing slot => available 0).
	if charge.GreaterThan(cur.available) {
		return preTradeResult{
			reason:        RejectInsufficientFunds,
			chargeAsset:   asset,
			chargeAmount:  charge,
			postAvailable: cur.available,
			postHeld:      cur.held,
		}
	}

	next := Holdings{
		available: cur.available.Sub(charge),
		held:      cur.held.Add(charge),
	}
	l.set(account, asset, next)
	return preTradeResult{
		reason:        RejectNone,
		chargeAsset:   asset,
		chargeAmount:  charge,
		postAvailable: next.available,
		postHeld:      next.held,
	}
}

// settlementResult is the predicted post-settlement balance of both affected
// assets after a full fill.
type settlementResult struct {
	// chargeAsset / underlyingAsset name the two slots a fill moves.
	heldAsset    string // asset whose held is consumed (settlement for Buy, underlying for Sell)
	creditAsset  string // asset credited to available (underlying for Buy, settlement for Sell)
	heldPost     Holdings
	creditPost   Holdings
	creditAmount decimal.Decimal
}

// settleFullFill mirrors SpotFundsPolicy fill handling (contract §2.3) for a
// full fill (leaves = 0, is_final = true):
//
//   - Buy:  held(settlement) -= q*p; available(underlying) += q.
//   - Sell: held(underlying) -= q;  available(settlement) += q*p.
//
// It must be called only for a previously accepted order so the held leg has
// the matching reservation to consume; the amounts equal the reserved charge.
func (l *ledger) settleFullFill(account string, side Side, underlying, settlement string, quantity, price decimal.Decimal) (settlementResult, error) {
	notional := chargeForBuy(quantity, price) // q*p, exact

	var heldAsset, creditAsset string
	var consume, credit decimal.Decimal
	if side == SideBuy {
		heldAsset, creditAsset = settlement, underlying
		consume, credit = notional, quantity
	} else {
		heldAsset, creditAsset = underlying, settlement
		consume, credit = quantity, notional
	}

	held, ok := l.get(account, heldAsset)
	if !ok || consume.GreaterThan(held.held) {
		// Would drive held negative - the generator must never schedule a fill
		// without a matching reservation; surface loudly rather than silently.
		return settlementResult{}, fmt.Errorf(
			"settlement underflow: account %s asset %s consume %s exceeds held %s",
			account, heldAsset, consume.String(), held.held.String())
	}
	heldNext := Holdings{available: held.available, held: held.held.Sub(consume)}
	l.set(account, heldAsset, heldNext)

	creditCur, _ := l.get(account, creditAsset)
	creditNext := Holdings{available: creditCur.available.Add(credit), held: creditCur.held}
	l.set(account, creditAsset, creditNext)

	return settlementResult{
		heldAsset:    heldAsset,
		creditAsset:  creditAsset,
		heldPost:     heldNext,
		creditPost:   creditNext,
		creditAmount: credit,
	}, nil
}

// fundingKind selects the adjustment semantics, mirroring contract §2.4.
type fundingKind uint8

const (
	// fundingAbsolute sets available = amount unconditionally; negative amount
	// is permitted (matches Rust AdjustmentAmount::Absolute). held is never
	// touched. A missing record is created (available = amount, held = 0).
	fundingAbsolute fundingKind = iota
	// fundingDelta adds amount to available unconditionally; the result may go
	// negative (matches Rust AdjustmentAmount::Delta). held is never touched. A
	// missing record is treated as zero (available = 0 + amount).
	fundingDelta
)

func (k fundingKind) String() string {
	if k == fundingAbsolute {
		return "Absolute"
	}
	return "Delta"
}

// fundingResult is the predicted post-funding balance of the funded asset.
type fundingResult struct {
	reason   RejectReason
	post     Holdings
	rejected bool
}

// applyFunding mirrors the AccountAdjustment balance operation on available
// (contract §2.4). held is never touched.
//
// Engine-faithful rules (Rust core is authoritative):
//   - fundingAbsolute: sets available = amount unconditionally; negative
//     amount is accepted. A missing record is created (held = 0).
//   - fundingDelta: available += amount unconditionally; the result may be
//     negative. A missing record is treated as zero before the delta is
//     applied (i.e. available = 0 + amount). No not-configured reject.
//
// Only true decimal-range arithmetic overflow would reject; that is not
// emulated here (the loadtest value space never reaches it).
func (l *ledger) applyFunding(account, asset string, kind fundingKind, amount decimal.Decimal) fundingResult {
	cur, _ := l.get(account, asset)

	switch kind {
	case fundingAbsolute:
		next := Holdings{available: amount, held: cur.held}
		l.set(account, asset, next)
		return fundingResult{post: next}
	case fundingDelta:
		// Missing record reads as zero; delta is applied regardless.
		next := Holdings{available: cur.available.Add(amount), held: cur.held}
		l.set(account, asset, next)
		return fundingResult{post: next}
	default:
		return fundingResult{reason: RejectAccountAssetNotConfigured, rejected: true, post: cur}
	}
}
