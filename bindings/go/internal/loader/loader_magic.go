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

package loader

import (
	"fmt"
	"io"
	"os"
)

// validateSharedLibraryMagic checks that the file starts with a recognized
// shared library magic number. This guards against the platform loader
// silently reusing an already-loaded library when given a corrupt cached file
// with the same name.
//
// The check is intentionally cheap (four bytes) and platform-agnostic: every
// supported runtime artifact carries one of the magics below, and the same
// magics are verified on every load path on every OS.
func validateSharedLibraryMagic(path string) error {
	f, err := os.Open(path) //nolint:gosec // file path comes from the loader's own resolution logic
	if err != nil {
		return fmt.Errorf("open: %w", err)
	}
	defer func() { _ = f.Close() }()

	var magic [4]byte
	if _, err := io.ReadFull(f, magic[:]); err != nil {
		return fmt.Errorf("read magic bytes: %w", err)
	}

	// ELF (Linux): \x7fELF
	if magic[0] == 0x7F && magic[1] == 'E' && magic[2] == 'L' && magic[3] == 'F' {
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
	// PE/COFF (Windows DLL/EXE): "MZ" DOS header at offset 0
	if magic[0] == 'M' && magic[1] == 'Z' {
		return nil
	}

	return fmt.Errorf("not a recognized shared library (magic: %02x %02x %02x %02x)", magic[0], magic[1], magic[2], magic[3])
}
