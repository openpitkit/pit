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

use std::fmt::{Display, Formatter};

/// Reject scope returned by policies.
///
/// # Examples
///
/// ```
/// use openpit::pretrade::RejectScope;
///
/// let scope = RejectScope::Order;
/// match scope {
///     RejectScope::Order => { /* retry is safe; engine remains operational */ }
///     RejectScope::Account => { /* halt trading until situation is resolved */ }
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectScope {
    /// Reject only the current order.
    Order,
    /// Account-level reject signal.
    ///
    /// Engine reports it; application decides whether to stop trading.
    Account,
}

/// Standardized reject code for blocked orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RejectCode {
    /// A mandatory order field was not provided.
    MissingRequiredField,
    /// A field format is syntactically invalid.
    InvalidFieldFormat,
    /// A field value is outside accepted domain values.
    InvalidFieldValue,
    /// The order type is not supported.
    UnsupportedOrderType,
    /// The time-in-force value is not supported.
    UnsupportedTimeInForce,
    /// A requested order attribute is unsupported.
    UnsupportedOrderAttribute,
    /// Client order ID is already in use.
    DuplicateClientOrderId,
    /// Order arrival is outside the allowed entry window.
    TooLateToEnter,
    /// Venue session is closed for trading.
    ExchangeClosed,
    /// Instrument identifier is not recognized.
    UnknownInstrument,
    /// Account identifier is not recognized.
    UnknownAccount,
    /// Venue identifier is not recognized.
    UnknownVenue,
    /// Clearing account is not recognized.
    UnknownClearingAccount,
    /// Collateral asset is not recognized.
    UnknownCollateralAsset,
    /// Available cash is not sufficient for this order.
    InsufficientFunds,
    /// Margin is insufficient for this order.
    InsufficientMargin,
    /// Position inventory is insufficient for this order.
    InsufficientPosition,
    /// Credit limit is exceeded.
    CreditLimitExceeded,
    /// A generic risk limit is exceeded.
    RiskLimitExceeded,
    /// Multiple size limits are exceeded by one order.
    OrderExceedsLimit,
    /// Quantity limit is exceeded.
    OrderQtyExceedsLimit,
    /// Notional limit is exceeded.
    OrderNotionalExceedsLimit,
    /// Position limit is exceeded.
    PositionLimitExceeded,
    /// Concentration limit is exceeded.
    ConcentrationLimitExceeded,
    /// Leverage limit is exceeded.
    LeverageLimitExceeded,
    /// Rate limit for order submissions is exceeded.
    RateLimitExceeded,
    /// PnL-based kill switch is currently triggered.
    PnlKillSwitchTriggered,
    /// Account is blocked from trading.
    AccountBlocked,
    /// Account is not authorized to place this order.
    AccountNotAuthorized,
    /// Compliance rule forbids this order.
    ComplianceRestriction,
    /// Instrument is restricted for this account or venue.
    InstrumentRestricted,
    /// Jurisdictional restriction forbids this order.
    JurisdictionRestriction,
    /// Wash-trade prevention blocked this order.
    WashTradePrevention,
    /// Self-match prevention blocked this order.
    SelfMatchPrevention,
    /// Short-sale restriction blocked this order.
    ShortSaleRestriction,
    /// Required risk configuration is missing.
    RiskConfigurationMissing,
    /// Required reference data is unavailable.
    ReferenceDataUnavailable,
    /// Order value could not be calculated.
    OrderValueCalculationFailed,
    /// Risk system is temporarily unavailable.
    SystemUnavailable,
    /// Reserved discriminant for caller-defined reject classes.
    ///
    /// Use together with `Reject::with_user_data` to attach a caller-defined
    /// payload that the receiving code can decode. The SDK does not interpret
    /// this code beyond mapping it to FFI value 254.
    Custom, // FFI: reserved as 254
    /// Reject reason does not fit a more specific code.
    Other,
}

