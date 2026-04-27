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

// Quantity is an instrument quantity.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Quantity only once via
// NewQuantityFromString / NewQuantityFromDecimal / NewQuantityFromNative.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Quantity struct {
	native native.ParamQuantity
}

var QuantityZero = newQuantityOrPanic(NewQuantityFromInt(0))

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
	return NewQuantityFromNative(nativeValue), nil
}

func NewQuantityFromString(v string) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromStr(v)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(nativeValue), nil
}

func NewQuantityFromInt(v int64) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromI64(v)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(nativeValue), nil
}

func NewQuantityFromUint(v uint64) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromU64(v)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(nativeValue), nil
}

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
	return NewQuantityFromNative(nativeValue), nil
}

func NewQuantityFromNative(v native.ParamQuantity) Quantity {
	return Quantity{native: v}
}

func NewQuantityOptionFromNative(v native.ParamQuantityOptional) optional.Option[Quantity] {
	if !native.ParamQuantityOptionalIsSet(v) {
		return optional.None[Quantity]()
	}
	return optional.Some(NewQuantityFromNative(native.ParamQuantityOptionalGet(v)))
}

func NewQuantityFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromStrRounded(v, scale, strategy.native())
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(nativeValue), nil
}

func NewQuantityFromFloatRounded(
	v float64,
	scale uint32,
	strategy RoundingStrategy,
) (Quantity, error) {
	nativeValue, err := native.CreateParamQuantityFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(nativeValue), nil
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
	return NewQuantityFromNative(nativeValue), nil
}

func (v Quantity) Decimal() decimal.Decimal {
	return newDecimalFromNative(native.ParamQuantityGetDecimal(v.native))
}

func (v Quantity) Native() native.ParamQuantity {
	return v.native
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewQuantityFromString or NewQuantityFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Quantity) Float() float64 {
	return newParamValueOrPanic(native.ParamQuantityToF64(v.native))
}

func (v Quantity) String() string {
	return newParamValueOrPanic(native.ParamQuantityToString(v.native))
}

func (v Quantity) IsZero() bool {
	return newParamValueOrPanic(native.ParamQuantityIsZero(v.native))
}

func (v Quantity) Equal(other Quantity) bool {
	return newParamValueOrPanic(native.ParamQuantityCompare(v.native, other.native)) == 0
}

func (v Quantity) Compare(other Quantity) int {
	return newParamValueOrPanic(native.ParamQuantityCompare(v.native, other.native))
}

func (v Quantity) CheckedAdd(other Quantity) (Quantity, error) {
	result, err := native.ParamQuantityCheckedAdd(v.native, other.native)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedSub(other Quantity) (Quantity, error) {
	result, err := native.ParamQuantityCheckedSub(v.native, other.native)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedMulInt(scalar int64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedMulI64(v.native, scalar)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedMulUint(scalar uint64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedMulU64(v.native, scalar)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedMulFloat(scalar float64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedMulF64(v.native, scalar)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedDivInt(divisor int64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedDivI64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedDivUint(divisor uint64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedDivU64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedDivFloat(divisor float64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedDivF64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedRemInt(divisor int64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedRemI64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedRemUint(divisor uint64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedRemU64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CheckedRemFloat(divisor float64) (Quantity, error) {
	result, err := native.ParamQuantityCheckedRemF64(v.native, divisor)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}

func (v Quantity) CalculateVolume(price Price) (Volume, error) {
	result, err := native.ParamQuantityCalculateVolume(v.native, price.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}
