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

/*
#include "openpit.h"
*/
import "C"

import (
	"errors"
	"runtime"
	"unsafe"
)

func CreatePretradePreTradeLock() PretradePreTradeLock {
	return C.openpit_create_pretrade_pre_trade_lock()
}

func CreatePretradePreTradeLockFromRaw(data []byte) (PretradePreTradeLock, error) {
	var ptr *C.uint8_t
	if len(data) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	var outError SharedString
	lock := C.openpit_create_pretrade_pre_trade_lock_from_raw(
		ptr,
		C.size_t(len(data)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(data)
	if lock == nil {
		return nil,
			consumeSharedStringAsError(outError, "openpit_create_pretrade_pre_trade_lock_from_raw failed")
	}
	return lock, nil
}

func CreatePretradePreTradeLockFromMsgPack(data []byte) (PretradePreTradeLock, error) {
	var ptr *C.uint8_t
	if len(data) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	var outError SharedString
	lock := C.openpit_create_pretrade_pre_trade_lock_from_msgpack(
		ptr,
		C.size_t(len(data)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(data)
	if lock == nil {
		return nil,
			consumeSharedStringAsError(
				outError,
				"openpit_create_pretrade_pre_trade_lock_from_msgpack failed",
			)
	}
	return lock, nil
}

func CreatePretradePreTradeLockFromJSON(text []byte) (PretradePreTradeLock, error) {
	var ptr *C.uint8_t
	if len(text) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&text[0]))
	}
	var outError SharedString
	lock := C.openpit_create_pretrade_pre_trade_lock_from_json(
		ptr,
		C.size_t(len(text)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(text)
	if lock == nil {
		return nil,
			consumeSharedStringAsError(
				outError,
				"openpit_create_pretrade_pre_trade_lock_from_json failed",
			)
	}
	return lock, nil
}

func CreatePretradePreTradeLockFromCBOR(data []byte) (PretradePreTradeLock, error) {
	var ptr *C.uint8_t
	if len(data) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	var outError SharedString
	lock := C.openpit_create_pretrade_pre_trade_lock_from_cbor(
		ptr,
		C.size_t(len(data)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(data)
	if lock == nil {
		return nil,
			consumeSharedStringAsError(
				outError,
				"openpit_create_pretrade_pre_trade_lock_from_cbor failed",
			)
	}
	return lock, nil
}

func DestroyPretradePreTradeLock(lock PretradePreTradeLock) {
	C.openpit_destroy_pretrade_pre_trade_lock(lock)
}

func PretradePreTradeLockClone(lock PretradePreTradeLock) PretradePreTradeLock {
	return C.openpit_pretrade_pre_trade_lock_clone(lock)
}

func PretradePreTradeLockLen(lock PretradePreTradeLock) int {
	return int(C.openpit_pretrade_pre_trade_lock_len(lock))
}

func PretradePreTradeLockIsEmpty(lock PretradePreTradeLock) bool {
	return bool(C.openpit_pretrade_pre_trade_lock_is_empty(lock))
}

func PretradePreTradeLockPush(
	lock PretradePreTradeLock,
	groupID PolicyGroupID,
	price ParamPrice,
) error {
	var outError SharedString
	if !C.openpit_pretrade_pre_trade_lock_push(
		lock,
		groupID,
		price,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_lock_push failed")
	}
	return nil
}

func PretradePreTradeLockPushMany(
	lock PretradePreTradeLock,
	entries []PretradePreTradeLockEntry,
) error {
	var ptr *C.OpenPitPretradePreTradeLockEntry
	if len(entries) > 0 {
		ptr = (*C.OpenPitPretradePreTradeLockEntry)(unsafe.Pointer(&entries[0]))
	}
	var outError SharedString
	ok := C.openpit_pretrade_pre_trade_lock_push_many(
		lock,
		ptr,
		C.size_t(len(entries)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(entries)
	if !ok {
		return consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_lock_push_many failed")
	}
	return nil
}

func CreatePretradePreTradeLockFromEntries(
	entries []PretradePreTradeLockEntry,
) (PretradePreTradeLock, error) {
	var ptr *C.OpenPitPretradePreTradeLockEntry
	if len(entries) > 0 {
		ptr = (*C.OpenPitPretradePreTradeLockEntry)(unsafe.Pointer(&entries[0]))
	}
	var outError SharedString
	lock := C.openpit_create_pretrade_pre_trade_lock_from_entries(
		ptr,
		C.size_t(len(entries)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(entries)
	if lock == nil {
		return nil,
			consumeSharedStringAsError(
				outError,
				"openpit_create_pretrade_pre_trade_lock_from_entries failed",
			)
	}
	return lock, nil
}

func PretradePreTradeLockMerge(dst PretradePreTradeLock, src PretradePreTradeLock) error {
	var outError SharedString
	if !C.openpit_pretrade_pre_trade_lock_merge(
		dst,
		src,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_lock_merge failed")
	}
	return nil
}

func PretradePreTradeLockPricesOf(
	lock PretradePreTradeLock,
	groupID PolicyGroupID,
) ([]ParamPrice, error) {
	var outError SharedString
	var price ParamPrice
	var prices PretradePreTradeLockPrices
	status := C.openpit_pretrade_pre_trade_lock_prices_of(
		lock,
		groupID,
		&price,
		&prices,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)

	switch status {
	case C.OpenPitPretradePreTradeLockPricesStatus_Error:
		if outError != nil {
			return nil,
				consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_lock_prices_of failed")
		}
		return nil, errors.New("openpit_pretrade_pre_trade_lock_prices_of failed")
	case C.OpenPitPretradePreTradeLockPricesStatus_Empty:
		return nil, nil
	case C.OpenPitPretradePreTradeLockPricesStatus_One:
		return []ParamPrice{price}, nil
	case C.OpenPitPretradePreTradeLockPricesStatus_List:
		if prices == nil {
			return nil,
				errors.New("openpit_pretrade_pre_trade_lock_prices_of returned list status with nil list")
		}
		defer C.openpit_destroy_pretrade_pre_trade_lock_prices(prices)
		return clonePretradePreTradeLockPrices(
			C.openpit_pretrade_pre_trade_lock_prices_view(prices),
		), nil
	default:
		return nil, errors.New("openpit_pretrade_pre_trade_lock_prices_of returned unknown status")
	}
}

func clonePretradePreTradeLockPrices(view PretradePreTradeLockPricesView) []ParamPrice {
	if view.ptr == nil || view.len == 0 {
		return nil
	}
	src := unsafe.Slice((*ParamPrice)(unsafe.Pointer(view.ptr)), int(view.len))
	out := make([]ParamPrice, len(src))
	copy(out, src)
	return out
}

func CreatePretradePreTradeLockEntry(
	policyGroupID PolicyGroupID,
	price ParamPrice,
) PretradePreTradeLockEntry {
	return PretradePreTradeLockEntry{policy_group_id: policyGroupID, price: price}
}

func PretradePreTradeLockEntryGetPolicyGroupID(entry PretradePreTradeLockEntry) PolicyGroupID {
	return entry.policy_group_id
}

func PretradePreTradeLockEntryGetPrice(entry PretradePreTradeLockEntry) ParamPrice {
	return entry.price
}

func PretradePreTradeLockEntriesOf(lock PretradePreTradeLock) []PretradePreTradeLockEntry {
	entries := C.openpit_pretrade_pre_trade_lock_entries(lock)
	defer C.openpit_destroy_pretrade_pre_trade_lock_entries(entries)
	return clonePretradePreTradeLockEntries(
		C.openpit_pretrade_pre_trade_lock_entries_view(entries),
	)
}

func clonePretradePreTradeLockEntries(
	view PretradePreTradeLockEntriesView,
) []PretradePreTradeLockEntry {
	if view.ptr == nil || view.len == 0 {
		return nil
	}
	src := unsafe.Slice((*PretradePreTradeLockEntry)(unsafe.Pointer(view.ptr)), int(view.len))
	out := make([]PretradePreTradeLockEntry, len(src))
	copy(out, src)
	return out
}

func PretradePreTradeLockToRaw(lock PretradePreTradeLock) SharedBytes {
	return C.openpit_pretrade_pre_trade_lock_to_raw(lock)
}

func PretradePreTradeLockToMsgPack(lock PretradePreTradeLock) (SharedBytes, error) {
	var outError SharedString
	data := C.openpit_pretrade_pre_trade_lock_to_msgpack(
		lock,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if data == nil {
		return nil,
			consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_lock_to_msgpack failed")
	}
	return data, nil
}

func PretradePreTradeLockToJSON(lock PretradePreTradeLock) (SharedString, error) {
	var outError SharedString
	text := C.openpit_pretrade_pre_trade_lock_to_json(
		lock,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if text == nil {
		return nil,
			consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_lock_to_json failed")
	}
	return text, nil
}

func PretradePreTradeLockToCBOR(lock PretradePreTradeLock) (SharedBytes, error) {
	var outError SharedString
	data := C.openpit_pretrade_pre_trade_lock_to_cbor(
		lock,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if data == nil {
		return nil,
			consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_lock_to_cbor failed")
	}
	return data, nil
}
