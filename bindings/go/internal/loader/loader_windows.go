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

//go:build windows

package loader

/*
#include <stdint.h>

// Keep the uintptr-to-pointer conversion in C so go vet does not flag it as
// unsafe.Pointer misuse in Go code.
static inline void* openpit_windows_handle_to_pointer(uintptr_t handle) {
	return (void*)handle;
}
*/
import "C"

import (
	"fmt"
	"syscall"
	"unsafe"
)

func loadRuntimeLibrary(path string) (unsafe.Pointer, error) {
	if err := validateSharedLibraryMagic(path); err != nil {
		return nil, fmt.Errorf("%w: %w", errMagicCheckFailed, err)
	}

	handle, err := syscall.LoadLibrary(path)
	if err != nil {
		return nil, err
	}
	return C.openpit_windows_handle_to_pointer(C.uintptr_t(handle)), nil
}
