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
	// Returning a non-empty reject list contributes rejects to the start-stage
	// reject result. All registered policies are evaluated in registration
	// order and their reject lists are merged before the engine returns to the
	// caller.
	//
	// Implementations must not let panics escape this method. A panic raised
	// here may propagate across the SDK boundary and terminate the process;
	// recovering from such panics is the implementer's responsibility.
	CheckPreTradeStart(Context, model.Order) []reject.Reject

	// PerformPreTradeCheck performs main-stage checks and can emit mutations
	// or rejects.
	//
	// Policies may inspect the order, append mutations to be committed or
	// rolled back later, fill the result collector with lock prices and
	// account-adjustment outcomes, and return zero or more rejects.
	// An empty rejects list means accept. The engine keeps the result
	// collector content only when the policy accepts.
	//
	// Rollback safety:
	// In this pre-trade pipeline, rollback may happen after external systems
	// observed intermediate reserved state. Avoid absolute-value rollback in
	// mutations registered here; prefer delta-based undo or restore values
	// captured at registration time.
	//
	// Implementations must not let panics escape this method. A panic raised
	// here may propagate across the SDK boundary and terminate the process;
	// recovering from such panics is the implementer's responsibility.
	PerformPreTradeCheck(Context, model.Order, tx.Mutations, Result) []reject.Reject

	// ApplyExecutionReport applies post-trade updates from execution reports.
	//
	// Returns account blocks representing the kill-switch state. An empty list
	// means no kill switch. A non-empty list means this policy entered a
	// blocked state after the report was applied. Policies may independently
	// fill the adjustment and account-PnL collectors.
	//
	// Implementations must not let panics escape this method. A panic raised
	// here may propagate across the SDK boundary and terminate the process;
	// recovering from such panics is the implementer's responsibility.
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
	// Implementations must not let panics escape this method. A panic raised
	// here may propagate across the SDK boundary and terminate the process;
	// recovering from such panics is the implementer's responsibility.
	ApplyAccountAdjustment(
		accountadjustment.Context,
		param.AccountID,
		model.AccountAdjustment,
		tx.Mutations,
		AccountOutcomes,
	) (PolicyAccountAdjustmentResult, []reject.Reject)
}
