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
	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// Policy is the interface implemented by all pre-trade risk policies.
type Policy interface {
	// Close releases any resources held by the policy.
	Close()

	// Name returns the stable policy name.
	//
	// Policy names must be unique across all policies registered in the same
	// engine instance.
	Name() string

	// PolicyGroupID returns the policy-group tag the engine assigns to records
	// produced by this policy.
	PolicyGroupID() model.PolicyGroupID

	// CheckPreTradeStart performs start-stage checks against an order.
	//
	// Return rejects or review marks. Each item's Mode is optional; its zero
	// value means Reject.
	// All registered policies are evaluated in registration order. Effective
	// rejects stop the order. Review marks alone are available via Request.Reviews.
	//
	// The order is a view of engine memory and is valid only during this hook.
	// Its string fields and model values derived from it, including nested
	// values returned by Values(), can alias engine-owned memory.
	// NewOrderFromValues(order.Values()) is not an independent copy because its
	// nested OrderOperation and OrderMargin values keep those string views. To
	// retain data, copy leaf values inside the hook: Instrument() and
	// CollateralAsset() return independent param values; scalar fields are
	// values.
	//
	// A panic is recovered at the SDK boundary and reported as a
	// SystemUnavailable reject with the panic value in its details.
	CheckPreTradeStart(Context, model.Order) []reject.Reject

	// PerformPreTradeCheck performs main-stage checks and can emit mutations
	// and return rejects or review marks. Each item's Mode is optional; its zero
	// value means Reject.
	//
	// Policies may inspect the order, append mutations to be committed or
	// rolled back later, fill the result collector with lock prices and
	// account-adjustment outcomes, and return zero or more rejects or marks.
	// An empty list means accept. Use the collector to provide a result even when
	// returning marks. Live pre-trade keeps the result on acceptance and when the
	// only reported marks are with-hold. An effective reject or a reported
	// without-hold mark clears the result. If this callback returns only review
	// marks and the instance was registered in a review mode via InstanceMode,
	// live pre-trade and dry-run discard that instance's contributions, even on a
	// confirmed rerun, which reports no marks. Live pre-trade rolls back its
	// discarded mutations.
	//
	// Dry-run never applies or rolls back mutations. It reports the lock prices
	// and account-adjustment outcomes live pre-trade would keep on a successful
	// run. With another instance's main-stage reject, it still reports the results
	// collected from the instances that kept theirs.
	//
	// Drop-copy keeps the result with review marks of either mode and with
	// ordinary, non-enforcing rejects. An evaluation failure aborts drop-copy and
	// discards the results.
	//
	// The order is a view of engine memory and is valid only during this hook.
	// Its string fields and model values derived from it, including nested
	// values returned by Values(), can alias engine-owned memory.
	// NewOrderFromValues(order.Values()) is not an independent copy because its
	// nested OrderOperation and OrderMargin values keep those string views. To
	// retain data, copy leaf values inside the hook: Instrument() and
	// CollateralAsset() return independent param values; scalar fields are
	// values.
	//
	// Rollback safety:
	// In this pre-trade pipeline, rollback may happen after external systems
	// observed intermediate reserved state. Avoid absolute-value rollback in
	// mutations registered here; prefer delta-based undo or restore values
	// captured at registration time.
	//
	// A panic is recovered at the SDK boundary and reported as a
	// SystemUnavailable reject with the panic value in its details.
	// The mutations value is valid only during this hook call. Do not retain it
	// or use it after return; doing so is undefined.
	PerformPreTradeCheck(Context, model.Order, tx.Mutations, Result) []reject.Reject

	// ApplyExecutionReport applies post-trade updates from execution reports.
	//
	// Returns account blocks representing the kill-switch state. An empty list
	// means no kill switch. A non-empty list means this policy entered a
	// blocked state after the report was applied. Policies may independently
	// fill the adjustment and account-PnL collectors.
	//
	// The report is a view of engine memory and is valid only during this hook.
	// Its string fields and a fill's pre-trade lock alias engine-owned memory,
	// as can model values derived from it, including nested values returned by
	// Values(). NewExecutionReportFromValues(report.Values()) is an independent
	// deep copy: Operation() and Fill() copy their instrument and fee-currency
	// assets into fresh buffers, and SetFill() clones a borrowed fill lock into
	// a new Go owner. Leaf accessors such as Instrument() and Fee() return
	// independent param values, Lock() returns independent bytes, and scalar
	// fields are values.
	//
	// A panic is recovered at the SDK boundary and reported as an account
	// block with code SystemUnavailable and the panic value in its details.
	ApplyExecutionReport(
		PostTradeContext,
		model.ExecutionReport,
		PostTradeAdjustments,
		PostTradePnls,
	) []reject.AccountBlock

	// ApplyAccountAdjustment validates one account adjustment.
	//
	// Returns the accepted account blocks and zero or more rejects. Policies
	// may fill the outcomes collector with account-outcome entries; the engine
	// keeps outcomes and account blocks only when the policy accepts.
	//
	// The adjustment is a view of engine memory and is valid only during this
	// hook. Its string fields and model values derived from it, including nested
	// values returned by Values(), can alias engine-owned memory.
	// NewAccountAdjustmentFromValues(adjustment.Values()) is not an independent
	// copy because its nested operation values keep those string views. To
	// retain data, copy leaf values inside the hook: Asset(), Instrument(), and
	// CollateralAsset() return independent param values, while scalar fields are
	// values.
	//
	// A panic is recovered at the SDK boundary and reported as a
	// SystemUnavailable reject with the panic value in its details.
	// The mutations value is valid only during this hook call. Do not retain it
	// or use it after return; doing so is undefined.
	ApplyAccountAdjustment(
		accountadjustment.Context,
		param.AccountID,
		model.AccountAdjustment,
		tx.Mutations,
		AccountOutcomes,
	) (PolicyAccountAdjustmentResult, []reject.Reject)
}
