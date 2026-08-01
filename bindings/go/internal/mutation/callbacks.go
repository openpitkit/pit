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

package mutation

/*
#cgo CFLAGS: -I${SRCDIR}/../native
#include "openpit.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime/cgo"
	"unsafe"

	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/diag"
	"go.openpit.dev/openpit/internal/native"
)

type Callbacks struct {
	handle   cgo.Handle
	commit   func()
	rollback func()
}

func NewCallbacks(commit, rollback func()) *Callbacks {
	c := &Callbacks{commit: commit, rollback: rollback}
	c.handle = cgo.NewHandle(c)
	return c
}

func (c *Callbacks) Close() {
	c.handle.Delete()
}

func (c *Callbacks) Handle() unsafe.Pointer {
	return callback.NewUserDataFromHandle(c.handle)
}

//export pitMutationCommit
func pitMutationCommit(
	userData unsafe.Pointer,
	outError **C.OpenPitSharedString,
) (succeeded C.bool) {
	defer func() {
		if recovered := recover(); recovered != nil {
			writeMutationCallbackError(
				outError,
				fmt.Sprintf("Go mutation commit callback panic: %v", recovered),
			)
			succeeded = C.bool(false)
		}
	}()
	getCallbacks(userData).commit()
	return C.bool(true)
}

//export pitMutationRollback
func pitMutationRollback(
	userData unsafe.Pointer,
	outError **C.OpenPitSharedString,
) (succeeded C.bool) {
	defer func() {
		if recovered := recover(); recovered != nil {
			writeMutationCallbackError(
				outError,
				fmt.Sprintf("Go mutation rollback callback panic: %v", recovered),
			)
			succeeded = C.bool(false)
		}
	}()
	getCallbacks(userData).rollback()
	return C.bool(true)
}

//export pitMutationFree
func pitMutationFree(userData unsafe.Pointer) {
	// Free carries no result, so a panic here has no reject to travel on. Keep it
	// contained so one bad handle cannot unwind across the cgo frame, and report
	// it through the SDK diagnostic sink instead of the embedder's logger.
	handle := callback.NewHandleFromUserData(userData)
	defer func() {
		if recovered := recover(); recovered != nil {
			diag.Report(fmt.Errorf("mutation free callback panicked: %v", recovered))
		}
	}()
	callbacks, ok := handle.Value().(*Callbacks)
	if !ok {
		// The handle is still allocated, and no Callbacks.Close will ever reach
		// it, so release it here rather than leak it.
		handle.Delete()
		diag.Report(errors.New("mutation free callback got foreign user data"))
		return
	}
	callbacks.Close()
}

func getCallbacks(userData unsafe.Pointer) *Callbacks {
	callbacks, ok := callback.NewHandleFromUserData(userData).Value().(*Callbacks)
	if !ok {
		panic("user data is not a mutation callback set")
	}
	return callbacks
}

func writeMutationCallbackError(outError **C.OpenPitSharedString, message string) {
	if outError == nil {
		return
	}
	*outError = (*C.OpenPitSharedString)(unsafe.Pointer(native.CreateSharedString(message)))
}
