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

extern bool pitMutationCommit(
	void* user_data,
	OpenPitSharedString** out_error
);
extern bool pitMutationRollback(
	void* user_data,
	OpenPitSharedString** out_error
);
extern void pitMutationFree(void* user_data);

static OpenPitMutationFn openpit_mutation_commit_fn = pitMutationCommit;
static OpenPitMutationFn openpit_mutation_rollback_fn = pitMutationRollback;
static OpenPitMutationFreeFn openpit_mutation_free_fn = pitMutationFree;

static inline OpenPitMutationFn* openpit_mutation_commit_fn_addr(void) {
	return &openpit_mutation_commit_fn;
}

static inline OpenPitMutationFn* openpit_mutation_rollback_fn_addr(void) {
	return &openpit_mutation_rollback_fn;
}

static inline OpenPitMutationFreeFn* openpit_mutation_free_fn_addr(void) {
	return &openpit_mutation_free_fn;
}
*/
import "C"
import "unsafe"

func GetCommitFnAddr() unsafe.Pointer {
	return unsafe.Pointer(C.openpit_mutation_commit_fn_addr())
}

func GetRollbackFnAddr() unsafe.Pointer {
	return unsafe.Pointer(C.openpit_mutation_rollback_fn_addr())
}

func GetFreeFnAddr() unsafe.Pointer {
	return unsafe.Pointer(C.openpit_mutation_free_fn_addr())
}
