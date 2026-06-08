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

package marketdata

import (
	"strconv"

	"go.openpit.dev/openpit/internal/native"
)

// InstrumentID is a type-safe market-data instrument identifier.
type InstrumentID struct {
	native native.MarketDataInstrumentID
}

// NewInstrumentIDFromUint64 constructs an instrument identifier from a uint64
// value.
func NewInstrumentIDFromUint64(source uint64) InstrumentID {
	return newInstrumentIDFromHandle(native.MarketDataInstrumentID(source))
}

func newInstrumentIDFromHandle(source native.MarketDataInstrumentID) InstrumentID {
	return InstrumentID{native: source}
}

// String formats the instrument identifier as a decimal string.
func (v InstrumentID) String() string {
	return strconv.FormatUint(uint64(v.native), 10)
}

// Handle exposes the underlying native instrument identifier.
func (v InstrumentID) Handle() native.MarketDataInstrumentID {
	return v.native
}
