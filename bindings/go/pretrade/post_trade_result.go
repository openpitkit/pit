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

package pretrade

import (
	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/reject"
)

// PostTradeResult holds the outcome of applying an execution report.
//
// This is the canonical post-trade result type for the SDK. It lives in the
// pretrade package - the shared home of the pre-/post-trade domain - so
// that the synchronous engine, the typed ClientEngine, and the optional
// async facade all speak the same type without any of them depending on
// another for it. The async engine returns this type directly;
// openpit.PostTradeResult is a thin alias to it for the root package's
// public surface.
//
// AccountPnls and AccountAdjustments describe already-applied state
// changes. Callers must consume both even when AccountBlocks is
// non-empty.
type PostTradeResult struct {
	// AccountBlocks lists the accounts that have been blocked after the
	// execution report was applied.
	AccountBlocks []reject.AccountBlock
	// AccountPnls lists policy-tagged account-level realized-PnL
	// computations. Amount returns an authoritative PnL when available;
	// HaltReason identifies the failure. SpotFunds emits a halted outcome only
	// for the report that transitions the accumulator to halted; later reports
	// omit the unchanged halt. Position force-sets do not re-arm it.
	AccountPnls []accountadjustment.AccountPnlOutcome
	// AccountAdjustments lists the account-adjustment outcomes
	// policies produced while applying the report.
	AccountAdjustments []accountadjustment.Outcome
}
