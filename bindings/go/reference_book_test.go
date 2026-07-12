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

package openpit

import (
	"errors"
	"testing"

	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/param"
)

func referenceBookInstrument(t *testing.T, underlying string) param.Instrument {
	t.Helper()
	asset, err := param.NewAsset(underlying)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", underlying, err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	return param.NewInstrument(asset, usd)
}

func TestReferenceBookStoresTypedSettlementAndResolvesID(t *testing.T) {
	book := NewReferenceBook()
	defer book.Close()

	aapl := referenceBookInstrument(t, "AAPL")
	id := NewInstrumentIDFromUint64(42)
	registered, err := book.RegisterWithID(aapl, id)
	if err != nil {
		t.Fatalf("RegisterWithID() error = %v", err)
	}
	if registered != id {
		t.Fatalf("RegisterWithID() id = %v, want %v", registered, id)
	}

	resolved, ok := book.Resolve(aapl)
	if !ok || resolved != id {
		t.Fatalf("Resolve() = (%v, %v), want (%v, true)", resolved, ok, id)
	}

	scheme := NewSettlementScheme(
		NewSettlementLag(2, SettlementUnitBusinessDays),
		NewSettlementLag(1, SettlementUnitCalendarDays),
	)
	if err := book.SetSettlementScheme(id, scheme); err != nil {
		t.Fatalf("SetSettlementScheme() error = %v", err)
	}
	actual, ok, err := book.SettlementScheme(id)
	if err != nil || !ok || actual != scheme {
		t.Fatalf("SettlementScheme() = (%v, %v, %v), want (%v, true, nil)", actual, ok, err, scheme)
	}
	if _, ok, err := book.SettlementScheme(NewInstrumentIDFromUint64(99)); ok || !errors.Is(err, ErrReferenceBookUnknownInstrument) {
		t.Fatalf("SettlementScheme(unknown) = (_, %v, %v), want (_, false, ErrReferenceBookUnknownInstrument)", ok, err)
	}
}

func TestReferenceBookReportsDomainErrorsAndValidatesUnits(t *testing.T) {
	book := NewReferenceBook()
	defer book.Close()

	aapl := referenceBookInstrument(t, "AAPL")
	msft := referenceBookInstrument(t, "MSFT")
	id := NewInstrumentIDFromUint64(42)
	if _, err := book.RegisterWithID(aapl, id); err != nil {
		t.Fatalf("RegisterWithID() error = %v", err)
	}
	if _, ok, err := book.SettlementScheme(id); ok || err != nil {
		t.Fatalf("SettlementScheme(unconfigured) = (_, %v, %v), want (_, false, nil)", ok, err)
	}
	_, err := book.RegisterWithID(msft, id)
	var duplicateID *ReferenceBookRegistrationError
	if !errors.As(err, &duplicateID) || !errors.Is(err, ErrReferenceBookDuplicateID) {
		t.Fatalf("duplicate id error = %v, want ReferenceBookRegistrationError", err)
	}
	if duplicateID.Kind != ReferenceBookRegistrationErrorKindDuplicateID ||
		duplicateID.InstrumentID == nil || *duplicateID.InstrumentID != id {
		t.Fatalf("duplicate id payload = %#v", duplicateID)
	}
	otherID := NewInstrumentIDFromUint64(43)
	_, err = book.RegisterWithID(aapl, otherID)
	var duplicateInstrument *ReferenceBookRegistrationError
	if !errors.As(err, &duplicateInstrument) ||
		!errors.Is(err, ErrReferenceBookDuplicateInstrument) {
		t.Fatalf("duplicate instrument error = %v, want ReferenceBookRegistrationError", err)
	}
	if duplicateInstrument.Kind != ReferenceBookRegistrationErrorKindDuplicateInstrument ||
		duplicateInstrument.Instrument == nil ||
		duplicateInstrument.Instrument.String() != aapl.String() {
		t.Fatalf("duplicate instrument payload = %#v", duplicateInstrument)
	}
	if err := book.SetSettlementScheme(NewInstrumentIDFromUint64(99), UniformSettlementScheme(1)); !errors.Is(err, ErrReferenceBookUnknownInstrument) {
		t.Fatalf("unknown id error = %v, want ErrReferenceBookUnknownInstrument", err)
	}
	if err := book.SetSettlementScheme(id, NewSettlementScheme(
		NewSettlementLag(1, SettlementUnit(99)),
		NewSettlementLag(1, SettlementUnitBusinessDays),
	)); !errors.Is(err, ErrInvalidSettlementUnit) {
		t.Fatalf("invalid unit error = %v, want ErrInvalidSettlementUnit", err)
	}
}

func TestInstrumentIDIsSharedWithMarketData(_ *testing.T) {
	root := NewInstrumentIDFromUint64(7)
	acceptMarketDataInstrumentID(root)
}

func acceptMarketDataInstrumentID(_ marketdata.InstrumentID) {}

func TestUniformSettlementSchemeDefaultsToBusinessDays(t *testing.T) {
	scheme := UniformSettlementScheme(2)
	if scheme.Delivery != NewSettlementLag(2, SettlementUnitBusinessDays) {
		t.Fatalf("delivery = %v, want 2 business days", scheme.Delivery)
	}
	if scheme.Payment != NewSettlementLag(2, SettlementUnitBusinessDays) {
		t.Fatalf("payment = %v, want 2 business days", scheme.Payment)
	}
	if (SettlementLag{}).Unit != SettlementUnitBusinessDays {
		t.Fatalf("zero SettlementLag unit = %v, want business days", (SettlementLag{}).Unit)
	}
}
