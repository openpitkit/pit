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

package policies

import (
	"runtime"
	"time"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/reject"
)

func rateLimitWindowNanoseconds(limit RateLimit) int64 {
	return int64(limit.Window)
}

// RateLimit defines the maximum number of orders allowed within a time
// window.
//
// Broker and asset barriers count orders with an approximate fixed-window
// counter: at a window boundary the observed burst can briefly reach up to
// 2 * MaxOrders. Account and account+asset barriers count them with a precise
// sliding-window log.
//
// Each barrier's mode is part of its key and has its own counter. When
// exceeded, a review-mode barrier adds a RateLimitExceeded review mark instead
// of rejecting. Every attempt that reaches the start hook counts on every
// threshold, a confirmed re-run included; under Confirmed the engine ignores
// review marks.
type RateLimit struct {
	// MaxOrders is the maximum number of orders accepted within Window.
	MaxOrders uint
	// Window is the length of the time window.
	Window time.Duration
}

// RateLimitBrokerBarrier applies a rate limit across the entire broker.
type RateLimitBrokerBarrier struct {
	// Mode is part of the rule's key. The zero value is RuleModeReject.
	Mode  reject.RuleMode
	Limit RateLimit
}

// RateLimitAssetBarrier applies a rate limit per settlement asset.
type RateLimitAssetBarrier struct {
	// Mode is part of the rule's key. The zero value is RuleModeReject.
	Mode            reject.RuleMode
	Limit           RateLimit
	SettlementAsset param.Asset
}

// RateLimitAccountBarrier applies a rate limit per account.
type RateLimitAccountBarrier struct {
	// Mode is part of the rule's key. The zero value is RuleModeReject.
	Mode      reject.RuleMode
	Limit     RateLimit
	AccountID param.AccountID
}

// RateLimitAccountAssetBarrier applies a rate limit per
// (account, settlement asset) pair.
type RateLimitAccountAssetBarrier struct {
	// Mode is part of the rule's key. The zero value is RuleModeReject.
	Mode            reject.RuleMode
	Limit           RateLimit
	AccountID       param.AccountID
	SettlementAsset param.Asset
}

//------------------------------------------------------------------------------
// RateLimitBuilder

// RateLimitBuilder is the entry point for the rate-limit policy. Each
// axis method returns a RateLimitReadyBuilder on which additional axes
// and Build are available.
type RateLimitBuilder struct {
	builder *RateLimitReadyBuilder
}

// RateLimitReadyBuilder holds a fully-configured rate-limit policy.
type RateLimitReadyBuilder struct {
	brokerBarriers                      []native.PretradePoliciesRateLimitBrokerBarrier
	assetBarriers                       []native.PretradePoliciesRateLimitAssetBarrier
	assetBarrierSettlementAssets        []param.Asset
	accountBarriers                     []native.PretradePoliciesRateLimitAccountBarrier
	accountAssetBarriers                []native.PretradePoliciesRateLimitAccountAssetBarrier
	accountAssetBarrierSettlementAssets []param.Asset
	policyGroupID                       model.PolicyGroupID
	err                                 error
}

// BuildRateLimit returns a new rate-limit policy builder.
// Calling an axis method with no barriers, such as AssetBarriers(), returns a
// ready builder for an empty policy that admits every order and can be
// configured later through the engine's Configure() method.
func BuildRateLimit() *RateLimitBuilder {
	return &RateLimitBuilder{
		builder: &RateLimitReadyBuilder{policyGroupID: model.DefaultPolicyGroupID},
	}
}

// PolicyGroupID assigns the policy to a pricing group and returns a ready
// builder. When not set the policy uses model.DefaultPolicyGroupID.
func (b *RateLimitBuilder) PolicyGroupID(groupID model.PolicyGroupID) *RateLimitReadyBuilder {
	b.builder.PolicyGroupID(groupID)
	return b.builder
}

// PolicyGroupID assigns the policy to a pricing group. When not set the
// policy uses model.DefaultPolicyGroupID.
func (b *RateLimitReadyBuilder) PolicyGroupID(groupID model.PolicyGroupID) *RateLimitReadyBuilder {
	b.policyGroupID = groupID
	return b
}

// BrokerBarriers appends broker-wide rate limits and returns a ready
// builder.
func (b *RateLimitBuilder) BrokerBarriers(barriers ...RateLimitBrokerBarrier) *RateLimitReadyBuilder {
	b.builder.BrokerBarriers(barriers...)
	return b.builder
}

// BrokerBarriers appends broker-wide rate limits.
func (b *RateLimitReadyBuilder) BrokerBarriers(
	barriers ...RateLimitBrokerBarrier,
) *RateLimitReadyBuilder {
	if b.err != nil {
		return b
	}
	for _, barrier := range barriers {
		if err := native.ValidateRuleMode(native.PretradeRuleMode(barrier.Mode), "broker", len(b.brokerBarriers)); err != nil {
			b.err = err
			return b
		}
		windowNanos := rateLimitWindowNanoseconds(barrier.Limit)
		b.brokerBarriers = append(b.brokerBarriers,
			native.NewPretradePoliciesRateLimitBrokerBarrier(
				native.PretradeRuleMode(barrier.Mode),
				barrier.Limit.MaxOrders,
				windowNanos,
			))
	}
	return b
}

