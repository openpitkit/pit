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

package param

import (
	"hash/fnv"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pkg/optional"
)

// Asset is an asset or currency identifier, for example USD, AAPL, or SPX.
//
// # Memory model
//
// Asset stores its identifier bytes in a C-heap buffer allocated once at
// construction time.  The buffer is freed automatically by a GC finalizer when
// the last copy of the Asset (and therefore the last reference to the
// underlying *native.String) is collected.
//
// This design solves a CGo constraint: every C struct that carries a string
// field (PitStringView) stores a raw pointer to the string bytes.  When such a
// struct is held in Go memory and passed to C via a Go pointer, the CGo checker
// enforces "Go memory must not contain Go pointers".  By keeping the bytes on
// the C heap, PitStringView.ptr is always a C pointer — invisible to the
// checker.  See internal/native/asset_buf.go for the allocation details.
//
// # Lifetime contract for model structs
//
// Model structs (Order, AccountAdjustment, ExecutionReport, …) write
// PitStringView pointers into their C structs when an Asset is set.  Those
// pointers remain valid only as long as at least one Asset value referencing
// the same *native.String stays alive.  To guarantee this, every model
// struct that accepts an Asset keeps a named retain* field that holds the Asset
// value for its own lifetime.  See the model package for details.
//
// # Equality and hashing
//
// Asset is NOT comparable with == because pointer identity does not imply
// string equality (two independently created "USD" assets have different
// *String pointers).  Use Equal for semantic comparison and Hash for use in
// hash-map implementations.
//
// # Finalizer guarantees
//
// runtime.SetFinalizer does NOT guarantee the finalizer runs before program
// exit — only that it runs before the GC reclaims the object.  In a
// high-frequency trading process the GC runs frequently under memory pressure,
// so C buffers are reclaimed promptly.  A small leak on abnormal exit is
// acceptable because the OS reclaims the process address space.
type Asset struct {
	buf *native.String
}

var ErrAssetEmpty = native.ErrAssetEmpty

// NewAsset validates v and creates an Asset whose bytes live in a C-heap
// buffer freed automatically by the GC.
func NewAsset(v string) (Asset, error) {
	validated, err := native.CreateParamAssetFromStr(v)
	if err != nil {
		return Asset{}, err
	}
	return Asset{buf: native.NewString(validated)}, nil
}

// NewAssetFromHandle builds an Asset from a native StringView returned by the
// C API.  Returns None when the view is not set (nil/empty).
func NewAssetFromHandle(v native.StringView) optional.Option[Asset] {
	if !v.IsSet() {
		return optional.None[Asset]()
	}
	// v.Safe() copies the C bytes into a Go string so we can pass a clean
	// string to NewString, which then copies them into its own C buffer.
	return optional.Some(Asset{buf: native.NewString(v.Safe())})
}

// Safe returns a Go-heap copy of the identifier.  The returned string is
// independent of the Asset's C buffer and remains valid after the Asset is
// collected.
func (a Asset) Safe() string {
	if a.buf == nil {
		return ""
	}
	return a.buf.Safe()
}

// Unsafe returns a Go string header whose backing bytes are the Asset's C
// buffer.  The string is valid only as long as this Asset (or any copy of it,
// or any model struct retaining it) is live.  Use on hot paths where avoiding
// a copy matters; prefer Safe() when the string may outlive the Asset.
func (a Asset) Unsafe() string {
	if a.buf == nil {
		return ""
	}
	return a.buf.Unsafe()
}

// String implements fmt.Stringer.  Returns the same value as Safe().
func (a Asset) String() string { return a.Safe() }

// Equal reports whether a and b identify the same asset by comparing their
// byte content.  Always use Equal instead of == for semantic comparison.
func (a Asset) Equal(b Asset) bool {
	return a.Unsafe() == b.Unsafe()
}

// Hash returns a content-based FNV-64a hash of the identifier.  Assets that
// are Equal always produce the same Hash value, making it safe to use as a
// hash-map key.
func (a Asset) Hash() uint64 {
	if a.buf == nil {
		return 0
	}
	h := fnv.New64a()
	_, _ = h.Write([]byte(a.Unsafe()))
	return h.Sum64()
}

// Handle returns a Go string backed by the Asset's C buffer, for use by
// importString in internal/native.  Because the backing bytes are C-heap,
// importString stores a C pointer in PitStringView.ptr — not a Go pointer —
// so no CGo pointer-check violation occurs.
func (a Asset) Handle() string { return a.Unsafe() }
