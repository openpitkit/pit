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

package reject

import "go.openpit.dev/openpit/internal/native"

// Scope identifies whether a reject applies to an order or an account.
type Scope uint8

const (
	// ScopeOrder means the reject applies to a single order.
	ScopeOrder Scope = native.RejectScopeOrder
	// ScopeAccount means the reject applies at the account level.
	ScopeAccount Scope = native.RejectScopeAccount
)

// RuleMode specifies the action to take when a pre-trade rule is breached.
// Its zero value is RuleModeReject. An unknown mode replaces the policy's entire
// result with a CodeSystemUnavailable reject.
type RuleMode uint8

const (
	// RuleModeReject rejects the order.
	RuleModeReject RuleMode = native.RuleModeReject
	// RuleModeReviewWithHold requires human review. Commit to keep holds
	// only if every mark is with-hold. Any without-hold mark clears them.
	// Keep the order unsent. On approval, rerun with Confirmed and add
	// AlreadyHeld only if Spot Funds kept its own hold in the committed review.
	// A Spot Funds instance registered in a review mode that marked the order
	// holds nothing. After such a review, rerun with Confirmed only.
	RuleModeReviewWithHold RuleMode = native.RuleModeReviewWithHold
	// RuleModeReviewWithoutHold requires human review with nothing held.
	// The reservation carries no mutations.
	RuleModeReviewWithoutHold RuleMode = native.RuleModeReviewWithoutHold
)

// Code is a stable machine-readable reject code.
type Code native.PretradeRejectCode

// IsEvaluationFailure reports whether the code means a policy could not
// evaluate the historical order. Evaluation failures abort an atomic drop-copy
// operation without applying any of its effects.
func (c Code) IsEvaluationFailure() bool {
	return native.PretradeRejectCodeIsEvaluationFailure(native.PretradeRejectCode(c))
}

// Predefined reject codes used by built-in policies and available to custom policies.
const (
	CodeMissingRequiredField            Code = native.RejectCodeMissingRequiredField
	CodeInvalidFieldFormat              Code = native.RejectCodeInvalidFieldFormat
	CodeInvalidFieldValue               Code = native.RejectCodeInvalidFieldValue
	CodeUnsupportedOrderType            Code = native.RejectCodeUnsupportedOrderType
	CodeUnsupportedTimeInForce          Code = native.RejectCodeUnsupportedTimeInForce
	CodeUnsupportedOrderAttribute       Code = native.RejectCodeUnsupportedOrderAttribute
	CodeDuplicateClientOrderID          Code = native.RejectCodeDuplicateClientOrderID
	CodeTooLateToEnter                  Code = native.RejectCodeTooLateToEnter
	CodeExchangeClosed                  Code = native.RejectCodeExchangeClosed
	CodeUnknownInstrument               Code = native.RejectCodeUnknownInstrument
	CodeUnknownAccount                  Code = native.RejectCodeUnknownAccount
	CodeUnknownVenue                    Code = native.RejectCodeUnknownVenue
	CodeUnknownClearingAccount          Code = native.RejectCodeUnknownClearingAccount
	CodeUnknownCollateralAsset          Code = native.RejectCodeUnknownCollateralAsset
	CodeInsufficientFunds               Code = native.RejectCodeInsufficientFunds
	CodeInsufficientMargin              Code = native.RejectCodeInsufficientMargin
	CodeInsufficientPosition            Code = native.RejectCodeInsufficientPosition
	CodeCreditLimitExceeded             Code = native.RejectCodeCreditLimitExceeded
	CodeRiskLimitExceeded               Code = native.RejectCodeRiskLimitExceeded
	CodeOrderExceedsLimit               Code = native.RejectCodeOrderExceedsLimit
	CodeOrderQtyExceedsLimit            Code = native.RejectCodeOrderQtyExceedsLimit
	CodeOrderNotionalExceedsLimit       Code = native.RejectCodeOrderNotionalExceedsLimit
	CodePositionLimitExceeded           Code = native.RejectCodePositionLimitExceeded
	CodeConcentrationLimitExceeded      Code = native.RejectCodeConcentrationLimitExceeded
	CodeLeverageLimitExceeded           Code = native.RejectCodeLeverageLimitExceeded
	CodeRateLimitExceeded               Code = native.RejectCodeRateLimitExceeded
	CodePnlKillSwitchTriggered          Code = native.RejectCodePnlKillSwitchTriggered
	CodeAccountBlocked                  Code = native.RejectCodeAccountBlocked
	CodeAccountNotAuthorized            Code = native.RejectCodeAccountNotAuthorized
	CodeComplianceRestriction           Code = native.RejectCodeComplianceRestriction
	CodeInstrumentRestricted            Code = native.RejectCodeInstrumentRestricted
	CodeJurisdictionRestriction         Code = native.RejectCodeJurisdictionRestriction
	CodeWashTradePrevention             Code = native.RejectCodeWashTradePrevention
	CodeSelfMatchPrevention             Code = native.RejectCodeSelfMatchPrevention
	CodeShortSaleRestriction            Code = native.RejectCodeShortSaleRestriction
	CodeRiskConfigurationMissing        Code = native.RejectCodeRiskConfigurationMissing
	CodeReferenceDataUnavailable        Code = native.RejectCodeReferenceDataUnavailable
	CodeOrderValueCalculationFailed     Code = native.RejectCodeOrderValueCalculationFailed
	CodeSystemUnavailable               Code = native.RejectCodeSystemUnavailable
	CodeMarkPriceUnavailable            Code = native.RejectCodeMarkPriceUnavailable
	CodeAccountAdjustmentBoundsExceeded Code = native.RejectCodeAccountAdjustmentBoundsExceeded
	CodeArithmeticOverflow              Code = native.RejectCodeArithmeticOverflow
	CodeCustom                          Code = native.RejectCodeCustom
	CodeOther                           Code = native.RejectCodeOther
)

