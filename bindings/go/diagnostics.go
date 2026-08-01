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

package openpit

import "go.openpit.dev/openpit/internal/diag"

// SetTeardownFailureHandler installs handler as the process-wide sink for
// failures the SDK detects while the engine releases a Go callback set: a panic
// raised by a custom policy's Close, or a teardown callback reached with user
// data the SDK did not create. Such a callback returns nothing to the engine,
// so without a handler the failure - and the cgo handle it leaks - leaves no
// trace at all.
//
// The default is silent. Pass nil to restore it. handler is called from the
// goroutine running the teardown, must not block, and must not panic; a panic
// inside it is discarded rather than unwound across the native frame.
func SetTeardownFailureHandler(handler func(error)) {
	diag.SetHandler(handler)
}
