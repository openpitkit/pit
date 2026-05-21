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

//go:generate python3 ../../../../scripts/generate_api_c_dlsym.py

/*
const char *openpit_native_init(void *handle);
*/
import "C"

import (
	"fmt"

	"go.openpit.dev/openpit/internal/loader"
)

func loadNativeSymbols() {
	missing := C.openpit_native_init(loader.LoadedHandle())
	if missing != nil {
		panic(&loader.RuntimeLoadError{
			Reason: loader.ReasonSymbolNotFound,
			Path:   loader.LoadedPath(),
			Cause:  fmt.Errorf("symbol %q not found in runtime library", C.GoString(missing)),
		})
	}
}
