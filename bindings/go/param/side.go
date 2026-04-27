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

// Side represents the side of a trade or order.
type Side native.ParamSide

const (
	// SideBuy means buy side.
	SideBuy Side = native.ParamSideBuy
	// SideSell means sell side.
	SideSell Side = native.ParamSideSell
)

func NewSideFromNative(v native.ParamSide) optional.Option[Side] {
	switch v {
	case native.ParamSideSell:
		return optional.Some(SideSell)
	case native.ParamSideBuy:
		return optional.Some(SideBuy)
	case native.ParamSideNotSet:
		return optional.None[Side]()
	default:
		panic(fmt.Sprintf("unknown native ParamSide value %d", v))
	}
}

// IsBuy returns true when side is buy.
func (v Side) IsBuy() bool {
	return v == SideBuy
}

// IsSell returns true when side is sell.
func (v Side) IsSell() bool {
	return v == SideSell
}

// Opposite returns the opposite side.
func (v Side) Opposite() Side {
	if v == SideBuy {
		return SideSell
	}
	return SideBuy
}

// Sign returns +1 for buy and -1 for sell.
func (v Side) Sign() int8 {
	if v == SideBuy {
		return 1
	}
	return -1
}

func (v Side) String() string {
	if v == SideBuy {
		return "buy"
	}
	return "sell"
}

func (v Side) Native() native.ParamSide {
	if v == SideSell {
		return native.ParamSideSell
	}
	return native.ParamSideBuy
}
