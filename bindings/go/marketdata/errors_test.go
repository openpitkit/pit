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

package marketdata

import (
	"errors"
	"testing"

	"go.openpit.dev/openpit/param"
)

func testInstrument(t *testing.T, underlying string) param.Instrument {
	t.Helper()
	underlyingAsset, err := param.NewAsset(underlying)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", underlying, err)
	}
	settlementAsset, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	return param.NewInstrument(underlyingAsset, settlementAsset)
}

func TestRegistrationErrorsCarryVariantPayloads(t *testing.T) {
	service, err := NewBuilder(InfiniteTTL()).NoSync().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer service.Close()

	aapl := testInstrument(t, "AAPL")
	msft := testInstrument(t, "MSFT")
	instrumentID := NewInstrumentIDFromUint64(42)
	otherID := NewInstrumentIDFromUint64(43)
	if _, err := service.RegisterWithID(aapl, instrumentID); err != nil {
		t.Fatalf("RegisterWithID() error = %v", err)
	}

	_, err = service.Register(aapl)
	var already *AlreadyRegisteredError
	if !errors.As(err, &already) || !errors.Is(err, ErrAlreadyRegistered) {
		t.Fatalf("Register() error = %v, want AlreadyRegisteredError", err)
	}
	if got := already.Instrument.String(); got != "AAPL/USD" {
		t.Fatalf("already instrument = %q, want AAPL/USD", got)
	}

	_, err = service.RegisterWithID(msft, instrumentID)
	var duplicateID *RegistrationError
	if !errors.As(err, &duplicateID) || !errors.Is(err, ErrDuplicateID) {
		t.Fatalf("duplicate id error = %v, want RegistrationError", err)
	}
	if duplicateID.Kind != RegistrationErrorKindDuplicateID ||
		duplicateID.InstrumentID == nil ||
		*duplicateID.InstrumentID != instrumentID {
		t.Fatalf("duplicate id payload = %#v", duplicateID)
	}

	_, err = service.RegisterWithID(aapl, otherID)
	var duplicateInstrument *RegistrationError
	if !errors.As(err, &duplicateInstrument) ||
		!errors.Is(err, ErrDuplicateInstrument) {
		t.Fatalf("duplicate instrument error = %v, want RegistrationError", err)
	}
	if duplicateInstrument.Kind != RegistrationErrorKindDuplicateInstrument ||
		duplicateInstrument.Instrument == nil ||
		duplicateInstrument.Instrument.String() != "AAPL/USD" {
		t.Fatalf("duplicate instrument payload = %#v", duplicateInstrument)
	}
}

func TestUnknownInstrumentIDErrorCarriesID(t *testing.T) {
	service, err := NewBuilder(InfiniteTTL()).NoSync().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer service.Close()

	instrumentID := NewInstrumentIDFromUint64(999)
	err = service.Push(instrumentID, NewQuote())
	var unknown *UnknownInstrumentIDError
	if !errors.As(err, &unknown) || !errors.Is(err, ErrUnknownInstrument) {
		t.Fatalf("Push() error = %v, want UnknownInstrumentIDError", err)
	}
	if unknown.InstrumentID != instrumentID {
		t.Fatalf("unknown id = %v, want %v", unknown.InstrumentID, instrumentID)
	}
}

func TestRegistrationErrorZeroValueIsSafe(t *testing.T) {
	err := &RegistrationError{}
	if got := err.Error(); got != "market-data registration failed" {
		t.Fatalf("Error() = %q", got)
	}
	if got := err.Unwrap(); got != nil {
		t.Fatalf("Unwrap() = %v, want nil", got)
	}

	missingID := &RegistrationError{Kind: RegistrationErrorKindDuplicateID}
	if got := missingID.Error(); got != "market-data registration failed" {
		t.Fatalf("missing-id Error() = %q", got)
	}
}
