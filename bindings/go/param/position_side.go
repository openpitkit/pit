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
	"fmt"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pkg/optional"
)

// PositionSide is open position direction.
type PositionSide native.ParamPositionSide

const (
	// PositionSideLong is long direction.
	PositionSideLong PositionSide = native.ParamPositionSideLong
	// PositionSideShort is short direction.
	PositionSideShort PositionSide = native.ParamPositionSideShort
)

func NewPositionSideFromHandle(v native.ParamPositionSide) optional.Option[PositionSide] {
	switch v {
	case native.ParamPositionSideLong:
		return optional.Some(PositionSideLong)
	case native.ParamPositionSideShort:
		return optional.Some(PositionSideShort)
	case native.ParamPositionSideNotSet:
		return optional.None[PositionSide]()
	default:
		panic(fmt.Sprintf("unknown native ParamPositionSide value %d", v))
	}
}

// IsLong returns true when side is long.
func (v PositionSide) IsLong() bool {
	return v == PositionSideLong
}

// IsShort returns true when side is short.
func (v PositionSide) IsShort() bool {
	return v == PositionSideShort
}

// Opposite returns the opposite position side.
func (v PositionSide) Opposite() PositionSide {
	if v == PositionSideLong {
		return PositionSideShort
	}
	return PositionSideLong
}

func (v PositionSide) String() string {
	if v == PositionSideLong {
		return "long"
	}
	return "short"
}
