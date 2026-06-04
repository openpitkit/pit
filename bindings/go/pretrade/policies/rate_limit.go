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
	"fmt"
	"runtime"
	"time"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/ptr"
)

// RateLimit defines the maximum number of orders allowed within a
// sliding window.
type RateLimit struct {
	// MaxOrders is the maximum number of orders accepted within Window.
	MaxOrders uint
	// Window is the length of the sliding time window.
	Window time.Duration
}

// RateLimitBrokerBarrier applies a rate limit across the entire broker.
type RateLimitBrokerBarrier struct {
	Limit RateLimit
}

// RateLimitAssetBarrier applies a rate limit per settlement asset.
type RateLimitAssetBarrier struct {
	Limit           RateLimit
	SettlementAsset param.Asset
}

// RateLimitAccountBarrier applies a rate limit per account.
type RateLimitAccountBarrier struct {
	Limit     RateLimit
	AccountID param.AccountID
}

// RateLimitAccountAssetBarrier applies a rate limit per
// (account, settlement asset) pair.
type RateLimitAccountAssetBarrier struct {
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
	broker               *native.PretradePoliciesRateLimitBrokerBarrier
	assetBarriers        []native.PretradePoliciesRateLimitAssetBarrier
	accountBarriers      []native.PretradePoliciesRateLimitAccountBarrier
	accountAssetBarriers []native.PretradePoliciesRateLimitAccountAssetBarrier
	policyGroupID        model.PolicyGroupID
	err                  error
}

// BuildRateLimit returns a new rate-limit policy builder.
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

func validateWindow(window time.Duration) error {
	if window < 0 {
		return fmt.Errorf(
			"rate limit window must be non-negative; got %s",
			window,
		)
	}
	return nil
}

// BrokerBarrier sets the broker-wide rate limit and returns a ready
// builder.
func (b *RateLimitBuilder) BrokerBarrier(barrier RateLimitBrokerBarrier) *RateLimitReadyBuilder {
	b.builder.BrokerBarrier(barrier)
	return b.builder
}

// BrokerBarrier sets or replaces the broker-wide rate limit.
func (b *RateLimitReadyBuilder) BrokerBarrier(
	barrier RateLimitBrokerBarrier,
) *RateLimitReadyBuilder {
	if b.err != nil {
		return b
	}
	if err := validateWindow(barrier.Limit.Window); err != nil {
		b.err = err
		return b
	}
	windowNanos := uint64(barrier.Limit.Window) //nolint:gosec // time.Duration is always non-negative after validateWindow
	b.broker = ptr.New(native.NewPretradePoliciesRateLimitBrokerBarrier(
		barrier.Limit.MaxOrders,
		windowNanos,
	))
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
		if err := validateWindow(barrier.Limit.Window); err != nil {
			b.err = err
			return b
		}
		windowNanos := uint64(barrier.Limit.Window) //nolint:gosec // time.Duration is always non-negative after validateWindow
		b.assetBarriers = append(
			b.assetBarriers,
			native.NewPretradePoliciesRateLimitAssetBarrier(
				barrier.Limit.MaxOrders,
				windowNanos,
				barrier.SettlementAsset.Handle(),
			),
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
		if err := validateWindow(barrier.Limit.Window); err != nil {
			b.err = err
			return b
		}
		windowNanos := uint64(barrier.Limit.Window) //nolint:gosec // time.Duration is always non-negative after validateWindow
		b.accountBarriers = append(
			b.accountBarriers,
			native.NewPretradePoliciesRateLimitAccountBarrier(
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
		if err := validateWindow(barrier.Limit.Window); err != nil {
			b.err = err
			return b
		}
		windowNanos := uint64(barrier.Limit.Window) //nolint:gosec // time.Duration is always non-negative after validateWindow
		b.accountAssetBarriers = append(
			b.accountAssetBarriers,
			native.NewPretradePoliciesRateLimitAccountAssetBarrier(
				barrier.AccountID.Handle(),
				barrier.Limit.MaxOrders,
				windowNanos,
				barrier.SettlementAsset.Handle(),
			),
		)
	}
	return b
}

// Build registers the built-in rate-limit policy on the given engine
// builder.
func (b *RateLimitReadyBuilder) Build(builder native.EngineBuilder) error {
	if b.err != nil {
		return b.err
	}
	err := native.EngineBuilderAddBuiltinRateLimit(
		builder,
		native.PolicyGroupID(b.policyGroupID),
		b.broker,
		b.assetBarriers,
		b.accountBarriers,
		b.accountAssetBarriers,
	)
	runtime.KeepAlive(b)
	return err
}
