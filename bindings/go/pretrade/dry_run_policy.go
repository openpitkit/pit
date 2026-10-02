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
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// DryRunPolicy is an optional interface that a Policy implementation may
// satisfy to provide explicit read-only dry-run hooks.
//
// When a policy that implements DryRunPolicy is registered, the engine calls
// the dry-run methods instead of the normal ones for non-mutating dry-run
// paths, so the policy can report what would happen without spending budget,
// applying holds, or producing any other side effect.
//
// A policy that mutates engine state in CheckPreTradeStart or
// PerformPreTradeCheck (for example, a rate limiter that spends budget) MUST
// implement DryRunPolicy with read-only variants. If DryRunPolicy is not
// implemented, the engine delegates to the normal hooks for the dry-run path,
// which will cause those side effects to happen during a dry-run.
//
// The method signatures mirror CheckPreTradeStart and PerformPreTradeCheck on
// Policy exactly; only the semantics differ (no mutations, no side effects).
type DryRunPolicy interface {
	// CheckPreTradeStartDryRun is the read-only variant of CheckPreTradeStart.
	//
	// Must not mutate any engine or external state. Returns the rejects the
	// start stage would produce for order with zero side effects.
	CheckPreTradeStartDryRun(Context, model.Order) []reject.Reject

	// PerformPreTradeCheckDryRun is the read-only variant of
	// PerformPreTradeCheck.
	//
	// Must not mutate any engine or external state. The mutations handle is
	// provided for API symmetry but any mutations pushed to it are discarded
	// by the engine. Returns the rejects the main stage would produce with
	// zero side effects.
	// The mutations value is valid only during this hook call. Do not retain it
	// or use it after return; Push then returns tx.ErrMutationsExpired.
	PerformPreTradeCheckDryRun(Context, model.Order, tx.Mutations, Result) []reject.Reject
}
