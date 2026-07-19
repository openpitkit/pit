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

package asyncengine

import (
	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/accounts"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// Driver is the subset of *openpit.Engine that AsyncEngine invokes.
// *openpit.Engine satisfies Driver when the engine was built with
// AccountSync. Provide a custom implementation only to mock the engine in
// tests or to interpose on the calls AsyncEngine makes.
//
// The asynchronous surface deliberately carries no result structs of its own:
// every method returns a future over the same values the synchronous engine
// returns (see pkg/future.Future and pkg/future.Future2); the post-trade
// result is the canonical pretrade.PostTradeResult.
type Driver interface {
	StartPreTrade(model.Order) (*pretrade.Request, []reject.Reject, error)
	ExecutePreTrade(model.Order) (*pretrade.Reservation, []reject.Reject, error)
	ApplyExecutionReport(model.ExecutionReport) (pretrade.PostTradeResult, error)
	ApplyAccountAdjustment(
		param.AccountID,
		[]model.AccountAdjustment,
	) (accountadjustment.BatchResult, error)
	Accounts() accounts.Accounts
}
