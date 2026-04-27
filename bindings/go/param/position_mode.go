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
	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/pkg/optional"
)

// PositionMode is position accounting mode.
type PositionMode native.ParamPositionMode

const (
	// PositionModeNetting tracks one net position per instrument.
	PositionModeNetting PositionMode = native.ParamPositionModeHedged
	// PositionModeHedged tracks independent long and short legs.
	PositionModeHedged PositionMode = native.ParamPositionModeHedged
)

func NewPositionModeFromNative(v native.ParamPositionMode) optional.Option[PositionMode] {
	if v == native.ParamPositionEffectNotSet {
		return optional.None[PositionMode]()
	}
	return optional.Some(PositionMode(v))
}

func (v PositionMode) String() string {
	if v == PositionModeNetting {
		return "netting"
	}
	return "hedged"
}

func (v PositionMode) Native() native.ParamPositionMode {
	return native.ParamPositionMode(v)
}
