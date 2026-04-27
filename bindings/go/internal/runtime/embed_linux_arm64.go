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

//go:build linux && arm64

package runtime

import "embed"

const name = "libpit_ffi.so"

//go:embed all:linux-arm64
var embeddedFS embed.FS

func load() ([]byte, string, error) {
	data, err := embeddedFS.ReadFile("linux-arm64/" + name)
	if err != nil {
		return nil, name, err
	}
	return data, name, nil
}