// AssetBarriers adds per-settlement-asset barriers and returns a ready
// builder.
func (b *RateLimitBuilder) AssetBarriers(barriers ...RateLimitAssetBarrier) *RateLimitReadyBuilder {
	b.builder.AssetBarriers(barriers...)
	return b.builder
}

// AssetBarriers appends per-settlement-asset rate-limit barriers.
func (b *RateLimitReadyBuilder) AssetBarriers(
	barriers ...RateLimitAssetBarrier,
) *RateLimitReadyBuilder {
	if b.err != nil {
		return b
	}
	for _, barrier := range barriers {
		if err := native.ValidateRuleMode(native.PretradeRuleMode(barrier.Mode), "asset", len(b.assetBarriers)); err != nil {
			b.err = err
			return b
		}
		windowNanos := rateLimitWindowNanoseconds(barrier.Limit)
		b.assetBarriers = append(
			b.assetBarriers,
			native.NewPretradePoliciesRateLimitAssetBarrier(
				native.PretradeRuleMode(barrier.Mode),
				barrier.Limit.MaxOrders,
				windowNanos,
				barrier.SettlementAsset.Handle(),
			),
		)
		b.assetBarrierSettlementAssets = append(
			b.assetBarrierSettlementAssets,
			barrier.SettlementAsset,
		)
	}
	return b
}

// AccountBarriers adds per-account barriers and returns a ready builder.
func (b *RateLimitBuilder) AccountBarriers(
	barriers ...RateLimitAccountBarrier,
) *RateLimitReadyBuilder {
	b.builder.AccountBarriers(barriers...)
	return b.builder
}

// AccountBarriers appends per-account rate-limit barriers.
func (b *RateLimitReadyBuilder) AccountBarriers(
	barriers ...RateLimitAccountBarrier,
) *RateLimitReadyBuilder {
	if b.err != nil {
		return b
	}
	for _, barrier := range barriers {
		if err := native.ValidateRuleMode(native.PretradeRuleMode(barrier.Mode), "account", len(b.accountBarriers)); err != nil {
			b.err = err
			return b
		}
		windowNanos := rateLimitWindowNanoseconds(barrier.Limit)
		b.accountBarriers = append(
			b.accountBarriers,
			native.NewPretradePoliciesRateLimitAccountBarrier(
				native.PretradeRuleMode(barrier.Mode),
				barrier.AccountID.Handle(),
				barrier.Limit.MaxOrders,
				windowNanos,
			),
		)
	}
	return b
}

// AccountAssetBarriers adds per-(account, settlement-asset) barriers and
// returns a ready builder.
func (b *RateLimitBuilder) AccountAssetBarriers(
	barriers ...RateLimitAccountAssetBarrier,
) *RateLimitReadyBuilder {
	b.builder.AccountAssetBarriers(barriers...)
	return b.builder
}

// AccountAssetBarriers appends per-(account, settlement-asset) rate-limit
// barriers.
func (b *RateLimitReadyBuilder) AccountAssetBarriers(
	barriers ...RateLimitAccountAssetBarrier,
) *RateLimitReadyBuilder {
	if b.err != nil {
		return b
	}
	for _, barrier := range barriers {
		if err := native.ValidateRuleMode(native.PretradeRuleMode(barrier.Mode), "account_asset", len(b.accountAssetBarriers)); err != nil {
			b.err = err
			return b
		}
		windowNanos := rateLimitWindowNanoseconds(barrier.Limit)
		b.accountAssetBarriers = append(
			b.accountAssetBarriers,
			native.NewPretradePoliciesRateLimitAccountAssetBarrier(
				native.PretradeRuleMode(barrier.Mode),
				barrier.AccountID.Handle(),
				barrier.Limit.MaxOrders,
				windowNanos,
				barrier.SettlementAsset.Handle(),
			),
		)
		b.accountAssetBarrierSettlementAssets = append(
			b.accountAssetBarrierSettlementAssets,
			barrier.SettlementAsset,
		)
	}
	return b
}

// Build registers the built-in rate-limit policy on the given engine
// builder.
func (b *RateLimitReadyBuilder) Build(builder native.EngineBuilder, instance *native.PolicyInstance) error {
	if b.err != nil {
		return b.err
	}
	err := native.EngineBuilderAddBuiltinRateLimit(
		builder,
		native.PolicyGroupID(b.policyGroupID),
		b.brokerBarriers,
		b.assetBarriers,
		b.accountBarriers,
		b.accountAssetBarriers,
		instance,
	)
	runtime.KeepAlive(b)
	return err
}
