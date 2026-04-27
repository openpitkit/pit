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
	"github.com/shopspring/decimal"
)

// Notional is the monetary position exposure used for margin and risk
// calculation.
//
// Notional represents the absolute monetary value of a position in the
// settlement currency: |price| × quantity. It is always non-negative and
// captures the full face value at risk regardless of leverage.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Notional only once via
// NewNotionalFromString / NewNotionalFromDecimal / NewNotionalFromNative.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Notional struct {
	native native.ParamNotional
}

var NotionalZero = newNotionalOrPanic(NewNotionalFromInt(0))

func newNotionalOrPanic(value Notional, err error) Notional {
	if err != nil {
		panic(err)
	}
	return value
}

// NewNotionalFromDecimal converts a shopspring decimal to a Notional.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewNotionalFromDecimal(v decimal.Decimal) (Notional, error) {
	nativeValue, err := native.CreateParamNotional(native.NewNativeDecimalFromDecimal(v))
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

func NewNotionalFromString(v string) (Notional, error) {
	nativeValue, err := native.CreateParamNotionalFromStr(v)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

func NewNotionalFromInt(v int64) (Notional, error) {
	nativeValue, err := native.CreateParamNotionalFromI64(v)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

func NewNotionalFromUint(v uint64) (Notional, error) {
	nativeValue, err := native.CreateParamNotionalFromU64(v)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewNotionalFromString or NewNotionalFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewNotionalFromFloat(v float64) (Notional, error) {
	nativeValue, err := native.CreateParamNotionalFromF64(v)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

func NewNotionalFromNative(v native.ParamNotional) Notional {
	return Notional{native: v}
}

func NewNotionalOptionFromNative(v native.ParamNotionalOptional) optional.Option[Notional] {
	if !native.ParamNotionalOptionalIsSet(v) {
		return optional.None[Notional]()
	}
	return optional.Some(NewNotionalFromNative(native.ParamNotionalOptionalGet(v)))
}

func NewNotionalFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Notional, error) {
	nativeValue, err := native.CreateParamNotionalFromStrRounded(v, scale, strategy.native())
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

func NewNotionalFromFloatRounded(
	v float64,
	scale uint32,
	strategy RoundingStrategy,
) (Notional, error) {
	nativeValue, err := native.CreateParamNotionalFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

// NewNotionalFromDecimalRounded converts a shopspring decimal to a rounded Notional.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewNotionalFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (Notional, error) {
	nativeValue, err := native.CreateParamNotionalFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

// NewNotionalFromVolume converts trade volume into position notional.
//
// Both types represent monetary amounts in the settlement currency; this cast
// changes the semantic context from "order size" to "position exposure".
func NewNotionalFromVolume(v Volume) (Notional, error) {
	nativeValue, err := native.ParamNotionalFromVolume(v.Native())
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(nativeValue), nil
}

func (v Notional) Decimal() decimal.Decimal {
	return newDecimalFromNative(native.ParamNotionalGetDecimal(v.native))
}

func (v Notional) Native() native.ParamNotional {
	return v.native
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewNotionalFromString or NewNotionalFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Notional) Float() float64 {
	return newParamValueOrPanic(native.ParamNotionalToF64(v.native))
}

func (v Notional) String() string {
	return newParamValueOrPanic(native.ParamNotionalToString(v.native))
}

func (v Notional) IsZero() bool {
	return newParamValueOrPanic(native.ParamNotionalIsZero(v.native))
}

func (v Notional) Equal(other Notional) bool {
	return newParamValueOrPanic(native.ParamNotionalCompare(v.native, other.native)) == 0
}

func (v Notional) Compare(other Notional) int {
	return newParamValueOrPanic(native.ParamNotionalCompare(v.native, other.native))
}

func (v Notional) CheckedAdd(other Notional) (Notional, error) {
	result, err := native.ParamNotionalCheckedAdd(v.native, other.native)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedSub(other Notional) (Notional, error) {
	result, err := native.ParamNotionalCheckedSub(v.native, other.native)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedMulInt(scalar int64) (Notional, error) {
	result, err := native.ParamNotionalCheckedMulI64(v.native, scalar)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedMulUint(scalar uint64) (Notional, error) {
	result, err := native.ParamNotionalCheckedMulU64(v.native, scalar)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedMulFloat(scalar float64) (Notional, error) {
	result, err := native.ParamNotionalCheckedMulF64(v.native, scalar)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedDivInt(divisor int64) (Notional, error) {
	result, err := native.ParamNotionalCheckedDivI64(v.native, divisor)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedDivUint(divisor uint64) (Notional, error) {
	result, err := native.ParamNotionalCheckedDivU64(v.native, divisor)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedDivFloat(divisor float64) (Notional, error) {
	result, err := native.ParamNotionalCheckedDivF64(v.native, divisor)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedRemInt(divisor int64) (Notional, error) {
	result, err := native.ParamNotionalCheckedRemI64(v.native, divisor)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedRemUint(divisor uint64) (Notional, error) {
	result, err := native.ParamNotionalCheckedRemU64(v.native, divisor)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

func (v Notional) CheckedRemFloat(divisor float64) (Notional, error) {
	result, err := native.ParamNotionalCheckedRemF64(v.native, divisor)
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}

// ToVolume converts position notional into settlement volume.
//
// The numeric value is preserved; only the semantic context changes from
// "position exposure" to "order size".
func (v Notional) Volume() (Volume, error) {
	result, err := native.ParamNotionalToVolume(v.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

// CalculateMarginRequired returns the margin needed to hold this position at
// the given leverage.
//
// Formula: margin = notional / leverage.
func (v Notional) CalculateMarginRequired(leverage Leverage) (Notional, error) {
	result, err := native.ParamNotionalCalculateMarginRequired(v.native, leverage.Native())
	if err != nil {
		return Notional{}, err
	}
	return NewNotionalFromNative(result), nil
}
