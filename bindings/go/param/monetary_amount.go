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
// Please see https://openpit.dev and the OWNERS file for details.

package param

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pkg/optional"
)

// MonetaryAmount is a fee amount paired with its currency.
type MonetaryAmount struct {
	Amount   Fee
	Currency Asset
}

// NewMonetaryAmount creates a monetary amount from an amount and currency.
func NewMonetaryAmount(amount Fee, currency Asset) MonetaryAmount {
	return MonetaryAmount{
		Amount:   amount,
		Currency: currency,
	}
}

// NewMonetaryAmountFromHandle creates an optional MonetaryAmount from a native handle.
func NewMonetaryAmountFromHandle(
	value native.ParamMonetaryAmount,
) optional.Option[MonetaryAmount] {
	currency, hasCurrency := NewAssetFromHandle(
		native.ParamMonetaryAmountGetCurrency(value),
	).Get()
	if !hasCurrency {
		return optional.None[MonetaryAmount]()
	}
	return optional.Some(NewMonetaryAmount(
		NewFeeFromHandle(native.ParamMonetaryAmountGetAmount(value)),
		currency,
	))
}

// NewMonetaryAmountOptionFromHandle creates an optional MonetaryAmount from a
// native optional handle.
func NewMonetaryAmountOptionFromHandle(
	value native.ParamMonetaryAmountOptional,
) optional.Option[MonetaryAmount] {
	if !native.ParamMonetaryAmountOptionalIsSet(value) {
		return optional.None[MonetaryAmount]()
	}
	return NewMonetaryAmountFromHandle(native.ParamMonetaryAmountOptionalGet(value))
}

// Handle returns the underlying native handle.
func (v MonetaryAmount) Handle() native.ParamMonetaryAmount {
	return native.NewParamMonetaryAmount(v.Amount.Handle(), v.Currency.Handle())
}

// Equal reports whether v and other have the same amount and currency.
func (v MonetaryAmount) Equal(other MonetaryAmount) bool {
	return v.Amount.Equal(other.Amount) && v.Currency.Equal(other.Currency)
}
