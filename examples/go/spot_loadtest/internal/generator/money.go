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

package generator

import "github.com/shopspring/decimal"

// Money/quantity/price precision is pinned so the shadow ledger's charge math
// is bit-exact against the engine's PositionSize arithmetic (rust_decimal: a
// 96-bit coefficient with scale 0..28).
//
// v1 (limit + quantity-denominated) deliberately restricts the value space so
// the only charge formula, q*p (Buy), is exact with no rounding:
//
//   - Quantity is an integer number of lots/shares (scale 0).
//   - Price has at most priceScale fractional digits (scale 2, classic ticks).
//
// Then q*p has at most priceScale fractional digits and a magnitude far inside
// the 96-bit coefficient, so shopspring/decimal.Mul reproduces the engine's
// Price::calculate_position_size result exactly. No truncation, no rounding
// mode dependence - the oracle stays strict.
const (
	// priceScale is the maximum number of fractional digits a price may carry.
	priceScale = 2
)

// chargeForBuy returns the settlement-asset charge for a Buy order: q*p.
// Inputs are integer quantity and a price already constrained to priceScale,
// so the product is exact.
func chargeForBuy(quantity, price decimal.Decimal) decimal.Decimal {
	return quantity.Mul(price)
}

// chargeForSell returns the underlying-asset charge for a Sell order: q.
func chargeForSell(quantity decimal.Decimal) decimal.Decimal {
	return quantity
}

// quantityDecimal converts an integer lot count to a scale-0 decimal.
func quantityDecimal(lots uint64) decimal.Decimal {
	return decimal.NewFromUint64(lots)
}

// priceFromCents builds a price from an integer number of cents, yielding an
// exact scale-2 decimal (e.g. 15000 -> 150.00). Keeping prices integer-cents
// internally guarantees they never exceed priceScale.
func priceFromCents(cents uint64) decimal.Decimal {
	return decimal.New(int64(cents), -priceScale) //nolint:gosec // cents range is bounded by the price grid below
}
