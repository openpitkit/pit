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

// CashFlow is a cash-flow contribution.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into CashFlow only once via
// NewCashFlowFromString / NewCashFlowFromDecimal / NewCashFlowFromHandle.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type CashFlow struct {
	native native.ParamCashFlow
}

var CashFlowZero = newCashFlowOrPanic(NewCashFlowFromInt(0))

func newCashFlowOrPanic(value CashFlow, err error) CashFlow {
	if err != nil {
		panic(err)
	}
	return value
}

// NewCashFlowFromDecimal converts a shopspring decimal to a CashFlow.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewCashFlowFromDecimal(v decimal.Decimal) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlow(native.NewNativeDecimalFromDecimal(v))
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromString(v string) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlowFromStr(v)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromInt(v int64) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlowFromI64(v)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromUint(v uint64) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlowFromU64(v)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewCashFlowFromString or NewCashFlowFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewCashFlowFromFloat(v float64) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlowFromF64(v)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromHandle(v native.ParamCashFlow) CashFlow {
	return CashFlow{native: v}
}

func NewCashFlowOptionFromHandle(
	v native.ParamCashFlowOptional,
) optional.Option[CashFlow] {
	if !native.ParamCashFlowOptionalIsSet(v) {
		return optional.None[CashFlow]()
	}
	return optional.Some(NewCashFlowFromHandle(native.ParamCashFlowOptionalGet(v)))
}

func NewCashFlowFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlowFromStrRounded(v, scale, strategy.native())
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromFloatRounded(
	v float64,
	scale uint32,
	strategy RoundingStrategy,
) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlowFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

// NewCashFlowFromDecimalRounded converts a shopspring decimal to a rounded CashFlow.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewCashFlowFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (CashFlow, error) {
	nativeValue, err := native.CreateParamCashFlowFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromPnl(pnl Pnl) (CashFlow, error) {
	nativeValue, err := native.ParamCashFlowFromPnl(pnl.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromFee(fee Fee) (CashFlow, error) {
	nativeValue, err := native.ParamCashFlowFromFee(fee.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromVolumeInflow(volume Volume) (CashFlow, error) {
	nativeValue, err := native.ParamCashFlowFromVolumeInflow(volume.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func NewCashFlowFromVolumeOutflow(volume Volume) (CashFlow, error) {
	nativeValue, err := native.ParamCashFlowFromVolumeOutflow(volume.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}

func (v CashFlow) Decimal() decimal.Decimal {
	return newDecimalFromHandle(native.ParamCashFlowGetDecimal(v.native))
}

func (v CashFlow) Handle() native.ParamCashFlow {
	return v.native
}

// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewCashFlowFromString or NewCashFlowFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v CashFlow) Float() float64 {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamCashFlowToF64(v.native))
}

func (v CashFlow) String() string {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamCashFlowToString(v.native))
}

func (v CashFlow) IsZero() bool {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamCashFlowIsZero(v.native))
}

func (v CashFlow) Equal(other CashFlow) bool {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamCashFlowCompare(v.native, other.native)) == 0
}

func (v CashFlow) Compare(other CashFlow) int {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamCashFlowCompare(v.native, other.native))
}

func (v CashFlow) CheckedAdd(other CashFlow) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedAdd(v.native, other.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedSub(other CashFlow) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedSub(v.native, other.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedNeg() (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedNeg(v.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedMulInt(scalar int64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedMulI64(v.native, scalar)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedMulUint(scalar uint64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedMulU64(v.native, scalar)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedMulFloat(scalar float64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedMulF64(v.native, scalar)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedDivInt(divisor int64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedDivI64(v.native, divisor)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedDivUint(divisor uint64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedDivU64(v.native, divisor)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedDivFloat(divisor float64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedDivF64(v.native, divisor)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedRemInt(divisor int64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedRemI64(v.native, divisor)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedRemUint(divisor uint64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedRemU64(v.native, divisor)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}

func (v CashFlow) CheckedRemFloat(divisor float64) (CashFlow, error) {
	result, err := native.ParamCashFlowCheckedRemF64(v.native, divisor)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(result), nil
}
