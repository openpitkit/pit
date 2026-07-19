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
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// OutcomeAmount describes the delta/absolute result for one position field.
type OutcomeAmount struct {
	// Delta is the signed change applied by this operation relative to the field
	// value at operation start. Authoritative for position bookkeeping.
	Delta param.PositionSize
	// Absolute is the field value at the moment the policy returned, before
	// deferred commit. Treat as a convenience hint only; prefer Delta.
	Absolute param.PositionSize
}

// NewOutcomeAmountFromHandle creates an OutcomeAmount from a native handle.
func NewOutcomeAmountFromHandle(handle native.OutcomeAmount) OutcomeAmount {
	return OutcomeAmount{
		Delta:    param.NewPositionSizeFromHandle(native.OutcomeAmountGetDelta(handle)),
		Absolute: param.NewPositionSizeFromHandle(native.OutcomeAmountGetAbsolute(handle)),
	}
}

// NewHandle returns a native handle for this outcome amount.
func (a OutcomeAmount) NewHandle() native.OutcomeAmount {
	return native.NewOutcomeAmount(a.Delta.Handle(), a.Absolute.Handle())
}

// NewOutcomeAmountOptionFromHandle creates an optional OutcomeAmount from a
// native optional handle.
func NewOutcomeAmountOptionFromHandle(
	handle native.OutcomeAmountOptional,
) optional.Option[OutcomeAmount] {
	if !native.OutcomeAmountOptionalIsSet(handle) {
		return optional.None[OutcomeAmount]()
	}
	return optional.Some(NewOutcomeAmountFromHandle(native.OutcomeAmountOptionalGet(handle)))
}

func newNativeOutcomeAmountOptional(
	value optional.Option[OutcomeAmount],
) native.OutcomeAmountOptional {
	amount, ok := value.Get()
	if !ok {
		return native.NewOutcomeAmountOptionalUnset()
	}
	return native.NewOutcomeAmountOptionalSet(amount.NewHandle())
}

// PnlOutcomeAmount is an account-currency delta/absolute pair for a realized
// PnL field.
type PnlOutcomeAmount struct {
	// Delta is the signed account-currency PnL change applied by this
	// operation.
	Delta param.Pnl
	// Absolute is the cumulative account-currency realized PnL after this
	// operation.
	Absolute param.Pnl
}

// NewPnlOutcomeAmountFromHandle creates a PnlOutcomeAmount from a native handle.
func NewPnlOutcomeAmountFromHandle(handle native.PnlOutcomeAmount) PnlOutcomeAmount {
	return PnlOutcomeAmount{
		Delta:    param.NewPnlFromHandle(native.PnlOutcomeAmountGetDelta(handle)),
		Absolute: param.NewPnlFromHandle(native.PnlOutcomeAmountGetAbsolute(handle)),
	}
}

// NewHandle returns a native handle for this PnL outcome amount.
func (a PnlOutcomeAmount) NewHandle() native.PnlOutcomeAmount {
	return native.NewPnlOutcomeAmount(a.Delta.Handle(), a.Absolute.Handle())
}

// NewPnlOutcomeOptionFromHandle creates an optional PnlOutcome from a native
// optional handle.
func NewPnlOutcomeOptionFromHandle(
	handle native.PnlOutcomeOptional,
) optional.Option[PnlOutcome] {
	if !native.PnlOutcomeOptionalIsSet(handle) {
		return optional.None[PnlOutcome]()
	}
	return optional.Some(NewPnlOutcomeFromHandle(native.PnlOutcomeOptionalGet(handle)))
}

func newNativePnlOutcomeOptional(value optional.Option[PnlOutcome]) native.PnlOutcomeOptional {
	outcome, ok := value.Get()
	if !ok {
		return native.NewPnlOutcomeOptionalUnset()
	}
	return native.NewPnlOutcomeOptionalSet(outcome.newNativeHandle())
}

func newNativeParamPriceOptional(
	value optional.Option[param.Price],
) native.ParamPriceOptional {
	price, ok := value.Get()
	if !ok {
		return native.NewParamPriceOptionalUnset()
	}
	return native.NewParamPriceOptionalSet(price.Handle())
}

