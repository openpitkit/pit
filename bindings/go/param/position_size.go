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

// PositionSize is a position size value.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into PositionSize only once via
// NewPositionSizeFromString / NewPositionSizeFromDecimal / NewPositionSizeFromHandle.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type PositionSize struct {
	native native.ParamPositionSize
}

var newPositionSizeZero = sync.OnceValue(
	func() PositionSize {
		return newPositionSizeOrPanic(NewPositionSizeFromInt64(0))
	},
)

// NewPositionSizeZero returns the canonical zero value of PositionSize.
func NewPositionSizeZero() PositionSize { return newPositionSizeZero() }

// newPositionSizeOrPanic returns a value or panics on impossible zero errors.
// The panic is deliberate fail-fast behavior: zero PositionSize must be valid.
func newPositionSizeOrPanic(value PositionSize, err error) PositionSize {
	if err != nil {
		panic(err)
	}
	return value
}

// newPositionSizeQuantitySideOrPanic returns decoded parts or panics on ABI drift.
// The panic is deliberate fail-fast behavior: continuing would hide wrong data.
func newPositionSizeQuantitySideOrPanic(
	quantity native.ParamQuantity,
	side native.ParamSide,
	err error,
) (native.ParamQuantity, native.ParamSide) {
	if err != nil {
		panic(err)
	}
	return quantity, side
}

// NewPositionSizeFromDecimal converts a shopspring decimal to a PositionSize.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewPositionSizeFromDecimal(v decimal.Decimal) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSize(
		native.NewNativeDecimalFromDecimal(v),
	)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromString creates a PositionSize from a decimal string.
func NewPositionSizeFromString(v string) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromString(v)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromInt64 creates a PositionSize from a signed integer.
func NewPositionSizeFromInt64(v int64) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromInt64(v)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromUint64 creates a PositionSize from an unsigned integer.
func NewPositionSizeFromUint64(v uint64) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromUint64(v)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromFloat constructs a PositionSize from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewPositionSizeFromString or NewPositionSizeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewPositionSizeFromFloat(v float64) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromF64(v)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromHandle creates a PositionSize from a native handle.
func NewPositionSizeFromHandle(v native.ParamPositionSize) PositionSize {
	return PositionSize{native: v}
}

// NewPositionSizeOptionFromHandle creates an optional PositionSize from a native optional handle.
func NewPositionSizeOptionFromHandle(
	v native.ParamPositionSizeOptional,
) optional.Option[PositionSize] {
	if !native.ParamPositionSizeOptionalIsSet(v) {
		return optional.None[PositionSize]()
	}
	return optional.Some(NewPositionSizeFromHandle(native.ParamPositionSizeOptionalGet(v)))
}

// NewPositionSizeFromStringRounded creates a PositionSize from a string, rounded to the given scale.
func NewPositionSizeFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromStringRounded(v, scale, strategy.native())
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromFloatRounded creates a PositionSize from a float64, rounded to the given scale.
func NewPositionSizeFromFloatRounded(
	v float64,
	scale uint32,
	strategy RoundingStrategy,
) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromDecimalRounded converts a shopspring decimal to a rounded PositionSize.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewPositionSizeFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromPnl converts a Pnl to a PositionSize.
func NewPositionSizeFromPnl(pnl Pnl) (PositionSize, error) {
	nativeValue, err := native.ParamPositionSizeFromPnl(pnl.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromFee converts a Fee to a PositionSize.
func NewPositionSizeFromFee(fee Fee) (PositionSize, error) {
	nativeValue, err := native.ParamPositionSizeFromFee(fee.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// NewPositionSizeFromQuantityAndSide creates a PositionSize from a quantity and trade side.
func NewPositionSizeFromQuantityAndSide(q Quantity, side Side) (PositionSize, error) {
	nativeValue, err := native.ParamPositionSizeFromQuantityAndSide(q.native, side.Handle())
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// Decimal returns the value as a shopspring decimal.
func (v PositionSize) Decimal() decimal.Decimal {
	return newDecimalFromHandle(native.ParamPositionSizeGetDecimal(v.native))
}

// Handle returns the underlying native handle.
func (v PositionSize) Handle() native.ParamPositionSize {
	return v.native
}

// Float returns the value as a float64.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewPositionSizeFromString or NewPositionSizeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v PositionSize) Float() float64 {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPositionSizeToF64(v.native))
}

// String returns the decimal string representation of the position size.
func (v PositionSize) String() string {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPositionSizeToString(v.native))
}

// IsZero reports whether the position size is zero.
func (v PositionSize) IsZero() bool {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPositionSizeIsZero(v.native))
}

// Equal reports whether v and other are equal.
func (v PositionSize) Equal(other PositionSize) bool {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamPositionSizeCompare(v.native, other.native)) == 0
}

// Compare returns -1, 0, or 1 comparing v to other.
func (v PositionSize) Compare(other PositionSize) int {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamPositionSizeCompare(v.native, other.native))
}

