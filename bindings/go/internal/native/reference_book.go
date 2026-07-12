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

/*
#include "openpit.h"
*/
import "C"

//------------------------------------------------------------------------------
// ReferenceBook

func CreateReferenceBook() ReferenceBook {
	return C.openpit_create_reference_book()
}

func DestroyReferenceBook(book ReferenceBook) {
	C.openpit_destroy_reference_book(book)
}

func ReferenceBookRegister(
	book ReferenceBook,
	instrument Instrument,
) (ReferenceBookRegisterStatus, InstrumentID, error) {
	var outID InstrumentID
	var outError SharedString
	status := C.openpit_reference_book_register(
		book,
		&instrument,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == ReferenceBookRegisterStatusError {
		return status, outID,
			consumeSharedStringAsError(outError, "openpit_reference_book_register failed")
	}
	return status, outID, nil
}

func ReferenceBookRegisterWithID(
	book ReferenceBook,
	instrument Instrument,
	id InstrumentID,
) (ReferenceBookRegisterStatus, InstrumentID, error) {
	var outID InstrumentID
	var outError SharedString
	status := C.openpit_reference_book_register_with_id(
		book,
		&instrument,
		id,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == ReferenceBookRegisterStatusError {
		return status, outID,
			consumeSharedStringAsError(outError, "openpit_reference_book_register_with_id failed")
	}
	return status, outID, nil
}

func ReferenceBookResolve(book ReferenceBook, instrument Instrument) (InstrumentID, bool) {
	var outID InstrumentID
	ok := bool(C.openpit_reference_book_resolve(book, &instrument, &outID)) //nolint:gocritic // C bool conversion is required at the FFI boundary.
	return outID, ok
}

func ReferenceBookSetSettlementScheme(
	book ReferenceBook,
	id InstrumentID,
	scheme SettlementScheme,
) (ReferenceBookStatus, error) {
	var outError SharedString
	status := C.openpit_reference_book_set_settlement_scheme(
		book,
		id,
		scheme,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == ReferenceBookStatusError {
		return status, consumeSharedStringAsError(
			outError,
			"openpit_reference_book_set_settlement_scheme failed",
		)
	}
	return status, nil
}

func ReferenceBookClearSettlementScheme(
	book ReferenceBook,
	id InstrumentID,
) (ReferenceBookStatus, error) {
	var outError SharedString
	status := C.openpit_reference_book_clear_settlement_scheme(
		book,
		id,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == ReferenceBookStatusError {
		return status, consumeSharedStringAsError(
			outError,
			"openpit_reference_book_clear_settlement_scheme failed",
		)
	}
	return status, nil
}

func ReferenceBookGetSettlementScheme(
	book ReferenceBook,
	id InstrumentID,
) (ReferenceBookStatus, SettlementScheme, bool, error) {
	var out SettlementScheme
	var outIsSet C.bool
	var outError SharedString
	status := C.openpit_reference_book_get_settlement_scheme(
		book,
		id,
		&out,
		&outIsSet,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == ReferenceBookStatusError {
		return status, out, false, consumeSharedStringAsError(
			outError,
			"openpit_reference_book_get_settlement_scheme failed",
		)
	}
	return status, out, bool(outIsSet), nil
}

func NewSettlementLag(n uint64, unit SettlementUnit) SettlementLag {
	return SettlementLag{n: C.uint64_t(n), unit: unit}
}

func NewSettlementScheme(delivery SettlementLag, payment SettlementLag) SettlementScheme {
	return SettlementScheme{delivery: delivery, payment: payment}
}

func SettlementSchemeParts(
	scheme SettlementScheme,
) (deliveryN uint64, deliveryUnit SettlementUnit, paymentN uint64, paymentUnit SettlementUnit) {
	return uint64(scheme.delivery.n), scheme.delivery.unit, uint64(scheme.payment.n), scheme.payment.unit
}
