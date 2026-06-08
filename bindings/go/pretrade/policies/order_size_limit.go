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

// Package policies provides built-in pre-trade policy builders.
package policies

import (
	"runtime"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/ptr"
)

// OrderSizeLimit defines maximum quantity and notional for a single order.
type OrderSizeLimit struct {
	// MaxQuantity is the maximum allowed order quantity.
	MaxQuantity param.Quantity
	// MaxNotional is the maximum allowed order notional.
	MaxNotional param.Volume
}

// OrderSizeBrokerBarrier applies an order size limit across the entire
// broker.
type OrderSizeBrokerBarrier struct {
	Limit OrderSizeLimit
}

// OrderSizeAssetBarrier applies an order size limit per settlement asset.
type OrderSizeAssetBarrier struct {
	Limit           OrderSizeLimit
	SettlementAsset param.Asset
}

// OrderSizeAccountAssetBarrier applies an order size limit per
// (account, settlement asset) pair.
type OrderSizeAccountAssetBarrier struct {
	Limit           OrderSizeLimit
	AccountID       param.AccountID
	SettlementAsset param.Asset
}

//------------------------------------------------------------------------------
// OrderSizeLimitBuilder

// OrderSizeLimitBuilder is the entry point for the order-size-limit policy.
// Call BrokerBarrier to obtain an OrderSizeLimitReadyBuilder on which
// additional axes and Build are available.
type OrderSizeLimitBuilder struct {
	builder *OrderSizeLimitReadyBuilder
}

// OrderSizeLimitReadyBuilder holds a fully-configured order-size-limit
// policy.
type OrderSizeLimitReadyBuilder struct {
	broker               *native.PretradePoliciesOrderSizeBrokerBarrier
	assetBarriers        []native.PretradePoliciesOrderSizeAssetBarrier
	accountAssetBarriers []native.PretradePoliciesOrderSizeAccountAssetBarrier
	policyGroupID        model.PolicyGroupID
}

// BuildOrderSizeLimit returns a new order-size-limit policy builder.
func BuildOrderSizeLimit() *OrderSizeLimitBuilder {
	return &OrderSizeLimitBuilder{
		builder: &OrderSizeLimitReadyBuilder{policyGroupID: model.DefaultPolicyGroupID},
	}
}

// PolicyGroupID assigns the policy to a pricing group and returns a ready
// builder. When not set the policy uses model.DefaultPolicyGroupID.
func (b *OrderSizeLimitBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *OrderSizeLimitReadyBuilder {
	b.builder.PolicyGroupID(groupID)
	return b.builder
}

// PolicyGroupID assigns the policy to a pricing group. When not set the
// policy uses model.DefaultPolicyGroupID.
func (b *OrderSizeLimitReadyBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *OrderSizeLimitReadyBuilder {
	b.policyGroupID = groupID
	return b
}

// BrokerBarrier sets the broker-wide size limit and returns a ready
// builder.
func (b *OrderSizeLimitBuilder) BrokerBarrier(
	barrier OrderSizeBrokerBarrier,
) *OrderSizeLimitReadyBuilder {
	b.builder.BrokerBarrier(barrier)
	return b.builder
}

// BrokerBarrier sets or replaces the broker-wide size limit.
func (b *OrderSizeLimitReadyBuilder) BrokerBarrier(
	barrier OrderSizeBrokerBarrier,
) *OrderSizeLimitReadyBuilder {
	b.broker = ptr.New(
		native.NewPretradePoliciesOrderSizeBrokerBarrier(
			native.NewPretradePoliciesOrderSizeLimit(
				barrier.Limit.MaxQuantity.Handle(),
				barrier.Limit.MaxNotional.Handle(),
			),
		),
	)
	return b
}

// AssetBarriers adds per-settlement-asset barriers and returns a ready
// builder.
func (b *OrderSizeLimitBuilder) AssetBarriers(
	barriers ...OrderSizeAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	b.builder.AssetBarriers(barriers...)
	return b.builder
}

// AssetBarriers appends per-settlement-asset order-size barriers.
func (b *OrderSizeLimitReadyBuilder) AssetBarriers(
	barriers ...OrderSizeAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	for _, barrier := range barriers {
		b.assetBarriers = append(
			b.assetBarriers,
			native.NewPretradePoliciesOrderSizeAssetBarrier(
				native.NewPretradePoliciesOrderSizeLimit(
					barrier.Limit.MaxQuantity.Handle(),
					barrier.Limit.MaxNotional.Handle(),
				),
				barrier.SettlementAsset.Handle(),
			),
		)
	}
	return b
}

// AccountAssetBarriers adds per-(account, settlement-asset) barriers and
// returns a ready builder.
func (b *OrderSizeLimitBuilder) AccountAssetBarriers(
	barriers ...OrderSizeAccountAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	b.builder.AccountAssetBarriers(barriers...)
	return b.builder
}

// AccountAssetBarriers appends per-(account, settlement-asset) order-size
// barriers.
func (b *OrderSizeLimitReadyBuilder) AccountAssetBarriers(
	barriers ...OrderSizeAccountAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	for _, barrier := range barriers {
		b.accountAssetBarriers = append(
			b.accountAssetBarriers,
			native.NewPretradePoliciesOrderSizeAccountAssetBarrier(
				native.NewPretradePoliciesOrderSizeLimit(
					barrier.Limit.MaxQuantity.Handle(),
					barrier.Limit.MaxNotional.Handle(),
				),
				barrier.AccountID.Handle(),
				barrier.SettlementAsset.Handle(),
			),
		)
	}
	return b
}

// Build marshals the configuration and registers the built-in
// order-PolicyGroupIDmit policy on the given engine builder.
func (b *OrderSizeLimitReadyBuilder) Build(builder native.EngineBuilder) error {
	err := native.EngineBuilderAddBuiltinOrderSizeLimit(
		builder,
		native.PolicyGroupID(b.policyGroupID),
		b.broker,
		b.assetBarriers,
		b.accountAssetBarriers,
	)
	runtime.KeepAlive(b)
	return err
}
