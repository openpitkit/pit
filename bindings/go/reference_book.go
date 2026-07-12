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
	"fmt"
	"runtime"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
)

// Errors returned by ReferenceBook registration and settlement configuration.
var (
	// ErrReferenceBookDuplicateID reports an instrument ID that is already
	// registered in this book.
	ErrReferenceBookDuplicateID = errors.New("reference book: instrument id is already registered")
	// ErrReferenceBookDuplicateInstrument reports an instrument that is already
	// registered under a different ID.
	ErrReferenceBookDuplicateInstrument = errors.New(
		"reference book: instrument is already registered",
	)
	// ErrReferenceBookUnknownInstrument reports an ID that has no entry in this
	// book.
	ErrReferenceBookUnknownInstrument = errors.New("reference book: unknown instrument")
	// ErrInvalidSettlementUnit reports a settlement scheme with an unsupported
	// unit value.
	ErrInvalidSettlementUnit = errors.New("reference book: invalid settlement unit")
)

// ReferenceBookRegistrationErrorKind identifies a reference-book conflict.
type ReferenceBookRegistrationErrorKind uint8

const (
	// ReferenceBookRegistrationErrorKindDuplicateID identifies an ID collision.
	ReferenceBookRegistrationErrorKindDuplicateID ReferenceBookRegistrationErrorKind = iota + 1
	// ReferenceBookRegistrationErrorKindDuplicateInstrument identifies an
	// instrument registered under another ID.
	ReferenceBookRegistrationErrorKindDuplicateInstrument
)

// ReferenceBookRegistrationError carries a reference-book conflict payload.
// Exactly one of InstrumentID and Instrument is set according to Kind.
type ReferenceBookRegistrationError struct {
	Instrument   *param.Instrument
	InstrumentID *InstrumentID
	Kind         ReferenceBookRegistrationErrorKind
}

func (e *ReferenceBookRegistrationError) Error() string {
	switch e.Kind {
	case ReferenceBookRegistrationErrorKindDuplicateID:
		if e.InstrumentID != nil {
			return fmt.Sprintf("instrument id %s is already registered", *e.InstrumentID)
		}
	case ReferenceBookRegistrationErrorKindDuplicateInstrument:
		if e.Instrument != nil {
			return fmt.Sprintf("instrument %s is already registered", *e.Instrument)
		}
	}
	return "reference-book registration failed"
}

// Unwrap preserves the corresponding sentinel for errors.Is.
func (e *ReferenceBookRegistrationError) Unwrap() error {
	switch e.Kind {
	case ReferenceBookRegistrationErrorKindDuplicateID:
		return ErrReferenceBookDuplicateID
	case ReferenceBookRegistrationErrorKindDuplicateInstrument:
		return ErrReferenceBookDuplicateInstrument
	default:
		return nil
	}
}

// SettlementUnit specifies how a settlement lag is measured.
type SettlementUnit uint8

const (
	// SettlementUnitBusinessDays measures lags in calendar-defined business days.
	SettlementUnitBusinessDays SettlementUnit = iota
	// SettlementUnitCalendarDays measures lags in consecutive calendar days.
	SettlementUnitCalendarDays
)

// SettlementLag is the delay for one settlement leg.
type SettlementLag struct {
	N    uint64
	Unit SettlementUnit
}

// NewSettlementLag creates a settlement lag.
func NewSettlementLag(n uint64, unit SettlementUnit) SettlementLag {
	return SettlementLag{N: n, Unit: unit}
}

// SettlementScheme configures independent delivery and payment settlement lags.
type SettlementScheme struct {
	Delivery SettlementLag
	Payment  SettlementLag
}

// NewSettlementScheme creates a settlement scheme with independent legs.
func NewSettlementScheme(delivery SettlementLag, payment SettlementLag) SettlementScheme {
	return SettlementScheme{Delivery: delivery, Payment: payment}
}

// UniformSettlementScheme creates a scheme where both legs settle after n
// business days.
func UniformSettlementScheme(n uint64) SettlementScheme {
	lag := NewSettlementLag(n, SettlementUnitBusinessDays)
	return NewSettlementScheme(lag, lag)
}

// ReferenceBook stores stable instrument identities and typed per-instrument
// reference attributes independently from market data.
type ReferenceBook struct{ handle native.ReferenceBook }

// NewReferenceBook creates an empty instrument reference book.
func NewReferenceBook() *ReferenceBook {
	return &ReferenceBook{handle: native.CreateReferenceBook()}
}

// Close releases this reference-book handle. It is safe to call more than
// once; subsequent calls have no effect.
func (b *ReferenceBook) Close() {
	native.DestroyReferenceBook(b.handle)
	b.handle = nil
}

// Register assigns the next available ID to instrument.
func (b *ReferenceBook) Register(instrument param.Instrument) (InstrumentID, error) {
	status, id, err := native.ReferenceBookRegister(b.handle, instrument.Handle())
	runtime.KeepAlive(instrument)
	switch status {
	case native.ReferenceBookRegisterStatusOK:
		return newInstrumentIDFromHandle(id), nil
	case native.ReferenceBookRegisterStatusDuplicateInstrument:
		return InstrumentID{}, &ReferenceBookRegistrationError{
			Instrument: &instrument,
			Kind:       ReferenceBookRegistrationErrorKindDuplicateInstrument,
		}
	default:
		return InstrumentID{}, err
	}
}

