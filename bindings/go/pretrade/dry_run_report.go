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
	"errors"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/reject"
)

// ErrDryRunReportClosed is returned by DryRunReport accessors once the report
// has been released.
var ErrDryRunReportClosed = errors.New("pre-trade dry-run report already closed")

// DryRunReport holds the result of a non-mutating pre-trade dry-run.
//
// Lifecycle: the caller takes ownership and must release it with Close when
// done. Nothing releases it on the caller's behalf - there is no finalizer, so
// a report dropped without Close leaks its native handle.
//
// Concurrency: callers must serialize methods on the same report. Every
// accessor reports ErrDryRunReportClosed once Close releases the handle,
// because the zero value it would otherwise return reads as a verdict.
type DryRunReport struct {
	handle native.PretradePreTradeDryRunReport
}

// NewDryRunReportFromHandle wraps a native dry-run report handle.
func NewDryRunReportFromHandle(handle native.PretradePreTradeDryRunReport) *DryRunReport {
	return &DryRunReport{handle: handle}
}

// IsClosed reports whether the report no longer owns a native handle.
func (r *DryRunReport) IsClosed() bool {
	return r.handle == nil
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
// Returns ErrDryRunReportClosed after Close: a fabricated verdict either way
// would be acted on as if the dry-run had produced it.
func (r *DryRunReport) IsPass() (bool, error) {
	if r.handle == nil {
		return false, ErrDryRunReportClosed
	}
	return native.PretradePreTradeDryRunReportIsPass(r.handle), nil
}

// Rejects returns the rejects the order would have collected.
//
// Returns nil when the order would have passed. The returned slice is
// independent of the report lifetime.
//
// Returns ErrDryRunReportClosed after Close: an empty slice would claim no
// policy objected.
func (r *DryRunReport) Rejects() ([]reject.Reject, error) {
	if r.handle == nil {
		return nil, ErrDryRunReportClosed
	}
	handle := native.PretradePreTradeDryRunReportGetRejects(r.handle)
	count := native.PretradeRejectListLen(handle)
	if count == 0 {
		native.DestroyPretradeRejectList(handle)
		return nil, nil
	}
	result := make([]reject.Reject, count)
	for i := 0; i < count; i++ {
		result[i] = reject.NewFromHandle(native.PretradeRejectListGet(handle, i))
	}
	native.DestroyPretradeRejectList(handle)
	return result, nil
}

// Lock returns a snapshot of the lock the main stage would have produced.
//
// The lock is empty when the start stage would have rejected (the main stage
// never runs in that case) or when no policy locks a price.
//
// Returns ErrDryRunReportClosed after Close: Bytes and Equal read the zero Lock
// without decoding it, so it would pass for the lock the main stage produced.
func (r *DryRunReport) Lock() (Lock, error) {
	if r.handle == nil {
		return Lock{}, ErrDryRunReportClosed
	}
	handle := native.PretradePreTradeDryRunReportGetLock(r.handle)
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// AccountAdjustments returns the account-adjustment outcomes the main stage
// would have produced. Returns nil when none were produced.
//
// Returns ErrDryRunReportClosed after Close: an empty slice would claim the
// order would move no funds.
func (r *DryRunReport) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	if r.handle == nil {
		return nil, ErrDryRunReportClosed
	}
	handle := native.PretradePreTradeDryRunReportGetAccountAdjustments(r.handle)
	result := accountadjustment.NewListFromHandle(handle)
	native.DestroyAccountAdjustmentOutcomeList(handle)
	return result, nil
}

// AccountBlock returns the account block an account-scope reject would have
// latched. Returns nil when no account-scope reject would have latched a
// block.
//
// Returns ErrDryRunReportClosed after Close: a nil block would claim the order
// would leave the account tradable.
func (r *DryRunReport) AccountBlock() (*reject.AccountBlock, error) {
	if r.handle == nil {
		return nil, ErrDryRunReportClosed
	}
	list := native.PretradePreTradeDryRunReportGetAccountBlock(r.handle)
	defer native.DestroyPretradeAccountBlockList(list)
	if native.PretradeAccountBlockListLen(list) == 0 {
		return nil, nil //nolint:nilnil // a nil block is the documented "none latched" answer, not a missing value
	}
	block := reject.NewAccountBlockFromHandle(native.PretradeAccountBlockListGet(list, 0))
	return &block, nil
}
