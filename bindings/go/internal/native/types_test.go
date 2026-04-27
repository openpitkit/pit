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

package native

import "testing"

func TestNativeEnumConstantsAreDistinct(t *testing.T) {
	if ParamSideNotSet == ParamSideBuy || ParamSideNotSet == ParamSideSell || ParamSideBuy == ParamSideSell {
		t.Fatalf(
			"ParamSide constants must be distinct: not_set=%v buy=%v sell=%v",
			ParamSideNotSet,
			ParamSideBuy,
			ParamSideSell,
		)
	}

	if TriBoolNotSet == TriBoolFalse || TriBoolNotSet == TriBoolTrue || TriBoolFalse == TriBoolTrue {
		t.Fatalf(
			"TriBool constants must be distinct: not_set=%v false=%v true=%v",
			TriBoolNotSet,
			TriBoolFalse,
			TriBoolTrue,
		)
	}

	if ParamTradeAmountKindNotSet == ParamTradeAmountKindQuantity ||
		ParamTradeAmountKindNotSet == ParamTradeAmountKindVolume ||
		ParamTradeAmountKindQuantity == ParamTradeAmountKindVolume {
		t.Fatalf(
			"ParamTradeAmountKind constants must be distinct: not_set=%v quantity=%v volume=%v",
			ParamTradeAmountKindNotSet,
			ParamTradeAmountKindQuantity,
			ParamTradeAmountKindVolume,
		)
	}

	if ParamAdjustmentAmountKindNotSet == ParamAdjustmentAmountKindDelta ||
		ParamAdjustmentAmountKindNotSet == ParamAdjustmentAmountKindAbsolute ||
		ParamAdjustmentAmountKindDelta == ParamAdjustmentAmountKindAbsolute {
		t.Fatalf(
			"ParamAdjustmentAmountKind constants must be distinct: not_set=%v delta=%v absolute=%v",
			ParamAdjustmentAmountKindNotSet,
			ParamAdjustmentAmountKindDelta,
			ParamAdjustmentAmountKindAbsolute,
		)
	}

	if RejectScopeOrder == RejectScopeAccount {
		t.Fatalf(
			"RejectScope constants must be distinct: order=%v account=%v",
			RejectScopeOrder,
			RejectScopeAccount,
		)
	}
}

func TestNativeLeverageConstantsAreConsistent(t *testing.T) {
	if ParamLeverageMin == 0 {
		t.Fatal("ParamLeverageMin = 0, want positive minimum")
	}
	if ParamLeverageNotSet != 0 && ParamLeverageNotSet >= ParamLeverageMin {
		t.Fatalf(
			"ParamLeverageNotSet = %v, want 0 or value below ParamLeverageMin = %v",
			ParamLeverageNotSet,
			ParamLeverageMin,
		)
	}
	if ParamLeverageMax <= ParamLeverageMin {
		t.Fatalf("ParamLeverageMax = %v, want > ParamLeverageMin = %v", ParamLeverageMax, ParamLeverageMin)
	}
	if ParamLeverageStep == 0 {
		t.Fatal("ParamLeverageStep = 0, want positive step")
	}
	if ParamLeverageScale == 0 {
		t.Fatal("ParamLeverageScale = 0, want positive scale")
	}
}
