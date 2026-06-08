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

package driver

import (
	"context"
	"time"

	"openpit-loadtest-spot-funds-go/internal/generator"
)

// sleepUntil blocks until the absolute deadline elapses or ctx is cancelled.
// It returns true once the deadline has passed (open-loop pacing to a virtual
// arrival), false if ctx was cancelled first. A deadline already in the past
// returns true immediately, so a saturated virtual schedule submits as fast as
// the submitter can issue without ever back-dating the stamped t0.
func sleepUntil(ctx context.Context, deadline time.Time) bool {
	d := time.Until(deadline)
	if d <= 0 {
		return ctx.Err() == nil
	}
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-t.C:
		return true
	case <-ctx.Done():
		return false
	}
}

// partitionChains splits the stream into one ordered slice per account,
// preserving each account's relative (emission/Seq) order. ALL of an account's
// measured events enter its chain - runtime (non-seed) funding, order-checks,
// AND settlements - because the open-loop scheduler submits each event at its
// own virtual arrival time, including settlements (they are no longer triggered
// by the collector). Seeds are applied synchronously on the underlying engine
// before the async run, so they are intentionally excluded. The returned slices
// reference the original events, so they stay cheap.
//
// Per-account ordering is the oracle's correctness premise: an account's
// predictions assume its ops apply in this exact order (a top-up before the
// order it funds, an order-check before its settlement, a settlement before a
// dependent later order). The driver preserves it by submitting each account's
// chain from a single goroutine in this order; the engine's FIFO-per-account
// dispatch then replays the shadow's offline-ordered decisions exactly.
func partitionChains(events []generator.Event) [][]*generator.Event {
	order := make([]string, 0)
	byAccount := make(map[string][]*generator.Event)
	for i := range events {
		ev := &events[i]
		if ev.Kind == generator.EventFunding && ev.FundingIsSeed {
			// Seeds are applied synchronously on the underlying engine before the
			// async run; they must not also be submitted on the measured path.
			continue
		}
		if _, seen := byAccount[ev.Account]; !seen {
			order = append(order, ev.Account)
		}
		byAccount[ev.Account] = append(byAccount[ev.Account], ev)
	}
	chains := make([][]*generator.Event, 0, len(order))
	for _, acc := range order {
		chains = append(chains, byAccount[acc])
	}
	return chains
}
