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
	"runtime"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// PnlBoundsBrokerBarrier defines broker-level P&L bounds applied across all
// accounts for one settlement asset.
type PnlBoundsBrokerBarrier struct {
	SettlementAsset param.Asset
	// LowerBound is typically negative and represents the loss limit.
	LowerBound optional.Option[param.Pnl]
	// UpperBound is typically positive and represents the profit-taking
	// limit.
	UpperBound optional.Option[param.Pnl]
}

// PnlBoundsAccountAssetBarrier defines per-(account, settlement asset) P&L
// bounds with an initial P&L seed.
type PnlBoundsAccountAssetBarrier struct {
	AccountID       param.AccountID
	SettlementAsset param.Asset
	// LowerBound is typically negative and represents the loss limit for
	// this account.
	LowerBound optional.Option[param.Pnl]
	// UpperBound is typically positive and represents the profit-taking
	// limit for this account.
	UpperBound optional.Option[param.Pnl]
	// InitialPnl is pre-loaded into storage at construction; accumulation
	// starts from this value.
	InitialPnl param.Pnl
}

//------------------------------------------------------------------------------
// PnlBoundsKillswitchBuilder

// PnlBoundsKillswitchBuilder is the entry point for the P&L bounds
// kill-switch policy. Each axis method returns a
// PnlBoundsKillswitchReadyBuilder on which additional axes and Build are
// available.
type PnlBoundsKillswitchBuilder struct {
	builder *PnlBoundsKillswitchReadyBuilder
}

// PnlBoundsKillswitchReadyBuilder holds a fully-configured P&L bounds
// kill-switch policy.
type PnlBoundsKillswitchReadyBuilder struct {
	brokerBarriers  []native.PretradePoliciesPnlBoundsBarrier
	accountBarriers []native.PretradePoliciesPnlBoundsAccountBarrier
	policyGroupID   model.PolicyGroupID
}

// BuildPnlBoundsKillswitch returns a new P&L bounds kill-switch policy
// builder.
func BuildPnlBoundsKillswitch() *PnlBoundsKillswitchBuilder {
	return &PnlBoundsKillswitchBuilder{
		builder: &PnlBoundsKillswitchReadyBuilder{policyGroupID: model.DefaultPolicyGroupID},
	}
}

// PolicyGroupID assigns the policy to a pricing group and returns a ready
// builder. When not set the policy uses model.DefaultPolicyGroupID.
func (b *PnlBoundsKillswitchBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *PnlBoundsKillswitchReadyBuilder {
	b.builder.PolicyGroupID(groupID)
	return b.builder
}

// PolicyGroupID assigns the policy to a pricing group. When not set the policy
// uses model.DefaultPolicyGroupID.
func (b *PnlBoundsKillswitchReadyBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *PnlBoundsKillswitchReadyBuilder {
	b.policyGroupID = groupID
	return b
}

// BrokerBarriers adds broker-level P&L bounds barriers and returns a
// ready builder.
func (b *PnlBoundsKillswitchBuilder) BrokerBarriers(
	barriers ...PnlBoundsBrokerBarrier,
) *PnlBoundsKillswitchReadyBuilder {
	b.builder.BrokerBarriers(barriers...)
	return b.builder
}

// BrokerBarriers appends broker-level P&L bounds barriers.
func (b *PnlBoundsKillswitchReadyBuilder) BrokerBarriers(
	barriers ...PnlBoundsBrokerBarrier,
) *PnlBoundsKillswitchReadyBuilder {
	for _, barrier := range barriers {
		b.brokerBarriers = append(
			b.brokerBarriers,
			native.NewPretradePoliciesPnlBoundsBarrier(
				barrier.SettlementAsset.Handle(),
				newParamPnlOptionalFromOptional(barrier.LowerBound),
				newParamPnlOptionalFromOptional(barrier.UpperBound),
			),
		)
	}
	return b
}

// AccountBarriers adds per-(account, settlement-asset) P&L bounds
// barriers and returns a ready builder.
func (b *PnlBoundsKillswitchBuilder) AccountBarriers(
	barriers ...PnlBoundsAccountAssetBarrier,
) *PnlBoundsKillswitchReadyBuilder {
	b.builder.AccountBarriers(barriers...)
	return b.builder
}

// AccountBarriers appends per-(account, settlement-asset) P&L bounds
// barriers.
func (b *PnlBoundsKillswitchReadyBuilder) AccountBarriers(
	barriers ...PnlBoundsAccountAssetBarrier,
) *PnlBoundsKillswitchReadyBuilder {
	for _, barrier := range barriers {
		b.accountBarriers = append(
			b.accountBarriers,
			native.NewPretradePoliciesPnlBoundsAccountBarrier(
				barrier.AccountID.Handle(),
				barrier.SettlementAsset.Handle(),
				newParamPnlOptionalFromOptional(barrier.LowerBound),
				newParamPnlOptionalFromOptional(barrier.UpperBound),
				barrier.InitialPnl.Handle(),
			),
		)
	}
	return b
}

// Build registers the built-in P&L bounds kill-switch policy on the
// given engine builder.
func (b *PnlBoundsKillswitchReadyBuilder) Build(builder native.EngineBuilder) error {
	err := native.EngineBuilderAddBuiltinPnlBoundsKillswitch(
		builder,
		native.PolicyGroupID(b.policyGroupID),
		b.brokerBarriers,
		b.accountBarriers,
	)
	runtime.KeepAlive(b)
	return err
}

func newParamPnlOptionalFromOptional(value optional.Option[param.Pnl]) native.ParamPnlOptional {
	if v, has := value.Get(); has {
		return native.NewParamPnlOptional(v.Handle())
	}
	return native.ParamPnlOptional{}
}
