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
	"os"
	"path/filepath"
	goruntime "runtime"
	"strings"
	"sync"
	"testing"

	pitruntime "github.com/openpitkit/pit-go/internal/runtime"
)

// ---------------------------------------------------------------------------
// ensureVersionedPath
// ---------------------------------------------------------------------------

func TestEnsureVersionedPath_Valid(t *testing.T) {
	cases := []struct {
		version string
		path    string
	}{
		{"0.2.0", filepath.Join("/cache", "pit-go", "0.2.0", "linux-amd64", "libpit_ffi.so")},
		{"1.0.0", filepath.Join("/cache", "1.0.0", "libpit.so")},
		{"0.2.0", filepath.Join("/cache", "pit-go", "0.2.0", "linux-amd64", "libpit_ffi.so")},
	}
	for _, tc := range cases {
		if err := ensureVersionedPath(tc.version, tc.path); err != nil {
			t.Errorf("ensureVersionedPath(%q, %q) unexpected error: %v", tc.version, tc.path, err)
		}
	}
}

func TestEnsureVersionedPath_Invalid(t *testing.T) {
	cases := []struct {
		name    string
		version string
		path    string
	}{
		{
			"missing version segment",
			"0.2.0",
			filepath.Join("/cache", "pit-go", "linux-amd64", "libpit_ffi.so"),
		},
		{
			"empty version",
			"",
			filepath.Join("/cache", "pit-go", "linux-amd64", "libpit_ffi.so"),
		},
		{
			"version of slashes only",
			"///",
			filepath.Join("/cache", "///", "linux-amd64", "libpit_ffi.so"),
		},
	}
	for _, tc := range cases {
		if err := ensureVersionedPath(tc.version, tc.path); err == nil {
			t.Errorf("ensureVersionedPath(%q, %q) [%s]: want error, got nil", tc.version, tc.path, tc.name)
		}
	}
}

// ---------------------------------------------------------------------------
// resolveCacheDir
// ---------------------------------------------------------------------------

func TestResolveCacheDir_Override(t *testing.T) {
	cacheRoot := t.TempDir()
	t.Setenv(envRuntimeCache, cacheRoot)

	got, err := resolveCacheDir("1.0.0")
	if err != nil {
		t.Fatalf("resolveCacheDir: %v", err)
	}

	want := filepath.Join(cacheRoot, "1.0.0", goruntime.GOOS+"-"+goruntime.GOARCH)
	if got != want {
		t.Fatalf("resolveCacheDir: got %q, want %q", got, want)
	}
}

func TestResolveCacheDir_Default(t *testing.T) {
	t.Setenv(envRuntimeCache, "")

	got, err := resolveCacheDir("1.2.3")
	if err != nil {
		t.Fatalf("resolveCacheDir: %v", err)
	}
	if !strings.Contains(got, "1.2.3") {
		t.Fatalf("resolveCacheDir: result %q should contain version", got)
	}
	if !strings.Contains(got, goruntime.GOOS+"-"+goruntime.GOARCH) {
		t.Fatalf("resolveCacheDir: result %q should contain platform", got)
	}
}

func TestResolveCacheDir_OverrideWhitespace(t *testing.T) {
	// Whitespace-only override should be ignored; fall back to user cache.
	t.Setenv(envRuntimeCache, "   ")

	got, err := resolveCacheDir("1.0.0")
	if err != nil {
		t.Fatalf("resolveCacheDir: %v", err)
	}
	if strings.TrimSpace(got) != got {
		t.Fatalf("resolveCacheDir: result %q should not have whitespace", got)
	}
}

// ---------------------------------------------------------------------------
// resolvePath — override via env var
// ---------------------------------------------------------------------------

func TestResolvePath_Override_FileExists(t *testing.T) {
	overridePath := filepath.Join(t.TempDir(), "pit_ffi")
	if err := os.WriteFile(overridePath, []byte("fake-runtime"), 0o755); err != nil {
		t.Fatalf("write override file: %v", err)
	}

	t.Setenv(envRuntimePath, overridePath)

	got, err := resolvePath()
	if err != nil {
		t.Fatalf("resolvePath: %v", err)
	}
	if got != overridePath {
		t.Fatalf("resolvePath: got %q, want %q", got, overridePath)
	}
}

func TestResolvePath_Override_FileMissing(t *testing.T) {
	t.Setenv(envRuntimePath, "/nonexistent/pit_ffi.so")

	_, err := resolvePath()
	if err == nil {
		t.Fatal("resolvePath: want error for missing override, got nil")
	}
}

