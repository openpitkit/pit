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

package accountadjustment

import (
	"fmt"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
)

// PnlOutcome is a realized-PnL result: either PnlOutcomeAmount or the reason
// why it could not be calculated.
type PnlOutcome struct {
	pnl        PnlOutcomeAmount
	haltReason model.PnlHaltReason
}

// NewPnlOutcome constructs an authoritative realized-PnL result.
func NewPnlOutcome(pnl PnlOutcomeAmount) PnlOutcome {
	return PnlOutcome{pnl: pnl}
}

// NewPnlHaltedOutcome constructs a realized-PnL halt result.
func NewPnlHaltedOutcome(reason model.PnlHaltReason) (PnlOutcome, error) {
	if !reason.Valid() {
		return PnlOutcome{}, fmt.Errorf("%w: %d", model.ErrInvalidPnlHaltReason, reason)
	}
	return PnlOutcome{haltReason: reason}, nil
}

// Amount returns the authoritative PnL and true for a computed outcome.
func (o PnlOutcome) Amount() (PnlOutcomeAmount, bool) {
	return o.pnl, !o.IsHalted()
}

// HaltReason returns the halt reason and true for a halted outcome.
func (o PnlOutcome) HaltReason() (model.PnlHaltReason, bool) {
	return o.haltReason, o.IsHalted()
}

// IsHalted reports whether this accumulator could not calculate PnL.
func (o PnlOutcome) IsHalted() bool {
	return o.haltReason != model.PnlHaltReason(native.PnlHaltReasonNone)
}

// NewPnlOutcomeFromHandle creates a PnlOutcome from a native handle.
func NewPnlOutcomeFromHandle(handle native.PnlOutcome) PnlOutcome {
	haltReason := model.PnlHaltReason(native.PnlOutcomeGetHaltReason(handle))
	pnl := PnlOutcomeAmount{
		Delta:    param.NewPnlZero(),
		Absolute: param.NewPnlZero(),
	}
	if haltReason == model.PnlHaltReason(native.PnlHaltReasonNone) {
		pnl = NewPnlOutcomeAmountFromHandle(native.PnlOutcomeGetAmount(handle))
	}
	return PnlOutcome{pnl: pnl, haltReason: haltReason}
}

func (o PnlOutcome) newNativeHandle() native.PnlOutcome {
	amount := native.NewPnlOutcomeAmountOptionalUnset()
	if !o.IsHalted() {
		amount = native.NewPnlOutcomeAmountOptionalSet(o.pnl.NewHandle())
	}
	return native.NewPnlOutcome(native.PnlHaltReason(o.haltReason), amount)
}

// AccountPnlOutcome is a policy-tagged account-level realized-PnL result.
// SpotFunds emits a halted outcome only for the report that transitions the
// account accumulator to halted. Later reports omit the unchanged halt until
// the account PnL is explicitly force-set. Position force-sets do not re-arm
// it.
type AccountPnlOutcome struct {
	PnlOutcome

	// PolicyGroupID identifies the policy group that produced the outcome.
	PolicyGroupID model.PolicyGroupID
	// AccountID identifies the account whose PnL was considered.
	AccountID param.AccountID
}

// NewAccountPnlOutcome constructs an authoritative account-level PnL result.
func NewAccountPnlOutcome(
	policyGroupID model.PolicyGroupID,
	accountID param.AccountID,
	pnl PnlOutcomeAmount,
) AccountPnlOutcome {
	return AccountPnlOutcome{
		PnlOutcome:    NewPnlOutcome(pnl),
		PolicyGroupID: policyGroupID,
		AccountID:     accountID,
	}
}

// NewAccountPnlHaltedOutcome constructs a halted account-level PnL result.
func NewAccountPnlHaltedOutcome(
	policyGroupID model.PolicyGroupID,
	accountID param.AccountID,
	reason model.PnlHaltReason,
) (AccountPnlOutcome, error) {
	pnl, err := NewPnlHaltedOutcome(reason)
	if err != nil {
		return AccountPnlOutcome{}, err
	}
	return AccountPnlOutcome{
		PnlOutcome:    pnl,
		PolicyGroupID: policyGroupID,
		AccountID:     accountID,
	}, nil
}

// NewAccountPnlOutcomeFromHandle creates an AccountPnlOutcome from a native
// handle.
func NewAccountPnlOutcomeFromHandle(handle native.AccountPnlOutcome) AccountPnlOutcome {
	return AccountPnlOutcome{
		PnlOutcome: NewPnlOutcomeFromHandle(native.AccountPnlOutcomeGetPnlOutcome(handle)),
		PolicyGroupID: model.PolicyGroupID(
			native.AccountPnlOutcomeGetPolicyGroupID(handle),
		),
		AccountID: param.NewAccountIDFromUint64(
			uint64(native.AccountPnlOutcomeGetAccountID(handle)),
		),
	}
}

// NewHandle returns a callback-scoped native view of this outcome.
func (o AccountPnlOutcome) NewHandle() native.AccountPnlOutcome {
	return native.NewAccountPnlOutcome(
		o.AccountID.Handle(),
		native.PolicyGroupID(o.PolicyGroupID),
		o.newNativeHandle(),
	)
}

// NewAccountPnlListFromHandle copies a native account-level PnL outcome list
// into a Go slice. The list is a view borrowed from its post-trade result.
func NewAccountPnlListFromHandle(handle native.AccountPnlOutcomeList) []AccountPnlOutcome {
	count := native.AccountPnlOutcomeListLen(handle)
	if count == 0 {
		return nil
	}
	result := make([]AccountPnlOutcome, count)
	for i := 0; i < count; i++ {
		result[i] = NewAccountPnlOutcomeFromHandle(
			native.AccountPnlOutcomeListGet(handle, i),
		)
	}
	return result
}
