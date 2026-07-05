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

package loader

import (
	"errors"
	"os"
	"path/filepath"
	"regexp"
	goruntime "runtime"
	"strings"
	"sync"
	"testing"

	pitruntime "go.openpit.dev/openpit/internal/runtime"
)

func TestSDKVersionMatchesOpenpitCrateVersionAndReleaseReplacement(t *testing.T) {
	loaderSource, err := os.ReadFile("loader.go")
	if err != nil {
		t.Fatalf("read loader.go: %v", err)
	}

	sdkVersionMatch := regexp.MustCompile(`const SDKVersion = "([^"]+)"`).FindSubmatch(loaderSource)
	if sdkVersionMatch == nil {
		t.Fatal("loader.go must declare const SDKVersion")
	}
	if got := string(sdkVersionMatch[1]); got != SDKVersion {
		t.Fatalf("loader.go SDKVersion = %q, want %q", got, SDKVersion)
	}

	cargoTomlPath := filepath.Join("..", "..", "..", "..", "crates", "openpit", "Cargo.toml")
	cargoToml, err := os.ReadFile(cargoTomlPath) //nolint:gosec // path is a known repository-relative literal
	if err != nil {
		t.Skipf("Cargo.toml not found (standalone repo context): %v", err)
	}

	crateVersionMatch := regexp.MustCompile(`(?m)^version = "([^"]+)"`).FindSubmatch(cargoToml)
	if crateVersionMatch == nil {
		t.Fatalf("%s must declare package version", cargoTomlPath)
	}
	if got := string(crateVersionMatch[1]); got != SDKVersion {
		t.Fatalf("openpit crate version = %q, want Go SDKVersion %q", got, SDKVersion)
	}

	for _, want := range []string{
		`file = "../../bindings/go/internal/loader/loader.go"`,
		`search = 'const SDKVersion = "[0-9]+\.[0-9]+\.[0-9]+"'`,
		`replace = "const SDKVersion = \"{{version}}\""`,
		`exactly = 1`,
	} {
		if !strings.Contains(string(cargoToml), want) {
			t.Fatalf("%s cargo-release replacement must contain %q", cargoTomlPath, want)
		}
	}
}

// ---------------------------------------------------------------------------
// ensureVersionedPath
// ---------------------------------------------------------------------------

