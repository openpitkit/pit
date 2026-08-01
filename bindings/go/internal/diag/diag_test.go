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

package diag

import (
	"errors"
	"testing"
)

func TestReportWithoutHandlerIsSilent(t *testing.T) {
	t.Cleanup(func() { SetHandler(nil) })

	SetHandler(nil)
	Report(errors.New("teardown failed"))
}

func TestReportReachesInstalledHandler(t *testing.T) {
	t.Cleanup(func() { SetHandler(nil) })

	want := errors.New("teardown failed")
	var got error
	SetHandler(func(err error) { got = err })

	Report(want)
	if !errors.Is(got, want) {
		t.Fatalf("handler got %v, want %v", got, want)
	}
}

func TestSetHandlerNilRemovesThePreviousHandler(t *testing.T) {
	t.Cleanup(func() { SetHandler(nil) })

	calls := 0
	SetHandler(func(error) { calls++ })
	SetHandler(nil)

	Report(errors.New("teardown failed"))
	if calls != 0 {
		t.Fatalf("handler calls = %d, want 0", calls)
	}
}

// TestReportContainsAPanickingHandler pins that a broken handler cannot unwind
// into the cgo teardown frame Report runs on.
func TestReportContainsAPanickingHandler(t *testing.T) {
	t.Cleanup(func() { SetHandler(nil) })

	SetHandler(func(error) { panic("handler exploded") })
	Report(errors.New("teardown failed"))
}
