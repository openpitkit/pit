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
		mustAccountGroup(t),
	)

	var groupErr *reject.AccountGroupError
	if !errors.As(err, &groupErr) {
		t.Fatalf("UnregisterGroup(absent) error = %v, want *reject.AccountGroupError", err)
	}
}

func mustAccountGroup(t *testing.T) param.AccountGroupID {
	t.Helper()
	const value uint32 = 7
	group, err := param.NewAccountGroupIDFromUint32(value)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32(%d) error = %v", value, err)
	}
	return group
}

// assertAccountBlocked drives one StartPreTrade for account 1 and fails unless
// it is rejected with reject.CodeAccountBlocked.
func assertAccountBlocked(t *testing.T, engine *Engine) {
	t.Helper()
	const accountID uint64 = 1
	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, accountID))
	if err != nil {
		t.Fatalf("StartPreTrade(%d) error = %v", accountID, err)
	}
	if request != nil {
		request.Close()
		t.Fatalf("StartPreTrade(%d): request != nil, want blocked", accountID)
	}
	if len(rejects) != 1 {
		t.Fatalf("StartPreTrade(%d): reject len = %d, want 1", accountID, len(rejects))
	}
	if rejects[0].Code != reject.CodeAccountBlocked {
		t.Fatalf(
			"StartPreTrade(%d): reject code = %v, want %v",
			accountID, rejects[0].Code, reject.CodeAccountBlocked,
		)
	}
}

// assertAccountBlockedWithReason drives one StartPreTrade for account 1 and
// fails unless it is rejected with reject.CodeAccountBlocked and the operator
// reason equals wantReason.
func assertAccountBlockedWithReason(t *testing.T, engine *Engine, wantReason string) {
	t.Helper()
	const accountID uint64 = 1
	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, accountID))
	if err != nil {
		t.Fatalf("StartPreTrade(%d) error = %v", accountID, err)
	}
	if request != nil {
		request.Close()
		t.Fatalf("StartPreTrade(%d): request != nil, want blocked", accountID)
	}
	if len(rejects) != 1 {
		t.Fatalf("StartPreTrade(%d): reject len = %d, want 1", accountID, len(rejects))
	}
	if rejects[0].Code != reject.CodeAccountBlocked {
		t.Fatalf(
			"StartPreTrade(%d): reject code = %v, want %v",
			accountID, rejects[0].Code, reject.CodeAccountBlocked,
		)
	}
	if rejects[0].Reason != wantReason {
		t.Fatalf(
			"StartPreTrade(%d): reject reason = %q, want %q (first reason must win)",
			accountID, rejects[0].Reason, wantReason,
		)
	}
}

// assertAccountPasses drives one StartPreTrade for account 1 and fails unless it
// is accepted with no rejects.
func assertAccountPasses(t *testing.T, engine *Engine) {
	t.Helper()
	const accountID uint64 = 1
	request, rejects, err := engine.StartPreTrade(rateLimitTestOrder(t, accountID))
	if err != nil {
		t.Fatalf("StartPreTrade(%d) error = %v", accountID, err)
	}
	if len(rejects) != 0 {
		t.Fatalf("StartPreTrade(%d): rejects = %v, want none", accountID, rejects)
	}
	if request == nil {
		t.Fatalf("StartPreTrade(%d): request == nil, want accepted", accountID)
	}
	request.Close()
}

func TestAccountsBlockGatesPreTradeAndUnblockRestores(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)

	assertAccountPasses(t, engine)

	accounts.Block(account, "manual kill-switch")
	assertAccountBlocked(t, engine)

	accounts.Unblock(account)
	assertAccountPasses(t, engine)
}

func TestAccountsUnblockAbsentIsNoOp(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	// Unblocking an account that was never blocked must not gate it.
	engine.Accounts().Unblock(param.NewAccountIDFromUint64(1))
	assertAccountPasses(t, engine)
}

func TestAccountsReplaceBlockReasonUpdatesBlockedAccount(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	accounts.Block(account, "first")

	if err := accounts.ReplaceBlockReason(account, "second"); err != nil {
		t.Fatalf("ReplaceBlockReason(blocked) error = %v, want nil", err)
	}
	// The account stays blocked after the reason is replaced.
	assertAccountBlocked(t, engine)
}

func TestAccountsReplaceBlockReasonRejectsUnblockedAccount(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	err := engine.Accounts().ReplaceBlockReason(param.NewAccountIDFromUint64(1), "reason")

	var blockErr *reject.AccountBlockError
	if !errors.As(err, &blockErr) {
		t.Fatalf("ReplaceBlockReason(unblocked) error = %v, want *reject.AccountBlockError", err)
	}
	if blockErr.Kind != reject.AccountBlockErrorKindAccountNotBlocked {
		t.Fatalf("Kind = %v, want AccountNotBlocked", blockErr.Kind)
	}
	if blockErr.Account == nil || blockErr.Account.String() != "1" {
		t.Fatalf("Account = %v, want 1", blockErr.Account)
	}
}

func TestAccountsBlockGroupGatesMember(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	group := mustAccountGroup(t)

	if err := accounts.RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("RegisterGroup(7) error = %v", err)
	}
	assertAccountPasses(t, engine)

	if err := accounts.BlockGroup(group, "group kill-switch"); err != nil {
		t.Fatalf("BlockGroup(7) error = %v, want nil", err)
	}
	assertAccountBlocked(t, engine)
}