impl RejectCode {
    /// Returns the stable string representation of this code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingRequiredField => "MissingRequiredField",
            Self::InvalidFieldFormat => "InvalidFieldFormat",
            Self::InvalidFieldValue => "InvalidFieldValue",
            Self::UnsupportedOrderType => "UnsupportedOrderType",
            Self::UnsupportedTimeInForce => "UnsupportedTimeInForce",
            Self::UnsupportedOrderAttribute => "UnsupportedOrderAttribute",
            Self::DuplicateClientOrderId => "DuplicateClientOrderId",
            Self::TooLateToEnter => "TooLateToEnter",
            Self::ExchangeClosed => "ExchangeClosed",
            Self::UnknownInstrument => "UnknownInstrument",
            Self::UnknownAccount => "UnknownAccount",
            Self::UnknownVenue => "UnknownVenue",
            Self::UnknownClearingAccount => "UnknownClearingAccount",
            Self::UnknownCollateralAsset => "UnknownCollateralAsset",
            Self::InsufficientFunds => "InsufficientFunds",
            Self::InsufficientMargin => "InsufficientMargin",
            Self::InsufficientPosition => "InsufficientPosition",
            Self::CreditLimitExceeded => "CreditLimitExceeded",
            Self::RiskLimitExceeded => "RiskLimitExceeded",
            Self::OrderExceedsLimit => "OrderExceedsLimit",
            Self::OrderQtyExceedsLimit => "OrderQtyExceedsLimit",
            Self::OrderNotionalExceedsLimit => "OrderNotionalExceedsLimit",
            Self::PositionLimitExceeded => "PositionLimitExceeded",
            Self::ConcentrationLimitExceeded => "ConcentrationLimitExceeded",
            Self::LeverageLimitExceeded => "LeverageLimitExceeded",
            Self::RateLimitExceeded => "RateLimitExceeded",
            Self::PnlKillSwitchTriggered => "PnlKillSwitchTriggered",
            Self::AccountBlocked => "AccountBlocked",
            Self::AccountNotAuthorized => "AccountNotAuthorized",
            Self::ComplianceRestriction => "ComplianceRestriction",
            Self::InstrumentRestricted => "InstrumentRestricted",
            Self::JurisdictionRestriction => "JurisdictionRestriction",
            Self::WashTradePrevention => "WashTradePrevention",
            Self::SelfMatchPrevention => "SelfMatchPrevention",
            Self::ShortSaleRestriction => "ShortSaleRestriction",
            Self::RiskConfigurationMissing => "RiskConfigurationMissing",
            Self::ReferenceDataUnavailable => "ReferenceDataUnavailable",
            Self::OrderValueCalculationFailed => "OrderValueCalculationFailed",
            Self::SystemUnavailable => "SystemUnavailable",
            Self::Custom => "Custom",
            Self::Other => "Other",
        }
    }
}

impl Display for RejectCode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Single rejection record returned by checks.
///
/// # Examples
///
/// ```
/// use openpit::pretrade::{Reject, RejectCode, RejectScope};
///
/// let reject = Reject::new(
///     "RateLimitPolicy",
///     RejectScope::Order,
///     RejectCode::RateLimitExceeded,
///     "rate limit exceeded",
///     "submitted 3 orders in 1s window, max allowed: 2",
/// );
/// assert_eq!(reject.code, RejectCode::RateLimitExceeded);
/// assert_eq!(reject.reason, "rate limit exceeded");
/// assert_eq!(reject.details, "submitted 3 orders in 1s window, max allowed: 2");
/// assert_eq!(reject.policy, "RateLimitPolicy");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Reject {
    /// Human-readable reject reason.
    pub reason: String,
    /// Case-specific reject details.
    pub details: String,
    /// Policy name that produced the reject.
    pub policy: String,
    /// Opaque caller-defined token.
    ///
    /// The SDK never inspects, dereferences, or frees this value. Its meaning,
    /// lifetime, and thread-safety are the caller's responsibility. `0` / null
    /// means "not set". See the project Threading Contract for the full lifetime
    /// model.
    ///
    /// The token flows through every reject path the SDK exposes (start-stage,
    /// main-stage, account-adjustment, batch results) and is preserved on
    /// `Clone`.
    pub user_data: usize,
    /// Stable machine-readable reject code.
    pub code: RejectCode,
    /// Reject scope.
    pub scope: RejectScope,
}

impl Display for Reject {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "[{}] {}: {}",
            self.policy, self.reason, self.details
        )
    }
}

impl std::error::Error for Reject {}

/// Collection of rejects returned by [`PreTradeRequest::execute`].
///
/// Implements `Deref` to `[Reject]` for direct element access.
///
/// [`PreTradeRequest::execute`]: crate::pretrade::PreTradeRequest::execute
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejects(Vec<Reject>);