// RegisterWithID records instrument under caller-assigned id. The same ID can
// be reused with marketdata.Service.RegisterWithID without coupling the two
// registries.
func (b *ReferenceBook) RegisterWithID(
	instrument param.Instrument,
	id InstrumentID,
) (InstrumentID, error) {
	status, outID, err := native.ReferenceBookRegisterWithID(
		b.handle,
		instrument.Handle(),
		id.Handle(),
	)
	runtime.KeepAlive(instrument)
	switch status {
	case native.ReferenceBookRegisterStatusOK:
		return newInstrumentIDFromHandle(outID), nil
	case native.ReferenceBookRegisterStatusDuplicateID:
		return InstrumentID{}, &ReferenceBookRegistrationError{
			InstrumentID: &id,
			Kind:         ReferenceBookRegistrationErrorKindDuplicateID,
		}
	case native.ReferenceBookRegisterStatusDuplicateInstrument:
		return InstrumentID{}, &ReferenceBookRegistrationError{
			Instrument: &instrument,
			Kind:       ReferenceBookRegistrationErrorKindDuplicateInstrument,
		}
	default:
		return InstrumentID{}, err
	}
}

// Resolve returns instrument's registered ID and whether it is present.
func (b *ReferenceBook) Resolve(instrument param.Instrument) (InstrumentID, bool) {
	id, ok := native.ReferenceBookResolve(b.handle, instrument.Handle())
	runtime.KeepAlive(instrument)
	return newInstrumentIDFromHandle(id), ok
}

// SetSettlementScheme sets settlement configuration for a registered
// instrument.
func (b *ReferenceBook) SetSettlementScheme(id InstrumentID, scheme SettlementScheme) error {
	raw, err := scheme.toNative()
	if err != nil {
		return err
	}
	status, err := native.ReferenceBookSetSettlementScheme(b.handle, id.Handle(), raw)
	if status == native.ReferenceBookStatusOK {
		return nil
	}
	if status == native.ReferenceBookStatusUnknownInstrument {
		return ErrReferenceBookUnknownInstrument
	}
	return err
}

// ClearSettlementScheme removes settlement configuration from a registered
// instrument.
func (b *ReferenceBook) ClearSettlementScheme(id InstrumentID) error {
	status, err := native.ReferenceBookClearSettlementScheme(b.handle, id.Handle())
	if status == native.ReferenceBookStatusOK {
		return nil
	}
	if status == native.ReferenceBookStatusUnknownInstrument {
		return ErrReferenceBookUnknownInstrument
	}
	return err
}

// SettlementScheme returns configured settlement data. A nil error and false
// result mean that id is registered but has no scheme. An unknown id returns
// ErrReferenceBookUnknownInstrument.
func (b *ReferenceBook) SettlementScheme(id InstrumentID) (SettlementScheme, bool, error) {
	status, raw, ok, err := native.ReferenceBookGetSettlementScheme(b.handle, id.Handle())
	switch status {
	case native.ReferenceBookStatusOK:
		if !ok {
			return SettlementScheme{}, false, nil
		}
		return settlementSchemeFromNative(raw), true, nil
	case native.ReferenceBookStatusUnknownInstrument:
		return SettlementScheme{}, false, ErrReferenceBookUnknownInstrument
	default:
		return SettlementScheme{}, false, err
	}
}

func (scheme SettlementScheme) toNative() (native.SettlementScheme, error) {
	delivery, err := settlementLagToNative(scheme.Delivery)
	if err != nil {
		return native.SettlementScheme{}, err
	}
	payment, err := settlementLagToNative(scheme.Payment)
	if err != nil {
		return native.SettlementScheme{}, err
	}
	return native.NewSettlementScheme(delivery, payment), nil
}

func settlementLagToNative(lag SettlementLag) (native.SettlementLag, error) {
	unit, err := settlementUnitToNative(lag.Unit)
	if err != nil {
		return native.SettlementLag{}, err
	}
	return native.NewSettlementLag(lag.N, unit), nil
}

func settlementUnitToNative(unit SettlementUnit) (native.SettlementUnit, error) {
	switch unit {
	case SettlementUnitBusinessDays:
		return native.SettlementUnitBusinessDays, nil
	case SettlementUnitCalendarDays:
		return native.SettlementUnitCalendarDays, nil
	default:
		return 0, ErrInvalidSettlementUnit
	}
}

func settlementSchemeFromNative(raw native.SettlementScheme) SettlementScheme {
	deliveryN, deliveryUnit, paymentN, paymentUnit := native.SettlementSchemeParts(raw)
	return NewSettlementScheme(
		SettlementLag{N: deliveryN, Unit: SettlementUnit(deliveryUnit)},
		SettlementLag{N: paymentN, Unit: SettlementUnit(paymentUnit)},
	)
}
