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

package accountadjustment

import (
	"fmt"

	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// ClientAccountAdjustment is the client-owned adjustment shape accepted by
// ClientEngine.
//
// EngineAccountAdjustment returns the standard adjustment view used by the
// native engine. The original client value is carried separately as callback
// payload.
type ClientAccountAdjustment interface {
	EngineAccountAdjustment() model.AccountAdjustment
}

// ClientPolicy is an account-adjustment policy written against a client-owned
// adjustment type.
type ClientPolicy[Adjustment ClientAccountAdjustment] interface {
	Close()
	Name() string
	ApplyAccountAdjustment(
		Context,
		param.AccountID,
		Adjustment,
		tx.Mutations,
	) reject.List
}

// NewSafeClientPolicy adapts a client typed account-adjustment policy to the
// standard policy interface with payload validation.
//
// Missing or mismatched adjustment payloads become an account-scoped reject.
func NewSafeClientPolicy[Adjustment ClientAccountAdjustment](
	policy ClientPolicy[Adjustment],
) Policy {
	return &safeClientPolicy[Adjustment]{policy: policy}
}

// NewUnsafeFastClientPolicy adapts a client typed account-adjustment policy
// without payload validation.
//
// It is intended for SDK-controlled paths such as ClientEngine. A missing or
// wrong payload panics.
func NewUnsafeFastClientPolicy[Adjustment ClientAccountAdjustment](
	policy ClientPolicy[Adjustment],
) Policy {
	return &unsafeFastClientPolicy[Adjustment]{policy: policy}
}

type safeClientPolicy[Adjustment ClientAccountAdjustment] struct {
	policy ClientPolicy[Adjustment]
}

func (p *safeClientPolicy[Adjustment]) Close() {
	p.policy.Close()
}

func (p *safeClientPolicy[Adjustment]) Name() string {
	return p.policy.Name()
}

func (p *safeClientPolicy[Adjustment]) ApplyAccountAdjustment(
	ctx Context,
	accountID param.AccountID,
	engineAdjustment model.AccountAdjustment,
	mutations tx.Mutations,
) reject.List {
	adjustment, ok := safeAdjustmentPayload[Adjustment](engineAdjustment)
	if !ok {
		return clientPayloadMismatchReject[Adjustment](p.Name())
	}
	return p.policy.ApplyAccountAdjustment(ctx, accountID, adjustment, mutations)
}

type unsafeFastClientPolicy[Adjustment ClientAccountAdjustment] struct {
	policy ClientPolicy[Adjustment]
}

func (p *unsafeFastClientPolicy[Adjustment]) Close() {
	p.policy.Close()
}

func (p *unsafeFastClientPolicy[Adjustment]) Name() string {
	return p.policy.Name()
}

func (p *unsafeFastClientPolicy[Adjustment]) ApplyAccountAdjustment(
	ctx Context,
	accountID param.AccountID,
	engineAdjustment model.AccountAdjustment,
	mutations tx.Mutations,
) reject.List {
	return p.policy.ApplyAccountAdjustment(
		ctx,
		accountID,
		unsafeFastAdjustmentPayload[Adjustment](engineAdjustment),
		mutations,
	)
}

func safeAdjustmentPayload[Adjustment ClientAccountAdjustment](
	adjustment model.AccountAdjustment,
) (value Adjustment, ok bool) {
	userData := native.AccountAdjustmentGetUserData(adjustment.Native())
	if userData == nil {
		return value, false
	}
	defer func() {
		if recover() != nil {
			var zero Adjustment
			value = zero
			ok = false
		}
	}()
	payload := callback.NewHandleFromUserData(userData).Value()
	value, ok = payload.(Adjustment)
	return value, ok
}

func unsafeFastAdjustmentPayload[Adjustment ClientAccountAdjustment](
	adjustment model.AccountAdjustment,
) Adjustment {
	return callback.NewHandleFromUserData(
		native.AccountAdjustmentGetUserData(adjustment.Native()),
	).Value().(Adjustment)
}

func clientPayloadMismatchReject[Adjustment ClientAccountAdjustment](policyName string) reject.List {
	return reject.NewSingleItemList(
		reject.CodeOther,
		policyName,
		"client account adjustment payload mismatch",
		fmt.Sprintf("expected client account adjustment payload type %T", *new(Adjustment)),
		reject.ScopeAccount,
	)
}
