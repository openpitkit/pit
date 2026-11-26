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

//! Builtin review-rule policy builder and its rule input.

use openpit::param::{AccountGroupId, AccountId};
use openpit::pretrade::policies::{
    ReviewRuleAccountGroupRule, ReviewRuleAccountRule, ReviewRuleInstruments, ReviewRulePolicy,
    ReviewRuleSettings,
};
use openpit::pretrade::PolicyGroupId;
use wasm_bindgen::prelude::*;

use crate::domain::{
    collect_cloned_wrappers, is_plain_object, read_field, resolve_optional_account_group_id,
    resolve_optional_account_id, IntegerNumber,
};
use crate::engine::StorageFactory;
use crate::error::{engine_build_configuration_error, make_error, ErrorKind};
use crate::marketdata::JsInstrument;

#[wasm_bindgen(typescript_custom_section)]
const REVIEW_RULE_INIT_TS: &str = r#"
/** The account scope and instrument selection of a review rule. */
export interface ReviewRuleInit {
  /** Required only for an account rule. */
  accountId?: AccountId | number | bigint | string | null;
  /** Required only for an account-group rule. */
  accountGroupId?: AccountGroupId | number | bigint | string | null;
  /**
   * Omit or pass null for all instruments. An exact iterable must be non-empty.
   */
  instruments?: Iterable<Instrument | InstrumentInit> | null;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// TypeScript options for constructing a review rule.
    #[wasm_bindgen(typescript_type = "ReviewRuleInit")]
    pub type ReviewRuleInitLike;
}

/// A rule for one account, one account group or all accounts.
#[wasm_bindgen(js_name = ReviewRule)]
#[derive(Clone)]
pub struct JsReviewRule {
    instruments: ReviewRuleInstruments,
    account_id: Option<AccountId>,
    account_group_id: Option<AccountGroupId>,
}

#[wasm_bindgen(js_class = ReviewRule)]
impl JsReviewRule {
    /// Creates a rule. Global rules omit both IDs. Omit the instrument list
    /// or pass null to match all instruments.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` for non-object options, a non-iterable instrument
    /// selection or a malformed instrument. Throws `AssetError` for an empty
    /// asset, `AccountIdError` for an invalid account ID, and `ParamError`
    /// for an invalid account group ID.
    #[wasm_bindgen(constructor)]
    pub fn new(init: Option<ReviewRuleInitLike>) -> Result<JsReviewRule, JsValue> {
        let Some(init) = init else {
            return Ok(Self {
                instruments: ReviewRuleInstruments::All,
                account_id: None,
                account_group_id: None,
            });
        };
        let init: JsValue = init.into();
        if !is_plain_object(&init) {
            return Err(make_error(
                ErrorKind::Type,
                "ReviewRule options must be an object",
                None,
            ));
        }
        let account = read_field(&init, "accountId")?;
        let group = read_field(&init, "accountGroupId")?;
        let instruments = read_field(&init, "instruments")?;
        let instruments = if instruments.is_null() || instruments.is_undefined() {
            ReviewRuleInstruments::All
        } else {
            let iterator = js_sys::try_iter(&instruments)?.ok_or_else(|| {
                make_error(
                    ErrorKind::Type,
                    "ReviewRule.instruments must be iterable",
                    None,
                )
            })?;
            let instruments = iterator
                .map(|item| JsInstrument::coerce(item?))
                .collect::<Result<Vec<_>, _>>()?;
            ReviewRuleInstruments::Exact(instruments)
        };
        Ok(Self {
            instruments,
            account_id: resolve_optional_account_id(account)?,
            account_group_id: resolve_optional_account_group_id(group)?,
        })
    }

    /// Creates an independent copy of the rule.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsReviewRule {
        self.clone()
    }
}

impl JsReviewRule {
    pub(crate) fn into_global(self) -> Result<ReviewRuleInstruments, String> {
        check_level(
            Some(self.instruments),
            self.account_id.is_some() || self.account_group_id.is_some(),
            "global",
        )
    }

    pub(crate) fn into_account_group(self) -> Result<ReviewRuleAccountGroupRule, String> {
        let account_group_id = check_level(
            self.account_group_id,
            self.account_id.is_some(),
            "account group",
        )?;
        Ok(ReviewRuleAccountGroupRule {
            instruments: self.instruments,
            account_group_id,
        })
    }

    pub(crate) fn into_account(self) -> Result<ReviewRuleAccountRule, String> {
        let account_id = check_level(self.account_id, self.account_group_id.is_some(), "account")?;
        Ok(ReviewRuleAccountRule {
            instruments: self.instruments,
            account_id,
        })
    }
}

