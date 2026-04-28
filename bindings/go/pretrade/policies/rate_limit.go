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

package policies

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pretrade"
)

// NewRateLimitPolicy creates a rate-limit policy.
//
// Start-stage policy that limits order rate in a sliding time window.
// Every check_pre_trade_start call, including rejected ones, consumes a slot
// in the window.
//
// Must be closed with Close.
func NewRateLimitPolicy(maxOrders int, windowSeconds uint64) pretrade.BuiltinPolicy {
	return newCheckStartPolicy(
		func() native.PretradeCheckPreTradeStartPolicy {
			return native.CreatePretradePoliciesRateLimitPolicy(maxOrders, windowSeconds)
		},
	)
}
