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

func TestCreateAndDestroyPretradePreTradeLock(t *testing.T) {
	lock := CreatePretradePreTradeLock()
	if lock == nil {
		t.Fatal("CreatePretradePreTradeLock() = nil, want non-nil handle")
	}
	if PretradePreTradeLockLen(lock) != 0 {
		t.Fatalf("PretradePreTradeLockLen() = %d, want 0", PretradePreTradeLockLen(lock))
	}
	DestroyPretradePreTradeLock(lock)
}

func TestPretradePreTradeLockPushAndClone(t *testing.T) {
	lock := CreatePretradePreTradeLock()
	defer DestroyPretradePreTradeLock(lock)

	price, err := CreateParamPriceFromString("99.5")
	if err != nil {
		t.Fatalf("CreateParamPriceFromString() error = %v", err)
	}

	if err := PretradePreTradeLockPush(lock, DefaultPolicyGroupID, price); err != nil {
		t.Fatalf("PretradePreTradeLockPush(default) error = %v", err)
	}
	if err := PretradePreTradeLockPush(lock, PolicyGroupID(7), price); err != nil {
		t.Fatalf("PretradePreTradeLockPush(7) error = %v", err)
	}
	if PretradePreTradeLockLen(lock) != 2 {
		t.Fatalf("PretradePreTradeLockLen() = %d, want 2", PretradePreTradeLockLen(lock))
	}

	cloned := PretradePreTradeLockClone(lock)
	defer DestroyPretradePreTradeLock(cloned)
	if PretradePreTradeLockLen(cloned) != 2 {
		t.Fatalf("PretradePreTradeLockLen(cloned) = %d, want 2", PretradePreTradeLockLen(cloned))
	}
}

func TestPretradePreTradeLockPricesOf(t *testing.T) {
	lock := CreatePretradePreTradeLock()
	defer DestroyPretradePreTradeLock(lock)

	price, err := CreateParamPriceFromString("100")
	if err != nil {
		t.Fatalf("CreateParamPriceFromString() error = %v", err)
	}

	if err := PretradePreTradeLockPush(lock, PolicyGroupID(3), price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}
	if err := PretradePreTradeLockPush(lock, PolicyGroupID(3), price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}

	got, err := PretradePreTradeLockPricesOf(lock, PolicyGroupID(3))
	if err != nil {
		t.Fatalf("PretradePreTradeLockPricesOf() error = %v", err)
	}
	if len(got) != 2 {
		t.Fatalf("len(PretradePreTradeLockPricesOf()) = %d, want 2", len(got))
	}

	missing, err := PretradePreTradeLockPricesOf(lock, PolicyGroupID(99))
	if err != nil {
		t.Fatalf("PretradePreTradeLockPricesOf(missing) error = %v", err)
	}
	if len(missing) != 0 {
		t.Fatalf("len(PretradePreTradeLockPricesOf(missing)) = %d, want 0", len(missing))
	}

	single := CreatePretradePreTradeLock()
	defer DestroyPretradePreTradeLock(single)
	if err := PretradePreTradeLockPush(single, DefaultPolicyGroupID, price); err != nil {
		t.Fatalf("PretradePreTradeLockPush(single) error = %v", err)
	}
	singlePrices, err := PretradePreTradeLockPricesOf(single, DefaultPolicyGroupID)
	if err != nil {
		t.Fatalf("PretradePreTradeLockPricesOf(single) error = %v", err)
	}
	if len(singlePrices) != 1 {
		t.Fatalf("len(PretradePreTradeLockPricesOf(single)) = %d, want 1", len(singlePrices))
	}
	if singlePrices[0] != price {
		t.Fatalf("PretradePreTradeLockPricesOf(single)[0] = %+v, want %+v", singlePrices[0], price)
	}
}