// Reject represents a single pre-trade rejection or review mark.
type Reject struct {
	// Human-readable reject reason.
	Reason string
	// Case-specific reject details.
	Details string
	// Policy name that produced the reject.
	Policy string
	// Opaque integer token the engine carries verbatim; 0 means not set. The
	// SDK never inspects or dereferences it.
	UserData uintptr
	// Stable machine-readable reject code.
	Code Code
	// Reject scope.
	Scope Scope
	// Mode defaults to RuleModeReject. It specifies rejection, review with
	// holds, or review without holds. Evaluation failures are always hard rejects.
	// An unknown mode from a custom policy replaces that policy's entire
	// result with a CodeSystemUnavailable reject.
	Mode RuleMode
}

// New creates a Reject with mode RuleModeReject. Set Mode for review.
// In pre-trade, holds survive review only if
// every mark is RuleModeReviewWithHold. Any RuleModeReviewWithoutHold clears them.
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
		UserData: 0,
	}
}

// NewFromHandle creates a Reject from a Reject handle with data copied from
// the handle.
func NewFromHandle(handle native.PretradeReject) Reject {
	return Reject{
		Code:     Code(native.PretradeRejectGetCode(handle)),
		Scope:    Scope(native.PretradeRejectGetScope(handle)),
		Mode:     RuleMode(native.PretradeRejectGetMode(handle)),
		Policy:   native.PretradeRejectGetPolicy(handle).Safe(),
		Reason:   native.PretradeRejectGetReason(handle).Safe(),
		Details:  native.PretradeRejectGetDetails(handle).Safe(),
		UserData: native.PretradeRejectGetUserData(handle),
	}
}

// NewHandle returns a native Reject handle that refers to the current Reject
// data.
func (r Reject) NewHandle() native.PretradeReject {
	return native.CreatePretradeReject(
		native.PretradeRejectCode(r.Code),
		native.PretradeRejectScope(r.Scope),
		native.PretradeRuleMode(r.Mode),
		native.NewStringView(r.Policy),
		native.NewStringView(r.Reason),
		native.NewStringView(r.Details),
		r.UserData,
	)
}

// WithUserData returns a copy of Reject with UserData set to userData, an
// opaque integer token the engine carries verbatim; 0 means not set. The SDK
// never inspects or dereferences it.
//
// Uses copy-on-write semantics. Original instance is unchanged.
func (r Reject) WithUserData(userData uintptr) Reject {
	r.UserData = userData
	return r
}
