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

// Package loader resolves and loads the native OpenPit runtime library during
// package initialization. On any failure the process aborts via panic with a
// typed *RuntimeLoadError; callers are not expected to recover.
package loader

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	goruntime "runtime"
	"strings"
	"sync"
	"unsafe"

	pitruntime "go.openpit.dev/openpit/internal/runtime"
)

// SDKVersion is the version baked into the Go SDK source tree. It is used both
// for runtime cache namespacing and for the runtime/SDK compatibility check.
const SDKVersion = "0.3.0"

const (
	envRuntimePath  = "OPENPIT_RUNTIME_LIBRARY_PATH"
	envRuntimeCache = "OPENPIT_RUNTIME_CACHE_DIR"
)

const (
	cacheDirMode  = 0o750 // group-read, no world access
	cacheFileMode = 0o755 // executable shared library
)

// Reason codes attached to *RuntimeLoadError. They are stable strings that
// callers (and crash-log readers) can match against.
const (
	ReasonOverrideStatFailed = "override path stat failed"
	ReasonEmbedNotFound      = "embedded runtime not found for platform"
	ReasonCacheResolveFailed = "runtime cache directory resolution failed"
	ReasonCacheWriteFailed   = "runtime cache write failed"
	ReasonMagicCheckFailed   = "shared library magic check failed"
	ReasonDlopenFailed       = "dlopen failed"
	ReasonSymbolNotFound     = "runtime symbol not found"
	ReasonVersionMismatch    = "runtime version mismatch"
)

// RuntimeLoadError is the typed panic value produced by the loader on any
// failure during package initialization.
type RuntimeLoadError struct {
	Reason string
	Path   string
	Cause  error
}

func (e *RuntimeLoadError) Error() string {
	switch {
	case e.Path != "" && e.Cause != nil:
		return fmt.Sprintf("%s: path=%q: %v", e.Reason, e.Path, e.Cause)
	case e.Path != "":
		return fmt.Sprintf("%s: path=%q", e.Reason, e.Path)
	case e.Cause != nil:
		return fmt.Sprintf("%s: %v", e.Reason, e.Cause)
	default:
		return e.Reason
	}
}

func (e *RuntimeLoadError) Unwrap() error { return e.Cause }

// Sentinel errors wrapped by resolvePath / loadRuntimeLibrary so that the
// panic site in load() can map a Cause back to a stable Reason code without
// changing the existing (path, error) return signatures.
var (
	errOverrideStatFailed = errors.New(ReasonOverrideStatFailed)
	errEmbedNotFound      = errors.New(ReasonEmbedNotFound)
	errCacheResolveFailed = errors.New(ReasonCacheResolveFailed)
	errCacheWriteFailed   = errors.New(ReasonCacheWriteFailed)
	errMagicCheckFailed   = errors.New(ReasonMagicCheckFailed)
)

func reasonForResolveError(err error) string {
	switch {
	case errors.Is(err, errOverrideStatFailed):
		return ReasonOverrideStatFailed
	case errors.Is(err, errEmbedNotFound):
		return ReasonEmbedNotFound
	case errors.Is(err, errCacheWriteFailed):
		return ReasonCacheWriteFailed
	case errors.Is(err, errCacheResolveFailed):
		return ReasonCacheResolveFailed
	default:
		return ReasonCacheResolveFailed
	}
}

var (
	loadOnce     sync.Once
	loadedPath   string
	loadedHandle unsafe.Pointer
)

func init() {
	loadOnce.Do(load)
}

// LoadedPath returns the filesystem path of the runtime library that the
// loader opened during package initialization. Useful for diagnostics such as
// the runtime/SDK version mismatch check in internal/native.
func LoadedPath() string {
	return loadedPath
}

// LoadedHandle returns the dlopen handle of the runtime library that the
// loader opened during package initialization. Used by the native package to
// resolve openpit_* symbols via dlsym instead of relying on PLT entries.
func LoadedHandle() unsafe.Pointer {
	return loadedHandle
}

func load() {
	path, err := resolvePath()
	if err != nil {
		panic(&RuntimeLoadError{Reason: reasonForResolveError(err), Path: path, Cause: err})
	}
	handle, err := loadRuntimeLibrary(path)
	if err != nil {
		reason := ReasonDlopenFailed
		if errors.Is(err, errMagicCheckFailed) {
			reason = ReasonMagicCheckFailed
		}
		panic(&RuntimeLoadError{Reason: reason, Path: path, Cause: err})
	}
	loadedPath = path
	loadedHandle = handle
}

