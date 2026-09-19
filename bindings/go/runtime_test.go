// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

package openpit_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"go.openpit.dev/openpit"
)

func TestRuntimeLibraryPathNamesLoadedLibrary(t *testing.T) {
	path := openpit.RuntimeLibraryPath()
	if path == "" {
		t.Fatal("RuntimeLibraryPath() = \"\", want the loaded library")
	}
	if !filepath.IsAbs(path) {
		t.Fatalf("RuntimeLibraryPath() = %q, want an absolute path", path)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("RuntimeLibraryPath() names a missing file: %v", err)
	}
	override := strings.TrimSpace(os.Getenv("OPENPIT_RUNTIME_LIBRARY_PATH"))
	if override != "" && path != filepath.Clean(override) {
		t.Fatalf("RuntimeLibraryPath() = %q, want the cleaned override %q", path, override)
	}
}
