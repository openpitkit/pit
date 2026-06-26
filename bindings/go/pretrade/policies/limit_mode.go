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

package policies

import (
	"fmt"

	"go.openpit.dev/openpit/internal/native"
)

// SpotFundsLimitMode selects how the spot-funds control reacts to insufficient
// available funds. It is a small copyable value type that encodes only the
// policy applied when a reservation would exceed what is available, never the
// funds themselves.
//
// The zero value is SpotFundsLimitModeEnforce, matching the core
// SpotFundsLimitMode default.
type SpotFundsLimitMode uint8

const (
	// SpotFundsLimitModeEnforce rejects a reservation when available funds are
	// insufficient (RejectCode InsufficientFunds); the reservation is not
	// recorded. This is the default.
	SpotFundsLimitModeEnforce SpotFundsLimitMode = SpotFundsLimitMode(native.PretradePoliciesSpotFundsLimitModeEnforce)
	// SpotFundsLimitModeTrackOnly always records the reservation; available may
	// go negative and a shortfall never rejects. Arithmetic overflow is still
	// surfaced.
	SpotFundsLimitModeTrackOnly SpotFundsLimitMode = SpotFundsLimitMode(native.PretradePoliciesSpotFundsLimitModeTrackOnly)
)

// Handle exposes the underlying native spot-funds limit mode.
func (v SpotFundsLimitMode) Handle() native.PretradePoliciesSpotFundsLimitMode {
	return native.PretradePoliciesSpotFundsLimitMode(v)
}

// Valid reports whether the value maps to a known spot-funds limit mode.
func (v SpotFundsLimitMode) Valid() bool {
	switch v {
	case SpotFundsLimitModeEnforce, SpotFundsLimitModeTrackOnly:
		return true
	default:
		return false
	}
}

// String returns a human-readable representation of the limit mode.
func (v SpotFundsLimitMode) String() string {
	switch v {
	case SpotFundsLimitModeEnforce:
		return "enforce"
	case SpotFundsLimitModeTrackOnly:
		return "track-only"
	default:
		return fmt.Sprintf("SpotFundsLimitMode(%d)", uint8(v))
	}
}
