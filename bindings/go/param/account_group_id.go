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

package param

import (
	"errors"
	"strconv"

	"go.openpit.dev/openpit/internal/native"
)

// AccountGroupID is a type-safe account-group identifier. Its zero value is
// uninitialized and names no account group. It must stay distinguishable from
// [DefaultAccountGroup]: handle 0 legally selects the global default currency
// tier, so an unset value cannot be caught further down. Every constructor
// initializes its result, including [NewAccountGroupIDFromHandle] when passed 0.
type AccountGroupID struct {
	native      native.ParamAccountGroupID
	initialized bool
}

// ErrUninitializedAccountGroupID is returned when an uninitialized
// AccountGroupID reaches an operation that accepts [DefaultAccountGroup] as a
// target - account-group currency, market-data TTL, quote publishing, and
// market-data group resolution - or the asynchronous engine's account-group
// routing. Its handle 0 is [DefaultAccountGroup], so the engine cannot tell the
// unset value from a deliberate one there; the SDK rejects it before the call
// reaches the engine. Operations that never accept the default group leave the
// unset value to the engine, which rejects it as the reserved default group.
var ErrUninitializedAccountGroupID = errors.New(
	"param: account group ID is uninitialized",
)

// DefaultAccountGroup is the reserved account group that every account belongs
// to until it is assigned to another group. It is the only account-group
// identifier that cannot be produced by NewAccountGroupIDFromUint32 or
// NewAccountGroupIDFromString.
var DefaultAccountGroup = AccountGroupID{
	native:      native.DefaultAccountGroup,
	initialized: true,
}

// NewAccountGroupIDFromUint32 constructs an account-group identifier from an
// unsigned 32-bit integer value. Returns an error when source is the reserved
// default account-group value (0); use DefaultAccountGroup to refer to it.
func NewAccountGroupIDFromUint32(source uint32) (AccountGroupID, error) {
	value, err := native.CreateParamAccountGroupIDFromUint32(source)
	if err != nil {
		return AccountGroupID{}, err
	}
	return AccountGroupID{native: value, initialized: true}, nil
}

// NewAccountGroupIDFromString constructs an account-group identifier by hashing
// the string with FNV-1a. Any non-empty string is accepted and produces a
// stable, deterministic ID. Use [NewAccountGroupIDFromUint32] for numeric IDs -
// the two constructors are not interchangeable.
func NewAccountGroupIDFromString(source string) (AccountGroupID, error) {
	value, err := native.CreateParamAccountGroupIDFromString(source)
	if err != nil {
		return AccountGroupID{}, err
	}
	return AccountGroupID{native: value, initialized: true}, nil
}

// NewAccountGroupIDFromHandle creates an AccountGroupID from a native handle.
func NewAccountGroupIDFromHandle(source native.ParamAccountGroupID) AccountGroupID {
	return AccountGroupID{native: source, initialized: true}
}

// IsInitialized reports whether the identifier was explicitly constructed or
// is [DefaultAccountGroup].
func (v AccountGroupID) IsInitialized() bool {
	return v.initialized
}

// String formats the account-group identifier as a decimal string.
func (v AccountGroupID) String() string {
	return strconv.FormatUint(uint64(v.native), 10)
}

// Handle exposes the underlying native account-group identifier.
func (v AccountGroupID) Handle() native.ParamAccountGroupID {
	return v.native
}