fn check_level<T>(target: Option<T>, has_other_id: bool, level: &str) -> Result<T, String> {
    target
        .filter(|_| !has_other_id)
        .ok_or_else(|| format!("ReviewRule has incompatible IDs for the {level} level"))
}

/// Configuring builder for the builtin review-rule policy.
#[wasm_bindgen(js_name = ReviewRuleBuilder)]
#[derive(Clone, Default)]
pub struct JsReviewRuleBuilder {
    global_rule: Option<ReviewRuleInstruments>,
    account_group_rules: Vec<ReviewRuleAccountGroupRule>,
    account_rules: Vec<ReviewRuleAccountRule>,
    policy_group_id: u16,
}

#[wasm_bindgen(js_class = ReviewRuleBuilder)]
impl JsReviewRuleBuilder {
    /// Stable name used for default registration.
    #[wasm_bindgen(getter, js_name = NAME)]
    pub fn name() -> String {
        ReviewRulePolicy::<StorageFactory>::NAME.to_owned()
    }

    /// Sets the global rule and returns a ready builder. The rule must omit
    /// both IDs.
    ///
    /// # Errors
    ///
    /// Throws `EngineBuildError` with `InvalidConfiguration` on a level
    /// mismatch.
    #[wasm_bindgen(js_name = globalRule, unchecked_return_type = "ReviewRuleReadyBuilder")]
    pub fn global_rule(&self, rule: &JsReviewRule) -> Result<JsReviewRuleBuilder, JsValue> {
        let mut next = self.clone();
        next.global_rule = Some(
            rule.clone()
                .into_global()
                .map_err(|error| engine_build_configuration_error(&error))?,
        );
        Ok(next)
    }

    /// Adds account-group rules and returns a ready builder.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` for a malformed iterable or entry, or
    /// `EngineBuildError` with `InvalidConfiguration` on a level mismatch.
    #[wasm_bindgen(js_name = accountGroupRules, unchecked_return_type = "ReviewRuleReadyBuilder")]
    pub fn account_group_rules(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<ReviewRule>")] rules: JsValue,
    ) -> Result<JsReviewRuleBuilder, JsValue> {
        let rules = collect_cloned_wrappers::<JsReviewRule>(&rules, "accountGroupRules")?
            .into_iter()
            .map(JsReviewRule::into_account_group)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| engine_build_configuration_error(&error))?;
        let mut next = self.clone();
        next.account_group_rules.extend(rules);
        Ok(next)
    }

    /// Adds account rules and returns a ready builder.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` for a malformed iterable or entry, or
    /// `EngineBuildError` with `InvalidConfiguration` on a level mismatch.
    #[wasm_bindgen(js_name = accountRules, unchecked_return_type = "ReviewRuleReadyBuilder")]
    pub fn account_rules(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<ReviewRule>")] rules: JsValue,
    ) -> Result<JsReviewRuleBuilder, JsValue> {
        let rules = collect_cloned_wrappers::<JsReviewRule>(&rules, "accountRules")?
            .into_iter()
            .map(JsReviewRule::into_account)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| engine_build_configuration_error(&error))?;
        let mut next = self.clone();
        next.account_rules.extend(rules);
        Ok(next)
    }

    /// Assigns the policy group tag and preserves the builder stage.
    #[wasm_bindgen(js_name = withPolicyGroupId)]
    pub fn with_policy_group_id(
        &self,
        policy_group_id: IntegerNumber,
    ) -> Result<JsReviewRuleBuilder, JsValue> {
        let mut next = self.clone();
        next.policy_group_id = crate::lock::parse_policy_group_id(policy_group_id.into())?.value();
        Ok(next)
    }

    /// Returns an independent builder with the same configuration.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsReviewRuleBuilder {
        self.clone()
    }
}

impl JsReviewRuleBuilder {
    /// Builds the policy from the collected rules.
    ///
    /// # Errors
    ///
    /// Throws `EngineBuildError` with `InvalidConfiguration` if the core
    /// rejects the settings, for example because a scope key is duplicated.
    pub(crate) fn build_policy(self) -> Result<ReviewRulePolicy<StorageFactory>, JsValue> {
        let settings = ReviewRuleSettings::new(
            self.global_rule,
            self.account_group_rules,
            self.account_rules,
        )
        .map_err(|error| engine_build_configuration_error(&error.to_string()))?;
        Ok(ReviewRulePolicy::new(settings)
            .with_policy_group_id(PolicyGroupId::new(self.policy_group_id)))
    }
}

/// Creates a review-rule builder. An empty account or account-group axis
/// makes the policy ready to register and configure later.
#[wasm_bindgen(js_name = buildReviewRule)]
pub fn build_review_rule() -> JsReviewRuleBuilder {
    JsReviewRuleBuilder::default()
}