func TestResolvePath_Override_Whitespace_Ignored(t *testing.T) {
	// Whitespace-only env var must not be treated as a path.
	t.Setenv(envRuntimePath, "   ")
	t.Setenv(envRuntimeCache, t.TempDir())

	_, err := resolvePath()
	// We don't assert success/failure (depends on embedded data).
	// We assert the error, if any, does not reference the whitespace string.
	if err != nil && strings.Contains(err.Error(), `"   "`) {
		t.Fatalf("resolvePath treated whitespace as override path: %v", err)
	}
}

// ---------------------------------------------------------------------------
// resolvePath — cache hit
// ---------------------------------------------------------------------------

func TestResolvePath_CacheHit(t *testing.T) {
	fileName, err := pitruntime.GetName()
	if err != nil {
		t.Skipf("unsupported platform: %v", err)
	}

	cacheRoot := t.TempDir()
	t.Setenv(envRuntimePath, "")
	t.Setenv(envRuntimeCache, cacheRoot)

	cacheDir, err := resolveCacheDir(version)
	if err != nil {
		t.Fatalf("resolveCacheDir: %v", err)
	}
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}

	targetPath := filepath.Join(cacheDir, fileName)
	originalContent := []byte("already-cached-runtime")
	if err := os.WriteFile(targetPath, originalContent, 0o755); err != nil {
		t.Fatalf("write cached file: %v", err)
	}

	got, err := resolvePath()
	if err != nil {
		t.Fatalf("resolvePath: %v", err)
	}
	if got != targetPath {
		t.Fatalf("resolvePath: got %q, want %q", got, targetPath)
	}

	// Cached file must not be overwritten.
	afterContent, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("read cached file: %v", err)
	}
	if string(afterContent) != string(originalContent) {
		t.Fatalf("cached content was modified: got %q, want %q", afterContent, originalContent)
	}
}

// ---------------------------------------------------------------------------
// write
// ---------------------------------------------------------------------------

func TestWrite_CreatesFile(t *testing.T) {
	dir := t.TempDir()
	targetPath := filepath.Join(dir, "libpit_ffi.so")
	data := []byte("test-library-data")

	if err := write(targetPath, data, 0o755); err != nil {
		t.Fatalf("write: %v", err)
	}

	got, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("read result: %v", err)
	}
	if string(got) != string(data) {
		t.Fatalf("write content: got %q, want %q", got, data)
	}
}

