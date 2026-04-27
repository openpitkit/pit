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

package optional

import "testing"

func TestBoolSomeTrueReturnsTrue(t *testing.T) {
	b := BoolSome(true)
	v, ok := b.Get()
	if !ok {
		t.Fatal("Get() ok = false, want true")
	}
	if !v {
		t.Fatal("Get() value = false, want true")
	}
}

func TestBoolSomeFalseReturnsFalse(t *testing.T) {
	b := BoolSome(false)
	v, ok := b.Get()
	if !ok {
		t.Fatal("Get() ok = false, want true")
	}
	if v {
		t.Fatal("Get() value = true, want false")
	}
}

func TestBoolGetOnNoneReturnsFalseAndUnset(t *testing.T) {
	v, ok := BoolNone.Get()
	if ok {
		t.Fatal("Get() ok = true, want false")
	}
	if v {
		t.Fatal("Get() value = true, want false")
	}
}

func TestBoolMustGetReturnsValueWhenSet(t *testing.T) {
	if !BoolSome(true).MustGet() {
		t.Fatal("MustGet() = false, want true")
	}
	if BoolSome(false).MustGet() {
		t.Fatal("MustGet() = true, want false")
	}
}

func TestBoolMustGetPanicsWhenNotSet(t *testing.T) {
	defer func() {
		if r := recover(); r == nil {
			t.Fatal("expected panic")
		}
	}()
	BoolNone.MustGet()
}

func TestBoolOrReturnsStoredValueWhenSet(t *testing.T) {
	if !BoolSome(true).Or(false) {
		t.Fatal("Or(false) = false, want true")
	}
	if BoolSome(false).Or(true) {
		t.Fatal("Or(true) = true, want false")
	}
}

func TestBoolOrReturnsDefaultWhenNotSet(t *testing.T) {
	if !BoolNone.Or(true) {
		t.Fatal("Or(true) = false, want true")
	}
	if BoolNone.Or(false) {
		t.Fatal("Or(false) = true, want false")
	}
}

func TestBoolIsSetReturnsTrueWhenSet(t *testing.T) {
	if !BoolSome(false).IsSet() {
		t.Fatal("IsSet() = false, want true")
	}
	if !BoolSome(true).IsSet() {
		t.Fatal("IsSet() = false, want true")
	}
}

func TestBoolIsSetReturnsFalseForNone(t *testing.T) {
	if BoolNone.IsSet() {
		t.Fatal("IsSet() = true for BoolNone, want false")
	}
}

func TestBoolSetChangesValue(t *testing.T) {
	b := BoolNone
	b.Set(true)
	v, ok := b.Get()
	if !ok {
		t.Fatal("Get() ok = false, want true")
	}
	if !v {
		t.Fatal("Get() = false, want true")
	}
}

func TestBoolUnsetClearsValue(t *testing.T) {
	b := BoolSome(true)
	b.Unset()
	if b.IsSet() {
		t.Fatal("IsSet() = true after Unset, want false")
	}
}