impl Rejects {
    /// Creates a reject collection from a vector.
    pub fn new(rejects: Vec<Reject>) -> Self {
        Self(rejects)
    }

    pub(crate) fn into_vec(self) -> Vec<Reject> {
        self.0
    }
}

impl From<Reject> for Rejects {
    fn from(value: Reject) -> Self {
        Self(vec![value])
    }
}

impl From<Vec<Reject>> for Rejects {
    fn from(value: Vec<Reject>) -> Self {
        Self(value)
    }
}

impl std::ops::Deref for Rejects {
    type Target = Vec<Reject>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Rejects {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, reject) in self.0.iter().enumerate() {
            if i > 0 {
                write!(formatter, "; ")?;
            }
            Display::fmt(reject, formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for Rejects {}

impl Reject {
    /// Creates a reject with human-readable reason and details.
    pub fn new(
        policy: impl Into<String>,
        scope: RejectScope,
        code: RejectCode,
        reason: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            code,
            reason: reason.into(),
            details: details.into(),
            policy: policy.into(),
            user_data: 0,
            scope,
        }
    }

    /// Returns a copy of this reject with caller-defined opaque token.
    pub fn with_user_data(mut self, user_data: usize) -> Self {
        self.user_data = user_data;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::RejectCode;

    #[test]
    fn reject_code_as_str_and_display_are_stable_for_all_values() {
        let cases = [
            (RejectCode::MissingRequiredField, "MissingRequiredField"),
            (RejectCode::InvalidFieldFormat, "InvalidFieldFormat"),
            (RejectCode::InvalidFieldValue, "InvalidFieldValue"),
            (RejectCode::UnsupportedOrderType, "UnsupportedOrderType"),
            (RejectCode::UnsupportedTimeInForce, "UnsupportedTimeInForce"),
            (
                RejectCode::UnsupportedOrderAttribute,
                "UnsupportedOrderAttribute",
            ),
            (RejectCode::DuplicateClientOrderId, "DuplicateClientOrderId"),
            (RejectCode::TooLateToEnter, "TooLateToEnter"),
            (RejectCode::ExchangeClosed, "ExchangeClosed"),
            (RejectCode::UnknownInstrument, "UnknownInstrument"),
            (RejectCode::UnknownAccount, "UnknownAccount"),
            (RejectCode::UnknownVenue, "UnknownVenue"),
            (RejectCode::UnknownClearingAccount, "UnknownClearingAccount"),
            (RejectCode::UnknownCollateralAsset, "UnknownCollateralAsset"),
            (RejectCode::InsufficientFunds, "InsufficientFunds"),
            (RejectCode::InsufficientMargin, "InsufficientMargin"),
            (RejectCode::InsufficientPosition, "InsufficientPosition"),
            (RejectCode::CreditLimitExceeded, "CreditLimitExceeded"),
            (RejectCode::RiskLimitExceeded, "RiskLimitExceeded"),
            (RejectCode::OrderExceedsLimit, "OrderExceedsLimit"),
            (RejectCode::OrderQtyExceedsLimit, "OrderQtyExceedsLimit"),
            (
                RejectCode::OrderNotionalExceedsLimit,
                "OrderNotionalExceedsLimit",
            ),
            (RejectCode::PositionLimitExceeded, "PositionLimitExceeded"),
            (
                RejectCode::ConcentrationLimitExceeded,
                "ConcentrationLimitExceeded",
            ),
            (RejectCode::LeverageLimitExceeded, "LeverageLimitExceeded"),
            (RejectCode::RateLimitExceeded, "RateLimitExceeded"),
            (RejectCode::PnlKillSwitchTriggered, "PnlKillSwitchTriggered"),
            (RejectCode::AccountBlocked, "AccountBlocked"),
            (RejectCode::AccountNotAuthorized, "AccountNotAuthorized"),
            (RejectCode::ComplianceRestriction, "ComplianceRestriction"),
            (RejectCode::InstrumentRestricted, "InstrumentRestricted"),
            (
                RejectCode::JurisdictionRestriction,
                "JurisdictionRestriction",
            ),
            (RejectCode::WashTradePrevention, "WashTradePrevention"),
            (RejectCode::SelfMatchPrevention, "SelfMatchPrevention"),
            (RejectCode::ShortSaleRestriction, "ShortSaleRestriction"),
            (
                RejectCode::RiskConfigurationMissing,
                "RiskConfigurationMissing",
            ),
            (
                RejectCode::ReferenceDataUnavailable,
                "ReferenceDataUnavailable",
            ),
            (
                RejectCode::OrderValueCalculationFailed,
                "OrderValueCalculationFailed",
            ),
            (RejectCode::SystemUnavailable, "SystemUnavailable"),
            (RejectCode::Custom, "Custom"),
            (RejectCode::Other, "Other"),
        ];

        for (code, expected_name) in cases {
            assert_eq!(code.as_str(), expected_name);
            assert_eq!(code.to_string(), expected_name);
        }
    }

    #[test]
    fn reject_display_formats_policy_reason_and_details() {
        let reject = super::Reject::new(
            "TestPolicy",
            super::RejectScope::Order,
            RejectCode::Other,
            "something went wrong",
            "extra info",
        );
        assert_eq!(
            reject.to_string(),
            "[TestPolicy] something went wrong: extra info"
        );
    }

    #[test]
    fn reject_new_sets_default_user_data() {
        let reject = super::Reject::new(
            "TestPolicy",
            super::RejectScope::Order,
            RejectCode::Other,
            "reason",
            "details",
        );
        assert_eq!(reject.user_data, 0);
    }

    #[test]
    fn reject_with_user_data_overrides_default_payload() {
        let reject = super::Reject::new(
            "TestPolicy",
            super::RejectScope::Order,
            RejectCode::Other,
            "reason",
            "details",
        )
        .with_user_data(42usize);
        assert_eq!(reject.user_data, 42usize);
    }

    #[test]
    fn rejects_deref_gives_slice_access() {
        let rejects = super::Rejects::new(vec![super::Reject::new(
            "P",
            super::RejectScope::Order,
            RejectCode::Other,
            "r",
            "d",
        )]);
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "P");
    }

    #[test]
    fn rejects_display_single() {
        let rejects = super::Rejects::new(vec![super::Reject::new(
            "P",
            super::RejectScope::Order,
            RejectCode::Other,
            "reason",
            "details",
        )]);
        assert_eq!(rejects.to_string(), "[P] reason: details");
    }

    #[test]
    fn rejects_display_multiple_joined_with_separator() {
        let rejects = super::Rejects::new(vec![
            super::Reject::new(
                "P1",
                super::RejectScope::Order,
                RejectCode::Other,
                "r1",
                "d1",
            ),
            super::Reject::new(
                "P2",
                super::RejectScope::Account,
                RejectCode::Other,
                "r2",
                "d2",
            ),
        ]);
        assert_eq!(rejects.to_string(), "[P1] r1: d1; [P2] r2: d2");
    }

    #[test]
    fn rejects_display_empty_produces_empty_string() {
        let rejects = super::Rejects::new(vec![]);
        assert_eq!(rejects.to_string(), "");
    }

    #[test]
    fn rejects_display_propagates_error_from_reject_fmt() {
        use std::fmt;
        use std::fmt::Write;

        struct ImmediateFailWriter;
        impl Write for ImmediateFailWriter {
            fn write_str(&mut self, _: &str) -> fmt::Result {
                Err(fmt::Error)
            }
        }

        let rejects = super::Rejects::new(vec![super::Reject::new(
            "P",
            super::RejectScope::Order,
            RejectCode::Other,
            "r",
            "d",
        )]);
        assert!(fmt::write(&mut ImmediateFailWriter, format_args!("{rejects}")).is_err());
    }

    #[test]
    fn rejects_display_propagates_write_error() {
        use std::fmt;
        use std::fmt::Write;

        struct FailOnSeparator;
        impl Write for FailOnSeparator {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                if s == "; " {
                    Err(fmt::Error)
                } else {
                    Ok(())
                }
            }
        }

        let rejects = super::Rejects::new(vec![
            super::Reject::new("P", super::RejectScope::Order, RejectCode::Other, "r", "d"),
            super::Reject::new("P", super::RejectScope::Order, RejectCode::Other, "r", "d"),
        ]);
        assert!(fmt::write(&mut FailOnSeparator, format_args!("{rejects}")).is_err());
    }
}
