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

package main

import (
	"github.com/openpitkit/pit/bindings/go/openpit"
	"github.com/openpitkit/pit/bindings/go/openpit/pretrade"
)

// This example demonstrates adapter-only wiring:
// - client payload types (`brokerOrderData`, `brokerReportData`)
// - thin payload adapters (`brokerOrder`, `brokerExecutionReport`)
// - policy adapters in SafeSlow cast mode.
//
// Engine construction/execution is intentionally omitted.

// brokerOrderData is client order payload from external API.
type brokerOrderData struct {
	symbol     string
	settlement string
	side       string
	quantity   string
	price      string
	clientTag  uint32
}

// brokerReportData is client report payload from external API.
type brokerReportData struct {
	symbol     string
	settlement string
	pnl        string
	fee        string
}

// brokerOrder is thin adapter over client payload.
//
// The actual method set mirrors openpit.Order contract.
type brokerOrder struct {
	source brokerOrderData
}

// brokerExecutionReport is thin adapter over client payload.
//
// The actual method set mirrors openpit.ExecutionReport contract.
type brokerExecutionReport struct {
	source brokerReportData
}

// tagPolicy demonstrates SafeSlow start-policy adapter usage.
type tagPolicy struct{}

func (v *tagPolicy) Name() string { return "TagPolicy" }

func (v *tagPolicy) CheckPreTradeStart(order *brokerOrder) *pretrade.Reject {
	if order.source.clientTag == 0 {
		return &pretrade.Reject{
			Policy:  v.Name(),
			Scope:   pretrade.RejectScopeOrder,
			Code:    pretrade.RejectCodeInvalidFieldValue,
			Reason:  "client_tag must be non-zero",
			Details: "broker payload requires non-zero tag",
		}
	}
	return nil
}

func (v *tagPolicy) ApplyExecutionReport(report *brokerExecutionReport) bool {
	_ = report
	return false
}

// lossGuardPolicy demonstrates SafeSlow main-policy adapter usage.
type lossGuardPolicy struct{}

func (v *lossGuardPolicy) Name() string { return "LossGuardPolicy" }

func (v *lossGuardPolicy) PerformPreTradeCheck(
	order *brokerOrder,
	context pretrade.Context,
	decision *pretrade.PolicyDecision,
) {
	_ = context
	if order.source.clientTag == 0 {
		decision.Rejects = append(decision.Rejects, pretrade.Reject{
			Policy:  v.Name(),
			Scope:   pretrade.RejectScopeOrder,
			Code:    pretrade.RejectCodeInvalidFieldValue,
			Reason:  "client_tag must be non-zero",
			Details: "broker payload requires non-zero tag",
		})
	}
}

func (v *lossGuardPolicy) ApplyExecutionReport(report *brokerExecutionReport) bool {
	_ = report
	return false
}

func main() {
	// The demo keeps only adapter wiring.
	// Real engine wiring uses constructors and builder.
	var startAdapter pretrade.CheckPreTradeStartPolicy = pretrade.NewStartPolicyAdapterWithSafeSlowArgType[
		*brokerOrder,
		*brokerExecutionReport,
	](&tagPolicy{})

	var mainAdapter pretrade.Policy = pretrade.NewPolicyAdapterWithSafeSlowArgType[
		*brokerOrder,
		*brokerExecutionReport,
	](&lossGuardPolicy{})

	// Keep references to avoid "unused" in sample-only file.
	var _ openpit.Order
	var _ openpit.ExecutionReport
	_, _ = startAdapter, mainAdapter
}
