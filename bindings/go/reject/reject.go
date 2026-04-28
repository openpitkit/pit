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

package reject

import (
	"unsafe"

	"go.openpit.dev/openpit/internal/native"
)

type Scope uint8

const (
	ScopeOrder   Scope = native.RejectScopeOrder
	ScopeAccount Scope = native.RejectScopeAccount
)

type Code native.RejectCode

const (
	CodeMissingRequiredField        Code = native.RejectCodeMissingRequiredField
	CodeInvalidFieldFormat          Code = native.RejectCodeInvalidFieldFormat
	CodeInvalidFieldValue           Code = native.RejectCodeInvalidFieldValue
	CodeUnsupportedOrderType        Code = native.RejectCodeUnsupportedOrderType
	CodeUnsupportedTimeInForce      Code = native.RejectCodeUnsupportedTimeInForce
	CodeUnsupportedOrderAttribute   Code = native.RejectCodeUnsupportedOrderAttribute
	CodeDuplicateClientOrderID      Code = native.RejectCodeDuplicateClientOrderID
	CodeTooLateToEnter              Code = native.RejectCodeTooLateToEnter
	CodeExchangeClosed              Code = native.RejectCodeExchangeClosed
	CodeUnknownInstrument           Code = native.RejectCodeUnknownInstrument
	CodeUnknownAccount              Code = native.RejectCodeUnknownAccount
	CodeUnknownVenue                Code = native.RejectCodeUnknownVenue
	CodeUnknownClearingAccount      Code = native.RejectCodeUnknownClearingAccount
	CodeUnknownCollateralAsset      Code = native.RejectCodeUnknownCollateralAsset
	CodeInsufficientFunds           Code = native.RejectCodeInsufficientFunds
	CodeInsufficientMargin          Code = native.RejectCodeInsufficientMargin
	CodeInsufficientPosition        Code = native.RejectCodeInsufficientPosition
	CodeCreditLimitExceeded         Code = native.RejectCodeCreditLimitExceeded
	CodeRiskLimitExceeded           Code = native.RejectCodeRiskLimitExceeded
	CodeOrderExceedsLimit           Code = native.RejectCodeOrderExceedsLimit
	CodeOrderQtyExceedsLimit        Code = native.RejectCodeOrderQtyExceedsLimit
	CodeOrderNotionalExceedsLimit   Code = native.RejectCodeOrderNotionalExceedsLimit
	CodePositionLimitExceeded       Code = native.RejectCodePositionLimitExceeded
	CodeConcentrationLimitExceeded  Code = native.RejectCodeConcentrationLimitExceeded
	CodeLeverageLimitExceeded       Code = native.RejectCodeLeverageLimitExceeded
	CodeRateLimitExceeded           Code = native.RejectCodeRateLimitExceeded
	CodePnlKillSwitchTriggered      Code = native.RejectCodePnlKillSwitchTriggered
	CodeAccountBlocked              Code = native.RejectCodeAccountBlocked
	CodeAccountNotAuthorized        Code = native.RejectCodeAccountNotAuthorized
	CodeComplianceRestriction       Code = native.RejectCodeComplianceRestriction
	CodeInstrumentRestricted        Code = native.RejectCodeInstrumentRestricted
	CodeJurisdictionRestriction     Code = native.RejectCodeJurisdictionRestriction
	CodeWashTradePrevention         Code = native.RejectCodeWashTradePrevention
	CodeSelfMatchPrevention         Code = native.RejectCodeSelfMatchPrevention
	CodeShortSaleRestriction        Code = native.RejectCodeShortSaleRestriction
	CodeRiskConfigurationMissing    Code = native.RejectCodeRiskConfigurationMissing
	CodeReferenceDataUnavailable    Code = native.RejectCodeReferenceDataUnavailable
	CodeOrderValueCalculationFailed Code = native.RejectCodeOrderValueCalculationFailed
	CodeSystemUnavailable           Code = native.RejectCodeSystemUnavailable
	CodeCustom                      Code = native.RejectCodeCustom
	CodeOther                       Code = native.RejectCodeOther
)

type Reject struct {
	// Human-readable reject reason.
	Reason string
	// Case-specific reject details.
	Details string
	// Policy name that produced the reject.
	Policy string
	// Opaque caller-defined payload copied through reject paths.
	//
	// Nil means "not set". Ownership and lifecycle are caller-managed.
	UserData unsafe.Pointer
	// Stable machine-readable reject code.
	Code Code
	// Reject scope.
	Scope Scope
}

func New(
	code Code, // stable machine-readable reject code
	policy string, // policy name that produced the reject
	reason string, // human-readable reject reason
	details string, // case-specific reject details
	scope Scope, // reject scope
) Reject {
	return Reject{
		Code:     code,
		Scope:    scope,
		Policy:   policy,
		Reason:   reason,
		Details:  details,
		UserData: nil,
	}
}

// NewFromHandle creates a Reject from a Reject handle with data copied from
// the handle.
func NewFromHandle(handle native.Reject) Reject {
	return Reject{
		Code:     Code(native.RejectGetCode(handle)),
		Scope:    Scope(native.RejectGetScope(handle)),
		Policy:   native.RejectGetPolicy(handle).Safe(),
		Reason:   native.RejectGetReason(handle).Safe(),
		Details:  native.RejectGetDetails(handle).Safe(),
		UserData: native.RejectGetUserData(handle),
	}
}

// NewHandle returns a native Reject handle that refers to the current Reject
// data.
func (r Reject) NewHandle() native.Reject {
	return native.CreateReject(
		native.RejectCode(r.Code),
		native.RejectScope(r.Scope),
		native.NewStringView(r.Policy),
		native.NewStringView(r.Reason),
		native.NewStringView(r.Details),
		r.UserData,
	)
}

func (r Reject) WithUserData(userData unsafe.Pointer) Reject {
	r.UserData = userData
	return r
}
