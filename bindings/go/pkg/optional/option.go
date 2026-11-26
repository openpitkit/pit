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

package optional

// Option represents a value that may or may not be present.
type Option[Value any] struct {
	v    Value
	some bool
}

// None returns an empty Option with no value set.
func None[Value any]() Option[Value] {
	return Option[Value]{}
}

// Some returns an Option with the given value set.
func Some[Value any](v Value) Option[Value] {
	return Option[Value]{v: v, some: true}
}

// From constructs an Option from a value and a presence flag.
func From[Value any](v Value, ok bool) Option[Value] {
	return Option[Value]{v: v, some: ok}
}

// Get returns the stored value and a boolean indicating whether it is set.
func (o Option[Value]) Get() (Value, bool) {
	return o.v, o.IsSet()
}

// MustGet returns the stored value or panics if no value is set.
//
// The panic is deliberate fail-fast behavior: continuing without a value
// would hide wrong behavior.
func (o Option[Value]) MustGet() Value {
	if !o.IsSet() {
		panic("optional: no value")
	}
	return o.v
}

// Or returns the stored value if set, otherwise returns the provided default.
func (o Option[Value]) Or(def Value) Value {
	if !o.IsSet() {
		return def
	}
	return o.v
}

// IsSet reports whether the Option contains a value.
func (o Option[Value]) IsSet() bool {
	return o.some
}

// Set assigns a value and marks the Option as set.
func (o *Option[Value]) Set(v Value) {
	o.v = v
	o.some = true
}

// Unset clears the value and marks the Option as not set.
func (o *Option[Value]) Unset() {
	var zero Value
	o.v = zero
	o.some = false
}
