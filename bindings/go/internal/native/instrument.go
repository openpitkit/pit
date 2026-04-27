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

/*
#include "pit.h"
*/
import "C"

func NewInstrument(underlying_asset string, settlement_asset string) Instrument {
	return Instrument{
		underlying_asset: importString(underlying_asset),
		settlement_asset: importString(settlement_asset),
	}
}

func InstrumentGetUnderlyingAsset(instrument Instrument) StringView {
	return newStringView(instrument.underlying_asset)
}

func InstrumentGetSettlementAsset(instrument Instrument) StringView {
	return newStringView(instrument.settlement_asset)
}
