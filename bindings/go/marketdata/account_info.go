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

package marketdata

import (
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// AccountInfo supplies the reading account's group on demand. The engine's
// pretrade.Context already satisfies it.
//
// AccountGroup runs inside the Service read that asked for it, while that read
// holds the service's locks. It must not call into the same market-data
// service, directly or indirectly: no method of the Service being read or of
// any other handle to the same service, such as one returned by Clone - Close
// included - and no engine operation whose policies read that service. Such a
// call is not detected and is not reported as an error; it can block the
// calling goroutine forever.
type AccountInfo interface {
	AccountGroup() optional.Option[param.AccountGroupID]
}
