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

// Package accounts provides account-group management and pre-trade account
// blocking bound to an engine.
package accounts

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/reject"
)

// Accounts manages account-group membership and account/group pre-trade blocks
// for an engine. Obtain it from an engine's Accounts accessor. It carries no
// state of its own: every call forwards to the engine it was created from, and
// it is valid for as long as that engine is.
type Accounts struct {
	engine native.Engine
}

// NewFromHandle wraps a native engine handle into an Accounts accessor.
func NewFromHandle(engine native.Engine) Accounts {
	return Accounts{engine: engine}
}

// RegisterGroup atomically registers one or more accounts into group. All
// accounts in the slice are registered together; the operation is
// all-or-nothing.
//
// Returns a *reject.AccountGroupError when any account is already in a group or
// when group is the reserved param.DefaultAccountGroup. Returns a Go error on
// transport failure.
func (a Accounts) RegisterGroup(accounts []param.AccountID, group param.AccountGroupID) error {
	nativeAccounts := make([]native.ParamAccountID, len(accounts))
	for i, account := range accounts {
		nativeAccounts[i] = account.Handle()
	}
	groupErr, err := native.EngineRegisterAccountGroup(a.engine, nativeAccounts, group.Handle())
	if err != nil {
		return err
	}
	if groupErr != nil {
		return reject.NewAccountGroupErrorFromHandle(groupErr)
	}
	return nil
}

// UnregisterGroup atomically removes one or more accounts from group. All
// accounts in the slice are unregistered together; the operation is
// all-or-nothing.
//
// Returns a *reject.AccountGroupError when any account is not in group or when
// group is the reserved param.DefaultAccountGroup. Returns a Go error on
// transport failure.
func (a Accounts) UnregisterGroup(accounts []param.AccountID, group param.AccountGroupID) error {
	nativeAccounts := make([]native.ParamAccountID, len(accounts))
	for i, account := range accounts {
		nativeAccounts[i] = account.Handle()
	}
	groupErr, err := native.EngineUnregisterAccountGroup(a.engine, nativeAccounts, group.Handle())
	if err != nil {
		return err
	}
	if groupErr != nil {
		return reject.NewAccountGroupErrorFromHandle(groupErr)
	}
	return nil
}

// GroupOf returns the account-group identifier of account. The option is empty
// when the account is not assigned to any group.
func (a Accounts) GroupOf(account param.AccountID) optional.Option[param.AccountGroupID] {
	id, ok := native.EngineAccountGroup(a.engine, account.Handle())
	return optional.From(param.NewAccountGroupIDFromHandle(id), ok)
}

// Block blocks account with reason, gating its pre-trade orders until it is
// unblocked. The first reason recorded for an account wins: blocking an
// already-blocked account keeps the original reason. reason may be empty.
func (a Accounts) Block(account param.AccountID, reason string) {
	native.EngineBlockAccount(a.engine, account.Handle(), reason)
}

// Unblock lifts the block on account. Unblocking an account that is not blocked
// is a no-op.
func (a Accounts) Unblock(account param.AccountID) {
	native.EngineUnblockAccount(a.engine, account.Handle())
}

// ReplaceBlockReason replaces the recorded reason of a blocked account.
//
// Returns a *reject.AccountBlockError with kind AccountNotBlocked when account
// is not blocked.
func (a Accounts) ReplaceBlockReason(account param.AccountID, reason string) error {
	blockErr := native.EngineReplaceAccountBlockReason(a.engine, account.Handle(), reason)
	if blockErr != nil {
		return reject.NewAccountBlockErrorFromHandle(blockErr)
	}
	return nil
}

// BlockGroup blocks group with reason, gating the pre-trade orders of every
// account in it. The first reason recorded for a group wins: blocking an
// already-blocked group keeps the original reason. reason may be empty.
//
// Returns a *reject.AccountBlockError with kind ReservedGroup when group is the
// reserved param.DefaultAccountGroup.
func (a Accounts) BlockGroup(group param.AccountGroupID, reason string) error {
	blockErr := native.EngineBlockAccountGroup(a.engine, group.Handle(), reason)
	if blockErr != nil {
		return reject.NewAccountBlockErrorFromHandle(blockErr)
	}
	return nil
}

// UnblockGroup lifts the block on group. Unblocking a group that is not blocked
// is a no-op.
//
// Returns a *reject.AccountBlockError with kind ReservedGroup when group is the
// reserved param.DefaultAccountGroup.
func (a Accounts) UnblockGroup(group param.AccountGroupID) error {
	blockErr := native.EngineUnblockAccountGroup(a.engine, group.Handle())
	if blockErr != nil {
		return reject.NewAccountBlockErrorFromHandle(blockErr)
	}
	return nil
}

// ReplaceGroupBlockReason replaces the recorded reason of a blocked group.
//
// Returns a *reject.AccountBlockError with kind ReservedGroup when group is the
// reserved param.DefaultAccountGroup, or GroupNotBlocked when group is not
// blocked.
func (a Accounts) ReplaceGroupBlockReason(group param.AccountGroupID, reason string) error {
	blockErr := native.EngineReplaceAccountGroupBlockReason(a.engine, group.Handle(), reason)
	if blockErr != nil {
		return reject.NewAccountBlockErrorFromHandle(blockErr)
	}
	return nil
}
