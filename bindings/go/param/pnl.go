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

// Pnl is a profit and loss value.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Pnl only once via
// NewPnlFromString / NewPnlFromDecimal / NewPnlFromHandle.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Pnl struct {
	native native.ParamPnl
}

var PnlZero = newPnlOrPanic(NewPnlFromInt(0))

func newPnlOrPanic(value Pnl, err error) Pnl {
	if err != nil {
		panic(err)
	}
	return value
}

// NewPnlFromDecimal converts a shopspring decimal to a Pnl.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewPnlFromDecimal(v decimal.Decimal) (Pnl, error) {
	nativeValue, err := native.CreateParamPnl(native.NewNativeDecimalFromDecimal(v))
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

func NewPnlFromString(v string) (Pnl, error) {
	nativeValue, err := native.CreateParamPnlFromStr(v)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

func NewPnlFromInt(v int64) (Pnl, error) {
	nativeValue, err := native.CreateParamPnlFromI64(v)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

func NewPnlFromUint(v uint64) (Pnl, error) {
	nativeValue, err := native.CreateParamPnlFromU64(v)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewPnlFromString or NewPnlFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewPnlFromFloat(v float64) (Pnl, error) {
	nativeValue, err := native.CreateParamPnlFromF64(v)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

func NewPnlFromHandle(v native.ParamPnl) Pnl {
	return Pnl{native: v}
}

func NewPnlOptionFromHandle(v native.ParamPnlOptional) optional.Option[Pnl] {
	if !native.ParamPnlOptionalIsSet(v) {
		return optional.None[Pnl]()
	}
	return optional.Some(NewPnlFromHandle(native.ParamPnlOptionalGet(v)))
}

func NewPnlFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Pnl, error) {
	nativeValue, err := native.CreateParamPnlFromStrRounded(v, scale, strategy.native())
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

func NewPnlFromFloatRounded(v float64, scale uint32, strategy RoundingStrategy) (Pnl, error) {
	nativeValue, err := native.CreateParamPnlFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

// NewPnlFromDecimalRounded converts a shopspring decimal to a rounded Pnl.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewPnlFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (Pnl, error) {
	nativeValue, err := native.CreateParamPnlFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

func NewPnlFromFee(fee Fee) (Pnl, error) {
	nativeValue, err := native.ParamPnlFromFee(fee.native)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

func (v Pnl) Decimal() decimal.Decimal {
	return newDecimalFromHandle(native.ParamPnlGetDecimal(v.native))
}

func (v Pnl) Handle() native.ParamPnl {
	return v.native
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewPnlFromString or NewPnlFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Pnl) Float() float64 {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPnlToF64(v.native))
}

func (v Pnl) String() string {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPnlToString(v.native))
}

func (v Pnl) IsZero() bool {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamPnlIsZero(v.native))
}

func (v Pnl) Equal(other Pnl) bool {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamPnlCompare(v.native, other.native)) == 0
}

func (v Pnl) Compare(other Pnl) int {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamPnlCompare(v.native, other.native))
}

func (v Pnl) CheckedAdd(other Pnl) (Pnl, error) {
	result, err := native.ParamPnlCheckedAdd(v.native, other.native)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedSub(other Pnl) (Pnl, error) {
	result, err := native.ParamPnlCheckedSub(v.native, other.native)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedNeg() (Pnl, error) {
	result, err := native.ParamPnlCheckedNeg(v.native)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedMulInt(scalar int64) (Pnl, error) {
	result, err := native.ParamPnlCheckedMulI64(v.native, scalar)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedMulUint(scalar uint64) (Pnl, error) {
	result, err := native.ParamPnlCheckedMulU64(v.native, scalar)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedMulFloat(scalar float64) (Pnl, error) {
	result, err := native.ParamPnlCheckedMulF64(v.native, scalar)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedDivInt(divisor int64) (Pnl, error) {
	result, err := native.ParamPnlCheckedDivI64(v.native, divisor)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedDivUint(divisor uint64) (Pnl, error) {
	result, err := native.ParamPnlCheckedDivU64(v.native, divisor)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedDivFloat(divisor float64) (Pnl, error) {
	result, err := native.ParamPnlCheckedDivF64(v.native, divisor)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedRemInt(divisor int64) (Pnl, error) {
	result, err := native.ParamPnlCheckedRemI64(v.native, divisor)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedRemUint(divisor uint64) (Pnl, error) {
	result, err := native.ParamPnlCheckedRemU64(v.native, divisor)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CheckedRemFloat(divisor float64) (Pnl, error) {
	result, err := native.ParamPnlCheckedRemF64(v.native, divisor)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(result), nil
}

func (v Pnl) CashFlow() (CashFlow, error) {
	nativeValue, err := native.ParamPnlToCashFlow(v.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func (v Pnl) PositionSize() (PositionSize, error) {
	nativeValue, err := native.ParamPnlToPositionSize(v.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}
