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

// PnlKillSwitchBarrier defines one settlement-specific loss barrier.
type PnlKillSwitchBarrier struct {
	// SettlementAsset selects the settlement asset being monitored.
	SettlementAsset param.Asset
	// Barrier is the maximum allowed realized loss for the settlement asset.
	Barrier param.Pnl
}

// NewPnlKillSwitchPolicy creates a P&L kill-switch policy.
//
// PnlKillSwitchPolicy is a start-stage policy that blocks trading after
// crossing configured loss limits.
//
// Must be closed with Close.
func NewPnlKillSwitchPolicy(
	barriers ...PnlKillSwitchBarrier,
) (pretrade.BuiltinPolicy, error) {
	params := make([]native.PretradePoliciesPnlKillSwitchParam, len(barriers))
	for i, barrier := range barriers {
		params[i] = native.NewPretradePoliciesPnlKillSwitchParam(
			barrier.SettlementAsset.Handle(),
			barrier.Barrier.Handle(),
		)
	}

	return newCheckPreTradeStartPolicyWithError(
		func() (native.PretradeCheckPreTradeStartPolicy, error) {
			return native.CreatePretradePoliciesPnlKillSwitchPolicy(params)
		},
	)
}
