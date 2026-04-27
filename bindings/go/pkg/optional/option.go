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

// Option represents a value that may or may not be present.
type Option[T any] struct {
	v    T
	some bool
}

// None returns an empty Option with no value set.
func None[T any]() Option[T] {
	return Option[T]{}
}

// Some returns an Option with the given value set.
func Some[T any](v T) Option[T] {
	return Option[T]{v: v, some: true}
}

// From constructs an Option from a value and a presence flag.
func From[T any](v T, ok bool) Option[T] {
	return Option[T]{v: v, some: ok}
}

// Get returns the stored value and a boolean indicating whether it is set.
func (o Option[T]) Get() (T, bool) {
	return o.v, o.IsSet()
}

// MustGet returns the stored value or panics if no value is set.
func (o Option[T]) MustGet() T {
	if !o.IsSet() {
		panic("optional: no value")
	}
	return o.v
}

// Or returns the stored value if set, otherwise returns the provided default.
func (o Option[T]) Or(def T) T {
	if !o.IsSet() {
		return def
	}
	return o.v
}

// IsSet reports whether the Option contains a value.
func (o Option[T]) IsSet() bool {
	return o.some
}

// Set assigns a value and marks the Option as set.
func (o *Option[T]) Set(v T) {
	o.v = v
	o.some = true
}

// Unset clears the value and marks the Option as not set.
func (o *Option[T]) Unset() {
	var zero T
	o.v = zero
	o.some = false
}
