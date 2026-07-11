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

// FillType identifies the type of fill event reported by a venue.
type FillType native.ParamFillType

const (
	// FillTypeTrade is a normal trade execution.
	FillTypeTrade FillType = native.ParamFillTypeTrade
	// FillTypeLiquidation is a forced liquidation by the venue.
	FillTypeLiquidation FillType = native.ParamFillTypeLiquidation
	// FillTypeAutoDeleverage is an auto-deleveraging event.
	FillTypeAutoDeleverage FillType = native.ParamFillTypeAutoDeleverage
	// FillTypeSettlement is settlement at expiry or delivery.
	FillTypeSettlement FillType = native.ParamFillTypeSettlement
	// FillTypeFunding is a funding payment.
	FillTypeFunding FillType = native.ParamFillTypeFunding
)

// String returns the stable wire name.
func (value FillType) String() string {
	switch value {
	case FillTypeTrade:
		return "TRADE"
	case FillTypeLiquidation:
		return "LIQUIDATION"
	case FillTypeAutoDeleverage:
		return "AUTO_DELEVERAGE"
	case FillTypeSettlement:
		return "SETTLEMENT"
	case FillTypeFunding:
		return "FUNDING"
	default:
		return ""
	}
}
