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

type Bool int8

const (
	// BoolNone returns a Bool no value set.
	BoolNone Bool = -1
	// False returns a Bool false value set.
	False Bool = 0
	// True returns a Bool true value set.
	True Bool = 1
)

// BoolSome returns an Bool with the given value set.
func BoolSome(v bool) Bool {
	if v {
		return True
	}
	return False
}

// Get returns the stored value and a boolean indicating whether it is set.
func (b Bool) Get() (value bool, isSet bool) {
	return b == True, b.IsSet()
}

// MustGet returns the stored value or panics if no value is set.
func (b Bool) MustGet() bool {
	if !b.IsSet() {
		panic("optional bool: no value")
	}
	return b == True
}

// Or returns the stored value if set, otherwise returns the provided default.
func (b Bool) Or(def bool) bool {
	if !b.IsSet() {
		return def
	}
	return b == True
}

// IsSet reports whether the Option contains a value.
func (b Bool) IsSet() bool {
	return b != BoolNone
}

// Set assigns a value and marks the Option as set.
func (b *Bool) Set(v bool) {
	*b = BoolSome(v)
}

// Unset clears the value and marks the Option as not set.
func (b *Bool) Unset() {
	*b = BoolNone
}
