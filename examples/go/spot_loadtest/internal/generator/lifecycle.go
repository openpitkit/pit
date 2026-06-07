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

import "openpit-loadtest-spot-funds-go/internal/config"

// action is the lifecycle transition chosen for one wake of an
// (account, instrument) pair.
type action uint8

const (
	// actionOpen establishes a new long position (Buy) when flat.
	actionOpen action = iota
	// actionAdd increases an existing long position (Buy).
	actionAdd
	// actionPartialClose sells part of an existing long position.
	actionPartialClose
	// actionFullClose sells the entire existing long position.
	actionFullClose
	// actionIdle emits no order this wake.
	actionIdle
)

// positionState is the shadow position for one (account, instrument) pair.
// v1 is long-only spot: qty is the net long lot count, always >= 0.
type positionState struct {
	qty uint64
}

// flat reports whether the position is empty.
func (p positionState) flat() bool { return p.qty == 0 }

// posKey identifies one (account, instrument) position slot.
type posKey struct {
	account    string
	underlying string
}

// lifecycle holds every position and the configured transition probabilities.
// It chooses the next action from the current state so transitions are always
// valid (no close beyond the position, no add to a flat book).
type lifecycle struct {
	cfg       config.Lifecycle
	positions map[posKey]positionState
}

func newLifecycle(cfg config.Lifecycle) *lifecycle {
	return &lifecycle{cfg: cfg, positions: make(map[posKey]positionState)}
}

// state returns the current position for the pair (zero if absent).
func (l *lifecycle) state(account, underlying string) positionState {
	return l.positions[posKey{account, underlying}]
}

// decide picks a valid action for the pair given its current state, using the
// configured transition probabilities. The set of admissible actions depends on
// the state: when flat the only position-changing move is open; when long the
// moves are add / partial-close / full-close. Probabilities are applied within
// the admissible set so a transition can never violate the state machine.
//
// Selection is a single weighted draw over the admissible actions, weighted by
// the configured probabilities (idle takes the residual). This keeps one RNG
// draw per decision and makes the mix track the configured ratios.
func (l *lifecycle) decide(g *rng, account, underlying string) action {
	st := l.state(account, underlying)

	if st.flat() {
		// Flat: open with p_open, else idle.
		if g.bernoulli(l.cfg.POpen) {
			return actionOpen
		}
		return actionIdle
	}

	// Long: weighted choice among add / partial / full / idle.
	weights := make([]float64, 0, 4)
	weights = append(weights, l.cfg.PAdd, l.cfg.PPartialClose, l.cfg.PFullClose)
	acts := make([]action, 0, 4)
	acts = append(acts, actionAdd, actionPartialClose, actionFullClose)
	residual := 1.0 - sum(weights)
	if residual < 0 {
		residual = 0 // probabilities over-subscribe; treat idle as impossible
	}
	weights = append(weights, residual)
	acts = append(acts, actionIdle)

	idx := g.pickWeighted(cumulative(weights))
	return acts[idx]
}

// applyOpenOrAdd records lots added to the position by an accepted Buy fill.
func (l *lifecycle) applyOpenOrAdd(account, underlying string, lots uint64) {
	key := posKey{account, underlying}
	st := l.positions[key]
	st.qty += lots
	l.positions[key] = st
}

// applyClose records lots removed by an accepted Sell fill. lots must not
// exceed the current position (the generator sizes closes from state).
func (l *lifecycle) applyClose(account, underlying string, lots uint64) {
	key := posKey{account, underlying}
	st := l.positions[key]
	if lots >= st.qty {
		delete(l.positions, key)
		return
	}
	st.qty -= lots
	l.positions[key] = st
}

// closeLots returns the number of lots to sell for a partial or full close.
// A full close sells the whole position; a partial close sells a random
// fraction in [1, qty-1] when qty > 1, or the whole single lot otherwise.
func (l *lifecycle) closeLots(g *rng, account, underlying string, full bool) uint64 {
	st := l.state(account, underlying)
	if st.flat() {
		return 0
	}
	if full || st.qty == 1 {
		return st.qty
	}
	// Partial: sell between 1 and qty-1 lots inclusive.
	return uint64(g.intn(int(st.qty-1))) + 1 //nolint:gosec // qty bounded by accumulated fills
}