func TestWrite_Permissions(t *testing.T) {
	if goruntime.GOOS == "windows" {
		t.Skip("permission bits not applicable on Windows")
	}

	dir := t.TempDir()
	targetPath := filepath.Join(dir, "libpit_ffi.so")

	if err := write(targetPath, []byte("data"), 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}

	info, err := os.Stat(targetPath)
	if err != nil {
		t.Fatalf("stat: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o644 {
		t.Fatalf("write permissions: got %o, want %o", got, 0o644)
	}
}

func TestWrite_Idempotent(t *testing.T) {
	dir := t.TempDir()
	targetPath := filepath.Join(dir, "libpit_ffi.so")
	data := []byte("library-data")

	if err := write(targetPath, data, 0o755); err != nil {
		t.Fatalf("first write: %v", err)
	}
	if err := write(targetPath, data, 0o755); err != nil {
		t.Fatalf("second write: %v", err)
	}

	got, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if string(got) != string(data) {
		t.Fatalf("content after second write: got %q, want %q", got, data)
	}
}

func TestWrite_LeavesNoTempFile(t *testing.T) {
	dir := t.TempDir()
	targetPath := filepath.Join(dir, "libpit_ffi.so")

	if err := write(targetPath, []byte("data"), 0o755); err != nil {
		t.Fatalf("write: %v", err)
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("readdir: %v", err)
	}
	if len(entries) != 1 {
		names := make([]string, 0, len(entries))
		for _, e := range entries {
			names = append(names, e.Name())
		}
		t.Fatalf("write left temp files: %v", names)
	}
}

func TestResolvePath_CacheMissWritesAndSecondCallUsesCache(t *testing.T) {
	if forcedPath, usingOverride := runtimeOverridePathIfEmbeddedUnavailable(t); usingOverride {
		t.Setenv(envRuntimePath, forcedPath)
		got, err := resolvePath()
		if err != nil {
			t.Fatalf("resolvePath() with override %q error = %v", forcedPath, err)
		}
		if got != forcedPath {
			t.Fatalf("resolvePath() with override = %q, want %q", got, forcedPath)
		}
		return
	}

	fileName, err := pitruntime.GetName()
	if err != nil {
		t.Skipf("unsupported platform: %v", err)
	}

	cacheRoot := t.TempDir()
	t.Setenv(envRuntimePath, "")
	t.Setenv(envRuntimeCache, cacheRoot)

	firstPath, err := resolvePath()
	if err != nil {
		t.Fatalf("first resolvePath() error = %v", err)
	}
	if filepath.Base(firstPath) != fileName {
		t.Fatalf("first resolvePath() file = %q, want %q", filepath.Base(firstPath), fileName)
	}
	firstData, err := os.ReadFile(firstPath)
	if err != nil {
		t.Fatalf("ReadFile(firstPath) error = %v", err)
	}
	if len(firstData) == 0 {
		t.Fatal("ReadFile(firstPath) len = 0, want non-zero runtime bytes")
	}

	secondPath, err := resolvePath()
	if err != nil {
		t.Fatalf("second resolvePath() error = %v", err)
	}
	if secondPath != firstPath {
		t.Fatalf("second resolvePath() path = %q, want %q", secondPath, firstPath)
	}
	secondData, err := os.ReadFile(secondPath)
	if err != nil {
		t.Fatalf("ReadFile(secondPath) error = %v", err)
	}
	if string(secondData) != string(firstData) {
		t.Fatal("cached runtime bytes changed between resolvePath() calls")
	}
}

func TestLoad_ReturnsErrorForCorruptCachedRuntime(t *testing.T) {
	fileName, err := pitruntime.GetName()
	if err != nil {
		t.Skipf("unsupported platform: %v", err)
	}

	cacheRoot := t.TempDir()
	t.Setenv(envRuntimePath, "")
	t.Setenv(envRuntimeCache, cacheRoot)

	cacheDir, err := resolveCacheDir(version)
	if err != nil {
		t.Fatalf("resolveCacheDir() error = %v", err)
	}
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		t.Fatalf("MkdirAll(cacheDir) error = %v", err)
	}
	targetPath := filepath.Join(cacheDir, fileName)
	if err := os.WriteFile(targetPath, []byte("not-a-shared-library"), 0o755); err != nil {
		t.Fatalf("WriteFile(targetPath) error = %v", err)
	}

	resetLoaderStateForTest()
	load()
	if loadErr == nil {
		t.Fatal("loadErr = nil, want non-nil")
	}
	if !strings.Contains(loadErr.Error(), "failed to load Pit runtime library") {
		t.Fatalf("loadErr = %q, want runtime-load failure", loadErr.Error())
	}
}

func TestLoadRuntimeLibraryReturnsErrorForInvalidPath(t *testing.T) {
	err := loadRuntimeLibrary(filepath.Join(t.TempDir(), "missing-runtime"))
	if err == nil {
		t.Fatal("loadRuntimeLibrary() error = nil, want non-nil")
	}
}

func TestLoadRuntimeLibraryLoadsResolvedPath(t *testing.T) {
	if forcedPath, usingOverride := runtimeOverridePathIfEmbeddedUnavailable(t); usingOverride {
		if err := loadRuntimeLibrary(forcedPath); err != nil {
			t.Fatalf("loadRuntimeLibrary(%q) error = %v", forcedPath, err)
		}
		return
	}

	cacheRoot := t.TempDir()
	t.Setenv(envRuntimePath, "")
	t.Setenv(envRuntimeCache, cacheRoot)

	path, err := resolvePath()
	if err != nil {
		t.Fatalf("resolvePath() error = %v", err)
	}
	if err := loadRuntimeLibrary(path); err != nil {
		t.Fatalf("loadRuntimeLibrary(%q) error = %v", path, err)
	}
}

func resetLoaderStateForTest() {
	loadOnce = sync.Once{}
	loadErr = nil
}

func runtimeOverridePathIfEmbeddedUnavailable(t *testing.T) (string, bool) {
	t.Helper()

	if _, _, err := pitruntime.Load(); err == nil {
		return "", false
	}

	forcedPath := strings.TrimSpace(os.Getenv(envRuntimePath))
	if forcedPath == "" {
		t.Fatalf("%s must be set when embedded runtime files are unavailable", envRuntimePath)
	}
	return forcedPath, true
}
