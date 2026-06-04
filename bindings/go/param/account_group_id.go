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
	"strconv"

	"go.openpit.dev/openpit/internal/native"
)

// AccountGroupID is a type-safe account-group identifier.
type AccountGroupID struct {
	native native.ParamAccountGroupID
}

// DefaultAccountGroup is the reserved account group that every account belongs
// to until it is assigned to another group. It is the only account-group
// identifier that cannot be produced by NewAccountGroupIDFromUint32 or
// NewAccountGroupIDFromString.
var DefaultAccountGroup = AccountGroupID{native: native.DefaultAccountGroup}

// NewAccountGroupIDFromUint32 constructs an account-group identifier from an
// unsigned 32-bit integer value. Returns an error when source is the reserved
// default account-group value (0); use DefaultAccountGroup to refer to it.
func NewAccountGroupIDFromUint32(source uint32) (AccountGroupID, error) {
	value, err := native.CreateParamAccountGroupIDFromUint32(source)
	if err != nil {
		return AccountGroupID{}, err
	}
	return AccountGroupID{native: value}, nil
}

// NewAccountGroupIDFromString constructs an account-group identifier by hashing
// the string with FNV-1a. Any non-empty string is accepted and produces a
// stable, deterministic ID. Use [NewAccountGroupIDFromUint32] for numeric IDs —
// the two constructors are not interchangeable.
func NewAccountGroupIDFromString(source string) (AccountGroupID, error) {
	value, err := native.CreateParamAccountGroupIDFromString(source)
	if err != nil {
		return AccountGroupID{}, err
	}
	return AccountGroupID{native: value}, nil
}

// NewAccountGroupIDFromHandle creates an AccountGroupID from a native handle.
func NewAccountGroupIDFromHandle(source native.ParamAccountGroupID) AccountGroupID {
	return AccountGroupID{native: source}
}

// String formats the account-group identifier as a decimal string.
func (v AccountGroupID) String() string {
	return strconv.FormatUint(uint64(v.native), 10)
}

// Handle exposes the underlying native account-group identifier.
func (v AccountGroupID) Handle() native.ParamAccountGroupID {
	return v.native
}
