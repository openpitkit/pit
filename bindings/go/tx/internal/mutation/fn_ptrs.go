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

package mutation

/*
#cgo CFLAGS: -I${SRCDIR}/../../../internal/native
#include "pit.h"

extern void pitMutationCommit(void* user_data);
extern void pitMutationRollback(void* user_data);
extern void pitMutationFree(void* user_data);

static PitMutationFn pit_mutation_commit_fn = pitMutationCommit;
static PitMutationFn pit_mutation_rollback_fn = pitMutationRollback;
static PitMutationFreeFn pit_mutation_free_fn = pitMutationFree;

static inline PitMutationFn* pit_mutation_commit_fn_addr(void) {
	return &pit_mutation_commit_fn;
}

static inline PitMutationFn* pit_mutation_rollback_fn_addr(void) {
	return &pit_mutation_rollback_fn;
}

static inline PitMutationFreeFn* pit_mutation_free_fn_addr(void) {
	return &pit_mutation_free_fn;
}
*/
import "C"
import "unsafe"

func GetCommitFnAddr() unsafe.Pointer {
	return unsafe.Pointer(C.pit_mutation_commit_fn_addr())
}

func GetRollbackFnAddr() unsafe.Pointer {
	return unsafe.Pointer(C.pit_mutation_rollback_fn_addr())
}

func GetFreeFnAddr() unsafe.Pointer {
	return unsafe.Pointer(C.pit_mutation_free_fn_addr())
}
