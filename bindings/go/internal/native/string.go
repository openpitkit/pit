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

/*
#include "pit.h"
*/
import "C"
import "unsafe"

var (
	stringViewNone = StringView{}
	stringEmpty    = ""
	stringEmptySet = [1]byte{0}
)

// StringView is a string-backed view without ownership.
//
// - safe: original string is owned and retained
// - unsafe: string aliases external memory
type StringView struct{ value C.PitStringView }

func NewStringView(value string) StringView {
	return StringView{value: importString(value)}
}

func importString(source string) C.PitStringView {
	if len(source) == 0 {
		return C.PitStringView{
			ptr: (*C.uint8_t)(unsafe.Pointer(&stringEmptySet[0])),
			len: 0,
		}
	}
	return C.PitStringView{
		ptr: (*C.uint8_t)(unsafe.Pointer(unsafe.StringData(source))),
		len: C.size_t(len(source)),
	}
}

func newStringView(v C.PitStringView) StringView {
	return StringView{value: v}
}

// Unsafe returns a string backed by the underlying memory without
// copying or nil for an empty and unset StringView.
//
// WARNING:
// - The returned string aliases external memory.
// - If the memory becomes invalid, this leads to undefined behavior.
func (v StringView) Unsafe() string {
	if !v.IsSet() {
		return stringEmpty
	}
	return unsafe.String((*byte)(unsafe.Pointer(v.value.ptr)), int(v.value.len))
}

// Safe returns a fully owned copy of the data as a Go string.
func (v StringView) Safe() string {
	if !v.IsSet() {
		return stringEmpty
	}
	return string(unsafe.Slice(v.value.ptr, v.value.len))
}

// IsSet returns true if the StringView is set and not empty.
func (v StringView) IsSet() bool {
	return v.value.ptr != nil && v.value.len > 0
}

func consumeSharedString(handle SharedString) string {
	if handle == nil {
		panic("shared string is not provided")
	}
	msg := newStringView(C.pit_shared_string_view(handle)).Safe()
	DestroySharedString(handle)
	return msg
}

func DestroySharedString(handle SharedString) {
	C.pit_destroy_shared_string(handle)
}
