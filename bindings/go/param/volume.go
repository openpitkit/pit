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

// Volume is a settlement notional volume.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Volume only once via
// NewVolumeFromString / NewVolumeFromDecimal / NewVolumeFromNative.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Volume struct {
	native native.ParamVolume
}

var VolumeZero = newVolumeOrPanic(NewVolumeFromInt(0))

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
	return NewVolumeFromNative(nativeValue), nil
}

func NewVolumeFromString(v string) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromStr(v)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(nativeValue), nil
}

func NewVolumeFromInt(v int64) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromI64(v)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(nativeValue), nil
}

func NewVolumeFromUint(v uint64) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromU64(v)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(nativeValue), nil
}

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
	return NewVolumeFromNative(nativeValue), nil
}

func NewVolumeFromNative(v native.ParamVolume) Volume {
	return Volume{native: v}
}

func NewVolumeOptionFromNative(v native.ParamVolumeOptional) optional.Option[Volume] {
	if !native.ParamVolumeOptionalIsSet(v) {
		return optional.None[Volume]()
	}
	return optional.Some(NewVolumeFromNative(native.ParamVolumeOptionalGet(v)))
}

func NewVolumeFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromStrRounded(v, scale, strategy.native())
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(nativeValue), nil
}

func NewVolumeFromFloatRounded(v float64, scale uint32, strategy RoundingStrategy) (Volume, error) {
	nativeValue, err := native.CreateParamVolumeFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(nativeValue), nil
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
	return NewVolumeFromNative(nativeValue), nil
}

func (v Volume) Decimal() decimal.Decimal {
	return newDecimalFromNative(native.ParamVolumeGetDecimal(v.native))
}

func (v Volume) Native() native.ParamVolume {
	return v.native
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewVolumeFromString or NewVolumeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Volume) Float() float64 {
	return newParamValueOrPanic(native.ParamVolumeToF64(v.native))
}

func (v Volume) String() string {
	return newParamValueOrPanic(native.ParamVolumeToString(v.native))
}

func (v Volume) IsZero() bool {
	return newParamValueOrPanic(native.ParamVolumeIsZero(v.native))
}

func (v Volume) Equal(other Volume) bool {
	return newParamValueOrPanic(native.ParamVolumeCompare(v.native, other.native)) == 0
}

func (v Volume) Compare(other Volume) int {
	return newParamValueOrPanic(native.ParamVolumeCompare(v.native, other.native))
}

func (v Volume) CheckedAdd(other Volume) (Volume, error) {
	result, err := native.ParamVolumeCheckedAdd(v.native, other.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedSub(other Volume) (Volume, error) {
	result, err := native.ParamVolumeCheckedSub(v.native, other.native)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedMulInt(scalar int64) (Volume, error) {
	result, err := native.ParamVolumeCheckedMulI64(v.native, scalar)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedMulUint(scalar uint64) (Volume, error) {
	result, err := native.ParamVolumeCheckedMulU64(v.native, scalar)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedMulFloat(scalar float64) (Volume, error) {
	result, err := native.ParamVolumeCheckedMulF64(v.native, scalar)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedDivInt(divisor int64) (Volume, error) {
	result, err := native.ParamVolumeCheckedDivI64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedDivUint(divisor uint64) (Volume, error) {
	result, err := native.ParamVolumeCheckedDivU64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedDivFloat(divisor float64) (Volume, error) {
	result, err := native.ParamVolumeCheckedDivF64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedRemInt(divisor int64) (Volume, error) {
	result, err := native.ParamVolumeCheckedRemI64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedRemUint(divisor uint64) (Volume, error) {
	result, err := native.ParamVolumeCheckedRemU64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CheckedRemFloat(divisor float64) (Volume, error) {
	result, err := native.ParamVolumeCheckedRemF64(v.native, divisor)
	if err != nil {
		return Volume{}, err
	}
	return NewVolumeFromNative(result), nil
}

func (v Volume) CashFlowInflow() CashFlow {
	return NewCashFlowFromNative(newParamValueOrPanic(native.ParamVolumeToCashFlowInflow(v.native)))
}

func (v Volume) CashFlowOutflow() CashFlow {
	return NewCashFlowFromNative(newParamValueOrPanic(native.ParamVolumeToCashFlowOutflow(v.native)))
}

func (v Volume) CalculateQuantity(price Price) (Quantity, error) {
	result, err := native.ParamVolumeCalculateQuantity(v.native, price.native)
	if err != nil {
		return Quantity{}, err
	}
	return NewQuantityFromNative(result), nil
}
