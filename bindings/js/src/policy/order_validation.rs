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

//! Builtin order-validation policy builder.
//!
//! `buildOrderValidation()` returns a ready-builder token passed to
//! `builder.builtin(token)`. The only knob is `withPolicyGroupId(id)`. The
//! builder remains opaque until the engine resolves it.

use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::PolicyGroupId;
use wasm_bindgen::prelude::*;

use crate::domain::IntegerNumber;

/// Ready-builder token for the builtin order-validation policy.
#[wasm_bindgen(js_name = OrderValidationBuilder)]
#[derive(Clone, Copy)]
pub struct JsOrderValidationBuilder {
    policy_group_id: u16,
}

#[wasm_bindgen(js_class = OrderValidationBuilder)]
impl JsOrderValidationBuilder {
    /// Assigns the policy group id and returns the builder for chaining.
    #[wasm_bindgen(
        js_name = withPolicyGroupId,
        unchecked_return_type = "OrderValidationReadyBuilder"
    )]
    pub fn with_policy_group_id(
        &self,
        policy_group_id: IntegerNumber,
    ) -> Result<JsOrderValidationBuilder, JsValue> {
        let mut next = *self;
        next.policy_group_id = crate::lock::parse_policy_group_id(policy_group_id.into())?.value();
        Ok(next)
    }

    /// Returns an independent builder with the same configuration.
    #[wasm_bindgen(
        js_name = clone,
        unchecked_return_type = "OrderValidationReadyBuilder"
    )]
    pub fn js_clone(&self) -> JsOrderValidationBuilder {
        *self
    }
}

impl JsOrderValidationBuilder {
    /// Builds the core policy from this token.
    pub(crate) fn build_policy(&self) -> OrderValidationPolicy {
        OrderValidationPolicy::new().with_policy_group_id(PolicyGroupId::new(self.policy_group_id))
    }
}

/// Creates a fresh order-validation ready-builder token.
///
/// Pass the returned token to `EngineBuilder.builtin(...)` /
/// `ReadyEngineBuilder.builtin(...)`.
#[wasm_bindgen(
    js_name = buildOrderValidation,
    unchecked_return_type = "OrderValidationReadyBuilder"
)]
pub fn build_order_validation() -> JsOrderValidationBuilder {
    JsOrderValidationBuilder { policy_group_id: 0 }
}
