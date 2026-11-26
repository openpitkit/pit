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

package naming

// ruleid: short-generic-parameter-go
func single[T any](value T) {}

// ruleid: short-generic-parameter-go
type Digit[T1 any] struct{ value T1 }

// ruleid: short-generic-parameter-go
func acronym[LPF any](value LPF) {}

// ruleid: short-generic-parameter-go
type ShortName[Id comparable] struct{ value Id }

// ruleid: short-generic-parameter-go
type Grouped[Value, T any] struct {
	first  Value
	second T
}

// ruleid: short-generic-parameter-go
func nested[
	Container Wrapper[Nested[Value]],
	Ab any,
	Value any,
]() {
}

// ruleid: short-generic-parameter-go
func (item *Full[T]) Get() T { return item.value }

// ok: short-generic-parameter-go
type Full[Value any] struct{ value Value }

// ok: short-generic-parameter-go
func (item *Full[Value]) Load() Value { return item.value }

// ok: short-generic-parameter-go
type Array [N]int

const N, Count = 2, 3

// ok: short-generic-parameter-go
type ProductArray [N * Count]int

// ok: short-generic-parameter-go
type ByteArray = [len("Box[T any]")]byte

type Fields struct {
	// ok: short-generic-parameter-go
	// like sync.Map[K any]
	// ok: short-generic-parameter-go
	data [N << 2]byte
}

// ruleid: short-generic-parameter-go
type Box[T any] struct{}

// ruleid: short-generic-parameter-go
type Set[T comparable] = map[T]struct{}

// ruleid: short-generic-parameter-go
type Pointer[T *struct{}] struct{}

// ruleid: short-generic-parameter-go
type InterfacePointer[T *interface{ M() }] struct{}

// ruleid: short-generic-parameter-go
type MapPointer[T *map[string]int] struct{}

// ruleid: short-generic-parameter-go
type SlicePointer[T *[]int] struct{}

// Keep this declaration on one line to exercise the inline form.
// ruleid: short-generic-parameter-go
type ( InlineBox[T any] struct{} )

type (
	FullBox[Value any] struct {
		value Value
	}
	// ruleid: short-generic-parameter-go
	GroupedBox[T any] struct{}
	// ok: short-generic-parameter-go
	GroupedArray    [N * Count]int
	GroupedCallback func(
		// ok: short-generic-parameter-go
		data [N << 2]byte,
	) (
		// ok: short-generic-parameter-go
		result [N << 2]byte,
	)
	// ruleid: short-generic-parameter-go
	PointerBox[T *int,] struct{}
)

// ruleid: short-generic-parameter-go
func pointer[T *int](value T) {}
