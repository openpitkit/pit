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

package callback

/*
#include <stdint.h>

static inline void* pit_create_user_data_from_handle(uintptr_t handle) {
	return (void*)handle;
}

static inline uintptr_t pit_create_handle_from_user_data(void *userData) {
	return (uintptr_t)userData;
}
*/
import "C"

import (
	"runtime/cgo"
	"unsafe"
)

func NewUserDataFromHandle(handle cgo.Handle) unsafe.Pointer {
	return C.pit_create_user_data_from_handle(C.uintptr_t(handle))
}

func NewHandleFromUserData(userData unsafe.Pointer) cgo.Handle {
	return cgo.Handle(C.pit_create_handle_from_user_data(userData))
}
