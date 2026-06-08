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
	"bytes"
	"errors"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
)

// Lock holds the serialized pre-trade lock token produced by a reservation.
type Lock struct {
	value []byte
}

// NewLock returns an empty lock. Useful for tests; engines normally produce
// locks via Reservation.Lock.
func NewLock() Lock {
	handle := native.CreatePretradePreTradeLock()
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result
}

// NewLockFromBytes wraps a value previously obtained from Lock.Bytes.
//
// The bytes must come from Lock.Bytes within the same library build. Passing
// arbitrary bytes is rejected when the engine first inspects the lock.
func NewLockFromBytes(value []byte) Lock {
	return Lock{value: value}
}

// NewLockFromMsgPack reconstructs a lock from a MessagePack payload produced
// by Lock.MarshalMsgpack.
func NewLockFromMsgPack(payload []byte) (Lock, error) {
	handle, err := native.CreatePretradePreTradeLockFromMsgPack(payload)
	if err != nil {
		return Lock{}, err
	}
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// NewLockFromJSON reconstructs a lock from a JSON payload produced by
// Lock.MarshalJSON.
func NewLockFromJSON(payload []byte) (Lock, error) {
	handle, err := native.CreatePretradePreTradeLockFromJSON(payload)
	if err != nil {
		return Lock{}, err
	}
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// NewLockFromCBOR reconstructs a lock from a CBOR payload produced by
// Lock.MarshalCBOR.
func NewLockFromCBOR(payload []byte) (Lock, error) {
	handle, err := native.CreatePretradePreTradeLockFromCBOR(payload)
	if err != nil {
		return Lock{}, err
	}
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// NewLockFromEntries builds a lock populated from the given (group, price)
// records.
func NewLockFromEntries(entries []Entry) (Lock, error) {
	handle, err := native.CreatePretradePreTradeLockFromEntries(newEntryHandles(entries))
	if err != nil {
		return Lock{}, err
	}
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

func newLockFromHandle(handle native.PretradePreTradeLock) Lock {
	raw := native.PretradePreTradeLockToRaw(handle)
	value := native.CloneBytes(native.SharedBytesView(raw))
	native.DestroySharedBytes(raw)
	return Lock{value: value}
}

// Bytes returns the lock's in-process representation. The exact layout is
// version-specific; use one of the Marshal methods for any payload that must
// outlive the current library build.
func (l Lock) Bytes() []byte {
	return l.value
}

// Equal reports whether two locks describe the same reservation snapshot.
func (l Lock) Equal(other Lock) bool {
	return bytes.Equal(l.value, other.value)
}

// Entry is a single (group, price) record stored in a lock.
type Entry struct {
	PolicyGroupID model.PolicyGroupID
	Price         param.Price
}

// Len returns the total number of stored prices across all groups.
func (l Lock) Len() (int, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return 0, err
	}
	result := native.PretradePreTradeLockLen(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// IsEmpty reports whether the lock carries no price records.
func (l Lock) IsEmpty() (bool, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return false, err
	}
	result := native.PretradePreTradeLockIsEmpty(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// PushMany appends every (group, price) record from entries into the lock.
func (l *Lock) PushMany(entries []Entry) error {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return err
	}
	if err := native.PretradePreTradeLockPushMany(handle, newEntryHandles(entries)); err != nil {
		native.DestroyPretradePreTradeLock(handle)
		return err
	}
	*l = newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return nil
}

// Merge appends every record from other into the lock, leaving other unchanged.
func (l *Lock) Merge(other Lock) error {
	dst, err := decodeLockBytes(l.value)
	if err != nil {
		return err
	}
	src, err := decodeLockBytes(other.value)
	if err != nil {
		native.DestroyPretradePreTradeLock(dst)
		return err
	}
	if err := native.PretradePreTradeLockMerge(dst, src); err != nil {
		native.DestroyPretradePreTradeLock(src)
		native.DestroyPretradePreTradeLock(dst)
		return err
	}
	native.DestroyPretradePreTradeLock(src)
	*l = newLockFromHandle(dst)
	native.DestroyPretradePreTradeLock(dst)
	return nil
}

// Entries returns a snapshot of every (group, price) record stored in the lock,
// in iteration order (default-group records first, then each non-default group
// in insertion order).
func (l Lock) Entries() ([]Entry, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return nil, err
	}
	result := newEntriesFromHandles(
		native.PretradePreTradeLockEntriesOf(handle),
	)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// Prices returns every price stored in the lock, in iteration order
// (default-group records first, then each non-default group in insertion
// order).
func (l Lock) Prices() ([]param.Price, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return nil, err
	}
	entries := native.PretradePreTradeLockEntriesOf(handle)
	native.DestroyPretradePreTradeLock(handle)
	if len(entries) == 0 {
		return nil, nil
	}
	result := make([]param.Price, len(entries))
	for i, entry := range entries {
		result[i] = param.NewPriceFromHandle(native.PretradePreTradeLockEntryGetPrice(entry))
	}
	return result, nil
}

// PricesOf returns prices stored under groupID.
func (l Lock) PricesOf(groupID model.PolicyGroupID) ([]param.Price, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return nil, err
	}
	prices, err := native.PretradePreTradeLockPricesOf(handle, native.PolicyGroupID(groupID))
	native.DestroyPretradePreTradeLock(handle)
	if err != nil {
		return nil, err
	}
	return newPricesFromHandles(prices), nil
}

// MarshalJSON encodes the lock into a JSON payload. Implements
// `encoding/json.Marshaler`, so `json.Marshal(lock)` works automatically.
func (l Lock) MarshalJSON() ([]byte, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return nil, err
	}
	payload, err := native.PretradePreTradeLockToJSON(handle)
	if err != nil {
		native.DestroyPretradePreTradeLock(handle)
		return nil, err
	}
	result := append([]byte(nil), native.SharedStringViewBytes(payload)...)
	native.DestroySharedString(payload)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// UnmarshalJSON replaces the lock with one decoded from the JSON payload.
// Implements `encoding/json.Unmarshaler`.
func (l *Lock) UnmarshalJSON(payload []byte) error {
	restored, err := NewLockFromJSON(payload)
	if err != nil {
		return err
	}
	*l = restored
	return nil
}

// MarshalMsgpack encodes the lock into a MessagePack payload. Follows the
// convention used by `github.com/vmihailenco/msgpack/v5` so `msgpack.Marshal`
// picks it up automatically.
func (l Lock) MarshalMsgpack() ([]byte, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return nil, err
	}
	payload, err := native.PretradePreTradeLockToMsgPack(handle)
	if err != nil {
		native.DestroyPretradePreTradeLock(handle)
		return nil, err
	}
	result := native.CloneBytes(native.SharedBytesView(payload))
	native.DestroySharedBytes(payload)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// UnmarshalMsgpack replaces the lock with one decoded from the MessagePack
// payload. Follows the `github.com/vmihailenco/msgpack/v5` convention.
func (l *Lock) UnmarshalMsgpack(payload []byte) error {
	restored, err := NewLockFromMsgPack(payload)
	if err != nil {
		return err
	}
	*l = restored
	return nil
}

// MarshalCBOR encodes the lock into a CBOR payload. Follows the convention
// used by `github.com/fxamacker/cbor/v2` so `cbor.Marshal` picks it up
// automatically.
func (l Lock) MarshalCBOR() ([]byte, error) {
	handle, err := decodeLockBytes(l.value)
	if err != nil {
		return nil, err
	}
	payload, err := native.PretradePreTradeLockToCBOR(handle)
	if err != nil {
		native.DestroyPretradePreTradeLock(handle)
		return nil, err
	}
	result := native.CloneBytes(native.SharedBytesView(payload))
	native.DestroySharedBytes(payload)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// UnmarshalCBOR replaces the lock with one decoded from the CBOR payload.
// Follows the `github.com/fxamacker/cbor/v2` convention.
func (l *Lock) UnmarshalCBOR(payload []byte) error {
	restored, err := NewLockFromCBOR(payload)
	if err != nil {
		return err
	}
	*l = restored
	return nil
}

func decodeLockBytes(value []byte) (native.PretradePreTradeLock, error) {
	if len(value) == 0 {
		return nil, errors.New("pre-trade lock byte set has no value")
	}
	return native.CreatePretradePreTradeLockFromRaw(value)
}

func newPricesFromHandles(source []native.ParamPrice) []param.Price {
	if len(source) == 0 {
		return nil
	}
	result := make([]param.Price, len(source))
	for i, value := range source {
		result[i] = param.NewPriceFromHandle(value)
	}
	return result
}

func newEntryHandles(entries []Entry) []native.PretradePreTradeLockEntry {
	if len(entries) == 0 {
		return nil
	}
	result := make([]native.PretradePreTradeLockEntry, len(entries))
	for i, entry := range entries {
		result[i] = native.CreatePretradePreTradeLockEntry(
			native.PolicyGroupID(entry.PolicyGroupID),
			entry.Price.Handle(),
		)
	}
	return result
}

func newEntriesFromHandles(source []native.PretradePreTradeLockEntry) []Entry {
	if len(source) == 0 {
		return nil
	}
	result := make([]Entry, len(source))
	for i, value := range source {
		result[i] = Entry{
			PolicyGroupID: model.PolicyGroupID(native.PretradePreTradeLockEntryGetPolicyGroupID(value)),
			Price:         param.NewPriceFromHandle(native.PretradePreTradeLockEntryGetPrice(value)),
		}
	}
	return result
}
