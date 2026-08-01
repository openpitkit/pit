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

package marketdata

import (
	"errors"
	"fmt"

	"go.openpit.dev/openpit/param"
)

var (
	// ErrAlreadyRegistered reports that an instrument is already registered.
	ErrAlreadyRegistered = errors.New("instrument is already registered")
	// ErrDuplicateID reports that an instrument id is already registered.
	ErrDuplicateID = errors.New("instrument id is already registered")
	// ErrDuplicateInstrument reports that an instrument is registered under a
	// different id.
	ErrDuplicateInstrument = errors.New(
		"instrument is already registered under a different id",
	)
)

// AlreadyRegisteredError carries the instrument rejected by Register or
// RegisterWithTTL.
type AlreadyRegisteredError struct {
	Instrument param.Instrument
}

func (e *AlreadyRegisteredError) Error() string {
	return fmt.Sprintf("instrument %s is already registered", e.Instrument)
}

// Unwrap preserves compatibility with ErrAlreadyRegistered and errors.Is.
func (*AlreadyRegisteredError) Unwrap() error { return ErrAlreadyRegistered }

// RegistrationErrorKind identifies an explicit-id registration conflict.
type RegistrationErrorKind uint8

const (
	// RegistrationErrorKindDuplicateID identifies an id collision.
	RegistrationErrorKindDuplicateID RegistrationErrorKind = iota + 1
	// RegistrationErrorKindDuplicateInstrument identifies an instrument-name
	// collision under a different id.
	RegistrationErrorKindDuplicateInstrument
)

// RegistrationError carries the payload of an explicit-id registration
// conflict. Exactly one of InstrumentID and Instrument is set according to
// Kind.
type RegistrationError struct {
	Instrument   *param.Instrument
	InstrumentID *InstrumentID
	Kind         RegistrationErrorKind
}

func (e *RegistrationError) Error() string {
	switch e.Kind {
	case RegistrationErrorKindDuplicateID:
		if e.InstrumentID != nil {
			return fmt.Sprintf("instrument id %s is already registered", *e.InstrumentID)
		}
	case RegistrationErrorKindDuplicateInstrument:
		if e.Instrument != nil {
			return fmt.Sprintf("instrument %s is already registered", *e.Instrument)
		}
	}
	return "market-data registration failed"
}

// Unwrap preserves the corresponding sentinel for errors.Is.
func (e *RegistrationError) Unwrap() error {
	switch e.Kind {
	case RegistrationErrorKindDuplicateID:
		return ErrDuplicateID
	case RegistrationErrorKindDuplicateInstrument:
		return ErrDuplicateInstrument
	default:
		return nil
	}
}

// UnknownInstrumentIDError carries an id that is not registered with the
// service.
type UnknownInstrumentIDError struct {
	InstrumentID InstrumentID
}

func newUnknownInstrumentIDError(instrumentID InstrumentID) error {
	return &UnknownInstrumentIDError{InstrumentID: instrumentID}
}

func (e *UnknownInstrumentIDError) Error() string {
	return fmt.Sprintf("unknown instrument id: %s", e.InstrumentID)
}

// Unwrap preserves compatibility with ErrUnknownInstrument and errors.Is.
func (*UnknownInstrumentIDError) Unwrap() error { return ErrUnknownInstrument }

// AccountGroupResolutionError reports that the supplied AccountInfo could not
// answer AccountGroup, so the reading account's group stayed unknown and the
// read failed as a whole. Panic carries the recovered value when the failure
// came from a panicking AccountGroup implementation.
//
// The group is never assumed absent on failure: that would silently move the
// read onto the default-group bucket and bypass every group-scoped rule.
type AccountGroupResolutionError struct {
	Panic any
}

func newAccountGroupResolutionError(recovered any) error {
	return &AccountGroupResolutionError{Panic: recovered}
}

func (e *AccountGroupResolutionError) Error() string {
	if e.Panic == nil {
		return "account group resolution failed"
	}
	return fmt.Sprintf("account group resolution failed: panic: %v", e.Panic)
}

// Unwrap preserves compatibility with ErrAccountGroupResolution and errors.Is.
func (*AccountGroupResolutionError) Unwrap() error { return ErrAccountGroupResolution }
