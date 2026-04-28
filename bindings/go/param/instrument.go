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

type Instrument struct {
	UnderlyingAsset Asset
	SettlementAsset Asset
}

func NewInstrument(underlyingAsset Asset, settlementAsset Asset) Instrument {
	return Instrument{UnderlyingAsset: underlyingAsset, SettlementAsset: settlementAsset}
}

func NewInstrumentFromHandle(i native.Instrument) optional.Option[Instrument] {
	underlyingAsset, hasUnderlyingAsset := NewAssetFromHandle(
		native.InstrumentGetUnderlyingAsset(i),
	).Get()
	if !hasUnderlyingAsset {
		return optional.None[Instrument]()
	}

	settlementAsset, hasSettlementAsset := NewAssetFromHandle(
		native.InstrumentGetSettlementAsset(i),
	).Get()
	if !hasSettlementAsset {
		return optional.None[Instrument]()
	}

	return optional.Some(NewInstrument(underlyingAsset, settlementAsset))
}

func (i Instrument) String() string {
	return fmt.Sprintf("%s/%s", i.UnderlyingAsset, i.SettlementAsset)
}

func (i Instrument) Handle() native.Instrument {
	return native.NewInstrument(i.UnderlyingAsset.Handle(), i.SettlementAsset.Handle())
}