// CheckedAdd returns v + other or an error on overflow.
func (v PositionSize) CheckedAdd(other PositionSize) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedAdd(v.native, other.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedSub returns v - other or an error on overflow.
func (v PositionSize) CheckedSub(other PositionSize) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedSub(v.native, other.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedNeg returns the negation of v or an error on overflow.
func (v PositionSize) CheckedNeg() (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedNeg(v.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedMulInt returns v * scalar or an error on overflow.
func (v PositionSize) CheckedMulInt(scalar int64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedMulI64(v.native, scalar)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedMulUint returns v * scalar or an error on overflow.
func (v PositionSize) CheckedMulUint(scalar uint64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedMulU64(v.native, scalar)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedMulFloat returns v * scalar or an error on overflow.
func (v PositionSize) CheckedMulFloat(scalar float64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedMulF64(v.native, scalar)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedDivInt returns v / divisor or an error on division by zero or overflow.
func (v PositionSize) CheckedDivInt(divisor int64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedDivI64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedDivUint returns v / divisor or an error on division by zero.
func (v PositionSize) CheckedDivUint(divisor uint64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedDivU64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedDivFloat returns v / divisor or an error on division by zero or overflow.
func (v PositionSize) CheckedDivFloat(divisor float64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedDivF64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedRemInt returns v % divisor or an error on division by zero.
func (v PositionSize) CheckedRemInt(divisor int64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedRemI64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedRemUint returns v % divisor or an error on division by zero.
func (v PositionSize) CheckedRemUint(divisor uint64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedRemU64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// CheckedRemFloat returns v % divisor or an error on division by zero.
func (v PositionSize) CheckedRemFloat(divisor float64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedRemF64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}

// OpenQuantity returns the absolute quantity and the side needed to open this position.
func (v PositionSize) OpenQuantity() (Quantity, Side) {
	// invariant: native value already validated on construction; quantity/side projection cannot fail.
	quantity, side := newPositionSizeQuantitySideOrPanic(
		native.ParamPositionSizeToOpenQuantity(v.native),
	)
	return NewQuantityFromHandle(quantity), NewSideFromHandle(side).MustGet()
}

// CloseQuantity returns the absolute quantity and optional side needed to close this position.
func (v PositionSize) CloseQuantity() (Quantity, optional.Option[Side]) {
	// invariant: native value already validated on construction; quantity/side projection cannot fail.
	quantity, side := newPositionSizeQuantitySideOrPanic(
		native.ParamPositionSizeToCloseQuantity(v.native),
	)
	result := NewQuantityFromHandle(quantity)
	if side == native.ParamSideNotSet {
		return result, optional.None[Side]()
	}
	return result, NewSideFromHandle(side)
}

// CheckedAddQuantity adds the given quantity on the given side to the position size.
func (v PositionSize) CheckedAddQuantity(q Quantity, side Side) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedAddQuantity(v.native, q.native, side.Handle())
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(result), nil
}
