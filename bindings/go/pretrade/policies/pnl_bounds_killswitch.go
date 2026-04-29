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
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
)

// PnlBoundsBarrier defines one settlement-specific P&L bounds barrier.
type PnlBoundsBarrier struct {
	// SettlementAsset selects the settlement asset being monitored.
	SettlementAsset param.Asset
	// LowerBound is typically negative; it represents the loss limit.
	LowerBound optional.Option[param.Pnl]
	// UpperBound is typically positive; it represents the profit-taking limit.
	UpperBound optional.Option[param.Pnl]
	// InitialPnl is the initial accumulated P&L for this settlement asset.
	InitialPnl param.Pnl
}

// NewPnlBoundsKillSwitchPolicy creates a P&L bounds kill-switch policy.
//
// The constructor does not validate signs of LowerBound/UpperBound, does not
// validate LowerBound <= UpperBound, and does not validate InitialPnl is
// inside the configured band.
//
// Notes:
//   - if InitialPnl is already outside bounds, first start_pre_trade is rejected
//     with PnlKillSwitchTriggered;
//   - if LowerBound > UpperBound, start_pre_trade rejects until accumulated P&L
//     returns inside bounds or engine is rebuilt.
//
// Must be closed with Close.
func NewPnlBoundsKillSwitchPolicy(
	barriers ...PnlBoundsBarrier,
) (pretrade.BuiltinPolicy, error) {
	params := make([]native.PretradePoliciesPnlBoundsBarrier, len(barriers))
	for i, barrier := range barriers {
		lower := native.ParamPnlOptional{}
		if value, ok := barrier.LowerBound.Get(); ok {
			lower = native.NewParamPnlOptional(value.Handle())
		}

		upper := native.ParamPnlOptional{}
		if value, ok := barrier.UpperBound.Get(); ok {
			upper = native.NewParamPnlOptional(value.Handle())
		}

		params[i] = native.NewPretradePoliciesPnlBoundsBarrier(
			barrier.SettlementAsset.Handle(),
			lower,
			upper,
			barrier.InitialPnl.Handle(),
		)
	}

	return newCheckPreTradeStartPolicyWithError(
		func() (native.PretradeCheckPreTradeStartPolicy, error) {
			return native.CreatePretradePoliciesPnlBoundsKillSwitchPolicy(params)
		},
	)
}