func TestPretradeLockRawRoundTrip(t *testing.T) {
	lock := CreatePretradePreTradeLock()
	defer DestroyPretradePreTradeLock(lock)

	price, err := CreateParamPriceFromString("185")
	if err != nil {
		t.Fatalf("CreateParamPriceFromString() error = %v", err)
	}

	if err := PretradePreTradeLockPush(lock, DefaultPolicyGroupID, price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}
	if err := PretradePreTradeLockPush(lock, PolicyGroupID(7), price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}
	if err := PretradePreTradeLockPush(lock, PolicyGroupID(7), price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}

	rawHandle := PretradePreTradeLockToRaw(lock)
	defer DestroySharedBytes(rawHandle)
	data := BytesViewAsSlice(SharedBytesView(rawHandle))
	if len(data) == 0 {
		t.Fatal("raw blob is empty for non-empty lock")
	}

	restored, err := CreatePretradePreTradeLockFromRaw(data)
	if err != nil {
		t.Fatalf("CreatePretradePreTradeLockFromRaw() error = %v", err)
	}
	defer DestroyPretradePreTradeLock(restored)
	if PretradePreTradeLockLen(restored) != 3 {
		t.Fatalf("PretradePreTradeLockLen(restored) = %d, want 3", PretradePreTradeLockLen(restored))
	}
}

func TestPretradeLockMsgPackRoundTrip(t *testing.T) {
	lock := CreatePretradePreTradeLock()
	defer DestroyPretradePreTradeLock(lock)

	price, err := CreateParamPriceFromString("200")
	if err != nil {
		t.Fatalf("CreateParamPriceFromString() error = %v", err)
	}

	if err := PretradePreTradeLockPush(lock, PolicyGroupID(5), price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}

	payload, err := PretradePreTradeLockToMsgPack(lock)
	if err != nil {
		t.Fatalf("PretradePreTradeLockToMsgPack() error = %v", err)
	}
	defer DestroySharedBytes(payload)

	data := BytesViewAsSlice(SharedBytesView(payload))
	if len(data) == 0 {
		t.Fatal("MessagePack payload is empty")
	}

	restored, err := CreatePretradePreTradeLockFromMsgPack(data)
	if err != nil {
		t.Fatalf("CreatePretradePreTradeLockFromMsgPack() error = %v", err)
	}
	defer DestroyPretradePreTradeLock(restored)

	if PretradePreTradeLockLen(restored) != 1 {
		t.Fatalf("PretradePreTradeLockLen(restored) = %d, want 1", PretradePreTradeLockLen(restored))
	}
}

func TestPretradeLockRawIsBitIdenticalAcrossRoundTrip(t *testing.T) {
	lock := CreatePretradePreTradeLock()
	defer DestroyPretradePreTradeLock(lock)

	price, err := CreateParamPriceFromString("1.25")
	if err != nil {
		t.Fatalf("CreateParamPriceFromString() error = %v", err)
	}

	if err := PretradePreTradeLockPush(lock, DefaultPolicyGroupID, price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}
	if err := PretradePreTradeLockPush(lock, PolicyGroupID(2), price); err != nil {
		t.Fatalf("PretradePreTradeLockPush() error = %v", err)
	}

	first := PretradePreTradeLockToRaw(lock)
	firstBytes := append([]byte(nil), BytesViewAsSlice(SharedBytesView(first))...)
	DestroySharedBytes(first)

	restored, err := CreatePretradePreTradeLockFromRaw(firstBytes)
	if err != nil {
		t.Fatalf("CreatePretradePreTradeLockFromRaw() error = %v", err)
	}
	defer DestroyPretradePreTradeLock(restored)

	second := PretradePreTradeLockToRaw(restored)
	secondBytes := append([]byte(nil), BytesViewAsSlice(SharedBytesView(second))...)
	DestroySharedBytes(second)

	if string(firstBytes) != string(secondBytes) {
		t.Fatalf("raw layout is not bit-stable across round-trip: %x vs %x", firstBytes, secondBytes)
	}
}
