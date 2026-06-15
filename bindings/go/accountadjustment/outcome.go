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
// Please see https://github.com/openpitkit and the OWNERS file for details.

package accountadjustment

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// OutcomeAmount is a delta/absolute pair for one position field.
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

// NewPnlOutcomeAmountOptionFromHandle creates an optional PnlOutcomeAmount from
// a native optional handle.
func NewPnlOutcomeAmountOptionFromHandle(
	handle native.PnlOutcomeAmountOptional,
) optional.Option[PnlOutcomeAmount] {
	if !native.PnlOutcomeAmountOptionalIsSet(handle) {
		return optional.None[PnlOutcomeAmount]()
	}
	return optional.Some(
		NewPnlOutcomeAmountFromHandle(native.PnlOutcomeAmountOptionalGet(handle)),
	)
}

func newNativePnlOutcomeAmountOptional(
	value optional.Option[PnlOutcomeAmount],
) native.PnlOutcomeAmountOptional {
	amount, ok := value.Get()
	if !ok {
		return native.NewPnlOutcomeAmountOptionalUnset()
	}
	return native.NewPnlOutcomeAmountOptionalSet(amount.NewHandle())
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
	// RealizedPnl is the account-currency realized PnL. Delta is the change
	// applied by this operation; absolute is the cumulative value after the
	// operation. None means realized PnL was not tracked or not emitted; missing
	// account currency or FX stops tracking without reject/block.
	RealizedPnl optional.Option[PnlOutcomeAmount]
	// AverageEntryPrice is the absolute current account-currency average entry
	// price after the operation. None means it was not tracked or not emitted;
	// missing account currency or FX stops tracking without reject/block.
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
		RealizedPnl: NewPnlOutcomeAmountOptionFromHandle(
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
		newNativePnlOutcomeAmountOptional(e.RealizedPnl),
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

// NewListFromHandle copies a native account-adjustment outcome list into a Go
// slice. The native list ownership stays with the caller, which must destroy it.
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
