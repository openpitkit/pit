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
//
// Barrier carries the settlement asset and bounds configuration (identical to
// the broker-level shape); AccountID and InitialPnl bind it to a specific
// account. Both the broker barrier (if any) and this account+asset barrier are
// evaluated on every check; the order passes only if neither is breached.
type PnlBoundsAccountAssetBarrier struct {
	// Barrier holds the settlement asset and P&L bounds for this
	// account+asset pair. The fields mirror PnlBoundsBrokerBarrier so that
	// per-account bounds can be expressed with the same type vocabulary as
	// broker-level bounds.
	Barrier PnlBoundsBrokerBarrier
	// AccountID is the account this barrier applies to.
	AccountID param.AccountID
	// InitialPnl is pre-loaded into storage at construction; accumulation
	// starts from this value.
	InitialPnl param.Pnl
}

// PnlBoundsAccountAssetBarrierUpdate updates bounds for an existing
// per-(account, settlement asset) accumulator without replacing its live P&L.
type PnlBoundsAccountAssetBarrierUpdate struct {
	Barrier PnlBoundsBrokerBarrier
	// AccountID is the account this barrier applies to.
	AccountID param.AccountID
}

//------------------------------------------------------------------------------
// PnlBoundsKillSwitchBuilder

// PnlBoundsKillSwitchBuilder is the entry point for the P&L bounds
// kill-switch policy. Each axis method returns a
// PnlBoundsKillSwitchReadyBuilder on which additional axes and Build are
// available.
type PnlBoundsKillSwitchBuilder struct {
	builder *PnlBoundsKillSwitchReadyBuilder
}

// PnlBoundsKillSwitchReadyBuilder holds a fully-configured P&L bounds
// kill-switch policy.
type PnlBoundsKillSwitchReadyBuilder struct {
	brokerBarriers  []native.PretradePoliciesPnlBoundsBarrier
	accountBarriers []native.PretradePoliciesPnlBoundsAccountBarrier
	policyGroupID   model.PolicyGroupID
}

// BuildPnlBoundsKillSwitch returns a new P&L bounds kill-switch policy
// builder.
func BuildPnlBoundsKillSwitch() *PnlBoundsKillSwitchBuilder {
	return &PnlBoundsKillSwitchBuilder{
		builder: &PnlBoundsKillSwitchReadyBuilder{policyGroupID: model.DefaultPolicyGroupID},
	}
}

// PolicyGroupID assigns the policy to a pricing group and returns a ready
// builder. When not set the policy uses model.DefaultPolicyGroupID.
func (b *PnlBoundsKillSwitchBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *PnlBoundsKillSwitchReadyBuilder {
	b.builder.PolicyGroupID(groupID)
	return b.builder
}

// PolicyGroupID assigns the policy to a pricing group. When not set the policy
// uses model.DefaultPolicyGroupID.
func (b *PnlBoundsKillSwitchReadyBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *PnlBoundsKillSwitchReadyBuilder {
	b.policyGroupID = groupID
	return b
}

// BrokerBarriers adds broker-level P&L bounds barriers and returns a
// ready builder.
func (b *PnlBoundsKillSwitchBuilder) BrokerBarriers(
	barriers ...PnlBoundsBrokerBarrier,
) *PnlBoundsKillSwitchReadyBuilder {
	b.builder.BrokerBarriers(barriers...)
	return b.builder
}

// BrokerBarriers appends broker-level P&L bounds barriers.
func (b *PnlBoundsKillSwitchReadyBuilder) BrokerBarriers(
	barriers ...PnlBoundsBrokerBarrier,
) *PnlBoundsKillSwitchReadyBuilder {
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
func (b *PnlBoundsKillSwitchBuilder) AccountBarriers(
	barriers ...PnlBoundsAccountAssetBarrier,
) *PnlBoundsKillSwitchReadyBuilder {
	b.builder.AccountBarriers(barriers...)
	return b.builder
}

// AccountBarriers appends per-(account, settlement-asset) P&L bounds
// barriers.
func (b *PnlBoundsKillSwitchReadyBuilder) AccountBarriers(
	barriers ...PnlBoundsAccountAssetBarrier,
) *PnlBoundsKillSwitchReadyBuilder {
	for _, barrier := range barriers {
		b.accountBarriers = append(
			b.accountBarriers,
			native.NewPretradePoliciesPnlBoundsAccountBarrier(
				barrier.AccountID.Handle(),
				barrier.Barrier.SettlementAsset.Handle(),
				newParamPnlOptionalFromOptional(barrier.Barrier.LowerBound),
				newParamPnlOptionalFromOptional(barrier.Barrier.UpperBound),
				barrier.InitialPnl.Handle(),
			),
		)
	}
	return b
}

// Build registers the built-in P&L bounds kill-switch policy on the
// given engine builder.
func (b *PnlBoundsKillSwitchReadyBuilder) Build(builder native.EngineBuilder) error {
	err := native.EngineBuilderAddBuiltinPnlBoundsKillSwitch(
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
