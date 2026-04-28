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
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
)

// OrderSizeLimit defines per-settlement order size limits.
type OrderSizeLimit struct {
	// SettlementAsset is the settlement asset the limit applies to.
	SettlementAsset param.Asset
	// MaxQuantity is the maximum allowed order quantity for the settlement asset.
	MaxQuantity param.Quantity
	// MaxNotional is the maximum allowed order notional for the settlement asset.
	MaxNotional param.Volume
}

// NewOrderSizeLimitPolicy creates an order-size policy.
//
// OrderSizeLimitPolicy is a start-stage policy enforcing per-settlement order
// size limits.
//
// Limits are configured per settlement asset. Orders for assets without a
// configured limit are rejected.
//
// Must be closed with Close.
func NewOrderSizeLimitPolicy(limits ...OrderSizeLimit) (pretrade.BuiltinPolicy, error) {
	params := make([]native.PretradePoliciesOrderSizeLimitParam, len(limits))
	for i, limit := range limits {
		params[i] = native.NewPretradePoliciesOrderSizeLimitParam(
			limit.SettlementAsset.Handle(),
			limit.MaxQuantity.Handle(),
			limit.MaxNotional.Handle(),
		)
	}

	return newCheckPreTradeStartPolicyWithError(
		func() (native.PretradeCheckPreTradeStartPolicy, error) {
			return native.CreatePretradePoliciesOrderSizeLimitPolicy(params)
		},
	)
}
