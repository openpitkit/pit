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

static OpenPitMarketDataGetStatus openpit_marketdata_service_get_cb(
    const OpenPitMarketDataService *service,
    OpenPitMarketDataInstrumentId id,
    OpenPitParamAccountId account,
    void *resolve_fn_ptr,
    void *user_data,
    OpenPitMarketDataQuoteResolution resolution,
    OpenPitMarketDataQuote *out_quote) {
    return openpit_marketdata_service_get(
        service, id, account,
        *(OpenPitMarketDataAccountGroupResolver *)resolve_fn_ptr,
        user_data, resolution, out_quote);
}
*/
import "C"

import "unsafe"

//------------------------------------------------------------------------------
// MarketDataQuote

func CreateMarketDataQuote() MarketDataQuote {
	return C.openpit_create_marketdata_quote()
}

func MarketDataQuoteSetMark(quote *MarketDataQuote, mark ParamPrice) {
	quote.mark.value = mark
	quote.mark.is_set = true
}

func MarketDataQuoteSetBid(quote *MarketDataQuote, bid ParamPrice) {
	quote.bid.value = bid
	quote.bid.is_set = true
}

func MarketDataQuoteSetAsk(quote *MarketDataQuote, ask ParamPrice) {
	quote.ask.value = ask
	quote.ask.is_set = true
}

func MarketDataQuoteGetMark(quote MarketDataQuote) ParamPriceOptional {
	return quote.mark
}

func MarketDataQuoteGetBid(quote MarketDataQuote) ParamPriceOptional {
	return quote.bid
}

func MarketDataQuoteGetAsk(quote MarketDataQuote) ParamPriceOptional {
	return quote.ask
}

//------------------------------------------------------------------------------
// MarketDataQuoteTTL

func CreateMarketDataQuoteTTLInfinite() MarketDataQuoteTTL {
	return C.openpit_create_marketdata_quote_ttl_infinite()
}

func CreateMarketDataQuoteTTLWithin(secs uint64, nanos uint32) MarketDataQuoteTTL {
	return C.openpit_create_marketdata_quote_ttl_within(C.uint64_t(secs), C.uint32_t(nanos))
}

//------------------------------------------------------------------------------
// MarketDataService (creation)