func TestAccountsBlockGroupGatesLaterMemberAndUngroupReleases(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	group := mustAccountGroup(t)

	if err := accounts.BlockGroup(group, "group kill-switch"); err != nil {
		t.Fatalf("BlockGroup(7) error = %v, want nil", err)
	}

	// An account registered into an already-blocked group is gated.
	if err := accounts.RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("RegisterGroup(7) error = %v", err)
	}
	assertAccountBlocked(t, engine)

	// Removing the account from the blocked group releases the gate.
	if err := accounts.UnregisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("UnregisterGroup(7) error = %v", err)
	}
	assertAccountPasses(t, engine)
}

func TestAccountsUnblockGroupRestoresMembers(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	group := mustAccountGroup(t)

	if err := accounts.RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("RegisterGroup(7) error = %v", err)
	}
	if err := accounts.BlockGroup(group, "group kill-switch"); err != nil {
		t.Fatalf("BlockGroup(7) error = %v", err)
	}
	assertAccountBlocked(t, engine)

	if err := accounts.UnblockGroup(group); err != nil {
		t.Fatalf("UnblockGroup(7) error = %v, want nil", err)
	}
	assertAccountPasses(t, engine)
}

func TestAccountsBlockGroupRejectsDefaultGroup(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	err := engine.Accounts().BlockGroup(param.DefaultAccountGroup, "reason")

	assertReservedGroupError(t, "BlockGroup(default)", err)
}

func TestAccountsUnblockGroupRejectsDefaultGroup(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	err := engine.Accounts().UnblockGroup(param.DefaultAccountGroup)

	assertReservedGroupError(t, "UnblockGroup(default)", err)
}

func TestAccountsReplaceGroupBlockReasonRejectsDefaultGroup(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	err := engine.Accounts().ReplaceGroupBlockReason(param.DefaultAccountGroup, "reason")

	assertReservedGroupError(t, "ReplaceGroupBlockReason(default)", err)
}

func TestAccountsReplaceGroupBlockReasonRejectsUnblockedGroup(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	err := engine.Accounts().ReplaceGroupBlockReason(mustAccountGroup(t), "reason")

	var blockErr *reject.AccountBlockError
	if !errors.As(err, &blockErr) {
		t.Fatalf(
			"ReplaceGroupBlockReason(unblocked) error = %v, want *reject.AccountBlockError", err,
		)
	}
	if blockErr.Kind != reject.AccountBlockErrorKindGroupNotBlocked {
		t.Fatalf("Kind = %v, want GroupNotBlocked", blockErr.Kind)
	}
	if blockErr.Group == nil || blockErr.Group.String() != "7" {
		t.Fatalf("Group = %v, want 7", blockErr.Group)
	}
}

// assertReservedGroupError fails unless err is a *reject.AccountBlockError with
// kind ReservedGroup.
func assertReservedGroupError(t *testing.T, op string, err error) {
	t.Helper()
	var blockErr *reject.AccountBlockError
	if !errors.As(err, &blockErr) {
		t.Fatalf("%s error = %v, want *reject.AccountBlockError", op, err)
	}
	if blockErr.Kind != reject.AccountBlockErrorKindReservedGroup {
		t.Fatalf("%s Kind = %v, want ReservedGroup", op, blockErr.Kind)
	}
}

func TestAccountsBlockIdempotentKeepsFirstReason(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)

	accounts.Block(account, "first")
	// Blocking again must not replace the recorded reason.
	accounts.Block(account, "second")

	// The account is still blocked and the first reason must be preserved.
	assertAccountBlockedWithReason(t, engine, "first")
}

func TestAccountsBlockGroupIdempotentKeepsFirstReason(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	group := mustAccountGroup(t)

	if err := accounts.RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("RegisterGroup(7) error = %v", err)
	}
	if err := accounts.BlockGroup(group, "first"); err != nil {
		t.Fatalf("BlockGroup(first) error = %v", err)
	}
	// Blocking the same group again must keep the first reason and not error.
	if err := accounts.BlockGroup(group, "second"); err != nil {
		t.Fatalf("BlockGroup(second) error = %v", err)
	}

	// The group is still blocked and the first reason must be preserved.
	assertAccountBlockedWithReason(t, engine, "first")
}

func TestAccountsUnblockGroupAbsentIsNoOp(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	group := mustAccountGroup(t)

	if err := accounts.RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("RegisterGroup(7) error = %v", err)
	}

	// Unblocking a group that was never blocked must not error.
	if err := accounts.UnblockGroup(group); err != nil {
		t.Fatalf("UnblockGroup(never-blocked) error = %v, want nil", err)
	}

	// The account must still pass pre-trade after the no-op unblock.
	assertAccountPasses(t, engine)
}

func TestAccountsReplaceGroupBlockReasonUpdatesBlockedGroup(t *testing.T) {
	engine := newAccountsTestEngine(t)
	defer engine.Stop()

	accounts := engine.Accounts()
	account := param.NewAccountIDFromUint64(1)
	group := mustAccountGroup(t)

	if err := accounts.RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("RegisterGroup(7) error = %v", err)
	}
	if err := accounts.BlockGroup(group, "original reason"); err != nil {
		t.Fatalf("BlockGroup(7) error = %v", err)
	}

	if err := accounts.ReplaceGroupBlockReason(group, "new reason"); err != nil {
		t.Fatalf("ReplaceGroupBlockReason(7) error = %v, want nil", err)
	}

	// The group must remain blocked after the reason replacement.
	assertAccountBlocked(t, engine)
}
