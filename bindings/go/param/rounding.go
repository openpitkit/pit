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
	"github.com/openpitkit/pit-go/internal/native"
)

// RoundingStrategy configures decimal rounding behavior.
type RoundingStrategy native.ParamRoundingStrategy

const (
	// RoundingStrategyMidpointNearestEven rounds midpoint to nearest even.
	RoundingStrategyMidpointNearestEven RoundingStrategy = native.ParamRoundingStrategyMidpointNearestEven
	// RoundingStrategyMidpointAwayFromZero rounds midpoint away from zero.
	RoundingStrategyMidpointAwayFromZero RoundingStrategy = native.ParamRoundingStrategyMidpointAwayFromZero
	// RoundingStrategyUp always rounds toward positive infinity.
	RoundingStrategyUp RoundingStrategy = native.ParamRoundingStrategyUp
	// RoundingStrategyDown always rounds toward negative infinity.
	RoundingStrategyDown RoundingStrategy = native.ParamRoundingStrategyDown
)

const (
	// RoundingStrategyDefault is the recommended default strategy.
	RoundingStrategyDefault RoundingStrategy = native.ParamRoundingStrategy_Default
	// RoundingStrategyBanker is banker's rounding alias.
	RoundingStrategyBanker RoundingStrategy = native.ParamRoundingStrategy_Banker
	// RoundingStrategyConservativeProfit rounds down for conservative profit estimates.
	RoundingStrategyConservativeProfit RoundingStrategy = native.ParamRoundingStrategy_ConservativeProfit
	// RoundingStrategyConservativeLoss rounds down for conservative loss estimates.
	RoundingStrategyConservativeLoss RoundingStrategy = native.ParamRoundingStrategy_ConservativeLoss
)

func (r RoundingStrategy) String() string {
	switch r {
	case RoundingStrategyMidpointNearestEven:
		return "MidpointNearestEven"
	case RoundingStrategyMidpointAwayFromZero:
		return "MidpointAwayFromZero"
	case RoundingStrategyUp:
		return "Up"
	case RoundingStrategyDown:
		return "Down"
	default:
		return "Unknown"
	}
}

func (r RoundingStrategy) native() native.ParamRoundingStrategy {
	return native.ParamRoundingStrategy(r)
}
