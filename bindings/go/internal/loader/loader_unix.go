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

//go:build linux || darwin

package loader

/*
#cgo linux LDFLAGS: -ldl
#include <dlfcn.h>
#include <stdlib.h>
*/
import "C"

import (
	"errors"
	"fmt"
	"io"
	"os"
	"unsafe"
)

func loadRuntimeLibrary(path string) error {
	if err := validateSharedLibraryMagic(path); err != nil {
		return err
	}

	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	C.dlerror()
	if handle := C.dlopen(cPath, C.RTLD_NOW|C.RTLD_GLOBAL); handle == nil {
		dlopenErr := C.dlerror()
		if dlopenErr == nil {
			return errors.New("dlopen failed")
		}
		return errors.New(C.GoString(dlopenErr))
	}
	return nil
}

// validateSharedLibraryMagic checks that the file starts with a recognized
// shared library magic number. This guards against dlopen silently reusing an
// already-loaded library when given a corrupt cached file with the same name.
func validateSharedLibraryMagic(path string) error {
	f, err := os.Open(path)
	if err != nil {
		return fmt.Errorf("open: %w", err)
	}
	defer f.Close()

	var magic [4]byte
	if _, err := io.ReadFull(f, magic[:]); err != nil {
		return fmt.Errorf("read magic bytes: %w", err)
	}

	// ELF (Linux): \x7fELF
	if magic[0] == 0x7f && magic[1] == 'E' && magic[2] == 'L' && magic[3] == 'F' {
		return nil
	}
	// Mach-O little-endian 32-bit: 0xCEFAEDFE
	if magic[0] == 0xCE && magic[1] == 0xFA && magic[2] == 0xED && magic[3] == 0xFE {
		return nil
	}
	// Mach-O little-endian 64-bit: 0xCFFAEDFE
	if magic[0] == 0xCF && magic[1] == 0xFA && magic[2] == 0xED && magic[3] == 0xFE {
		return nil
	}
	// Mach-O fat binary: 0xCAFEBABE
	if magic[0] == 0xCA && magic[1] == 0xFE && magic[2] == 0xBA && magic[3] == 0xBE {
		return nil
	}

	return fmt.Errorf("not a recognized shared library (magic: %02x %02x %02x %02x)", magic[0], magic[1], magic[2], magic[3])
}
