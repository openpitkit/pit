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

package runtime

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"runtime"
)

// GetName returns the name of the embedded FFI runtime library for the current platform.
func GetName() (string, error) {
	if name == "" {
		return "", fmt.Errorf("unsupported platform %s/%s", runtime.GOOS, runtime.GOARCH)
	}
	return name, nil
}

// Load returns the embedded FFI runtime library for the current platform.
// Returns library bytes, expected filename, and error if unavailable.
func Load() ([]byte, string, error) { return load() }

// Hash returns the lowercase hex SHA-256 digest of the embedded FFI runtime
// library for the current platform. The loader keys its extraction cache on
// this value so that a changed embedded artifact maps to a distinct cache
// location and a stale extraction is never reused. Computed per process from
// the embedded bytes; returns an error on unsupported platforms.
func Hash() (string, error) {
	data, _, err := load()
	if err != nil {
		return "", err
	}
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:]), nil
}
