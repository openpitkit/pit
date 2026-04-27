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

package native

import (
	"testing"
	"unsafe"
)

func TestImportStringEmptyUsesNonNilPointerWithZeroLength(t *testing.T) {
	view := importString("")
	if unsafe.Pointer(view.ptr) == nil {
		t.Fatal(`importString("").ptr = nil, want non-nil`)
	}
	if view.len != 0 {
		t.Fatalf(`importString("").len = %d, want 0`, view.len)
	}
}

func TestStringViewEmptyAndUnsetPaths(t *testing.T) {
	empty := NewStringView("")
	if empty.IsSet() {
		t.Fatal(`NewStringView("").IsSet() = true, want false`)
	}
	if got := empty.Safe(); got != "" {
		t.Fatalf(`NewStringView("").Safe() = %q, want ""`, got)
	}
	if got := empty.Unsafe(); got != "" {
		t.Fatalf(`NewStringView("").Unsafe() = %q, want ""`, got)
	}

	if stringViewNone.IsSet() {
		t.Fatal("stringViewNone.IsSet() = true, want false")
	}
	if got := stringViewNone.Safe(); got != "" {
		t.Fatalf("stringViewNone.Safe() = %q, want empty", got)
	}
	if got := stringViewNone.Unsafe(); got != "" {
		t.Fatalf("stringViewNone.Unsafe() = %q, want empty", got)
	}
}

func TestStringViewSafeAndUnsafeRoundTrip(t *testing.T) {
	longUTF8 := "alpha-\u4f60\u597d-\U0001f642-omega"
	view := NewStringView(longUTF8)
	if !view.IsSet() {
		t.Fatal("NewStringView(non-empty).IsSet() = false, want true")
	}
	if got := view.Safe(); got != longUTF8 {
		t.Fatalf("Safe() = %q, want %q", got, longUTF8)
	}
	if got := view.Unsafe(); got != longUTF8 {
		t.Fatalf("Unsafe() = %q, want %q", got, longUTF8)
	}
}

func TestConsumeSharedStringSuccessPath(t *testing.T) {
	price, err := CreateParamPriceFromStr("123.45")
	if err != nil {
		t.Fatalf("CreateParamPriceFromStr() error = %v", err)
	}

	text, err := ParamPriceToString(price)
	if err != nil {
		t.Fatalf("ParamPriceToString() error = %v", err)
	}
	if text == "" {
		t.Fatal("ParamPriceToString() = empty string, want non-empty")
	}
}

func TestConsumeSharedStringPanicsOnNilHandle(t *testing.T) {
	didPanic := false
	func() {
		defer func() {
			if recover() != nil {
				didPanic = true
			}
		}()
		_ = consumeSharedString(nil)
	}()
	if !didPanic {
		t.Fatal("consumeSharedString(nil) panic = nil, want non-nil")
	}
}