// AccountOutcomeEntry is the raw outcome data produced by a policy for one asset.
type AccountOutcomeEntry struct {
	// Asset this outcome refers to.
	Asset param.Asset
	// Balance is the settled balance/position outcome.
	Balance optional.Option[OutcomeAmount]
	// Held is the held (reserved) amount outcome.
	Held optional.Option[OutcomeAmount]
	// Incoming is the incoming (pending inflow) amount outcome.
	Incoming optional.Option[OutcomeAmount]
	// RealizedPnl is the optional account-currency realized-PnL result. It is
	// either PnlOutcomeAmount or the halt reason from the operation that first
	// failed. Later operations omit it until an adjustment force-sets this
	// position's PnL. Re-arming account PnL or another position does not re-arm
	// it.
	RealizedPnl optional.Option[PnlOutcome]
	// AverageEntryPrice is the absolute current account-currency average entry
	// price after the operation. None means it was not tracked or not emitted;
	// missing inputs may also prevent the operation from emitting an average.
	AverageEntryPrice optional.Option[param.Price]
}

// NewAccountOutcomeEntryFromHandle creates an AccountOutcomeEntry from a native
// handle.
func NewAccountOutcomeEntryFromHandle(handle native.AccountOutcomeEntry) AccountOutcomeEntry {
	return AccountOutcomeEntry{
		Asset: param.NewAssetFromHandle(
			native.AccountOutcomeEntryGetAsset(handle),
		).Or(param.Asset{}),
		Balance: NewOutcomeAmountOptionFromHandle(
			native.AccountOutcomeEntryGetBalance(handle),
		),
		Held: NewOutcomeAmountOptionFromHandle(
			native.AccountOutcomeEntryGetHeld(handle),
		),
		Incoming: NewOutcomeAmountOptionFromHandle(
			native.AccountOutcomeEntryGetIncoming(handle),
		),
		RealizedPnl: NewPnlOutcomeOptionFromHandle(
			native.AccountOutcomeEntryGetRealizedPnl(handle),
		),
		AverageEntryPrice: param.NewPriceOptionFromHandle(
			native.AccountOutcomeEntryGetAverageEntryPrice(handle),
		),
	}
}

// NewHandle returns a native handle for this account outcome entry.
//
// The returned handle borrows the asset string bytes from this entry's Asset;
// the entry must stay alive while the handle is in use.
func (e AccountOutcomeEntry) NewHandle() native.AccountOutcomeEntry {
	return native.NewAccountOutcomeEntry(
		native.NewStringView(e.Asset.Handle()),
		newNativeOutcomeAmountOptional(e.Balance),
		newNativeOutcomeAmountOptional(e.Held),
		newNativeOutcomeAmountOptional(e.Incoming),
		newNativePnlOutcomeOptional(e.RealizedPnl),
		newNativeParamPriceOptional(e.AverageEntryPrice),
	)
}

// Outcome is an account position outcome with the group tag of the business
// entity that produced it.
type Outcome struct {
	// PolicyGroupID is the policy-group tag of the policy that produced this
	// outcome.
	PolicyGroupID model.PolicyGroupID
	// Entry is the account adjustment outcome entry.
	Entry AccountOutcomeEntry
}

// NewAccountAdjustmentOutcomeFromHandle creates an Outcome from a native handle.
func NewAccountAdjustmentOutcomeFromHandle(
	handle native.AccountAdjustmentOutcome,
) Outcome {
	return Outcome{
		PolicyGroupID: model.PolicyGroupID(native.AccountAdjustmentOutcomeGetPolicyGroupID(handle)),
		Entry:         NewAccountOutcomeEntryFromHandle(native.AccountAdjustmentOutcomeGetEntry(handle)),
	}
}

// NewHandle returns a callback-scoped native view of this outcome.
// The outcome and its entry asset must stay alive through the native call.
func (o Outcome) NewHandle() native.AccountAdjustmentOutcome {
	return native.NewAccountAdjustmentOutcome(
		native.PolicyGroupID(o.PolicyGroupID),
		o.Entry.NewHandle(),
	)
}

// NewListFromHandle copies a native account-adjustment outcome list into a Go
// slice without taking ownership. The producer defines whether the list is
// caller-owned or borrowed; lists obtained from a PostTradeResult must not be
// destroyed separately.
func NewListFromHandle(handle native.AccountAdjustmentOutcomeList) []Outcome {
	count := native.AccountAdjustmentOutcomeListLen(handle)
	if count == 0 {
		return nil
	}
	result := make([]Outcome, count)
	for i := 0; i < count; i++ {
		result[i] = NewAccountAdjustmentOutcomeFromHandle(
			native.AccountAdjustmentOutcomeListGet(handle, i),
		)
	}
	return result
}