func CreateMarketDataService(
	syncPolicy SyncPolicy,
	defaultTTL MarketDataQuoteTTL,
) (MarketDataService, error) {
	var outError SharedString
	service := C.openpit_create_marketdata_service(
		syncPolicy,
		defaultTTL,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if service == nil {
		return nil, consumeSharedStringAsError(outError, "openpit_create_marketdata_service failed")
	}
	return service, nil
}

//------------------------------------------------------------------------------
// MarketDataService

func DestroyMarketDataService(service MarketDataService) {
	C.openpit_destroy_marketdata_service(service)
}

func MarketDataServiceClone(service MarketDataService) MarketDataService {
	return C.openpit_marketdata_service_clone(service)
}

func MarketDataServiceRegister(
	service MarketDataService,
	instrument Instrument,
) (MarketDataRegisterStatus, MarketDataInstrumentID, error) {
	var outID MarketDataInstrumentID
	var outError SharedString
	status := C.openpit_marketdata_service_register(
		service,
		&instrument,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status, outID,
			consumeSharedStringAsError(outError, "openpit_marketdata_service_register failed")
	}
	return status, outID, nil
}

func MarketDataServiceRegisterWithTTL(
	service MarketDataService,
	instrument Instrument,
	ttl MarketDataQuoteTTL,
) (MarketDataRegisterStatus, MarketDataInstrumentID, error) {
	var outID MarketDataInstrumentID
	var outError SharedString
	status := C.openpit_marketdata_service_register_with_ttl(
		service,
		&instrument,
		ttl,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status, outID,
			consumeSharedStringAsError(outError, "openpit_marketdata_service_register_with_ttl failed")
	}
	return status, outID, nil
}

func MarketDataServiceRegisterWithID(
	service MarketDataService,
	instrument Instrument,
	id MarketDataInstrumentID,
) (MarketDataRegisterStatus, MarketDataInstrumentID, error) {
	var outID MarketDataInstrumentID
	var outError SharedString
	status := C.openpit_marketdata_service_register_with_id(
		service,
		&instrument,
		id,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status, outID,
			consumeSharedStringAsError(outError, "openpit_marketdata_service_register_with_id failed")
	}
	return status, outID, nil
}

func MarketDataServiceRegisterWithIDAndTTL(
	service MarketDataService,
	instrument Instrument,
	id MarketDataInstrumentID,
	ttl MarketDataQuoteTTL,
) (MarketDataRegisterStatus, MarketDataInstrumentID, error) {
	var outID MarketDataInstrumentID
	var outError SharedString
	status := C.openpit_marketdata_service_register_with_id_and_ttl(
		service,
		&instrument,
		id,
		ttl,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status, outID,
			consumeSharedStringAsError(
				outError,
				"openpit_marketdata_service_register_with_id_and_ttl failed",
			)
	}
	return status, outID, nil
}

func MarketDataServiceClear(service MarketDataService, instrumentID MarketDataInstrumentID) {
	C.openpit_marketdata_service_clear(service, instrumentID)
}

func MarketDataServicePush(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	quote MarketDataQuote,
) (MarketDataRegisterStatus, error) {
	var outError SharedString
	status := C.openpit_marketdata_service_push(
		service,
		instrumentID,
		quote,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status, consumeSharedStringAsError(outError, "openpit_marketdata_service_push failed")
	}
	return status, nil
}

func MarketDataServicePushPatch(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	quote MarketDataQuote,
) (MarketDataRegisterStatus, error) {
	var outError SharedString
	status := C.openpit_marketdata_service_push_patch(
		service,
		instrumentID,
		quote,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status,
			consumeSharedStringAsError(outError, "openpit_marketdata_service_push_patch failed")
	}
	return status, nil
}

func MarketDataServicePushByInstrument(
	service MarketDataService,
	instrument Instrument,
	quote MarketDataQuote,
) (MarketDataInstrumentID, error) {
	var outID MarketDataInstrumentID
	var outError SharedString
	if !C.openpit_marketdata_service_push_by_instrument(
		service,
		&instrument,
		quote,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outID,
			consumeSharedStringAsError(outError, "openpit_marketdata_service_push_by_instrument failed")
	}
	return outID, nil
}

func MarketDataServicePushByInstrumentPatch(
	service MarketDataService,
	instrument Instrument,
	quote MarketDataQuote,
) (MarketDataInstrumentID, error) {
	var outID MarketDataInstrumentID
	var outError SharedString
	if !C.openpit_marketdata_service_push_by_instrument_patch(
		service,
		&instrument,
		quote,
		&outID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outID,
			consumeSharedStringAsError(
				outError,
				"openpit_marketdata_service_push_by_instrument_patch failed",
			)
	}
	return outID, nil
}

func MarketDataServicePushFor(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	quote MarketDataQuote,
	accountIDs []ParamAccountID,
	accountGroupIDs []ParamAccountGroupID,
) (MarketDataRegisterStatus, error) {
	var outError SharedString
	var accountsPtr *C.OpenPitParamAccountId
	var groupsPtr *C.OpenPitParamAccountGroupId
	if len(accountIDs) > 0 {
		accountsPtr = &accountIDs[0]
	}
	if len(accountGroupIDs) > 0 {
		groupsPtr = &accountGroupIDs[0]
	}
	status := C.openpit_marketdata_service_push_for(
		service,
		instrumentID,
		quote,
		accountsPtr,
		C.size_t(len(accountIDs)),
		groupsPtr,
		C.size_t(len(accountGroupIDs)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status,
			consumeSharedStringAsError(outError, "openpit_marketdata_service_push_for failed")
	}
	return status, nil
}

func MarketDataServicePushForPatch(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	quote MarketDataQuote,
	accountIDs []ParamAccountID,
	accountGroupIDs []ParamAccountGroupID,
) (MarketDataRegisterStatus, error) {
	var outError SharedString
	var accountsPtr *C.OpenPitParamAccountId
	var groupsPtr *C.OpenPitParamAccountGroupId
	if len(accountIDs) > 0 {
		accountsPtr = &accountIDs[0]
	}
	if len(accountGroupIDs) > 0 {
		groupsPtr = &accountGroupIDs[0]
	}
	status := C.openpit_marketdata_service_push_for_patch(
		service,
		instrumentID,
		quote,
		accountsPtr,
		C.size_t(len(accountIDs)),
		groupsPtr,
		C.size_t(len(accountGroupIDs)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if status == MarketDataRegisterStatusError {
		return status,
			consumeSharedStringAsError(outError, "openpit_marketdata_service_push_for_patch failed")
	}
	return status, nil
}

func MarketDataServiceGet(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	accountID ParamAccountID,
	resolveAccountGroup unsafe.Pointer,
	userData unsafe.Pointer,
	resolution MarketDataQuoteResolution,
) (MarketDataGetStatus, MarketDataQuote) {
	var outQuote MarketDataQuote
	status := C.openpit_marketdata_service_get_cb(
		service,
		instrumentID,
		accountID,
		resolveAccountGroup,
		userData,
		resolution,
		&outQuote,
	)
	return status, outQuote
}

func MarketDataServiceResolve(
	service MarketDataService,
	instrument Instrument,
) (MarketDataInstrumentID, bool) {
	var outID MarketDataInstrumentID
	ok := C.openpit_marketdata_service_resolve(service, &instrument, &outID) //nolint:gocritic // CGO call; dupSubExpr is a false positive
	return outID, bool(ok)
}

func MarketDataServiceSetAccountTTL(
	service MarketDataService,
	accountID ParamAccountID,
	ttl MarketDataQuoteTTL,
) {
	C.openpit_marketdata_service_set_account_ttl(service, accountID, ttl)
}

func MarketDataServiceClearAccountTTL(service MarketDataService, accountID ParamAccountID) {
	C.openpit_marketdata_service_clear_account_ttl(service, accountID)
}

func MarketDataServiceSetAccountGroupTTL(
	service MarketDataService,
	accountGroupID ParamAccountGroupID,
	ttl MarketDataQuoteTTL,
) {
	C.openpit_marketdata_service_set_account_group_ttl(service, accountGroupID, ttl)
}

func MarketDataServiceClearAccountGroupTTL(
	service MarketDataService,
	accountGroupID ParamAccountGroupID,
) {
	C.openpit_marketdata_service_clear_account_group_ttl(service, accountGroupID)
}

func MarketDataServiceSetInstrumentTTL(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	ttl MarketDataQuoteTTL,
) MarketDataRegisterStatus {
	return C.openpit_marketdata_service_set_instrument_ttl(service, instrumentID, ttl)
}

func MarketDataServiceClearInstrumentTTL(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
) MarketDataRegisterStatus {
	return C.openpit_marketdata_service_clear_instrument_ttl(service, instrumentID)
}

func MarketDataServiceSetInstrumentAccountTTL(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	accountID ParamAccountID,
	ttl MarketDataQuoteTTL,
) MarketDataRegisterStatus {
	return C.openpit_marketdata_service_set_instrument_account_ttl(
		service,
		instrumentID,
		accountID,
		ttl,
	)
}

func MarketDataServiceClearInstrumentAccountTTL(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	accountID ParamAccountID,
) MarketDataRegisterStatus {
	return C.openpit_marketdata_service_clear_instrument_account_ttl(
		service,
		instrumentID,
		accountID,
	)
}

func MarketDataServiceSetInstrumentAccountGroupTTL(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	accountGroupID ParamAccountGroupID,
	ttl MarketDataQuoteTTL,
) MarketDataRegisterStatus {
	return C.openpit_marketdata_service_set_instrument_account_group_ttl(
		service,
		instrumentID,
		accountGroupID,
		ttl,
	)
}

func MarketDataServiceClearInstrumentAccountGroupTTL(
	service MarketDataService,
	instrumentID MarketDataInstrumentID,
	accountGroupID ParamAccountGroupID,
) MarketDataRegisterStatus {
	return C.openpit_marketdata_service_clear_instrument_account_group_ttl(
		service,
		instrumentID,
		accountGroupID,
	)
}

//------------------------------------------------------------------------------
