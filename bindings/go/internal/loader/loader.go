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
	"os"
	"path/filepath"
	goruntime "runtime"
	"strings"
	"sync"

	pitruntime "github.com/openpitkit/pit-go/internal/runtime"
)

const version = "0.2.1"

const (
	envRuntimePath  = "OPENPIT_RUNTIME_LIBRARY_PATH"
	envRuntimeCache = "OPENPIT_RUNTIME_CACHE_DIR"
)

var (
	loadOnce sync.Once
	loadErr  error
)

func EnsureRuntimeLoaded() error {
	loadOnce.Do(load)
	return loadErr
}

func load() {
	path, err := resolvePath()
	if err != nil {
		loadErr = fmt.Errorf("failed to check Pit runtime library %q: %w", path, err)
		return
	}
	if err := loadRuntimeLibrary(path); err != nil {
		loadErr = fmt.Errorf("failed to load Pit runtime library %q: %w", path, err)
	}
}

func resolvePath() (string, error) {
	if forcedPath := strings.TrimSpace(os.Getenv(envRuntimePath)); forcedPath != "" {
		if _, err := os.Stat(forcedPath); err != nil {
			return "", fmt.Errorf("failed to stat override path %q: %w", forcedPath, err)
		}
		return forcedPath, nil
	}

	fileName, err := pitruntime.GetName()
	if err != nil {
		return "", err
	}

	cacheDir, err := resolveCacheDir(version)
	if err != nil {
		return "", err
	}
	targetPath := filepath.Join(cacheDir, fileName)
	if err := ensureVersionedPath(version, targetPath); err != nil {
		return "", err
	}
	if _, statErr := os.Stat(targetPath); statErr == nil {
		return targetPath, nil
	} else if !os.IsNotExist(statErr) {
		return "", fmt.Errorf("failed to stat file %q: %w", targetPath, statErr)
	}

	data, embeddedName, err := pitruntime.Load()
	if err != nil {
		return "", fmt.Errorf(
			"not found for %s/%s, set %s to override: %w",
			goruntime.GOOS,
			goruntime.GOARCH,
			envRuntimePath,
			err,
		)
	}
	if embeddedName != "" && embeddedName != fileName {
		return "", fmt.Errorf("filename mismatch: expected %q, got %q", fileName, embeddedName)
	}

	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		return "", fmt.Errorf("failed to create cache dir %q: %w", cacheDir, err)
	}

	if err := write(targetPath, data, 0o755); err != nil {
		return "", err
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
