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
	"errors"

	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/tx/internal/mutation"
)

type Mutations struct{ handle native.Mutations }

func NewMutationsFromHandle(handle native.Mutations) Mutations {
	return Mutations{handle: handle}
}

// Push registers one mutation with commit and rollback callbacks.
//
// Exactly one of commit or rollback is called by the engine, followed by the
// free callback.
func (m Mutations) Push(commit, rollback func()) error {
	if commit == nil {
		return errors.New("mutation commit callback is nil")
	}
	if rollback == nil {
		return errors.New("mutation rollback callback is nil")
	}

	callbacks := mutation.NewCallbacks(commit, rollback)
	if err := native.MutationsPush(
		m.handle,
		mutation.GetCommitFnAddr(),
		mutation.GetRollbackFnAddr(),
		callbacks.Handle(),
		mutation.GetFreeFnAddr(),
	); err != nil {
		callbacks.Close()
		return err
	}

	return nil
}
