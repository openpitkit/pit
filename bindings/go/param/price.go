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
	"sync"

	"github.com/shopspring/decimal"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pkg/optional"
)

// Price is a per-unit instrument price.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Price only once via
// NewPriceFromString / NewPriceFromDecimal / NewPriceFromHandle.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Price struct {
	native native.ParamPrice
}

var newPriceZero = sync.OnceValue(func() Price { return newPriceOrPanic(NewPriceFromInt64(0)) })

// NewPriceZero returns the canonical zero value of Price.
func NewPriceZero() Price { return newPriceZero() }

func newPriceOrPanic(value Price, err error) Price {
	if err != nil {
		panic(err)
	}
	return value
}

// NewPriceFromDecimal converts a shopspring decimal to a Price.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewPriceFromDecimal(v decimal.Decimal) (Price, error) {
	nativeValue, err := native.CreateParamPrice(native.NewNativeDecimalFromDecimal(v))
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// NewPriceFromString creates a Price from a decimal string.
func NewPriceFromString(v string) (Price, error) {
	nativeValue, err := native.CreateParamPriceFromString(v)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// NewPriceFromInt64 creates a Price from a signed integer.
func NewPriceFromInt64(v int64) (Price, error) {
	nativeValue, err := native.CreateParamPriceFromInt64(v)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// NewPriceFromUint64 creates a Price from an unsigned integer.
func NewPriceFromUint64(v uint64) (Price, error) {
	nativeValue, err := native.CreateParamPriceFromUint64(v)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// NewPriceFromFloat constructs a Price from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewPriceFromString or NewPriceFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewPriceFromFloat(v float64) (Price, error) {
	nativeValue, err := native.CreateParamPriceFromF64(v)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// NewPriceFromHandle creates a Price from a native handle.
func NewPriceFromHandle(v native.ParamPrice) Price {
	return Price{native: v}
}

// NewPriceOptionFromHandle creates an optional Price from a native optional handle.
func NewPriceOptionFromHandle(v native.ParamPriceOptional) optional.Option[Price] {
	if !native.ParamPriceOptionalIsSet(v) {
		return optional.None[Price]()
	}
	return optional.Some(NewPriceFromHandle(native.ParamPriceOptionalGet(v)))
}

// NewPriceFromStringRounded creates a Price from a string, rounded to the given scale.
func NewPriceFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Price, error) {
	nativeValue, err := native.CreateParamPriceFromStringRounded(v, scale, strategy.native())
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// NewPriceFromFloatRounded creates a Price from a float64, rounded to the given scale.
func NewPriceFromFloatRounded(v float64, scale uint32, strategy RoundingStrategy) (Price, error) {
	nativeValue, err := native.CreateParamPriceFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// NewPriceFromDecimalRounded converts a shopspring decimal to a rounded Price.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewPriceFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (Price, error) {
	nativeValue, err := native.CreateParamPriceFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(nativeValue), nil
}

// Decimal returns the value as a shopspring decimal.
func (v Price) Decimal() decimal.Decimal {
	return newDecimalFromHandle(native.ParamPriceGetDecimal(v.native))
}

// Handle returns the underlying native handle.
func (v Price) Handle() native.ParamPrice {
	return v.native
}

// Float returns the value as a float64.
//
// NewPriceFromFloat constructs a Price from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewPriceFromString or NewPriceFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Price) Float() float64 {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPriceToF64(v.native))
}

// String returns the decimal string representation of the price.
func (v Price) String() string {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPriceToString(v.native))
}

// IsZero reports whether the price is zero.
func (v Price) IsZero() bool {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPriceIsZero(v.native))
}

// Equal reports whether v and other are equal.
func (v Price) Equal(other Price) bool {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamPriceCompare(v.native, other.native)) == 0
}

// Compare returns -1, 0, or 1 comparing v to other.
func (v Price) Compare(other Price) int {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamPriceCompare(v.native, other.native))
}

// CheckedAdd returns v + other or an error on overflow.
func (v Price) CheckedAdd(other Price) (Price, error) {
	result, err := native.ParamPriceCheckedAdd(v.native, other.native)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedSub returns v - other or an error on overflow.
func (v Price) CheckedSub(other Price) (Price, error) {
	result, err := native.ParamPriceCheckedSub(v.native, other.native)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedNeg returns the negation of v or an error on overflow.
func (v Price) CheckedNeg() (Price, error) {
	result, err := native.ParamPriceCheckedNeg(v.native)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedMulInt returns v * scalar or an error on overflow.
func (v Price) CheckedMulInt(scalar int64) (Price, error) {
	result, err := native.ParamPriceCheckedMulI64(v.native, scalar)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedMulUint returns v * scalar or an error on overflow.
func (v Price) CheckedMulUint(scalar uint64) (Price, error) {
	result, err := native.ParamPriceCheckedMulU64(v.native, scalar)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedMulFloat returns v * scalar or an error on overflow.
func (v Price) CheckedMulFloat(scalar float64) (Price, error) {
	result, err := native.ParamPriceCheckedMulF64(v.native, scalar)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedDivInt returns v / divisor or an error on division by zero or overflow.
func (v Price) CheckedDivInt(divisor int64) (Price, error) {
	result, err := native.ParamPriceCheckedDivI64(v.native, divisor)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedDivUint returns v / divisor or an error on division by zero.
func (v Price) CheckedDivUint(divisor uint64) (Price, error) {
	result, err := native.ParamPriceCheckedDivU64(v.native, divisor)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedDivFloat returns v / divisor or an error on division by zero or overflow.
func (v Price) CheckedDivFloat(divisor float64) (Price, error) {
	result, err := native.ParamPriceCheckedDivF64(v.native, divisor)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedRemInt returns v % divisor or an error on division by zero.
func (v Price) CheckedRemInt(divisor int64) (Price, error) {
	result, err := native.ParamPriceCheckedRemI64(v.native, divisor)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedRemUint returns v % divisor or an error on division by zero.
func (v Price) CheckedRemUint(divisor uint64) (Price, error) {
	result, err := native.ParamPriceCheckedRemU64(v.native, divisor)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CheckedRemFloat returns v % divisor or an error on division by zero.
func (v Price) CheckedRemFloat(divisor float64) (Price, error) {
	result, err := native.ParamPriceCheckedRemF64(v.native, divisor)
	if err != nil {
		return Price{}, err
	}
	return NewPriceFromHandle(result), nil
}

// CalculateVolume returns the volume equivalent to price × quantity.
func (v Price) CalculateVolume(quantity Quantity) (Volume, error) {
	result, err := native.ParamPriceCalculateVolume(v.native, quantity.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CalculatePositionSize returns the position size equivalent to price × quantity.
func (v Price) CalculatePositionSize(quantity Quantity) (PositionSize, error) {
	result, err := native.ParamPriceCalculatePositionSize(v.native, quantity.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}
