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
	"testing"

	"github.com/openpitkit/pit-go/internal/native"
)

func TestTradeAmountQuantity(t *testing.T) {
	t.Parallel()

	quantity := newQuantityOrPanic(NewQuantityFromString("5"))
	amount := NewQuantityTradeAmount(quantity)

	if !amount.IsQuantity() || amount.IsVolume() {
		t.Fatal("quantity trade amount has unexpected kind flags")
	}
	if got := amount.MustQuantity(); !got.Equal(quantity) {
		t.Fatalf("MustQuantity() = %v, want %v", got, quantity)
	}

	var gotQuantity bool
	var gotVolume bool
	amount.Choose(
		func(v Quantity) {
			gotQuantity = v.Equal(quantity)
		},
		func(Volume) {
			gotVolume = true
		},
	)
	if !gotQuantity {
		t.Fatal("Choose() did not call quantity branch")
	}
	if gotVolume {
		t.Fatal("Choose() called volume branch unexpectedly")
	}

	roundTrip, ok := NewTradeAmountFromNative(amount.Native()).Get()
	if !ok {
		t.Fatal("round-trip trade amount should be present")
	}
	if !roundTrip.IsQuantity() {
		t.Fatal("round-trip amount should keep quantity kind")
	}
}

func TestTradeAmountVolume(t *testing.T) {
	t.Parallel()

	volume := newVolumeOrPanic(NewVolumeFromString("12.5"))
	amount := NewVolumeTradeAmount(volume)

	if amount.IsQuantity() || !amount.IsVolume() {
		t.Fatal("volume trade amount has unexpected kind flags")
	}
	if got := amount.MustVolume(); !got.Equal(volume) {
		t.Fatalf("MustVolume() = %v, want %v", got, volume)
	}

	var gotQuantity bool
	var gotVolume bool
	amount.Choose(
		func(Quantity) {
			gotQuantity = true
		},
		func(v Volume) {
			gotVolume = v.Equal(volume)
		},
	)
	if gotQuantity {
		t.Fatal("Choose() called quantity branch unexpectedly")
	}
	if !gotVolume {
		t.Fatal("Choose() did not call volume branch")
	}
}

func TestTradeAmountFromNativeUnset(t *testing.T) {
	t.Parallel()

	if got := NewTradeAmountFromNative(native.ParamTradeAmount{}); got.IsSet() {
		t.Fatal("unset native trade amount should map to empty option")
	}
}

func TestTradeAmountMustQuantityPanicsOnVolume(t *testing.T) {
	t.Parallel()

	volume := newVolumeOrPanic(NewVolumeFromString("1"))
	amount := NewVolumeTradeAmount(volume)

	defer func() {
		if recover() == nil {
			t.Fatal("MustQuantity() should panic for volume amount")
		}
	}()

	_ = amount.MustQuantity()
}

func TestTradeAmountMustVolumePanicsOnQuantity(t *testing.T) {
	t.Parallel()

	quantity := newQuantityOrPanic(NewQuantityFromString("1"))
	amount := NewQuantityTradeAmount(quantity)

	defer func() {
		if recover() == nil {
			t.Fatal("MustVolume() should panic for quantity amount")
		}
	}()

	_ = amount.MustVolume()
}
