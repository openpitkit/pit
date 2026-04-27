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
// NewPositionSizeFromString / NewPositionSizeFromDecimal / NewPositionSizeFromNative.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type PositionSize struct {
	native native.ParamPositionSize
}

var PositionSizeZero = newPositionSizeOrPanic(NewPositionSizeFromInt(0))

func newPositionSizeOrPanic(value PositionSize, err error) PositionSize {
	if err != nil {
		panic(err)
	}
	return value
}

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
	return NewPositionSizeFromNative(nativeValue), nil
}

func NewPositionSizeFromString(v string) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromStr(v)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(nativeValue), nil
}

func NewPositionSizeFromInt(v int64) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromI64(v)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(nativeValue), nil
}

func NewPositionSizeFromUint(v uint64) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromU64(v)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(nativeValue), nil
}

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
	return NewPositionSizeFromNative(nativeValue), nil
}

func NewPositionSizeFromNative(v native.ParamPositionSize) PositionSize {
	return PositionSize{native: v}
}

func NewPositionSizeOptionFromNative(
	v native.ParamPositionSizeOptional,
) optional.Option[PositionSize] {
	if !native.ParamPositionSizeOptionalIsSet(v) {
		return optional.None[PositionSize]()
	}
	return optional.Some(NewPositionSizeFromNative(native.ParamPositionSizeOptionalGet(v)))
}

func NewPositionSizeFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromStrRounded(v, scale, strategy.native())
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(nativeValue), nil
}

func NewPositionSizeFromFloatRounded(
	v float64,
	scale uint32,
	strategy RoundingStrategy,
) (PositionSize, error) {
	nativeValue, err := native.CreateParamPositionSizeFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(nativeValue), nil
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
	return NewPositionSizeFromNative(nativeValue), nil
}

func NewPositionSizeFromPnl(pnl Pnl) PositionSize {
	return NewPositionSizeFromNative(
		newParamValueOrPanic(native.ParamPositionSizeFromPnl(pnl.native)),
	)
}

func NewPositionSizeFromFee(fee Fee) PositionSize {
	return NewPositionSizeFromNative(
		newParamValueOrPanic(native.ParamPositionSizeFromFee(fee.native)),
	)
}

func NewPositionSizeFromQuantityAndSide(q Quantity, side Side) PositionSize {
	return NewPositionSizeFromNative(
		newParamValueOrPanic(
			native.ParamPositionSizeFromQuantityAndSide(q.native, side.Native()),
		),
	)
}

func (v PositionSize) Decimal() decimal.Decimal {
	return newDecimalFromNative(native.ParamPositionSizeGetDecimal(v.native))
}

func (v PositionSize) Native() native.ParamPositionSize {
	return v.native
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewPositionSizeFromString or NewPositionSizeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v PositionSize) Float() float64 {
	return newParamValueOrPanic(native.ParamPositionSizeToF64(v.native))
}

func (v PositionSize) String() string {
	return newParamValueOrPanic(native.ParamPositionSizeToString(v.native))
}

func (v PositionSize) IsZero() bool {
	return newParamValueOrPanic(native.ParamPositionSizeIsZero(v.native))
}

func (v PositionSize) Equal(other PositionSize) bool {
	return newParamValueOrPanic(native.ParamPositionSizeCompare(v.native, other.native)) == 0
}

func (v PositionSize) Compare(other PositionSize) int {
	return newParamValueOrPanic(native.ParamPositionSizeCompare(v.native, other.native))
}

func (v PositionSize) CheckedAdd(other PositionSize) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedAdd(v.native, other.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedSub(other PositionSize) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedSub(v.native, other.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedNeg() (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedNeg(v.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedMulInt(scalar int64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedMulI64(v.native, scalar)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedMulUint(scalar uint64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedMulU64(v.native, scalar)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedMulFloat(scalar float64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedMulF64(v.native, scalar)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedDivInt(divisor int64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedDivI64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedDivUint(divisor uint64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedDivU64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedDivFloat(divisor float64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedDivF64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedRemInt(divisor int64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedRemI64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedRemUint(divisor uint64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedRemU64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) CheckedRemFloat(divisor float64) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedRemF64(v.native, divisor)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}

func (v PositionSize) OpenQuantity() (Quantity, Side) {
	quantity, side := newPositionSizeQuantitySideOrPanic(
		native.ParamPositionSizeToOpenQuantity(v.native),
	)
	return NewQuantityFromNative(quantity), NewSideFromNative(side).MustGet()
}

func (v PositionSize) CloseQuantity() (Quantity, optional.Option[Side]) {
	quantity, side := newPositionSizeQuantitySideOrPanic(
		native.ParamPositionSizeToCloseQuantity(v.native),
	)
	result := NewQuantityFromNative(quantity)
	if side == native.ParamSideNotSet {
		return result, optional.None[Side]()
	}
	return result, NewSideFromNative(side)
}

func (v PositionSize) CheckedAddQuantity(q Quantity, side Side) (PositionSize, error) {
	result, err := native.ParamPositionSizeCheckedAddQuantity(v.native, q.native, side.Native())
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromNative(result), nil
}
