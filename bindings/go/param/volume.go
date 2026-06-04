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

// Volume is a settlement notional volume.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Volume only once via
// NewVolumeFromString / NewVolumeFromDecimal / NewVolumeFromHandle.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Volume struct {
	native native.ParamVolume
}

var newVolumeZero = sync.OnceValue(func() Volume { return newVolumeOrPanic(NewVolumeFromInt64(0)) })

// NewVolumeZero returns the canonical zero value of Volume.
func NewVolumeZero() Volume { return newVolumeZero() }

func newVolumeOrPanic(value Volume, err error) Volume {
	if err != nil {
		panic(err)
	}
	return value
}

// NewVolumeFromDecimal converts a shopspring decimal to a Volume.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewVolumeFromDecimal(v decimal.Decimal) (Volume, error) {
	nativeValue, err := native.CreateParamVolume(native.NewNativeDecimalFromDecimal(v))
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// NewVolumeFromString creates a Volume from a decimal string.
func NewVolumeFromString(v string) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromString(v)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// NewVolumeFromInt64 creates a Volume from a signed integer.
func NewVolumeFromInt64(v int64) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromInt64(v)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// NewVolumeFromUint64 creates a Volume from an unsigned integer.
func NewVolumeFromUint64(v uint64) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromUint64(v)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// NewVolumeFromFloat constructs a Volume from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewVolumeFromString or NewVolumeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewVolumeFromFloat(v float64) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromF64(v)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// NewVolumeFromHandle creates a Volume from a native handle.
func NewVolumeFromHandle(v native.ParamVolume) Volume {
	return Volume{native: v}
}

// NewVolumeOptionFromHandle creates an optional Volume from a native optional handle.
func NewVolumeOptionFromHandle(v native.ParamVolumeOptional) optional.Option[Volume] {
	if !native.ParamVolumeOptionalIsSet(v) {
		return optional.None[Volume]()
	}
	return optional.Some(NewVolumeFromHandle(native.ParamVolumeOptionalGet(v)))
}

// NewVolumeFromStringRounded creates a Volume from a string, rounded to the given scale.
func NewVolumeFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromStringRounded(v, scale, strategy.native())
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// NewVolumeFromFloatRounded creates a Volume from a float64, rounded to the given scale.
func NewVolumeFromFloatRounded(v float64, scale uint32, strategy RoundingStrategy) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// NewVolumeFromDecimalRounded converts a shopspring decimal to a rounded Volume.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewVolumeFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(nativeValue), nil
}

// Decimal returns the value as a shopspring decimal.
func (v Volume) Decimal() decimal.Decimal {
	return newDecimalFromHandle(native.ParamVolumeGetDecimal(v.native))
}

// Handle returns the underlying native handle.
func (v Volume) Handle() native.ParamVolume {
	return v.native
}

// Float returns the value as a float64.
//
// NewVolumeFromFloat constructs a Volume from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewVolumeFromString or NewVolumeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Volume) Float() float64 {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamVolumeToF64(v.native))
}

// String returns the decimal string representation of the volume.
func (v Volume) String() string {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamVolumeToString(v.native))
}

// IsZero reports whether the volume is zero.
func (v Volume) IsZero() bool {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamVolumeIsZero(v.native))
}

// Equal reports whether v and other are equal.
func (v Volume) Equal(other Volume) bool {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamVolumeCompare(v.native, other.native)) == 0
}

// Compare returns -1, 0, or 1 comparing v to other.
func (v Volume) Compare(other Volume) int {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamVolumeCompare(v.native, other.native))
}

// CheckedAdd returns v + other or an error on overflow.
func (v Volume) CheckedAdd(other Volume) (Volume, error) {
	result, err := native.ParamVolumeCheckedAdd(v.native, other.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedSub returns v - other or an error on overflow.
func (v Volume) CheckedSub(other Volume) (Volume, error) {
	result, err := native.ParamVolumeCheckedSub(v.native, other.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedMulInt returns v * scalar or an error on overflow.
func (v Volume) CheckedMulInt(scalar int64) (Volume, error) {
	result, err := native.ParamVolumeCheckedMulI64(v.native, scalar)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedMulUint returns v * scalar or an error on overflow.
func (v Volume) CheckedMulUint(scalar uint64) (Volume, error) {
	result, err := native.ParamVolumeCheckedMulU64(v.native, scalar)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedMulFloat returns v * scalar or an error on overflow.
func (v Volume) CheckedMulFloat(scalar float64) (Volume, error) {
	result, err := native.ParamVolumeCheckedMulF64(v.native, scalar)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedDivInt returns v / divisor or an error on division by zero or overflow.
func (v Volume) CheckedDivInt(divisor int64) (Volume, error) {
	result, err := native.ParamVolumeCheckedDivI64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedDivUint returns v / divisor or an error on division by zero.
func (v Volume) CheckedDivUint(divisor uint64) (Volume, error) {
	result, err := native.ParamVolumeCheckedDivU64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedDivFloat returns v / divisor or an error on division by zero or overflow.
func (v Volume) CheckedDivFloat(divisor float64) (Volume, error) {
	result, err := native.ParamVolumeCheckedDivF64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedRemInt returns v % divisor or an error on division by zero.
func (v Volume) CheckedRemInt(divisor int64) (Volume, error) {
	result, err := native.ParamVolumeCheckedRemI64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedRemUint returns v % divisor or an error on division by zero.
func (v Volume) CheckedRemUint(divisor uint64) (Volume, error) {
	result, err := native.ParamVolumeCheckedRemU64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CheckedRemFloat returns v % divisor or an error on division by zero.
func (v Volume) CheckedRemFloat(divisor float64) (Volume, error) {
	result, err := native.ParamVolumeCheckedRemF64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromHandle(result), nil
}

// CashFlowInflow returns the volume as a positive inflow CashFlow.
func (v Volume) CashFlowInflow() CashFlow {
	// invariant: source value already validated by constructor and not caller-modifiable here.
	return NewCashFlowFromHandle(newParamValueOrPanic(native.ParamVolumeToCashFlowInflow(v.native)))
}

// CashFlowOutflow returns the volume as a negative outflow CashFlow.
func (v Volume) CashFlowOutflow() CashFlow {
	// invariant: source value already validated by constructor and not caller-modifiable here.
	return NewCashFlowFromHandle(newParamValueOrPanic(native.ParamVolumeToCashFlowOutflow(v.native)))
}

// CalculateQuantity returns the quantity equivalent to volume / price.
func (v Volume) CalculateQuantity(price Price) (Quantity, error) {
	result, err := native.ParamVolumeCalculateQuantity(v.native, price.native)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromHandle(result), nil
}

// ToPositionSize returns the volume as a PositionSize.
func (v Volume) ToPositionSize() PositionSize {
	// invariant: native value already validated on construction; conversion cannot fail.
	return NewPositionSizeFromHandle(newParamValueOrPanic(native.ParamVolumeToPositionSize(v.native)))
}
