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

package custompolicy

import (
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

type fakeAccountAdjustmentPolicy struct {
	name string
}

func (p *fakeAccountAdjustmentPolicy) Close() {}

func (p *fakeAccountAdjustmentPolicy) Name() string { return p.name }

func (p *fakeAccountAdjustmentPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
) reject.List {
	return nil
}

func TestStartAccountAdjustmentSuccess(t *testing.T) {
	policy := &fakeAccountAdjustmentPolicy{name: "test-aa-policy"}
	handle, err := StartAccountAdjustment(policy)
	if err != nil {
		t.Fatalf("StartAccountAdjustment() error = %v, want nil", err)
	}
	if handle == nil {
		t.Fatal("StartAccountAdjustment() = nil, want non-nil")
	}
	t.Cleanup(func() { native.DestroyAccountAdjustmentPolicy(handle) })
}

func TestStartAccountAdjustmentErrorOnInvalidName(t *testing.T) {
	policy := &fakeAccountAdjustmentPolicy{name: ""}
	handle, err := StartAccountAdjustment(policy)
	if handle != nil {
		native.DestroyAccountAdjustmentPolicy(handle)
		t.Fatal("StartAccountAdjustment() handle != nil, want nil on invalid name")
	}
	if err == nil {
		t.Fatal("StartAccountAdjustment() error = nil, want non-nil for invalid name")
	}
}
