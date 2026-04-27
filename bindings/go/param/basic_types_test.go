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

func TestNewAccountIDFromStringStable(t *testing.T) {
	t.Parallel()

	a := NewAccountIDFromString("acc-1")
	b := NewAccountIDFromString("acc-1")
	c := NewAccountIDFromString("acc-2")

	if a.Native() != b.Native() {
		t.Fatalf("same source hash mismatch: %v vs %v", a.Native(), b.Native())
	}
	if a.Native() == c.Native() {
		t.Fatalf("different source hash collision for short inputs: %v", a.Native())
	}
	if got := a.String(); got == "" {
		t.Fatal("String() should not be empty")
	}
}

func TestAssetStringAndNative(t *testing.T) {
	t.Parallel()

	asset := NewAsset("USD")
	if got := asset.String(); got != "USD" {
		t.Fatalf("String() = %q, want %q", got, "USD")
	}
	if got := asset.Native(); got != "USD" {
		t.Fatalf("Native() = %q, want %q", got, "USD")
	}
}

func TestSideHelpers(t *testing.T) {
	t.Parallel()

	if !SideBuy.IsBuy() || SideBuy.IsSell() {
		t.Fatal("SideBuy helpers returned invalid flags")
	}
	if SideBuy.Opposite() != SideSell {
		t.Fatalf("SideBuy.Opposite() = %v, want %v", SideBuy.Opposite(), SideSell)
	}
	if SideBuy.Sign() != 1 {
		t.Fatalf("SideBuy.Sign() = %d, want %d", SideBuy.Sign(), 1)
	}
	if got := SideBuy.String(); got != "buy" {
		t.Fatalf("SideBuy.String() = %q, want %q", got, "buy")
	}
	if SideBuy.Native() != native.ParamSideBuy {
		t.Fatalf("SideBuy.Native() = %v, want %v", SideBuy.Native(), native.ParamSideBuy)
	}

	if SideSell.IsBuy() || !SideSell.IsSell() {
		t.Fatal("SideSell helpers returned invalid flags")
	}
	if SideSell.Opposite() != SideBuy {
		t.Fatalf("SideSell.Opposite() = %v, want %v", SideSell.Opposite(), SideBuy)
	}
	if SideSell.Sign() != -1 {
		t.Fatalf("SideSell.Sign() = %d, want %d", SideSell.Sign(), -1)
	}
	if got := SideSell.String(); got != "sell" {
		t.Fatalf("SideSell.String() = %q, want %q", got, "sell")
	}
	if SideSell.Native() != native.ParamSideSell {
		t.Fatalf("SideSell.Native() = %v, want %v", SideSell.Native(), native.ParamSideSell)
	}
}

func TestNewSideFromNative(t *testing.T) {
	t.Parallel()

	unset := NewSideFromNative(native.ParamSideNotSet)
	if unset.IsSet() {
		t.Fatal("not-set native side should map to empty option")
	}

	buy, ok := NewSideFromNative(native.ParamSideBuy).Get()
	if !ok {
		t.Fatal("buy side should be set")
	}
	if buy != SideBuy {
		t.Fatalf("buy side = %v, want %v", buy, SideBuy)
	}
}

func TestNewSideFromNativePanicsOnUnknown(t *testing.T) {
	t.Parallel()

	defer func() {
		if recover() == nil {
			t.Fatal("expected panic for unknown native side")
		}
	}()

	_ = NewSideFromNative(native.ParamSide(200))
}

func TestPositionSideHelpers(t *testing.T) {
	t.Parallel()

	if !PositionSideLong.IsLong() || PositionSideLong.IsShort() {
		t.Fatal("PositionSideLong helpers returned invalid flags")
	}
	if PositionSideLong.Opposite() != PositionSideShort {
		t.Fatalf(
			"PositionSideLong.Opposite() = %v, want %v",
			PositionSideLong.Opposite(),
			PositionSideShort,
		)
	}
	if got := PositionSideLong.String(); got != "long" {
		t.Fatalf("PositionSideLong.String() = %q, want %q", got, "long")
	}

	if PositionSideShort.IsLong() || !PositionSideShort.IsShort() {
		t.Fatal("PositionSideShort helpers returned invalid flags")
	}
	if PositionSideShort.Opposite() != PositionSideLong {
		t.Fatalf(
			"PositionSideShort.Opposite() = %v, want %v",
			PositionSideShort.Opposite(),
			PositionSideLong,
		)
	}
	if got := PositionSideShort.String(); got != "short" {
		t.Fatalf("PositionSideShort.String() = %q, want %q", got, "short")
	}
}

func TestNewPositionSideFromNativePanicsOnUnknown(t *testing.T) {
	t.Parallel()

	defer func() {
		if recover() == nil {
			t.Fatal("expected panic for unknown native position side")
		}
	}()

	_ = NewPositionSideFromNative(native.ParamPositionSide(200))
}

func TestPositionEffectString(t *testing.T) {
	t.Parallel()

	if got := PositionEffectOpen.String(); got != "open" {
		t.Fatalf("PositionEffectOpen.String() = %q, want %q", got, "open")
	}
	if got := PositionEffectClose.String(); got != "close" {
		t.Fatalf("PositionEffectClose.String() = %q, want %q", got, "close")
	}
}

func TestRoundingStrategyStringAndNative(t *testing.T) {
	t.Parallel()

	if got := RoundingStrategyMidpointNearestEven.String(); got != "MidpointNearestEven" {
		t.Fatalf("midpoint-even String() = %q, want %q", got, "MidpointNearestEven")
	}
	if got := RoundingStrategyMidpointAwayFromZero.String(); got != "MidpointAwayFromZero" {
		t.Fatalf("midpoint-away String() = %q, want %q", got, "MidpointAwayFromZero")
	}
	if got := RoundingStrategyUp.String(); got != "Up" {
		t.Fatalf("up String() = %q, want %q", got, "Up")
	}
	if got := RoundingStrategyDown.String(); got != "Down" {
		t.Fatalf("down String() = %q, want %q", got, "Down")
	}
	if got := RoundingStrategy(255).String(); got != "Unknown" {
		t.Fatalf("unknown String() = %q, want %q", got, "Unknown")
	}
	if got := RoundingStrategyUp.native(); got != native.ParamRoundingStrategyUp {
		t.Fatalf("native() = %v, want %v", got, native.ParamRoundingStrategyUp)
	}
}

func TestTradeString(t *testing.T) {
	t.Parallel()

	price, err := NewPriceFromString("123.45")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	quantity, err := NewQuantityFromString("2")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}

	trade := Trade{Price: price, Quantity: quantity}
	if got := trade.String(); got != "2 @ 123.45" {
		t.Fatalf("Trade.String() = %q, want %q", got, "2 @ 123.45")
	}
}
