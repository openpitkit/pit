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

/*
#cgo CFLAGS: -I${SRCDIR}/../internal/native
#include "openpit.h"

extern bool pitMarketDataAccountGroupResolver(void *user_data, OpenPitParamAccountGroupId *out_account_group_id);

static OpenPitMarketDataAccountGroupResolver
    openpit_market_data_account_group_resolver_fn = pitMarketDataAccountGroupResolver;

static void *
pitMarketDataAccountGroupResolverFnAddr(void) {
    return (void *)&openpit_market_data_account_group_resolver_fn;
}
*/
import "C"

import (
	"unsafe"

	"go.openpit.dev/openpit/internal/callback"
)

// accountGroupResolverFnAddr returns the address of the static C variable
// holding the account-group resolver function pointer. It is passed to
// native.MarketDataServiceGet as the resolve_account_group argument; the native
// shim casts it back to OpenPitMarketDataAccountGroupResolver before calling C.
func accountGroupResolverFnAddr() unsafe.Pointer {
	return C.pitMarketDataAccountGroupResolverFnAddr()
}

//export pitMarketDataAccountGroupResolver
func pitMarketDataAccountGroupResolver(
	userData unsafe.Pointer,
	outAccountGroupID *C.OpenPitParamAccountGroupId,
) C.bool {
	info := callback.NewHandleFromUserData(userData).Value().(AccountInfo)
	group, ok := info.AccountGroup().Get()
	if !ok {
		return C.bool(false)
	}
	*outAccountGroupID = C.OpenPitParamAccountGroupId(group.Handle())
	return C.bool(true)
}
