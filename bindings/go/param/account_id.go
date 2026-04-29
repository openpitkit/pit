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
	"go.openpit.dev/openpit/pkg/optional"
)

// AccountID is a type-safe account identifier.
type AccountID struct {
	native native.ParamAccountID
}

var ErrAccountIDEmpty = native.ErrAccountIdEmpty

// NewAccountIDFromInt constructs an account identifier from an integer value.
func NewAccountIDFromInt(source uint64) AccountID {
	return NewAccountIDFromHandle(native.CreateParamAccountIDFromU64(source))
}

// NewAccountIDFromString constructs an account identifier by hashing input string with FNV-1a
// 64-bit.
//
// Collision probability (birthday bound approximation):
//
// - 1,000 accounts: < 3 x 10^-14
// - 10,000 accounts: < 3 x 10^-12
// - 100,000 accounts: < 3 x 10^-10
// - 1,000,000 accounts: < 3 x 10^-8
//
// If collision risk is unacceptable, use your own collision-free
// string-to-integer mapping and construct account identifiers from integers.
func NewAccountIDFromString(source string) (AccountID, error) {
	value, err := native.CreateParamAccountIDFromStr(source)
	if err != nil {
		return AccountID{}, err
	}
	return NewAccountIDFromHandle(value), nil
}

func NewAccountIDFromHandle(source native.ParamAccountID) AccountID {
	return AccountID{native: source}
}

func NewAccountIDOptionFromHandle(
	source native.ParamAccountIDOptional,
) optional.Option[AccountID] {
	if !native.ParamAccountIDOptionalIsSet(source) {
		return optional.None[AccountID]()
	}
	return optional.Some(NewAccountIDFromHandle(native.ParamAccountIDOptionalGet(source)))
}

// String formats account identifier as decimal string.
func (v AccountID) String() string {
	return strconv.FormatUint(uint64(v.native), 10)
}

// Handle exposes the underlying native account identifier.
func (v AccountID) Handle() native.ParamAccountID {
	return v.native
}
