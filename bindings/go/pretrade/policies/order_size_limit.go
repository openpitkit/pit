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

// Package policies provides built-in pre-trade policy builders.
package policies

import (
	"runtime"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pkg/ptr"
)

// OrderSizeLimit defines optional quantity and notional caps for one order.
//
// Quantity resolves by underlying asset, while notional resolves by settlement
// asset. An absent cap does not constrain its metric. Within the account+asset
// then asset chain for a metric, a matching barrier without that metric is
// skipped. The broker barrier applies each cap it carries to every order in
// addition to those chains. A cap rejects an order whose metric value is above
// it; a zero cap rejects positive metric values and admits a value of exactly
// zero. The core requires at least one cap in every limit.
type OrderSizeLimit struct {
	// MaxQuantity is the optional maximum quantity, keyed by underlying asset.
	MaxQuantity optional.Option[param.Quantity]
	// MaxNotional is the optional maximum notional, keyed by settlement asset.
	MaxNotional optional.Option[param.Volume]
}

// OrderSizeBrokerBarrier applies broker-wide caps to every order, in addition
// to the account+asset and asset chains. Each cap applies independently.
type OrderSizeBrokerBarrier struct {
	Limit OrderSizeLimit
}

// OrderSizeAssetBarrier applies its quantity cap when Asset is the underlying
// asset and its notional cap when Asset is the settlement asset. A missing cap
// is skipped while resolving that metric.
type OrderSizeAssetBarrier struct {
	Limit OrderSizeLimit
	Asset param.Asset
}

// OrderSizeAccountAssetBarrier applies its quantity cap per (account,
// underlying asset) and its notional cap per (account, settlement asset). A
// missing cap is skipped while resolving that metric.
type OrderSizeAccountAssetBarrier struct {
	Limit     OrderSizeLimit
	AccountID param.AccountID
	Asset     param.Asset
}

//------------------------------------------------------------------------------
// OrderSizeLimitBuilder

// OrderSizeLimitBuilder is the entry point for the order-size-limit policy.
// Each cap is optional, and the core validates that every configured limit
// carries at least one cap. Call an axis method to obtain an
// OrderSizeLimitReadyBuilder on which additional axes and Build are available.
type OrderSizeLimitBuilder struct {
	builder *OrderSizeLimitReadyBuilder
}

// OrderSizeLimitReadyBuilder holds an order-size-limit policy ready to build.
// Quantity barriers are keyed by underlying asset and notional barriers by
// settlement asset. Within its account+asset then asset chain, each metric
// skips matching barriers that omit that cap.
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

// BrokerBarrier sets additive broker-wide caps and returns a ready builder.
// Each cap applies to every order in addition to the account+asset and asset
// chains. An absent cap constrains nothing; an explicitly set zero cap rejects
// positive metric values and admits a value of exactly zero.
func (b *OrderSizeLimitBuilder) BrokerBarrier(
	barrier OrderSizeBrokerBarrier,
) *OrderSizeLimitReadyBuilder {
	b.builder.BrokerBarrier(barrier)
	return b.builder
}

// BrokerBarrier sets or replaces the additive broker-wide caps. Each cap
// applies to every order in addition to the account+asset and asset chains. An
// absent cap constrains nothing; an explicitly set zero cap rejects positive
// metric values and admits a value of exactly zero.
func (b *OrderSizeLimitReadyBuilder) BrokerBarrier(
	barrier OrderSizeBrokerBarrier,
) *OrderSizeLimitReadyBuilder {
	b.broker = ptr.New(
		native.NewPretradePoliciesOrderSizeBrokerBarrier(
			native.NewPretradePoliciesOrderSizeLimit(
				newParamQuantityOptionalFromOptional(barrier.Limit.MaxQuantity),
				newParamVolumeOptionalFromOptional(barrier.Limit.MaxNotional),
			),
		),
	)
	return b
}

// AssetBarriers adds barriers keyed by underlying for quantity and settlement
// for notional, then returns a ready builder. Resolution skips a barrier that
// omits the metric being resolved.
func (b *OrderSizeLimitBuilder) AssetBarriers(
	barriers ...OrderSizeAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	b.builder.AssetBarriers(barriers...)
	return b.builder
}

// AssetBarriers appends barriers keyed by underlying for quantity and
// settlement for notional. Resolution skips a barrier that omits the metric
// being resolved.
func (b *OrderSizeLimitReadyBuilder) AssetBarriers(
	barriers ...OrderSizeAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	for _, barrier := range barriers {
		b.assetBarriers = append(
			b.assetBarriers,
			native.NewPretradePoliciesOrderSizeAssetBarrier(
				native.NewPretradePoliciesOrderSizeLimit(
					newParamQuantityOptionalFromOptional(barrier.Limit.MaxQuantity),
					newParamVolumeOptionalFromOptional(barrier.Limit.MaxNotional),
				),
				barrier.Asset.Handle(),
			),
		)
	}
	return b
}

// AccountAssetBarriers adds barriers keyed by (account, underlying) for
// quantity and (account, settlement) for notional, then returns a ready
// builder. Resolution skips a barrier that omits the metric being resolved.
func (b *OrderSizeLimitBuilder) AccountAssetBarriers(
	barriers ...OrderSizeAccountAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	b.builder.AccountAssetBarriers(barriers...)
	return b.builder
}

// AccountAssetBarriers appends barriers keyed by (account, underlying) for
// quantity and (account, settlement) for notional. Resolution skips a barrier
// that omits the metric being resolved.
func (b *OrderSizeLimitReadyBuilder) AccountAssetBarriers(
	barriers ...OrderSizeAccountAssetBarrier,
) *OrderSizeLimitReadyBuilder {
	for _, barrier := range barriers {
		b.accountAssetBarriers = append(
			b.accountAssetBarriers,
			native.NewPretradePoliciesOrderSizeAccountAssetBarrier(
				native.NewPretradePoliciesOrderSizeLimit(
					newParamQuantityOptionalFromOptional(barrier.Limit.MaxQuantity),
					newParamVolumeOptionalFromOptional(barrier.Limit.MaxNotional),
				),
				barrier.AccountID.Handle(),
				barrier.Asset.Handle(),
			),
		)
	}
	return b
}

// Build marshals the configuration and registers the built-in order-size-limit
// policy on the given engine builder. The core rejects capless limits and
// duplicate keys.
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

func newParamQuantityOptionalFromOptional(
	value optional.Option[param.Quantity],
) native.ParamQuantityOptional {
	if v, has := value.Get(); has {
		return native.NewParamQuantityOptional(v.Handle())
	}
	return native.ParamQuantityOptional{}
}

func newParamVolumeOptionalFromOptional(
	value optional.Option[param.Volume],
) native.ParamVolumeOptional {
	if v, has := value.Get(); has {
		return native.NewParamVolumeOptional(v.Handle())
	}
	return native.ParamVolumeOptional{}
}
