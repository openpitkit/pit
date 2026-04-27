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

package convert

import (
	"testing"

	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/pkg/optional"
)

func TestNewNativeTriBoolTrueReturnsTriBoolTrue(t *testing.T) {
	if got := NewNativeTriBool(true); got != native.TriBoolTrue {
		t.Fatalf("NewNativeTriBool(true) = %v, want TriBoolTrue", got)
	}
}

func TestNewNativeTriBoolFalseReturnsTriBoolFalse(t *testing.T) {
	if got := NewNativeTriBool(false); got != native.TriBoolFalse {
		t.Fatalf("NewNativeTriBool(false) = %v, want TriBoolFalse", got)
	}
}

func TestNewBoolOptionFromNativeNotSetReturnsBoolNone(t *testing.T) {
	got := NewBoolOptionFromNative(native.TriBoolNotSet)
	if got != optional.BoolNone {
		t.Fatalf("NewBoolOptionFromNative(TriBoolNotSet) = %v, want BoolNone", got)
	}
}

func TestNewBoolOptionFromNativeTrueReturnsBoolTrue(t *testing.T) {
	got := NewBoolOptionFromNative(native.TriBoolTrue)
	v, ok := got.Get()
	if !ok {
		t.Fatal("Get() ok = false, want true")
	}
	if !v {
		t.Fatal("Get() value = false, want true")
	}
}

func TestNewBoolOptionFromNativeFalseReturnsBoolFalse(t *testing.T) {
	got := NewBoolOptionFromNative(native.TriBoolFalse)
	v, ok := got.Get()
	if !ok {
		t.Fatal("Get() ok = false, want true")
	}
	if v {
		t.Fatal("Get() value = true, want false")
	}
}

func TestNativeTriBoolRoundTrip(t *testing.T) {
	for _, input := range []bool{true, false} {
		native := NewNativeTriBool(input)
		got := NewBoolOptionFromNative(native)
		v, ok := got.Get()
		if !ok {
			t.Fatalf("round-trip(%v): Get() ok = false, want true", input)
		}
		if v != input {
			t.Fatalf("round-trip(%v): Get() = %v, want %v", input, v, input)
		}
	}
}
