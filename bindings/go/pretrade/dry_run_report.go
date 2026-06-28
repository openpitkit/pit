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
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/reject"
)

// DryRunReport holds the result of a non-mutating pre-trade dry-run.
//
// The caller takes ownership and must release it with Close when done.
type DryRunReport struct {
	handle native.PretradePreTradeDryRunReport
}

// NewDryRunReportFromHandle wraps a native dry-run report handle.
func NewDryRunReportFromHandle(handle native.PretradePreTradeDryRunReport) *DryRunReport {
	return &DryRunReport{handle: handle}
}

// Close releases the report.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (r *DryRunReport) Close() {
	if r.handle == nil {
		return
	}
	native.DestroyPretradePreTradeDryRunReport(r.handle)
	r.handle = nil
}

// IsPass reports whether the order would have passed every pre-trade stage.
//
// Panics if the report is already closed.
// The panic is deliberate fail-fast behavior: continuing with a closed
// native handle would hide wrong behavior.
func (r *DryRunReport) IsPass() bool {
	if r.handle == nil {
		panic("pre-trade dry-run report already closed")
	}
	return native.PretradePreTradeDryRunReportIsPass(r.handle)
}

// Rejects returns the rejects the order would have collected.
//
// Returns nil when the order would have passed. The returned slice is
// independent of the report lifetime.
//
// Panics if the report is already closed.
// The panic is deliberate fail-fast behavior: continuing with a closed
// native handle would hide wrong behavior.
func (r *DryRunReport) Rejects() []reject.Reject {
	if r.handle == nil {
		panic("pre-trade dry-run report already closed")
	}
	handle := native.PretradePreTradeDryRunReportGetRejects(r.handle)
	count := native.PretradeRejectListLen(handle)
	if count == 0 {
		native.DestroyPretradeRejectList(handle)
		return nil
	}
	result := make([]reject.Reject, count)
	for i := 0; i < count; i++ {
		result[i] = reject.NewFromHandle(native.PretradeRejectListGet(handle, i))
	}
	native.DestroyPretradeRejectList(handle)
	return result
}

// Lock returns a snapshot of the lock the main stage would have produced.
//
// The lock is empty when the start stage would have rejected (the main stage
// never runs in that case) or when no policy locks a price.
//
// Panics if the report is already closed.
// The panic is deliberate fail-fast behavior: continuing with a closed
// native handle would hide wrong behavior.
func (r *DryRunReport) Lock() Lock {
	if r.handle == nil {
		panic("pre-trade dry-run report already closed")
	}
	handle := native.PretradePreTradeDryRunReportGetLock(r.handle)
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result
}

// AccountAdjustments returns the account-adjustment outcomes the main stage
// would have produced. Returns nil when none were produced.
//
// Panics if the report is already closed.
// The panic is deliberate fail-fast behavior: continuing with a closed
// native handle would hide wrong behavior.
func (r *DryRunReport) AccountAdjustments() []accountadjustment.Outcome {
	if r.handle == nil {
		panic("pre-trade dry-run report already closed")
	}
	handle := native.PretradePreTradeDryRunReportGetAccountAdjustments(r.handle)
	result := accountadjustment.NewListFromHandle(handle)
	native.DestroyAccountAdjustmentOutcomeList(handle)
	return result
}

// AccountBlock returns the account block an account-scope reject would have
// latched. Returns nil when no account-scope reject would have latched a
// block.
//
// Panics if the report is already closed.
// The panic is deliberate fail-fast behavior: continuing with a closed
// native handle would hide wrong behavior.
func (r *DryRunReport) AccountBlock() *reject.AccountBlock {
	if r.handle == nil {
		panic("pre-trade dry-run report already closed")
	}
	list := native.PretradePreTradeDryRunReportGetAccountBlock(r.handle)
	defer native.DestroyPretradeAccountBlockList(list)
	if native.PretradeAccountBlockListLen(list) == 0 {
		return nil
	}
	block := reject.NewAccountBlockFromHandle(native.PretradeAccountBlockListGet(list, 0))
	return &block
}
