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

package openpit

import (
	"errors"
	"testing"

	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

func newAccountsTestEngine(t *testing.T) *Engine {
	t.Helper()
	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

func TestAccountsGroupOfAbsent(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	if got := engine.Accounts().GroupOf(param.NewAccountIDFromUint64(1)); got.IsSet() {
		t.Fatalf("GroupOf(ungrouped) = %v, want empty option", got)
	}
}

func TestAccountsRegisterGroupRejectsDefaultGroup(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	err := engine.Accounts().RegisterGroup(
		[]param.AccountID{param.NewAccountIDFromUint64(1)},
		param.DefaultAccountGroup,
	)

	var groupErr *reject.AccountGroupError
	if !errors.As(err, &groupErr) {
		t.Fatalf("RegisterGroup(default) error = %v, want *reject.AccountGroupError", err)
	}
}

func TestAccountsRegisterGroupRejectsConflict(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	first, err := param.NewAccountGroupIDFromUint32(7)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32(7) error = %v", err)
	}
	if err := accounts.RegisterGroup([]param.AccountID{account}, first); err != nil {
		t.Fatalf("RegisterGroup(7) error = %v", err)
	}

	second, err := param.NewAccountGroupIDFromUint32(8)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32(8) error = %v", err)
	}
	err = accounts.RegisterGroup([]param.AccountID{account}, second)

	var groupErr *reject.AccountGroupError
	if !errors.As(err, &groupErr) {
		t.Fatalf("RegisterGroup(conflict) error = %v, want *reject.AccountGroupError", err)
	}
	if groupErr.CurrentGroup == nil || groupErr.CurrentGroup.String() != first.String() {
		t.Fatalf("CurrentGroup = %v, want %v", groupErr.CurrentGroup, first)
	}
}

func TestAccountsUnregisterGroupRejectsAbsentMember(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	err := engine.Accounts().UnregisterGroup(
		[]param.AccountID{param.NewAccountIDFromUint64(1)},
		mustAccountGroup(t, 7),
	)

	var groupErr *reject.AccountGroupError
	if !errors.As(err, &groupErr) {
		t.Fatalf("UnregisterGroup(absent) error = %v, want *reject.AccountGroupError", err)
	}
}

func mustAccountGroup(t *testing.T, value uint32) param.AccountGroupID {
	t.Helper()
	group, err := param.NewAccountGroupIDFromUint32(value)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32(%d) error = %v", value, err)
	}
	return group
}
