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

// pit.h pulls in the system size_t definition, so malloc and free can be
// forward-declared here without a separate system header include.
extern void *malloc(size_t n);
extern void  free(void *ptr);
*/
import "C"
import (
	"runtime"
	"unsafe"
)

//------------------------------------------------------------------------------
// StringView

var (
	stringViewNone = StringView{}
	stringEmpty    = ""
)

// StringView is a string-backed view without ownership.
//
// - safe: original string is owned and retained
// - unsafe: string aliases external memory
type StringView struct{ value C.PitStringView }

func NewStringView(value string) StringView {
	return StringView{value: importString(value)}
}

func newStringView(v C.PitStringView) StringView {
	return StringView{value: v}
}

func importString(source string) C.PitStringView {
	if len(source) == 0 {
		return C.PitStringView{}
	}
	return C.PitStringView{
		ptr: (*C.uint8_t)(unsafe.Pointer(unsafe.StringData(source))),
		len: C.size_t(len(source)),
	}
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

//------------------------------------------------------------------------------
// String

// String is the C-heap backing store for a local string that is passed to C.
//
// # Why C heap?
//
// Every C struct that carries a string field (PitStringView) stores a raw
// pointer into the string's backing bytes.  When such a struct lives in
// Go-allocated memory and is passed to a C function via a Go pointer, the CGo
// checker enforces the rule:
//
//	"Go memory passed to C must not contain Go pointers."
//
// If the string bytes were on the Go heap, their address stored in
// PitStringView.ptr would be a Go pointer, triggering a panic:
//
//	"argument of cgo function has Go pointer to unpinned Go pointer"
//
// Allocating the bytes on the C heap makes PitStringView.ptr a C pointer.
// The CGo checker does not flag C pointers, so the panic never occurs.
//
// # Lifetime and cleanup
//
// String is allocated once per business value and freed automatically:
//   - domain types like param.Asset is a struct that holds *String as its only
//     field.
//   - runtime.SetFinalizer is attached to *String. When the GC collects
//     the last domain type instance (and therefore the last *String
//     reference), the finalizer calls C.free on the buffer.
//   - runtime.SetFinalizer does NOT guarantee execution before program exit,
//     only before the GC reclaims the object. In a high-frequency trading
//     process the GC runs under memory pressure, so C buffers are reclaimed
//     promptly during normal operation.  A small leak on abnormal exit is
//     acceptable because the OS reclaims the process address space anyway.
//
// # Thread safety
//
// String is immutable after construction.  Concurrent reads from multiple
// goroutines are safe without synchronization.
type String struct {
	ptr *C.uint8_t
	len int
}

// NewString copies the bytes of validated into a new C-heap buffer and
// registers a GC finalizer that calls C.free when the last reference to the
// returned *String is dropped.
//
// validated must be a non-empty, already-validated asset identifier string.
// The caller (like param.NewAsset / param.NewAssetFromHandle) is responsible
// for validation before calling this function.
func NewString(source string) *String {
	n := len(source)
	ptr := (*C.uint8_t)(C.malloc(C.size_t(n)))
	copy(unsafe.Slice((*byte)(unsafe.Pointer(ptr)), n), source)
	b := &String{ptr: ptr, len: n}
	runtime.SetFinalizer(b, func(b *String) { C.free(unsafe.Pointer(b.ptr)) })
	return b
}

// Unsafe returns a Go string header whose backing bytes are the C buffer.
//
// The string is valid only as long as the *String is reachable (directly or
// through a param.Asset that holds it). Use in hot paths where avoiding a
// copy matters; prefer Safe when the string may outlive the String.
func (b *String) Unsafe() string {
	return unsafe.String((*byte)(unsafe.Pointer(b.ptr)), b.len)
}

// Safe returns a Go-heap copy of the identifier. The returned string is
// fully independent of the C buffer and remains valid after the String is
// collected.
func (b *String) Safe() string {
	return string(unsafe.Slice((*byte)(unsafe.Pointer(b.ptr)), b.len))
}

// Len returns the byte length of the string.
func (b *String) Len() int { return b.len }

//------------------------------------------------------------------------------
