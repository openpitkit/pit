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

// Registration names of the built-in policies. Each built-in registers under
// its fixed name; pass the matching constant to the engine configurator (for
// example engine.Configure().RateLimit) to retune that policy at runtime.
const (
	// RateLimitPolicyName is the registration name of the rate-limit policy.
	RateLimitPolicyName = "RateLimitPolicy"
	// OrderSizeLimitPolicyName is the registration name of the order-size-limit
	// policy.
	OrderSizeLimitPolicyName = "OrderSizeLimitPolicy"
	// PnlBoundsKillSwitchPolicyName is the registration name of the P&L bounds
	// kill-switch policy.
	PnlBoundsKillSwitchPolicyName = "PnlBoundsKillSwitchPolicy"
	// SpotFundsPolicyName is the registration name of the spot funds policy.
	SpotFundsPolicyName = "SpotFundsPolicy"
)
