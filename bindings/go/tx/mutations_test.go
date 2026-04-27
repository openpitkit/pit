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

package tx

import (
	"strings"
	"testing"
)

func TestMutationsPushRequiresCommitCallback(t *testing.T) {
	err := Mutations{}.Push(nil, func() {})
	if err == nil {
		t.Fatal("expected error when commit callback is nil")
	}
	if !strings.Contains(err.Error(), "commit callback is nil") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestMutationsPushRequiresRollbackCallback(t *testing.T) {
	err := Mutations{}.Push(func() {}, nil)
	if err == nil {
		t.Fatal("expected error when rollback callback is nil")
	}
	if !strings.Contains(err.Error(), "rollback callback is nil") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestMutationsPushReturnsErrorOnInvalidMutationHandle(t *testing.T) {
	err := Mutations{}.Push(func() {}, func() {})
	if err == nil {
		t.Fatal("expected error when mutation handle is nil")
	}
	if !strings.Contains(err.Error(), "mutations is null") {
		t.Fatalf("unexpected error: %v", err)
	}
}
