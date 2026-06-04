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
#include "openpit.h"
*/
import "C"

import "unsafe"

func DestroySharedBytes(handle SharedBytes) {
	C.openpit_destroy_shared_bytes(handle)
}

func SharedBytesView(handle SharedBytes) BytesView {
	return C.openpit_shared_bytes_view(handle)
}

func BytesViewAsSlice(view BytesView) []byte {
	if view.ptr == nil || view.len == 0 {
		return nil
	}
	return C.GoBytes(unsafe.Pointer(view.ptr), C.int(view.len))
}

func CloneBytes(view BytesView) []byte {
	src := BytesViewAsSlice(view)
	if len(src) == 0 {
		return nil
	}
	out := make([]byte, len(src))
	copy(out, src)
	return out
}
