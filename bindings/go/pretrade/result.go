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

package pretrade

import (
	"runtime"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/reject"
)

// Result is the callback-scoped collector passed to PerformPreTradeCheck.
//
// It carries the extra channels a main-stage pre-trade policy may fill in
// addition to its reject return: lock prices and account-adjustment outcomes.
// The engine assigns the policy group to every pushed item.
//
// The collector is non-owning and callback-scoped: it borrows engine state
// that is valid only for the duration of the hook that received it. Retaining
// the collector or using it after that hook has returned is undefined.
type Result struct{ handle native.PretradePreTradeResult }

// NewPreTradeResultFromHandle wraps a native handle into a Result.
func NewPreTradeResultFromHandle(handle native.PretradePreTradeResult) Result {
	return Result{handle: handle}
}

// PushLockPrice appends one lock price to the result.
func (r Result) PushLockPrice(price param.Price) error {
	return native.PretradePreTradeResultPushLockPrice(r.handle, price.Handle())
}

// PushAccountAdjustment appends one account-adjustment outcome to the result.
func (r Result) PushAccountAdjustment(
	entry accountadjustment.AccountOutcomeEntry,
) error {
	err := native.PretradePreTradeResultPushAccountAdjustment(r.handle, entry.NewHandle())
	runtime.KeepAlive(entry)
	return err
}

// PostTradeAdjustments is the callback-scoped collector passed to
// ApplyExecutionReport. It carries group-tagged account-adjustment outcomes.
//
// The collector is non-owning and callback-scoped: it borrows engine state
// that is valid only for the duration of the hook that received it. Retaining
// the collector or using it after that hook has returned is undefined.
type PostTradeAdjustments struct {
	handle native.PostTradeAdjustmentList
}

// NewPostTradeAdjustmentsFromHandle wraps a native handle into a
// PostTradeAdjustments.
func NewPostTradeAdjustmentsFromHandle(handle native.PostTradeAdjustmentList) PostTradeAdjustments {
	return PostTradeAdjustments{handle: handle}
}

// Push appends one group-tagged account-adjustment outcome to the list.
func (a PostTradeAdjustments) Push(
	groupID model.PolicyGroupID,
	entry accountadjustment.AccountOutcomeEntry,
) error {
	err := native.PostTradeAdjustmentListPush(
		a.handle,
		native.PolicyGroupID(groupID), entry.NewHandle(),
	)
	runtime.KeepAlive(entry)
	return err
}

// PostTradePnls is the callback-scoped collector passed to
// ApplyExecutionReport. It carries group-tagged account-level PnL outcomes.
//
// The collector is non-owning and callback-scoped: it borrows engine state
// that is valid only for the duration of the hook that received it. Retaining
// the collector or using it after that hook has returned is undefined.
type PostTradePnls struct {
	handle native.PostTradeAccountPnlList
}

// NewPostTradePnlsFromHandle wraps a native handle into a PostTradePnls.
func NewPostTradePnlsFromHandle(handle native.PostTradeAccountPnlList) PostTradePnls {
	return PostTradePnls{handle: handle}
}

// Push appends one group-tagged account-level PnL outcome to the list.
func (p PostTradePnls) Push(outcome accountadjustment.AccountPnlOutcome) error {
	err := native.PostTradeAccountPnlListPush(p.handle, outcome.NewHandle())
	runtime.KeepAlive(outcome)
	return err
}

// AccountOutcomes is the callback-scoped collector passed to
// ApplyAccountAdjustment. It carries account-outcome entries; the engine
// assigns the policy group to every pushed entry.
//
// The collector is non-owning and callback-scoped: it borrows engine state
// that is valid only for the duration of the hook that received it. Retaining
// the collector or using it after that hook has returned is undefined.
type AccountOutcomes struct {
	handle native.PretradeAccountAdjustmentResult
}

// PolicyAccountAdjustmentResult is the accepted contribution of one custom
// policy to an account-adjustment batch.
type PolicyAccountAdjustmentResult struct {
	AccountBlocks []reject.AccountBlock
}

// NewAccountOutcomesFromHandle wraps a native handle into an AccountOutcomes.
func NewAccountOutcomesFromHandle(handle native.PretradeAccountAdjustmentResult) AccountOutcomes {
	return AccountOutcomes{handle: handle}
}

// Push appends one account-outcome entry to the list.
func (o AccountOutcomes) Push(entry accountadjustment.AccountOutcomeEntry) error {
	err := native.PretradeAccountAdjustmentResultPushAccountOutcome(o.handle, entry.NewHandle())
	runtime.KeepAlive(entry)
	return err
}
