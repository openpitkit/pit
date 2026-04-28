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

type TradeAmount struct {
	native native.ParamTradeAmount
}

func NewQuantityTradeAmount(v Quantity) TradeAmount {
	return newTradeAmount(
		native.CreateParamTradeAmount(
			native.ParamTradeAmountKindQuantity,
			native.ParamQuantityGetDecimal(v.native),
		),
	)
}

func NewVolumeTradeAmount(v Volume) TradeAmount {
	return newTradeAmount(
		native.CreateParamTradeAmount(
			native.ParamTradeAmountKindVolume,
			native.ParamVolumeGetDecimal(v.native),
		),
	)
}

func NewTradeAmountFromHandle(amount native.ParamTradeAmount) optional.Option[TradeAmount] {
	if native.ParamTradeAmountGetKind(amount) == native.ParamTradeAmountKindNotSet {
		return optional.None[TradeAmount]()
	}
	return optional.Some(newTradeAmount(amount))
}

func newTradeAmount(amount native.ParamTradeAmount) TradeAmount {
	return TradeAmount{native: amount}
}

func (a TradeAmount) IsQuantity() bool {
	return native.ParamTradeAmountGetKind(a.native) == native.ParamTradeAmountKindQuantity
}

func (a TradeAmount) IsVolume() bool {
	return native.ParamTradeAmountGetKind(a.native) == native.ParamTradeAmountKindVolume
}

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

func (a TradeAmount) Handle() native.ParamTradeAmount {
	return a.native
}

func (a TradeAmount) Choose(getQuantity func(Quantity), getVolume func(Volume)) {
	if a.IsQuantity() {
		getQuantity(a.MustQuantity())
		return
	}
	getVolume(a.MustVolume())
}
