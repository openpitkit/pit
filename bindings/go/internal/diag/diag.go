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

// Package diag routes SDK failures that have no result channel to carry them.
//
// A teardown callback invoked by the native engine returns nothing, so a panic
// or a foreign user-data pointer inside it leaves a leaked cgo handle and no
// trace. The sink lets an embedder observe those failures without the SDK
// writing to the process-wide logger of a host it does not own.
package diag

import "sync/atomic"

// sink holds the installed handler. A nil pointer means no handler, which is
// the default: the SDK stays silent until an embedder asks for the reports.
var sink atomic.Pointer[func(error)]

// SetHandler installs handler as the process-wide teardown-failure sink. A nil
// handler restores the silent default.
func SetHandler(handler func(error)) {
	if handler == nil {
		sink.Store(nil)
		return
	}
	sink.Store(&handler)
}

// Report hands err to the installed handler, if any. A handler that panics is
// contained: Report runs on cgo teardown frames that must not unwind.
func Report(err error) {
	handler := sink.Load()
	if handler == nil {
		return
	}
	defer func() { _ = recover() }()
	(*handler)(err)
}
