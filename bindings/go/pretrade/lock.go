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

package pretrade

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// Lock is a snapshot of values that the engine has reserved for this request.
// New fields may be added here in future minor releases; access fields only
// through Lock methods.
type Lock struct{ value native.PretradePreTradeLock }

func newLock(value native.PretradePreTradeLock) Lock { return Lock{value: value} }

func (l Lock) Price() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.PretradePreTradeLockGetPrice(l.value))
}
