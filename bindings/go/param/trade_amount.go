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
	"fmt"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pkg/optional"
)

// TradeAmount is a trade amount expressed as either a quantity or a volume.
type TradeAmount struct {
	native native.ParamTradeAmount
}

// NewQuantityTradeAmount creates a quantity-denominated TradeAmount.
func NewQuantityTradeAmount(v Quantity) TradeAmount {
	return newTradeAmount(
		native.CreateParamTradeAmount(
			native.ParamTradeAmountKindQuantity,
			native.ParamQuantityGetDecimal(v.native),
		),
	)
}

// NewVolumeTradeAmount creates a volume-denominated TradeAmount.
func NewVolumeTradeAmount(v Volume) TradeAmount {
	return newTradeAmount(
		native.CreateParamTradeAmount(
			native.ParamTradeAmountKindVolume,
			native.ParamVolumeGetDecimal(v.native),
		),
	)
}

// NewTradeAmountFromHandle creates an optional TradeAmount from a native handle.
func NewTradeAmountFromHandle(amount native.ParamTradeAmount) optional.Option[TradeAmount] {
	if native.ParamTradeAmountGetKind(amount) == native.ParamTradeAmountKindNotSet {
		return optional.None[TradeAmount]()
	}
	return optional.Some(newTradeAmount(amount))
}

func newTradeAmount(amount native.ParamTradeAmount) TradeAmount {
	return TradeAmount{native: amount}
}

// IsQuantity reports whether the trade amount is quantity-denominated.
func (a TradeAmount) IsQuantity() bool {
	return native.ParamTradeAmountGetKind(a.native) == native.ParamTradeAmountKindQuantity
}

// IsVolume reports whether the trade amount is volume-denominated.
func (a TradeAmount) IsVolume() bool {
	return native.ParamTradeAmountGetKind(a.native) == native.ParamTradeAmountKindVolume
}

// MustQuantity returns the quantity or panics if not quantity-denominated.
//
// The panic is deliberate fail-fast behavior: callers must check IsQuantity
// before reading the quantity variant.
func (a TradeAmount) MustQuantity() Quantity {
	if !a.IsQuantity() {
		panic("requested trade amount as quantity, but it is not")
	}
	value, err := native.CreateParamQuantity(native.ParamTradeAmountGetValue(a.native))
	if err != nil {
		panic(fmt.Sprintf("failed to decode quantity trade amount: %v", err))
	}
	return NewQuantityFromHandle(value)
}

// MustVolume returns the volume or panics if not volume-denominated.
//
// The panic is deliberate fail-fast behavior: callers must check IsVolume
// before reading the volume variant.
func (a TradeAmount) MustVolume() Volume {
	if !a.IsVolume() {
		panic("requested trade amount as volume, but it is not")
	}
	value, err := native.CreateParamVolume(native.ParamTradeAmountGetValue(a.native))
	if err != nil {
		panic(fmt.Sprintf("failed to decode volume trade amount: %v", err))
	}
	return NewVolumeFromHandle(value)
}

// Handle returns the underlying native handle.
func (a TradeAmount) Handle() native.ParamTradeAmount {
	return a.native
}

// String returns a human-readable representation of the trade amount.
func (a TradeAmount) String() string {
	switch native.ParamTradeAmountGetKind(a.native) {
	case native.ParamTradeAmountKindQuantity:
		return "qty: " + a.MustQuantity().String()
	case native.ParamTradeAmountKindVolume:
		return "vol: " + a.MustVolume().String()
	default:
		return "not set"
	}
}

// Choose calls getQuantity or getVolume depending on the trade amount kind.
func (a TradeAmount) Choose(getQuantity func(Quantity), getVolume func(Volume)) {
	if a.IsQuantity() {
		getQuantity(a.MustQuantity())
		return
	}
	getVolume(a.MustVolume())
}
