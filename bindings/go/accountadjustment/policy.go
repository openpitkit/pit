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
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

type Policy interface {
	// Close releases any resources held by the policy.
	Close()

	// Name returns the stable policy name.
	//
	// Policy names must be unique across all policies registered in the same
	// engine instance.
	Name() string

	// ApplyAccountAdjustment validates a batch of account adjustments for a
	// single account.
	//
	// Returns zero or more rejects when an adjustment violates policy
	// constraints. Empty list means accept.
	//
	// Rollback safety:
	// In this account-adjustment pipeline, rollback by absolute value is safe
	// because validation and mutation execution happen within a single engine
	// borrow and no external system observes the intermediate state.
	//
	// Implementations must not let panics escape this method. A panic raised
	// here may propagate across the SDK boundary and terminate the process;
	// recovering from such panics is the implementer's responsibility.
	ApplyAccountAdjustment(
		Context,
		param.AccountID,
		model.AccountAdjustment,
		tx.Mutations,
	) reject.List
}
