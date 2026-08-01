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

// Package mdhandle hands an owned native market-data handle from a public
// marketdata.Service to the policy builders of this module.
//
// Releasing such a handle needs internal/native, which nothing outside this
// module can import, so the escape hatch must not sit on marketdata.Service
// itself: an external caller could obtain the handle but never free it.
// Package marketdata installs Clone from its init; the service value travels as
// any because naming it here would close an import cycle.
package mdhandle

import (
	"errors"

	"go.openpit.dev/openpit/internal/native"
)

// ErrUnknownService reports a value that package marketdata did not create.
var ErrUnknownService = errors.New("openpit: not a market-data service")

// Clone returns a caller-owned handle to service, released with
// native.DestroyMarketDataService, or an error when service is already closed.
var Clone = func(_ any) (native.MarketDataService, error) {
	return nil, ErrUnknownService
}
