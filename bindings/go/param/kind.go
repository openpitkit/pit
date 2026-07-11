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
	"go.openpit.dev/openpit/internal/native"
)

// Kind identifies a numeric domain value category.
type Kind native.ParamKind

const (
	// KindQuantity identifies Quantity values.
	KindQuantity Kind = native.ParamKindQuantity
	// KindVolume identifies Volume values.
	KindVolume Kind = native.ParamKindVolume
	// KindNotional identifies Notional values.
	KindNotional Kind = native.ParamKindNotional
	// KindPrice identifies Price values.
	KindPrice Kind = native.ParamKindPrice
	// KindPnl identifies Pnl values.
	KindPnl Kind = native.ParamKindPnl
	// KindCashFlow identifies CashFlow values.
	KindCashFlow Kind = native.ParamKindCashFlow
	// KindPositionSize identifies PositionSize values.
	KindPositionSize Kind = native.ParamKindPositionSize
	// KindFee identifies Fee values.
	KindFee Kind = native.ParamKindFee
	// KindLeverage identifies Leverage values.
	KindLeverage Kind = native.ParamKindLeverage
)

// String returns the stable domain category name.
func (value Kind) String() string {
	switch value {
	case KindQuantity:
		return "Quantity"
	case KindVolume:
		return "Volume"
	case KindNotional:
		return "Notional"
	case KindPrice:
		return "Price"
	case KindPnl:
		return "Pnl"
	case KindCashFlow:
		return "CashFlow"
	case KindPositionSize:
		return "PositionSize"
	case KindFee:
		return "Fee"
	case KindLeverage:
		return "Leverage"
	default:
		return ""
	}
}
