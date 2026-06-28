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

// Fee is a fee amount.
//
// Values are validated and stored in the native value layout. Every arithmetic
// or conversion call on this type crosses the Go/C boundary via FFI and has
// a per-operation cost. For ultra-low-latency paths that need many
// intermediate computations, prefer performing the math on primitive types
// or a custom representation and cross into Fee only once via
// NewFeeFromString / NewFeeFromDecimal / NewFeeFromHandle.
//
// This cost exists because the SDK guarantees that the same input produces
// bit-for-bit identical results across all language bindings (Rust, Go,
// Python). Running arithmetic through the core is the mechanism that
// enforces that determinism.
type Fee struct {
	native native.ParamFee
}

var newFeeZero = sync.OnceValue(func() Fee { return newFeeOrPanic(NewFeeFromInt64(0)) })

// NewFeeZero returns the canonical zero value of Fee.
func NewFeeZero() Fee { return newFeeZero() }

// newFeeOrPanic returns a Fee or panics on impossible zero construction errors.
// The panic is deliberate fail-fast behavior: zero Fee must always be valid.
func newFeeOrPanic(value Fee, err error) Fee {
	if err != nil {
		panic(err)
	}
	return value
}

// NewFeeFromDecimal converts a shopspring decimal to a Fee.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewFeeFromDecimal(v decimal.Decimal) (Fee, error) {
	nativeValue, err := native.CreateParamFee(native.NewNativeDecimalFromDecimal(v))
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// NewFeeFromString creates a Fee from a decimal string.
func NewFeeFromString(v string) (Fee, error) {
	nativeValue, err := native.CreateParamFeeFromString(v)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// NewFeeFromInt64 creates a Fee from a signed integer.
func NewFeeFromInt64(v int64) (Fee, error) {
	nativeValue, err := native.CreateParamFeeFromInt64(v)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// NewFeeFromUint64 creates a Fee from an unsigned integer.
func NewFeeFromUint64(v uint64) (Fee, error) {
	nativeValue, err := native.CreateParamFeeFromUint64(v)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// NewFeeFromFloat constructs a Fee from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewFeeFromString or NewFeeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func NewFeeFromFloat(v float64) (Fee, error) {
	nativeValue, err := native.CreateParamFeeFromF64(v)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// NewFeeFromHandle creates a Fee from a native handle.
func NewFeeFromHandle(v native.ParamFee) Fee {
	return Fee{native: v}
}

// NewFeeOptionFromHandle creates an optional Fee from a native optional handle.
func NewFeeOptionFromHandle(v native.ParamFeeOptional) optional.Option[Fee] {
	if !native.ParamFeeOptionalIsSet(v) {
		return optional.None[Fee]()
	}
	return optional.Some(NewFeeFromHandle(native.ParamFeeOptionalGet(v)))
}

// NewFeeFromStringRounded creates a Fee from a string, rounded to the given scale.
func NewFeeFromStringRounded(
	v string,
	scale uint32,
	strategy RoundingStrategy,
) (Fee, error) {
	nativeValue, err := native.CreateParamFeeFromStringRounded(v, scale, strategy.native())
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// NewFeeFromFloatRounded creates a Fee from a float64, rounded to the given scale.
func NewFeeFromFloatRounded(v float64, scale uint32, strategy RoundingStrategy) (Fee, error) {
	nativeValue, err := native.CreateParamFeeFromF64Rounded(v, scale, strategy.native())
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// NewFeeFromDecimalRounded converts a shopspring decimal to a rounded Fee.
//
// WARNING:
// This constructor delegates to native decimal conversion that truncates the
// coefficient to 64 bits. If the decimal mantissa exceeds int64 range, higher
// bits are silently discarded without any error or panic.
func NewFeeFromDecimalRounded(
	v decimal.Decimal,
	scale uint32,
	strategy RoundingStrategy,
) (Fee, error) {
	nativeValue, err := native.CreateParamFeeFromDecimalRounded(
		native.NewNativeDecimalFromDecimal(v),
		scale,
		strategy.native(),
	)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(nativeValue), nil
}

// Decimal returns the value as a shopspring decimal.
func (v Fee) Decimal() decimal.Decimal {
	return newDecimalFromHandle(native.ParamFeeGetDecimal(v.native))
}

// Handle returns the underlying native handle.
func (v Fee) Handle() native.ParamFee {
	return v.native
}

// Float returns the value as a float64.
//
// NewFeeFromFloat constructs a Fee from a float64 value.
//
// WARNING: float64 values are inherently imprecise. The same numeric literal
// interpreted as float64 can differ by one ULP from its string representation
// and may produce different values on different platforms or compilers.
// DO NOT use for monetary data received from external systems — prefer
// NewFeeFromString or NewFeeFromDecimal. This constructor is provided
// for parity and test convenience only; cross-platform determinism is NOT
// guaranteed when construction goes through float64.
func (v Fee) Float() float64 {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamFeeToF64(v.native))
}

// String returns the decimal string representation of the fee.
func (v Fee) String() string {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamFeeToString(v.native))
}

// IsZero reports whether the fee is zero.
func (v Fee) IsZero() bool {
	// invariant: native value already validated on construction; conversion cannot fail.
	return newParamValueOrPanic(native.ParamFeeIsZero(v.native))
}

// Equal reports whether v and other are equal.
func (v Fee) Equal(other Fee) bool {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamFeeCompare(v.native, other.native)) == 0
}

// Compare returns -1, 0, or 1 comparing v to other.
func (v Fee) Compare(other Fee) int {
	// invariant: native values already validated on construction; comparison cannot fail.
	return newParamValueOrPanic(native.ParamFeeCompare(v.native, other.native))
}

// CheckedAdd returns v + other or an error on overflow.
func (v Fee) CheckedAdd(other Fee) (Fee, error) {
	result, err := native.ParamFeeCheckedAdd(v.native, other.native)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedSub returns v - other or an error on overflow.
func (v Fee) CheckedSub(other Fee) (Fee, error) {
	result, err := native.ParamFeeCheckedSub(v.native, other.native)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedNeg returns the negation of v or an error on overflow.
func (v Fee) CheckedNeg() (Fee, error) {
	result, err := native.ParamFeeCheckedNeg(v.native)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedMulInt returns v * scalar or an error on overflow.
func (v Fee) CheckedMulInt(scalar int64) (Fee, error) {
	result, err := native.ParamFeeCheckedMulI64(v.native, scalar)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedMulUint returns v * scalar or an error on overflow.
func (v Fee) CheckedMulUint(scalar uint64) (Fee, error) {
	result, err := native.ParamFeeCheckedMulU64(v.native, scalar)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedMulFloat returns v * scalar or an error on overflow.
func (v Fee) CheckedMulFloat(scalar float64) (Fee, error) {
	result, err := native.ParamFeeCheckedMulF64(v.native, scalar)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedDivInt returns v / divisor or an error on division by zero or overflow.
func (v Fee) CheckedDivInt(divisor int64) (Fee, error) {
	result, err := native.ParamFeeCheckedDivI64(v.native, divisor)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedDivUint returns v / divisor or an error on division by zero.
func (v Fee) CheckedDivUint(divisor uint64) (Fee, error) {
	result, err := native.ParamFeeCheckedDivU64(v.native, divisor)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedDivFloat returns v / divisor or an error on division by zero or overflow.
func (v Fee) CheckedDivFloat(divisor float64) (Fee, error) {
	result, err := native.ParamFeeCheckedDivF64(v.native, divisor)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedRemInt returns v % divisor or an error on division by zero.
func (v Fee) CheckedRemInt(divisor int64) (Fee, error) {
	result, err := native.ParamFeeCheckedRemI64(v.native, divisor)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedRemUint returns v % divisor or an error on division by zero.
func (v Fee) CheckedRemUint(divisor uint64) (Fee, error) {
	result, err := native.ParamFeeCheckedRemU64(v.native, divisor)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// CheckedRemFloat returns v % divisor or an error on division by zero.
func (v Fee) CheckedRemFloat(divisor float64) (Fee, error) {
	result, err := native.ParamFeeCheckedRemF64(v.native, divisor)
	if err != nil {
		return Fee{}, err
	}
	return NewFeeFromHandle(result), nil
}

// Pnl converts the fee to a Pnl value.
func (v Fee) Pnl() (Pnl, error) {
	nativeValue, err := native.ParamFeeToPnl(v.native)
	if err != nil {
		return Pnl{}, err
	}
	return NewPnlFromHandle(nativeValue), nil
}

// PositionSize converts the fee to a PositionSize value.
func (v Fee) PositionSize() (PositionSize, error) {
	nativeValue, err := native.ParamFeeToPositionSize(v.native)
	if err != nil {
		return PositionSize{}, err
	}
	return NewPositionSizeFromHandle(nativeValue), nil
}

// CashFlow converts the fee to a CashFlow value.
func (v Fee) CashFlow() (CashFlow, error) {
	nativeValue, err := native.ParamFeeToCashFlow(v.native)
	if err != nil {
		return CashFlow{}, err
	}
	return NewCashFlowFromHandle(nativeValue), nil
}