func resolvePath() (string, error) {
	if forcedPath := strings.TrimSpace(os.Getenv(envRuntimePath)); forcedPath != "" {
		forcedPath = filepath.Clean(forcedPath)
		if !filepath.IsAbs(forcedPath) {
			return forcedPath, fmt.Errorf("%w: override path %q must be absolute",
				errOverrideStatFailed, forcedPath)
		}
		// #nosec G703 -- this path is an explicit runtime override.
		if _, err := os.Stat(forcedPath); err != nil {
			return forcedPath, fmt.Errorf("%w: failed to stat override path %q: %w",
				errOverrideStatFailed, forcedPath, err)
		}
		return forcedPath, nil
	}

	fileName, err := pitruntime.GetName()
	if err != nil {
		return "", fmt.Errorf("%w: %w", errEmbedNotFound, err)
	}

	cacheDir, err := resolveCacheDir(SDKVersion)
	if err != nil {
		return "", fmt.Errorf("%w: %w", errCacheResolveFailed, err)
	}
	targetPath := filepath.Join(cacheDir, fileName)
	if err := ensureVersionedPath(SDKVersion, targetPath); err != nil {
		return targetPath, fmt.Errorf("%w: %w", errCacheResolveFailed, err)
	}
	if _, statErr := os.Stat(targetPath); statErr == nil {
		return targetPath, nil
	} else if !os.IsNotExist(statErr) {
		return targetPath, fmt.Errorf("%w: failed to stat file %q: %w",
			errCacheResolveFailed, targetPath, statErr)
	}

	data, embeddedName, err := pitruntime.Load()
	if err != nil {
		return "", fmt.Errorf(
			"%w: not found for %s/%s, set %s to override: %w",
			errEmbedNotFound,
			goruntime.GOOS,
			goruntime.GOARCH,
			envRuntimePath,
			err,
		)
	}
	if embeddedName != "" && embeddedName != fileName {
		return targetPath, fmt.Errorf("%w: filename mismatch: expected %q, got %q",
			errEmbedNotFound, fileName, embeddedName)
	}

	if err := os.MkdirAll(cacheDir, cacheDirMode); err != nil {
		return targetPath, fmt.Errorf("%w: failed to create cache dir %q: %w",
			errCacheWriteFailed, cacheDir, err)
	}

	if err := write(targetPath, data, cacheFileMode); err != nil {
		return targetPath, fmt.Errorf("%w: %w", errCacheWriteFailed, err)
	}

	return targetPath, nil
}

func resolveCacheDir(version string) (string, error) {
	if override := strings.TrimSpace(os.Getenv(envRuntimeCache)); override != "" {
		return filepath.Join(override, version, goruntime.GOOS+"-"+goruntime.GOARCH), nil
	}
	userCacheDir, err := os.UserCacheDir()
	if err != nil {
		return "", fmt.Errorf("failed to resolve user cache dir: %w", err)
	}
	return filepath.Join(userCacheDir, "pit-go", version, goruntime.GOOS+"-"+goruntime.GOARCH), nil
}

func ensureVersionedPath(version, targetPath string) error {
	path := filepath.ToSlash(filepath.Clean(targetPath))
	normalizedVersion := strings.Trim(version, "/")
	if normalizedVersion == "" {
		return fmt.Errorf("version '%s' is empty or contains invalid characters", version)
	}
	versionMarker := "/" + normalizedVersion + "/"
	if !strings.Contains(path, versionMarker) {
		return fmt.Errorf("cache path %q must contain version marker %q", targetPath, versionMarker)
	}
	return nil
}

func write(targetPath string, data []byte, fileMode os.FileMode) error {
	targetDir := filepath.Dir(targetPath)
	tmpFile, err := os.CreateTemp(targetDir, "."+filepath.Base(targetPath)+".tmp-")
	if err != nil {
		return fmt.Errorf("create temp runtime file in %q: %w", targetDir, err)
	}

	tmpPath := tmpFile.Name()
	removeTmp := true
	defer func() {
		if removeTmp {
			_ = os.Remove(tmpPath)
		}
	}()

	if _, err := tmpFile.Write(data); err != nil {
		_ = tmpFile.Close()
		return fmt.Errorf("write temp file %q: %w", tmpPath, err)
	}
	if err := tmpFile.Chmod(fileMode); err != nil {
		_ = tmpFile.Close()
		return fmt.Errorf("chmod temp file %q: %w", tmpPath, err)
	}
	if err := tmpFile.Close(); err != nil {
		return fmt.Errorf("close temp file %q: %w", tmpPath, err)
	}
	if err := os.Rename(tmpPath, targetPath); err != nil {
		if _, statErr := os.Stat(targetPath); statErr == nil {
			return nil
		}
		return fmt.Errorf("rename temp file %q -> %q: %w", tmpPath, targetPath, err)
	}

	removeTmp = false
	return nil
}
