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

// Quantity is an instrument quantity.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Quantity only once via
// NewQuantityFromString / NewQuantityFromDecimal / NewQuantityFromHandle.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Quantity struct {
	native native.ParamQuantity
}

var newQuantityZero = sync.OnceValue(
	func() Quantity {
		return newQuantityOrPanic(NewQuantityFromInt64(0))
	},
)

// NewQuantityZero returns the canonical zero value of Quantity.
func NewQuantityZero() Quantity { return newQuantityZero() }

// newQuantityOrPanic returns a Quantity or panics on impossible zero errors.
// The panic is deliberate fail-fast behavior: zero Quantity must be valid.
func newQuantityOrPanic(value Quantity, err error) Quantity {
	if err != nil {
		panic(err)
	}
	return value
}

// NewQuantityFromDecimal converts a shopspring decimal to a Quantity.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewQuantityFromDecimal(v decimal.Decimal) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantity(native.NewNativeDecimalFromDecimal(v))
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// NewQuantityFromString creates a Quantity from a decimal string.
func NewQuantityFromString(v string) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromString(v)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// NewQuantityFromInt64 creates a Quantity from a signed integer.
func NewQuantityFromInt64(v int64) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromInt64(v)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// NewQuantityFromUint64 creates a Quantity from an unsigned integer.
func NewQuantityFromUint64(v uint64) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromUint64(v)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// NewQuantityFromFloat constructs a Quantity from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewQuantityFromString or NewQuantityFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewQuantityFromFloat(v float64) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromF64(v)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// NewQuantityFromHandle creates a Quantity from a native handle.
func NewQuantityFromHandle(v native.ParamQuantity) Quantity {
	return Quantity{native: v}
}

// NewQuantityOptionFromHandle creates an optional Quantity from a native optional handle.
func NewQuantityOptionFromHandle(v native.ParamQuantityOptional) optional.Option[Quantity] {
	if !native.ParamQuantityOptionalIsSet(v) {
		return optional.None[Quantity]()
	}
	return optional.Some(NewQuantityFromHandle(native.ParamQuantityOptionalGet(v)))
}

// NewQuantityFromStringRounded creates a Quantity from a string, rounded to the given scale.
func NewQuantityFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromStringRounded(v, scale, strategy.native())
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// NewQuantityFromFloatRounded creates a Quantity from a float64, rounded to the given scale.
func NewQuantityFromFloatRounded(
	v float64,
	scale uint32,
	strategy RoundingStrategy,
) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// NewQuantityFromDecimalRounded converts a shopspring decimal to a rounded Quantity.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewQuantityFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(nativeValue), nil
}

// Decimal returns the value as a shopspring decimal.
func (v Quantity) Decimal() decimal.Decimal {
	return newDecimalFromHandle(native.ParamQuantityGetDecimal(v.native))
}

// Handle returns the underlying native handle.
func (v Quantity) Handle() native.ParamQuantity {
	return v.native
}

// Float returns the value as a float64.
//
// NewQuantityFromFloat constructs a Quantity from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewQuantityFromString or NewQuantityFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Quantity) Float() float64 {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamQuantityToF64(v.native))
}

// String returns the decimal string representation of the quantity.
func (v Quantity) String() string {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamQuantityToString(v.native))
}

// IsZero reports whether the quantity is zero.
func (v Quantity) IsZero() bool {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamQuantityIsZero(v.native))
}

// Equal reports whether v and other are equal.
func (v Quantity) Equal(other Quantity) bool {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamQuantityCompare(v.native, other.native)) == 0
}

// Compare returns -1, 0, or 1 comparing v to other.
func (v Quantity) Compare(other Quantity) int {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamQuantityCompare(v.native, other.native))
}

// CheckedAdd returns v + other or an error on overflow.
func (v Quantity) CheckedAdd(other Quantity) (Quantity, error) {
	result, err := native.ParamQuantityCheckedAdd(v.native, other.native)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedSub returns v - other or an error on overflow.
func (v Quantity) CheckedSub(other Quantity) (Quantity, error) {
	result, err := native.ParamQuantityCheckedSub(v.native, other.native)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedMulInt returns v * scalar or an error on overflow.
func (v Quantity) CheckedMulInt(scalar int64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedMulI64(v.native, scalar)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedMulUint returns v * scalar or an error on overflow.
func (v Quantity) CheckedMulUint(scalar uint64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedMulU64(v.native, scalar)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedMulFloat returns v * scalar or an error on overflow.
func (v Quantity) CheckedMulFloat(scalar float64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedMulF64(v.native, scalar)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedDivInt returns v / divisor or an error on division by zero or overflow.
func (v Quantity) CheckedDivInt(divisor int64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedDivI64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedDivUint returns v / divisor or an error on division by zero.
func (v Quantity) CheckedDivUint(divisor uint64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedDivU64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedDivFloat returns v / divisor or an error on division by zero or overflow.
func (v Quantity) CheckedDivFloat(divisor float64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedDivF64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedRemInt returns v % divisor or an error on division by zero.
func (v Quantity) CheckedRemInt(divisor int64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedRemI64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedRemUint returns v % divisor or an error on division by zero.
func (v Quantity) CheckedRemUint(divisor uint64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedRemU64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CheckedRemFloat returns v % divisor or an error on division by zero.
func (v Quantity) CheckedRemFloat(divisor float64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedRemF64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// CalculateVolume returns the volume equivalent to quantity × price.
func (v Quantity) CalculateVolume(price Price) (Volume, error) {
	result, err := native.ParamQuantityCalculateVolume(v.native, price.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// ToPositionSize returns the quantity as a PositionSize.
func (v Quantity) ToPositionSize() PositionSize {
	// invariant: native value already validated on construction; conversion cannot fail.
	return NewPositionSizeFromHandle(newParamValueOrPanic(native.ParamQuantityToPositionSize(v.native)))
}
