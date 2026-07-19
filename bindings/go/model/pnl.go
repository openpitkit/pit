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

package model

import (
	"errors"
	"fmt"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
)

// PnlHaltReason identifies why a realized-PnL value could not be calculated.
type PnlHaltReason uint8

const (
	// PnlHaltReasonMissingFx means that a required FX quote was unavailable.
	PnlHaltReasonMissingFx = PnlHaltReason(native.PnlHaltReasonMissingFx)
	// PnlHaltReasonMissingAccountCurrency means that the account currency
	// required to calculate account PnL was unavailable.
	PnlHaltReasonMissingAccountCurrency = PnlHaltReason(
		native.PnlHaltReasonMissingAccountCurrency,
	)
	// PnlHaltReasonMissingInitialPnl means that an authoritative initial PnL
	// was unavailable for a position accumulator.
	PnlHaltReasonMissingInitialPnl = PnlHaltReason(
		native.PnlHaltReasonMissingInitialPnl,
	)
	// PnlHaltReasonMissingCostBasis means that a position did not have the cost
	// basis required to calculate position realized PnL.
	PnlHaltReasonMissingCostBasis = PnlHaltReason(
		native.PnlHaltReasonMissingCostBasis,
	)
	// PnlHaltReasonArithmeticOverflow means that exact PnL arithmetic exceeded
	// the supported numeric range for the affected accumulator.
	PnlHaltReasonArithmeticOverflow = PnlHaltReason(
		native.PnlHaltReasonArithmeticOverflow,
	)
)

// ErrInvalidPnlHaltReason identifies an unsupported or non-halted reason.
var ErrInvalidPnlHaltReason = errors.New("model: invalid PnL halt reason")

// Valid reports whether the value identifies a supported halt reason.
func (r PnlHaltReason) Valid() bool {
	switch r {
	case PnlHaltReasonMissingFx,
		PnlHaltReasonMissingAccountCurrency,
		PnlHaltReasonMissingInitialPnl,
		PnlHaltReasonMissingCostBasis,
		PnlHaltReasonArithmeticOverflow:
		return true
	default:
		return false
	}
}

// String returns a stable human-readable halt reason.
func (r PnlHaltReason) String() string {
	switch r {
	case PnlHaltReasonMissingFx:
		return "missing-fx"
	case PnlHaltReasonMissingAccountCurrency:
		return "missing-account-currency"
	case PnlHaltReasonMissingInitialPnl:
		return "missing-initial-pnl"
	case PnlHaltReasonMissingCostBasis:
		return "missing-cost-basis"
	case PnlHaltReasonArithmeticOverflow:
		return "arithmetic-overflow"
	default:
		return fmt.Sprintf("PnlHaltReason(%d)", uint8(r))
	}
}

// PnlState is an explicit PnL accumulator state: either an authoritative
// numeric value or a halt reason.
type PnlState struct {
	value      param.Pnl
	haltReason PnlHaltReason
}

// NewPnlState constructs an authoritative numeric PnL state.
func NewPnlState(value param.Pnl) PnlState {
	return PnlState{value: value}
}

// NewPnlHaltedState constructs a halted PnL state.
func NewPnlHaltedState(reason PnlHaltReason) (PnlState, error) {
	if !reason.Valid() {
		return PnlState{}, fmt.Errorf("%w: %d", ErrInvalidPnlHaltReason, reason)
	}
	return PnlState{haltReason: reason}, nil
}

// NewPnlStateFromHandle creates a PnlState from a native value.
func NewPnlStateFromHandle(state native.PnlState) PnlState {
	if native.PnlStateGetKind(state) == native.PnlStateKindHalted {
		return PnlState{haltReason: PnlHaltReason(native.PnlStateGetHaltReason(state))}
	}
	return NewPnlState(param.NewPnlFromHandle(native.PnlStateGetValue(state)))
}

// Value returns the authoritative PnL and true for a numeric state.
func (s PnlState) Value() (param.Pnl, bool) {
	return s.value, !s.IsHalted()
}

// HaltReason returns the halt reason and true for a halted state.
func (s PnlState) HaltReason() (PnlHaltReason, bool) {
	return s.haltReason, s.IsHalted()
}

// IsHalted reports whether PnL calculation is halted.
func (s PnlState) IsHalted() bool {
	return s.haltReason != PnlHaltReason(native.PnlHaltReasonNone)
}

// Handle returns the native value.
func (s PnlState) Handle() native.PnlState {
	if s.IsHalted() {
		return native.NewPnlStateHalted(native.PnlHaltReason(s.haltReason))
	}
	return native.NewPnlStateValue(s.value.Handle())
}
