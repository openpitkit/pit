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

package model

import (
	"testing"

	"go.openpit.dev/openpit/param"
)

func TestPnlStateRepresentsValueOrExplicitHalt(t *testing.T) {
	t.Parallel()

	value, err := param.NewPnlFromString("12.5")
	if err != nil {
		t.Fatalf("NewPnlFromString() error = %v", err)
	}
	numeric := NewPnlState(value)
	got, ok := numeric.Value()
	if !ok || !got.Equal(value) {
		t.Fatalf("Value() = (%v, %v), want (12.5, true)", got, ok)
	}
	if _, ok := numeric.HaltReason(); ok {
		t.Fatal("numeric HaltReason() is set")
	}

	halted, err := NewPnlHaltedState(PnlHaltReasonMissingFx)
	if err != nil {
		t.Fatalf("NewPnlHaltedState() error = %v", err)
	}
	if _, ok := halted.Value(); ok {
		t.Fatal("halted Value() is authoritative")
	}
	reason, ok := halted.HaltReason()
	if !ok || reason != PnlHaltReasonMissingFx {
		t.Fatalf("HaltReason() = (%v, %v), want (missing-fx, true)", reason, ok)
	}
}
