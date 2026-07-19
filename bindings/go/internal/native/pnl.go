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

package native

func NewPnlStateValue(value ParamPnl) PnlState {
	return PnlState{kind: PnlStateKindValue, value: value}
}

func NewPnlStateHalted(reason PnlHaltReason) PnlState {
	return PnlState{kind: PnlStateKindHalted, halt_reason: reason}
}

func PnlStateGetKind(state PnlState) PnlStateKind {
	return state.kind
}

func PnlStateGetValue(state PnlState) ParamPnl {
	return state.value
}

func PnlStateGetHaltReason(state PnlState) PnlHaltReason {
	return state.halt_reason
}

func PnlStateOptionalIsSet(value PnlStateOptional) bool {
	return bool(value.is_set)
}

func PnlStateOptionalGet(value PnlStateOptional) PnlState {
	return value.value
}