func TestEnsureVersionedPath_Valid(t *testing.T) {
	cases := []struct {
		version string
		path    string
	}{
		{"0.2.0", filepath.Join("/cache", "pit-go", "0.2.0", "linux-amd64", "libopenpit_ffi.so")},
		{"1.0.0", filepath.Join("/cache", "1.0.0", "libopenpit_ffi.so")},
		{"0.2.0", filepath.Join("/cache", "pit-go", "0.2.0", "linux-amd64", "libopenpit_ffi.so")},
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
			filepath.Join("/cache", "pit-go", "linux-amd64", "libopenpit_ffi.so"),
		},
		{
			"empty version",
			"",
			filepath.Join("/cache", "pit-go", "linux-amd64", "libopenpit_ffi.so"),
		},
		{
			"version of slashes only",
			"///",
			filepath.Join("/cache", "///", "linux-amd64", "libopenpit_ffi.so"),
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
	overridePath := filepath.Join(t.TempDir(), "openpit")
	if err := os.WriteFile(overridePath, []byte("fake-runtime"), 0o755); err != nil { //nolint:gosec // test fixture, permission bits intentional
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
	t.Setenv(envRuntimePath, "/nonexistent/openpit.so")

	_, err := resolvePath()
	if err == nil {
		t.Fatal("resolvePath: want error for missing override, got nil")
	}
	if !errors.Is(err, errOverrideStatFailed) {
		t.Fatalf("resolvePath: want errOverrideStatFailed, got %v", err)
	}
}

func TestResolvePath_Override_RelativeRejected(t *testing.T) {
	t.Setenv(envRuntimePath, "openpit.so")

	_, err := resolvePath()
	if err == nil {
		t.Fatal("resolvePath: want error for relative override, got nil")
	}
	if !errors.Is(err, errOverrideStatFailed) {
		t.Fatalf("resolvePath: want errOverrideStatFailed, got %v", err)
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

	contentHash, err := pitruntime.Hash()
	if err != nil {
		t.Fatalf("pitruntime.Hash: %v", err)
	}

	cacheDir, err := resolveCacheDir(SDKVersion)
	if err != nil {
		t.Fatalf("resolveCacheDir: %v", err)
	}
	hashDir := filepath.Join(cacheDir, contentHash)
	if err := os.MkdirAll(hashDir, 0o755); err != nil { //nolint:gosec // test setup, exact mode not security-sensitive
		t.Fatalf("mkdir: %v", err)
	}

	targetPath := filepath.Join(hashDir, fileName)
	originalContent := []byte("already-cached-runtime")
	if err := os.WriteFile(targetPath, originalContent, 0o755); err != nil { //nolint:gosec // test fixture, permission bits intentional
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
	afterContent, err := os.ReadFile(targetPath) //nolint:gosec // path derived from known test temp dir
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
	targetPath := filepath.Join(dir, "libopenpit_ffi.so")
	data := []byte("test-library-data")

	if err := write(targetPath, data, 0o755); err != nil {
		t.Fatalf("write: %v", err)
	}

	got, err := os.ReadFile(targetPath) //nolint:gosec // path derived from known test temp dir
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
	targetPath := filepath.Join(dir, "libopenpit_ffi.so")

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
	targetPath := filepath.Join(dir, "libopenpit_ffi.so")
	data := []byte("library-data")

	if err := write(targetPath, data, 0o755); err != nil {
		t.Fatalf("first write: %v", err)
	}
	if err := write(targetPath, data, 0o755); err != nil {
		t.Fatalf("second write: %v", err)
	}

	got, err := os.ReadFile(targetPath) //nolint:gosec // path derived from known test temp dir
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if string(got) != string(data) {
		t.Fatalf("content after second write: got %q, want %q", got, data)
	}
}

func TestWrite_LeavesNoTempFile(t *testing.T) {
	dir := t.TempDir()
	targetPath := filepath.Join(dir, "libopenpit_ffi.so")

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
	firstData, err := os.ReadFile(firstPath) //nolint:gosec // path from resolvePath, not user input
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
	secondData, err := os.ReadFile(secondPath) //nolint:gosec // path from resolvePath, not user input
	if err != nil {
		t.Fatalf("ReadFile(secondPath) error = %v", err)
	}
	if string(secondData) != string(firstData) {
		t.Fatal("cached runtime bytes changed between resolvePath() calls")
	}
}

// ---------------------------------------------------------------------------
// load — panic contract
// ---------------------------------------------------------------------------

func TestLoad_PanicsForCorruptCachedRuntime(t *testing.T) {
	fileName, err := pitruntime.GetName()
	if err != nil {
		t.Skipf("unsupported platform: %v", err)
	}

	cacheRoot := t.TempDir()
	t.Setenv(envRuntimePath, "")
	t.Setenv(envRuntimeCache, cacheRoot)

	contentHash, err := pitruntime.Hash()
	if err != nil {
		t.Fatalf("pitruntime.Hash() error = %v", err)
	}

	cacheDir, err := resolveCacheDir(SDKVersion)
	if err != nil {
		t.Fatalf("resolveCacheDir() error = %v", err)
	}
	hashDir := filepath.Join(cacheDir, contentHash)
	if err := os.MkdirAll(hashDir, 0o755); err != nil { //nolint:gosec // test setup, exact mode not security-sensitive
		t.Fatalf("MkdirAll(hashDir) error = %v", err)
	}
	targetPath := filepath.Join(hashDir, fileName)
	if err := os.WriteFile(targetPath, []byte("not-a-shared-library"), 0o755); err != nil { //nolint:gosec // test fixture, permission bits intentional
		t.Fatalf("WriteFile(targetPath) error = %v", err)
	}

	resetLoaderStateForTest()
	loadErr := recoverLoad(t)
	if loadErr == nil {
		t.Fatal("load() did not panic, want *RuntimeLoadError")
	}
	if loadErr.Reason != ReasonMagicCheckFailed {
		t.Fatalf("load() panic reason = %q, want %q",
			loadErr.Reason, ReasonMagicCheckFailed)
	}
	if loadErr.Path != targetPath {
		t.Fatalf("load() panic path = %q, want %q", loadErr.Path, targetPath)
	}
	if loadErr.Cause == nil {
		t.Fatal("load() panic Cause = nil, want underlying magic-check error")
	}
}

func TestLoad_PanicsForUnresolvableOverride(t *testing.T) {
	t.Setenv(envRuntimePath, "/nonexistent/openpit.so")

	resetLoaderStateForTest()
	loadErr := recoverLoad(t)
	if loadErr == nil {
		t.Fatal("load() did not panic, want *RuntimeLoadError")
	}
	if loadErr.Reason != ReasonOverrideStatFailed {
		t.Fatalf("load() panic reason = %q, want %q",
			loadErr.Reason, ReasonOverrideStatFailed)
	}
	if !errors.Is(loadErr, errOverrideStatFailed) {
		t.Fatalf("errors.Is(load panic, errOverrideStatFailed) = false; got %v", loadErr.Cause)
	}
}

func TestLoad_NoPanicAndSecondInitIsNoop(t *testing.T) {
	forcedPath := strings.TrimSpace(os.Getenv(envRuntimePath))
	if forcedPath == "" {
		t.Skipf("%s not set; package init has already loaded the runtime", envRuntimePath)
	}

	resetLoaderStateForTest()
	if loadErr := recoverLoad(t); loadErr != nil {
		t.Fatalf("load() panicked unexpectedly: %v", loadErr)
	}
	if LoadedPath() == "" {
		t.Fatal("LoadedPath() empty after successful load()")
	}

	// Second init through loadOnce must be a no-op.
	prev := LoadedPath()
	loadOnce.Do(func() {
		t.Fatal("loadOnce.Do invoked again after a successful load")
	})
	if LoadedPath() != prev {
		t.Fatalf("LoadedPath changed after no-op re-init: was %q, now %q", prev, LoadedPath())
	}
}

// ---------------------------------------------------------------------------
// RuntimeLoadError
// ---------------------------------------------------------------------------

func TestRuntimeLoadError_FieldsAndUnwrap(t *testing.T) {
	cause := errors.New("underlying")
	e := &RuntimeLoadError{
		Reason: ReasonDlopenFailed,
		Path:   "/tmp/libopenpit.so",
		Cause:  cause,
	}

	if e.Reason != ReasonDlopenFailed {
		t.Fatalf("Reason = %q, want %q", e.Reason, ReasonDlopenFailed)
	}
	if e.Path != "/tmp/libopenpit.so" {
		t.Fatalf("Path = %q, want %q", e.Path, "/tmp/libopenpit.so")
	}
	if errors.Unwrap(e) != cause { //nolint:errorlint // testing Unwrap directly, not just errors.Is
		t.Fatalf("errors.Unwrap = %v, want %v", errors.Unwrap(e), cause)
	}
	if !errors.Is(e, cause) {
		t.Fatal("errors.Is(e, cause) = false, want true")
	}
	if !strings.Contains(e.Error(), ReasonDlopenFailed) {
		t.Fatalf("Error() = %q, want it to contain Reason", e.Error())
	}
	if !strings.Contains(e.Error(), "/tmp/libopenpit.so") {
		t.Fatalf("Error() = %q, want it to contain Path", e.Error())
	}
	if !strings.Contains(e.Error(), "underlying") {
		t.Fatalf("Error() = %q, want it to contain underlying cause", e.Error())
	}
}

// ---------------------------------------------------------------------------
// loadRuntimeLibrary
// ---------------------------------------------------------------------------

func TestLoadRuntimeLibraryReturnsErrorForInvalidPath(t *testing.T) {
	_, err := loadRuntimeLibrary(filepath.Join(t.TempDir(), "missing-runtime"))
	if err == nil {
		t.Fatal("loadRuntimeLibrary() error = nil, want non-nil")
	}
}

func TestLoadRuntimeLibraryLoadsResolvedPath(t *testing.T) {
	if forcedPath, usingOverride := runtimeOverridePathIfEmbeddedUnavailable(t); usingOverride {
		if _, err := loadRuntimeLibrary(forcedPath); err != nil {
			t.Fatalf("loadRuntimeLibrary(%q) error = %v", forcedPath, err)
		}
		return
	}

	cacheRoot := runtimeLoadTestCacheDir(t)
	t.Setenv(envRuntimePath, "")
	t.Setenv(envRuntimeCache, cacheRoot)

	path, err := resolvePath()
	if err != nil {
		t.Fatalf("resolvePath() error = %v", err)
	}
	if _, err := loadRuntimeLibrary(path); err != nil {
		t.Fatalf("loadRuntimeLibrary(%q) error = %v", path, err)
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

func resetLoaderStateForTest() {
	loadOnce = sync.Once{}
	loadedPath = ""
	loadedHandle = nil
}

func recoverLoad(t *testing.T) (out *RuntimeLoadError) {
	t.Helper()
	defer func() {
		r := recover()
		if r == nil {
			return
		}
		rle, ok := r.(*RuntimeLoadError)
		if !ok {
			t.Fatalf("load() panicked with non-*RuntimeLoadError value: %#v", r)
		}
		out = rle
	}()
	loadOnce.Do(load)
	return nil
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

func runtimeLoadTestCacheDir(t *testing.T) string {
	t.Helper()

	if goruntime.GOOS != "windows" {
		return t.TempDir()
	}
	dir, err := os.MkdirTemp("", "openpit-loader-test-*")
	if err != nil {
		t.Fatalf("MkdirTemp: %v", err)
	}
	t.Cleanup(func() { _ = os.RemoveAll(dir) })
	return dir
}
