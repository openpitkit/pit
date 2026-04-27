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

package param

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pkg/optional"
)

// AdjustmentAmount is a signed balance/position adjustment payload.
type AdjustmentAmount struct {
	native native.ParamAdjustmentAmount
}

func NewDeltaAdjustmentAmount(v PositionSize) AdjustmentAmount {
	return newAdjustmentAmount(
		native.CreateParamAdjustmentAmount(
			native.ParamAdjustmentAmountKindDelta,
			v.native,
		),
	)
}

func NewAbsoluteAdjustmentAmount(v PositionSize) AdjustmentAmount {
	return newAdjustmentAmount(
		native.CreateParamAdjustmentAmount(
			native.ParamAdjustmentAmountKindAbsolute,
			v.native,
		),
	)
}

func NewAdjustmentAmountFromNative(
	amount native.ParamAdjustmentAmount,
) optional.Option[AdjustmentAmount] {
	if native.ParamAdjustmentAmountGetKind(amount) == native.ParamAdjustmentAmountKindNotSet {
		return optional.None[AdjustmentAmount]()
	}
	return optional.Some(newAdjustmentAmount(amount))
}

func newAdjustmentAmount(amount native.ParamAdjustmentAmount) AdjustmentAmount {
	return AdjustmentAmount{native: amount}
}

func (a AdjustmentAmount) IsDelta() bool {
	return native.ParamAdjustmentAmountGetKind(a.native) == native.ParamAdjustmentAmountKindDelta
}

func (a AdjustmentAmount) IsAbsolute() bool {
	return native.ParamAdjustmentAmountGetKind(a.native) == native.ParamAdjustmentAmountKindAbsolute
}

func (a AdjustmentAmount) MustDelta() PositionSize {
	if !a.IsDelta() {
		panic("requested adjustment amount as delta, but it is not")
	}
	return NewPositionSizeFromNative(native.ParamAdjustmentAmountGetValue(a.native))
}

func (a AdjustmentAmount) MustAbsolute() PositionSize {
	if !a.IsAbsolute() {
		panic("requested adjustment amount as absolute, but it is not")
	}
	return NewPositionSizeFromNative(native.ParamAdjustmentAmountGetValue(a.native))
}

func (a AdjustmentAmount) Native() native.ParamAdjustmentAmount {
	return a.native
}

func (a AdjustmentAmount) Choose(getDelta func(PositionSize), getAbsolute func(PositionSize)) {
	if a.IsDelta() {
		getDelta(a.MustDelta())
		return
	}
	if a.IsAbsolute() {
		getAbsolute(a.MustAbsolute())
		return
	}
	panic("requested adjustment amount variant, but it is not set")
}

func (a AdjustmentAmount) String() string {
	switch native.ParamAdjustmentAmountGetKind(a.native) {
	case native.ParamAdjustmentAmountKindDelta:
		return "delta: " + a.MustDelta().String()
	case native.ParamAdjustmentAmountKindAbsolute:
		return "absolute: " + a.MustAbsolute().String()
	default:
		return "not set"
	}
}
