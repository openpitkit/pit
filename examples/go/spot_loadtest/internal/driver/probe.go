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
	"fmt"
	"time"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// probeAccount is a synthetic account used exclusively by the overhead probe.
// It must differ from any account in the workload stream to avoid perturbing
// the oracle or shadow ledger.
const probeAccount = "__overhead_probe__"

// overheadProbe submits one trivial ApplyAccountAdjustment (zero-value
// adjustment on the probe account) through the same async engine path as the
// real workload, measures the submit->decision round-trip, and returns the
// latency. This characterises the bare FFI+asyncengine overhead with no
// meaningful policy work.
//
// The probe is called sequentially before the workload starts (quiescent
// engine, no queue contention from the workload), so it measures the overhead
// of the submit path itself, not queueing delay from concurrency.
func (r *run) overheadProbe(ctx context.Context) (time.Duration, error) {
	acc, err := param.NewAccountIDFromString(probeAccount)
	if err != nil {
		return 0, fmt.Errorf("probe account: %w", err)
	}
	asset, err := param.NewAsset("USD")
	if err != nil {
		return 0, fmt.Errorf("probe asset: %w", err)
	}
	amount, err := param.NewPositionSizeFromString("0")
	if err != nil {
		return 0, fmt.Errorf("probe amount: %w", err)
	}
	adj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(model.NewAccountAdjustmentBalanceOperationFromValues(
			model.AccountAdjustmentBalanceOperationValues{Asset: optional.Some(asset)})),
		Amount: optional.Some(model.NewAccountAdjustmentAmountFromValues(
			model.AccountAdjustmentAmountValues{Balance: optional.Some(param.NewDeltaAdjustmentAmount(amount))})),
	})
	if err != nil {
		return 0, fmt.Errorf("probe adjustment: %w", err)
	}

	t0 := time.Now()
	fut := r.engine.ApplyAccountAdjustment(ctx, acc, []model.AccountAdjustment{adj})
	_, _, err = fut.Await(ctx)
	latency := time.Since(t0)
	if err != nil {
		return 0, fmt.Errorf("probe await: %w", err)
	}
	return latency, nil
}
