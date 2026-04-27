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
	"math"
	"strconv"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pkg/optional"
)

const (
	// LeverageScale is the fixed-point scale used by leverage payloads.
	//
	// Raw leverage is stored as:
	//   raw = multiplier * LeverageScale
	//
	// Examples:
	//   raw 10   -> 1.0x
	//   raw 11   -> 1.1x
	//   raw 1005 -> 100.5x
	LeverageScale native.ParamLeverage = native.ParamLeverageScale

	// LeverageMin is the minimum business leverage multiplier in whole units.
	LeverageMin uint16 = uint16(native.ParamLeverageMin)

	// LeverageMax is the maximum business leverage multiplier in whole units.
	LeverageMax uint16 = uint16(native.ParamLeverageMax)

	// LeverageStep is the fractional increment between adjacent leverage values.
	LeverageStep float32 = float32(native.ParamLeverageStep)
)

// Leverage is a fixed-point leverage multiplier payload.
//
// Storage format:
//   - underlying value is uint16 in fixed-point scale 10;
//   - one decimal place is encoded in the raw integer.
//
// API behavior:
//   - this type does not perform upfront business validation;
//   - it is a transport-friendly wrapper around native leverage payload;
//   - invalid values are expected to be rejected by operations that consume
//     leverage and return their own errors.
//
// Zero value:
//   - zero value is valid as a payload state and represents "not set"
//     (`native.ParamLeverageNotSet`) when interpreted as optional leverage.
type Leverage struct {
	native native.ParamLeverage
}

// LeverageZero is the zero-value leverage payload (`not set` state).
var LeverageZero = Leverage{}

// NewLeverageFromNative wraps native leverage payload as Leverage.
//
// No business validation is performed.
func NewLeverageFromNative(v native.ParamLeverage) Leverage {
	return Leverage{native: v}
}

// NewLeverageOptionFromNative converts native leverage payload into optional
// Leverage.
//
// `native.ParamLeverageNotSet` maps to empty option; any other value maps to
// `Some(Leverage)`.
func NewLeverageOptionFromNative(v native.ParamLeverage) optional.Option[Leverage] {
	if v == native.ParamLeverageNotSet {
		return optional.None[Leverage]()
	}
	return optional.Some(NewLeverageFromNative(v))
}

// NewLeverageFromInt builds leverage from integer multiplier.
//
// Conversion uses fixed-point encoding:
//
//	raw = multiplier * LeverageScale
//
// No business validation is performed.
func NewLeverageFromInt(multiplier uint16) Leverage {
	return Leverage{
		native: native.ParamLeverage(multiplier) * LeverageScale,
	}
}

// NewLeverageFromFloat32 builds leverage from float32 multiplier.
//
// Conversion uses fixed-point encoding with rounding to nearest tenth:
//
//	raw = round(multiplier * LeverageScale)
//
// No business validation is performed.
func NewLeverageFromFloat32(multiplier float32) Leverage {
	raw := native.ParamLeverage(math.Round(float64(multiplier * float32(LeverageScale))))
	return Leverage{native: raw}
}

// Raw returns underlying fixed-point leverage payload.
func (v Leverage) Raw() native.ParamLeverage {
	return v.native
}

// IsSet reports whether leverage payload is explicitly set.
//
// False means payload is equal to `native.ParamLeverageNotSet`.
func (v Leverage) IsSet() bool {
	return v.native != native.ParamLeverageNotSet
}

// Value returns leverage multiplier as float32.
//
// Example:
//
//	raw 1005 -> 100.5
func (v Leverage) Value() float32 {
	return float32(v.native) / float32(LeverageScale)
}

// CalculateMarginRequired returns margin for the provided notional.
//
// Formula:
//
//	margin = notional / leverage
func (v Leverage) CalculateMarginRequired(notional Notional) (Notional, error) {
	result, err := native.ParamLeverageCalculateMarginRequired(v.Native(), notional.Native())
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

// String returns normalized decimal leverage string.
//
// Formatting rules:
//   - no trailing ".0" for integer multipliers;
//   - one decimal digit for fractional multipliers.
func (v Leverage) String() string {
	integer := uint16(v.native / LeverageScale)
	fractional := uint16(v.native % LeverageScale)
	if fractional == 0 {
		return strconv.FormatUint(uint64(integer), 10)
	}
	return strconv.FormatUint(uint64(integer), 10) + "." + strconv.FormatUint(uint64(fractional), 10)
}

// Native returns underlying native leverage payload.
func (v Leverage) Native() native.ParamLeverage {
	return v.native
}
