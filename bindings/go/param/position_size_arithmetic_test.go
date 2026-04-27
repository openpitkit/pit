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
	"math"
	"testing"
)

func TestPositionSizeCheckedArithmeticHappyPaths(t *testing.T) {
	t.Parallel()

	value := mustPositionSizeValue(t, "12.5")
	zero := mustPositionSizeValue(t, "0")

	added, err := value.CheckedAdd(zero)
	if err != nil {
		t.Fatalf("CheckedAdd() error = %v", err)
	}
	if !added.Equal(value) {
		t.Fatalf("CheckedAdd() = %v, want %v", added, value)
	}

	subtracted, err := value.CheckedSub(zero)
	if err != nil {
		t.Fatalf("CheckedSub() error = %v", err)
	}
	if !subtracted.Equal(value) {
		t.Fatalf("CheckedSub() = %v, want %v", subtracted, value)
	}

	negated, err := value.CheckedNeg()
	if err != nil {
		t.Fatalf("CheckedNeg() error = %v", err)
	}
	if got := negated.String(); got != "-12.5" {
		t.Fatalf("CheckedNeg() = %q, want %q", got, "-12.5")
	}

	mulInt, err := value.CheckedMulInt(1)
	if err != nil {
		t.Fatalf("CheckedMulInt() error = %v", err)
	}
	if !mulInt.Equal(value) {
		t.Fatalf("CheckedMulInt() = %v, want %v", mulInt, value)
	}

	mulUint, err := value.CheckedMulUint(1)
	if err != nil {
		t.Fatalf("CheckedMulUint() error = %v", err)
	}
	if !mulUint.Equal(value) {
		t.Fatalf("CheckedMulUint() = %v, want %v", mulUint, value)
	}

	mulFloat, err := value.CheckedMulFloat(1.0)
	if err != nil {
		t.Fatalf("CheckedMulFloat() error = %v", err)
	}
	if !mulFloat.Equal(value) {
		t.Fatalf("CheckedMulFloat() = %v, want %v", mulFloat, value)
	}

	divInt, err := value.CheckedDivInt(1)
	if err != nil {
		t.Fatalf("CheckedDivInt() error = %v", err)
	}
	if !divInt.Equal(value) {
		t.Fatalf("CheckedDivInt() = %v, want %v", divInt, value)
	}

	divUint, err := value.CheckedDivUint(1)
	if err != nil {
		t.Fatalf("CheckedDivUint() error = %v", err)
	}
	if !divUint.Equal(value) {
		t.Fatalf("CheckedDivUint() = %v, want %v", divUint, value)
	}

	divFloat, err := value.CheckedDivFloat(1.0)
	if err != nil {
		t.Fatalf("CheckedDivFloat() error = %v", err)
	}
	if !divFloat.Equal(value) {
		t.Fatalf("CheckedDivFloat() = %v, want %v", divFloat, value)
	}

	remainderBase := mustPositionSizeValue(t, "5")
	remInt, err := remainderBase.CheckedRemInt(2)
	if err != nil {
		t.Fatalf("CheckedRemInt() error = %v", err)
	}
	if got := remInt.String(); got != "1" {
		t.Fatalf("CheckedRemInt() = %q, want %q", got, "1")
	}

	remUint, err := remainderBase.CheckedRemUint(2)
	if err != nil {
		t.Fatalf("CheckedRemUint() error = %v", err)
	}
	if got := remUint.String(); got != "1" {
		t.Fatalf("CheckedRemUint() = %q, want %q", got, "1")
	}

	remFloat, err := remainderBase.CheckedRemFloat(2.0)
	if err != nil {
		t.Fatalf("CheckedRemFloat() error = %v", err)
	}
	if got := remFloat.String(); got != "1" {
		t.Fatalf("CheckedRemFloat() = %q, want %q", got, "1")
	}
}

func TestPositionSizeCheckedArithmeticErrors(t *testing.T) {
	t.Parallel()

	max := mustPositionSizeValue(t, decimalMaxValue)
	min := mustPositionSizeValue(t, decimalMinValue)
	one := mustPositionSizeValue(t, "1")

	if _, err := max.CheckedAdd(one); err == nil {
		t.Fatal("CheckedAdd() error = nil, want overflow error")
	}
	if _, err := min.CheckedSub(one); err == nil {
		t.Fatal("CheckedSub() error = nil, want overflow error")
	}
	if _, err := max.CheckedMulInt(2); err == nil {
		t.Fatal("CheckedMulInt() error = nil, want overflow error")
	}
	if _, err := max.CheckedMulUint(2); err == nil {
		t.Fatal("CheckedMulUint() error = nil, want overflow error")
	}
	if _, err := one.CheckedMulFloat(math.NaN()); err == nil {
		t.Fatal("CheckedMulFloat() error = nil, want invalid-float error")
	}

	if _, err := one.CheckedDivInt(0); err == nil {
		t.Fatal("CheckedDivInt() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedDivUint(0); err == nil {
		t.Fatal("CheckedDivUint() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedDivFloat(0); err == nil {
		t.Fatal("CheckedDivFloat() error = nil, want divide-by-zero error")
	}

	if _, err := one.CheckedRemInt(0); err == nil {
		t.Fatal("CheckedRemInt() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedRemUint(0); err == nil {
		t.Fatal("CheckedRemUint() error = nil, want divide-by-zero error")
	}
	if _, err := one.CheckedRemFloat(0); err == nil {
		t.Fatal("CheckedRemFloat() error = nil, want divide-by-zero error")
	}
}
