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

package native

import "testing"

func TestPretradePreTradeLockResetAndUnsetPrice(t *testing.T) {
	lock := NewPretradePreTradeLock()
	if ParamPriceOptionalIsSet(PretradePreTradeLockGetPrice(lock)) {
		t.Fatal("new lock price is set, want unset")
	}

	price, err := CreateParamPriceFromStr("99.5")
	if err != nil {
		t.Fatalf("CreateParamPriceFromStr() error = %v", err)
	}

	PretradePreTradeLockSetPrice(&lock, price)
	if !ParamPriceOptionalIsSet(PretradePreTradeLockGetPrice(lock)) {
		t.Fatal("lock price is unset after SetPrice(), want set")
	}

	PretradePreTradeLockUnsetPrice(&lock)
	if ParamPriceOptionalIsSet(PretradePreTradeLockGetPrice(lock)) {
		t.Fatal("lock price is set after UnsetPrice(), want unset")
	}

	PretradePreTradeLockSetPrice(&lock, price)
	PretradePreTradeLockReset(&lock)
	if ParamPriceOptionalIsSet(PretradePreTradeLockGetPrice(lock)) {
		t.Fatal("lock price is set after Reset(), want unset")
	}
}
