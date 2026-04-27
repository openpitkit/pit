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

func TestFromSetCreatesSetOption(t *testing.T) {
	o := From(42, true)
	v, ok := o.Get()
	if !ok {
		t.Fatal("Get() ok = false, want true")
	}
	if v != 42 {
		t.Fatalf("Get() = %d, want 42", v)
	}
}

func TestFromUnsetCreatesUnsetOption(t *testing.T) {
	o := From(42, false)
	_, ok := o.Get()
	if ok {
		t.Fatal("Get() ok = true, want false")
	}
}

func TestMustGetReturnsValueWhenSet(t *testing.T) {
	o := Some("hello")
	if got := o.MustGet(); got != "hello" {
		t.Fatalf("MustGet() = %q, want %q", got, "hello")
	}
}

func TestMustGetPanicsWhenNotSet(t *testing.T) {
	defer func() {
		if r := recover(); r == nil {
			t.Fatal("expected panic")
		}
	}()
	None[int]().MustGet()
}

func TestOrReturnsStoredValueWhenSet(t *testing.T) {
	if got := Some(10).Or(99); got != 10 {
		t.Fatalf("Or() = %d, want 10", got)
	}
}

func TestOrReturnsDefaultWhenNotSet(t *testing.T) {
	if got := None[int]().Or(99); got != 99 {
		t.Fatalf("Or() = %d, want 99", got)
	}
}

func TestSetMarksOptionAsSet(t *testing.T) {
	var o Option[string]
	o.Set("world")
	v, ok := o.Get()
	if !ok {
		t.Fatal("Get() ok = false, want true")
	}
	if v != "world" {
		t.Fatalf("Get() = %q, want %q", v, "world")
	}
}

func TestUnsetClearsValueAndPresence(t *testing.T) {
	o := Some(7)
	o.Unset()
	if o.IsSet() {
		t.Fatal("IsSet() = true after Unset, want false")
	}
	if v, _ := o.Get(); v != 0 {
		t.Fatalf("Get() value = %d after Unset, want 0", v)
	}
}

func TestSomeNoneRoundTrip(t *testing.T) {
	v, ok := Some(5).Get()
	if !ok || v != 5 {
		t.Fatalf("Some(5).Get() = (%d, %v), want (5, true)", v, ok)
	}
	_, ok = None[int]().Get()
	if ok {
		t.Fatal("None[int]().Get() ok = true, want false")
	}
}
