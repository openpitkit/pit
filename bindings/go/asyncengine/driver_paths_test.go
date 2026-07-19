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

package asyncengine

import (
	"context"
	"errors"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/accounts"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// rejectDriver is a fake driver that always returns non-empty rejects (no
// request/reservation, no error) — the "policy reject" path.
type rejectDriver struct {
	startRejects   []reject.Reject
	executeRejects []reject.Reject
}

func newRejectDriver() *rejectDriver {
	return &rejectDriver{
		// Non-empty slices to trigger the rejects branch. The reject values
		// themselves are zero-valued structs; the async layer only checks nil
		// vs non-nil.
		startRejects:   []reject.Reject{{}},
		executeRejects: []reject.Reject{{}},
	}
}

func (d *rejectDriver) StartPreTrade(
	_ model.Order,
) (*pretrade.Request, []reject.Reject, error) {
	return nil, d.startRejects, nil
}

func (d *rejectDriver) ExecutePreTrade(
	_ model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	return nil, d.executeRejects, nil
}

func (*rejectDriver) ApplyExecutionReport(
	_ model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	return pretrade.PostTradeResult{}, nil
}

func (*rejectDriver) ApplyAccountAdjustment(
	_ param.AccountID,
	_ []model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	return accountadjustment.BatchResult{}, nil
}

func (*rejectDriver) Accounts() accounts.Accounts {
	return accounts.Accounts{}
}

// transportErrorDriver is a fake driver that returns a non-nil transport error
// from StartPreTrade and ExecutePreTrade.
type transportErrorDriver struct {
	startErr   error
	executeErr error
}

func newTransportErrorDriver() *transportErrorDriver {
	return &transportErrorDriver{
		startErr:   errors.New("transport: start failed"),
		executeErr: errors.New("transport: execute failed"),
	}
}

func (d *transportErrorDriver) StartPreTrade(
	_ model.Order,
) (*pretrade.Request, []reject.Reject, error) {
	return nil, nil, d.startErr
}

func (d *transportErrorDriver) ExecutePreTrade(
	_ model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	return nil, nil, d.executeErr
}

func (*transportErrorDriver) ApplyExecutionReport(
	_ model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	return pretrade.PostTradeResult{}, nil
}

func (*transportErrorDriver) ApplyAccountAdjustment(
	_ param.AccountID,
	_ []model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	return accountadjustment.BatchResult{}, nil
}

func (*transportErrorDriver) Accounts() accounts.Accounts {
	return accounts.Accounts{}
}

// TestAsyncEngineStartPreTradeRejectsPath asserts that a non-nil rejects
// slice from the driver resolves the future as (nil, rejects, nil).
func TestAsyncEngineStartPreTradeRejectsPath(t *testing.T) {
	t.Parallel()
	driver := newRejectDriver()
	async, err := NewBuilder(driver).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	f := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	req, rejects, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() err = %v, want nil", err)
	}
	if req != nil {
		t.Errorf("request = %v, want nil (rejected)", req)
	}
	if len(rejects) == 0 {
		t.Errorf("rejects = %v, want non-empty", rejects)
	}
}

// TestAsyncEngineStartPreTradeTransportErrorPath asserts that a non-nil
// transport error from the driver resolves the future with that error.
func TestAsyncEngineStartPreTradeTransportErrorPath(t *testing.T) {
	t.Parallel()
	driver := newTransportErrorDriver()
	async, err := NewBuilder(driver).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	f := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	req, rejects, err := f.Await(context.Background())
	if !errors.Is(err, driver.startErr) {
		t.Fatalf("Await() err = %v, want %v", err, driver.startErr)
	}
	if req != nil {
		t.Errorf("request = %v, want nil on transport error", req)
	}
	if rejects != nil {
		t.Errorf("rejects = %v, want nil on transport error", rejects)
	}
}

// TestAsyncEngineExecutePreTradeRejectsPath asserts that a non-nil rejects
// slice from ExecutePreTrade resolves the future as (nil, rejects, nil).
func TestAsyncEngineExecutePreTradeRejectsPath(t *testing.T) {
	t.Parallel()
	driver := newRejectDriver()
	async, err := NewBuilder(driver).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	f := async.ExecutePreTrade(context.Background(), buildTestOrder(t, 1))
	res, rejects, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() err = %v, want nil", err)
	}
	if res != nil {
		t.Errorf("reservation = %v, want nil (rejected)", res)
	}
	if len(rejects) == 0 {
		t.Errorf("rejects = %v, want non-empty", rejects)
	}
}

// TestAsyncEngineExecutePreTradeTransportErrorPath asserts that a non-nil
// transport error from ExecutePreTrade resolves the future with that error.
func TestAsyncEngineExecutePreTradeTransportErrorPath(t *testing.T) {
	t.Parallel()
	driver := newTransportErrorDriver()
	async, err := NewBuilder(driver).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	f := async.ExecutePreTrade(context.Background(), buildTestOrder(t, 1))
	res, rejects, err := f.Await(context.Background())
	if !errors.Is(err, driver.executeErr) {
		t.Fatalf("Await() err = %v, want %v", err, driver.executeErr)
	}
	if res != nil {
		t.Errorf("reservation = %v, want nil on transport error", res)
	}
	if rejects != nil {
		t.Errorf("rejects = %v, want nil on transport error", rejects)
	}
}
