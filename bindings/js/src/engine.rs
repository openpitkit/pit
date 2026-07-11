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

//! Engine handle, account registry, and the staged engine builder.
//!
//! The engine always uses `no_sync`: on `wasm32` the runtime is single-threaded,
//! so the staged builder does not expose a sync-mode choice. The resulting
//! engine and its `Rc`-based, `!Send` handles are correct for the
//! single-threaded WASM model.
//!
//! Policy registration (`preTrade`/`builtin`) resolves eagerly: `preTrade`
//! builds the JS [`JsPreTradePolicyAdapter`] and `builtin` downcasts the token
//! to a builtin ready-builder and constructs its core policy. Both are pushed
//! into the core builder through the `BoxedPreTradePolicy` shim and advance the
//! builder to the `Ready` state.

use std::cell::RefCell;

use js_sys::{Object, Reflect};
use openpit::param::AccountId;
use openpit::pretrade::policies::{
    OrderSizeLimitPolicy, OrderValidationPolicy, PnlBoundsKillSwitchPolicy, RateLimitPolicy,
    SpotFundsPolicy,
};
use openpit::pretrade::{
    PolicyGroupId, PolicyPreTradeResult, PostTradeContext, PostTradeResult, PreTradeContext,
    PreTradePolicy, Rejects,
};
use openpit::{
    AccountAdjustmentContext, AccountOutcomeEntry, EngineBuilder, EngineTraitOf, Mutations,
    ReadyEngineBuilder, StorageBuilder, SyncedEngineBuilder,
};
pub(crate) use openpit_interop::EngineLocking;
use openpit_interop::{RequestWithPayload, SyncMode};
use wasm_bindgen::prelude::*;

use crate::configure::JsConfigurator;
use crate::context::LifecycleToken;
use crate::domain::{
    extract_cloned_wrapper, parse_asset, resolve_account_group_id, resolve_account_id,
    AccountGroupIdLike, AccountIdLike,
};

#[wasm_bindgen]
extern "C" {
    /// An iterable of account adjustments (`AccountAdjustment` objects or plain
    /// `AccountAdjustmentInit` literals).
    #[wasm_bindgen(typescript_type = "Iterable<AccountAdjustment | AccountAdjustmentInit>")]
    pub type AccountAdjustmentIterable;

    /// An iterable of account ids (`AccountId` objects or numeric/string ids).
    #[wasm_bindgen(typescript_type = "Iterable<AccountId | number | bigint | string>")]
    pub type AccountIdIterable;
}
use crate::account_adjustment::JsAccountAdjustment;
use crate::error::{
    account_block_error_to_js, account_group_error_to_js, make_error, make_error_with,
    policy_callback_error, ErrorKind,
};
use crate::execution_report::{ExecutionReportLike, JsExecutionReport};
use crate::marketdata::{JsMarketDataBuilder, JsQuoteTtl};
use crate::order::{JsOrder, OrderLike};
use crate::param::ids::JsAccountGroupId;
use crate::policy::order_size_limit::JsOrderSizeLimitBuilder;
use crate::policy::order_validation::JsOrderValidationBuilder;
use crate::policy::pnl_killswitch::JsPnlBoundsKillswitchBuilder;
use crate::policy::rate_limit::JsRateLimitBuilder;
use crate::policy::spot_funds::{JsSpotFundsBuilder, JsSpotFundsPnlBoundsKillswitchBuilder};
use crate::policy::{BuiltinReadyBuilder, CallbackErrorScope, JsPreTradePolicyAdapter, PolicyLike};
use crate::result::{
    JsAccountAdjustmentBatchResult, JsDryRunReport, JsExecuteResult, JsPostTradeResult,
    JsStartResult,
};

#[wasm_bindgen(inline_js = r#"
function cloneGraph(value, initialSeen) {
  const seen = initialSeen ?? new WeakMap();
  const clone = (input) => {
    if (input === null || (typeof input !== "object" && typeof input !== "function")) {
      return input;
    }
    if (typeof input === "function") {
      return input;
    }
    const prior = seen.get(input);
    if (prior !== undefined) {
      return prior;
    }

    let output;
    if (typeof input.clone === "function" && typeof input.__wbg_ptr === "number") {
      output = input.clone();
    } else if (Array.isArray(input)) {
      output = [];
    } else if (input instanceof Date) {
      output = new Date(input.getTime());
    } else if (input instanceof Map) {
      output = new Map();
    } else if (input instanceof Set) {
      output = new Set();
    } else if (input instanceof ArrayBuffer) {
      output = input.slice(0);
    } else if (input instanceof DataView) {
      const buffer = input.buffer.slice(
        input.byteOffset,
        input.byteOffset + input.byteLength,
      );
      output = new DataView(buffer);
    } else if (ArrayBuffer.isView(input)) {
      output = new input.constructor(input);
    } else {
      output = Object.create(Object.getPrototypeOf(input));
    }
    seen.set(input, output);

    // A normalized wasm value may have been decorated with a caller's custom
    // class prototype. Preserve that exact prototype while its canonical
    // fields remain materialized as own normalized properties.
    const prototype = Object.getPrototypeOf(input);
    if (Object.getPrototypeOf(output) !== prototype) {
      Object.setPrototypeOf(output, prototype);
    }

    if (input instanceof Map) {
      for (const [key, item] of input) output.set(clone(key), clone(item));
      return output;
    }
    if (input instanceof Set) {
      for (const item of input) output.add(clone(item));
      return output;
    }

    for (const key of Reflect.ownKeys(input)) {
      if (key === "__wbg_ptr") continue;
      const descriptor = Object.getOwnPropertyDescriptor(input, key);
      if (descriptor === undefined) continue;
      if ("value" in descriptor) descriptor.value = clone(descriptor.value);
      Object.defineProperty(output, key, descriptor);
    }
    return output;
  };
  return clone(value);
}

export function clonePolicyPayload(value) {
  return cloneGraph(value);
}

function mergeRecord(normalized, original, canonicalFields, seen) {
  if (original === null || (typeof original !== "object" && typeof original !== "function")) {
    return normalized;
  }

  seen.set(original, normalized);
  const canonicalValues = canonicalFields.map((field) => [field, normalized[field]]);
  const originalPrototype = Object.getPrototypeOf(original);
  // Plain init records acquire the public wasm model prototype. Custom class
  // instances retain their prototype (including null prototypes).
  if (originalPrototype !== Object.prototype) {
    Object.setPrototypeOf(normalized, originalPrototype);
  }

  // Keep normalized fields usable even when a custom prototype does not
  // inherit from the public wasm class.
  for (const [field, value] of canonicalValues) {
    Object.defineProperty(normalized, field, {
      configurable: true,
      enumerable: true,
      writable: true,
      value,
    });
  }

  const canonical = new Set(canonicalFields);
  for (const key of Reflect.ownKeys(original)) {
    if (key === "__wbg_ptr" || canonical.has(key)) continue;
    const descriptor = Object.getOwnPropertyDescriptor(original, key);
    if (descriptor === undefined) continue;
    if ("value" in descriptor) descriptor.value = cloneGraph(descriptor.value, seen);
    Object.defineProperty(normalized, key, descriptor);
  }
  return normalized;
}

function normalizedPolicyPayload(normalized, original, schema) {
  const seen = new WeakMap();
  const topLevelFields = Object.keys(schema);
  const groups = Object.entries(schema).map(([field, nestedFields]) => {
    const normalizedGroup = normalized[field];
    const originalGroup = original?.[field];
    if (
      normalizedGroup !== null &&
      normalizedGroup !== undefined &&
      originalGroup !== null &&
      originalGroup !== undefined &&
      (typeof originalGroup === "object" || typeof originalGroup === "function")
    ) {
      // Register canonical nested records before cloning custom top-level
      // metadata. A metadata field may alias one of these records and must
      // resolve to the same decorated normalized wrapper.
      seen.set(originalGroup, normalizedGroup);
    }
    return [field, nestedFields, normalizedGroup, originalGroup];
  });
  mergeRecord(normalized, original, topLevelFields, seen);

  for (const [field, nestedFields, normalizedGroup, originalGroup] of groups) {
    if (normalizedGroup === null || normalizedGroup === undefined) continue;
    if (originalGroup !== null && originalGroup !== undefined) {
      mergeRecord(normalizedGroup, originalGroup, nestedFields, seen);
    }
    // Shadow the wasm prototype getter with this decorated, normalized group;
    // otherwise each getter call would manufacture a new undecorated wrapper.
    Object.defineProperty(normalized, field, {
      configurable: true,
      enumerable: true,
      writable: true,
      value: normalizedGroup,
    });
  }
  return normalized;
}

export function makeOrderPolicyPayload(normalized, original) {
  return normalizedPolicyPayload(normalized, original, {
    operation: ["underlyingAsset", "settlementAsset", "accountId", "side", "tradeAmount", "price"],
    position: ["positionSide", "reduceOnly", "closePosition"],
    margin: ["collateralAsset", "leverage", "autoBorrow"],
  });
}

export function makeExecutionReportPolicyPayload(normalized, original) {
  return normalizedPolicyPayload(normalized, original, {
    operation: ["underlyingAsset", "settlementAsset", "accountId", "side"],
    financialImpact: ["pnl", "fee"],
    fill: ["lastTrade", "fee", "leavesQuantity", "lock", "isFinal"],
    positionImpact: ["positionEffect", "positionSide"],
  });
}
"#)]
extern "C" {
    /// Creates an isolated deep snapshot while preserving custom prototypes,
    /// symbols, cycles, and wasm-wrapper value semantics.
    #[wasm_bindgen(catch, js_name = clonePolicyPayload)]
    fn clone_policy_payload(value: &JsValue) -> Result<JsValue, JsValue>;

    /// Overlays custom order metadata onto a normalized public model.
    #[wasm_bindgen(catch, js_name = makeOrderPolicyPayload)]
    fn make_order_policy_payload(
        normalized: &JsValue,
        original: &JsValue,
    ) -> Result<JsValue, JsValue>;

    /// Overlays custom report metadata onto a normalized public model.
    #[wasm_bindgen(catch, js_name = makeExecutionReportPolicyPayload)]
    fn make_execution_report_policy_payload(
        normalized: &JsValue,
        original: &JsValue,
    ) -> Result<JsValue, JsValue>;
}

/// Storage-locking factory chosen by the engine sync mode.
pub(crate) type StorageFactory = openpit_interop::StorageLockingPolicyFactory;

/// Borrowed storage builder passed to builtin policy constructors.
///
/// The builtin builders take this reference so their internal storage tables
/// share the engine's synchronization factory.
pub(crate) type StorageBuilderRef = StorageBuilder<StorageFactory>;

/// Normalized order payload retained for custom-policy callbacks.
///
/// The Rust wrapper is cloned into a *fresh* wasm object for every callback.
/// One policy can therefore mutate or free its argument without changing what
/// later policies observe.  The lifecycle token scopes every context/control
/// handed out while this order is processed.
#[derive(Clone)]
pub(crate) struct OrderPayload {
    snapshot: JsValue,
    lifecycle: LifecycleToken,
}

impl OrderPayload {
    pub(crate) fn fresh_js(&self) -> Result<JsValue, JsValue> {
        clone_policy_payload(&self.snapshot)
    }

    pub(crate) fn lifecycle(&self) -> LifecycleToken {
        self.lifecycle.clone()
    }
}

/// Normalized execution-report payload retained for custom-policy callbacks.
#[derive(Clone)]
pub(crate) struct ExecutionReportPayload(JsValue);

impl ExecutionReportPayload {
    pub(crate) fn fresh_js(&self) -> Result<JsValue, JsValue> {
        clone_policy_payload(&self.0)
    }
}

/// Normalized account-adjustment payload retained for custom-policy callbacks.
#[derive(Clone)]
pub(crate) struct AccountAdjustmentPayload {
    adjustment: JsAccountAdjustment,
    lifecycle: LifecycleToken,
}

impl AccountAdjustmentPayload {
    pub(crate) fn fresh_js(&self) -> Result<JsValue, JsValue> {
        // Account-adjustment callbacks have a concrete public model type. Keep
        // that contract even when the caller submitted an init literal.
        Ok(JsValue::from(self.adjustment.clone()))
    }

    pub(crate) fn lifecycle(&self) -> LifecycleToken {
        self.lifecycle.clone()
    }
}

/// Engine-facing order request carrying a normalized binding payload.
pub(crate) type Order = RequestWithPayload<openpit_interop::Order, OrderPayload>;

/// Engine-facing execution report carrying a normalized binding payload.
pub(crate) type ExecutionReport =
    RequestWithPayload<openpit_interop::ExecutionReport, ExecutionReportPayload>;

/// Engine-facing account adjustment carrying a normalized binding payload.
pub(crate) type AccountAdjustment =
    RequestWithPayload<openpit_interop::AccountAdjustment, AccountAdjustmentPayload>;

/// Engine trait instantiation for the binding's request types.
pub(crate) type EngineTrait =
    EngineTraitOf<Order, ExecutionReport, AccountAdjustment, EngineLocking>;

/// Owned boxed policy object satisfying the core builder's `+ Send` bound.
///
/// The core's binding-layer policy object is `dyn PreTradePolicy<...> + Send`.
/// `BoxedPreTradePolicy` forwards every hook to its boxed inner policy and is
/// itself `Send` (the inner box is `+ Send`), so it can be handed to
/// `pre_trade`. On `wasm32-unknown-unknown` (no atomics) `JsValue` is `Send`,
/// so the JS policy adapter satisfies the bound while staying correct for the
/// single-threaded runtime.
pub(crate) struct BoxedPreTradePolicy {
    inner: Box<dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment, EngineLocking> + Send>,
}

impl BoxedPreTradePolicy {
    /// Wraps a boxed policy object.
    pub(crate) fn new(
        inner: Box<
            dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment, EngineLocking> + Send,
        >,
    ) -> Self {
        Self { inner }
    }
}

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment, EngineLocking>
    for BoxedPreTradePolicy
{
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.inner.policy_group_id()
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<StorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.inner.check_pre_trade_start(ctx, order)
    }

    fn check_pre_trade_start_dry_run(
        &self,
        ctx: &PreTradeContext<StorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.inner.check_pre_trade_start_dry_run(ctx, order)
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<StorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.inner.perform_pre_trade_check(ctx, order, mutations)
    }

    fn perform_pre_trade_check_dry_run(
        &self,
        ctx: &PreTradeContext<StorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.inner
            .perform_pre_trade_check_dry_run(ctx, order, mutations)
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<StorageFactory>,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        self.inner.apply_execution_report(ctx, report)
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<StorageFactory>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<Vec<AccountOutcomeEntry>, Rejects> {
        self.inner
            .apply_account_adjustment(ctx, account_id, adjustment, mutations)
    }
}

/// Binding-owned staged builder state.
///
/// `Synced` is the post-sync, pre-policy state; `Ready` holds at least one
/// resolved policy. Each `preTrade`/`builtin` call resolves its policy and
/// advances the state from `Synced`/`Ready` to `Ready`.
enum BuilderState {
    /// No policy resolved yet.
    Synced(SyncedEngineBuilder<Order, ExecutionReport, AccountAdjustment, EngineLocking>),
    /// At least one policy resolved.
    Ready(ReadyEngineBuilder<Order, ExecutionReport, AccountAdjustment, EngineLocking>),
}

enum BuiltinPolicy {
    OrderValidation(OrderValidationPolicy),
    OrderSizeLimit(OrderSizeLimitPolicy<StorageFactory>),
    RateLimit(RateLimitPolicy<StorageFactory>),
    PnlBoundsKillswitch(PnlBoundsKillSwitchPolicy<StorageFactory>),
    SpotFunds(SpotFundsPolicy<EngineLocking, EngineLocking>),
}

impl BuilderState {
    /// Returns the storage builder owned by the current state.
    fn storage_builder(&self) -> &StorageBuilderRef {
        match self {
            BuilderState::Synced(builder) => builder.storage_builder(),
            BuilderState::Ready(builder) => builder.storage_builder(),
        }
    }

    /// Registers `policy` and returns the advanced `Ready` state.
    fn pre_trade<Policy>(self, policy: Policy) -> BuilderState
    where
        Policy: PreTradePolicy<Order, ExecutionReport, AccountAdjustment, EngineLocking>
            + Send
            + 'static,
    {
        BuilderState::Ready(match self {
            BuilderState::Synced(builder) => builder.pre_trade(policy),
            BuilderState::Ready(builder) => builder.pre_trade(policy),
        })
    }

    /// Registers a builtin policy, preserving its concrete configurable type.
    fn builtin(self, policy: BuiltinPolicy) -> BuilderState {
        match policy {
            BuiltinPolicy::OrderValidation(policy) => self.pre_trade(policy),
            BuiltinPolicy::OrderSizeLimit(policy) => self.pre_trade(policy),
            BuiltinPolicy::RateLimit(policy) => self.pre_trade(policy),
            BuiltinPolicy::PnlBoundsKillswitch(policy) => self.pre_trade(policy),
            BuiltinPolicy::SpotFunds(policy) => self.pre_trade(policy),
        }
    }
}

// ─── Engine ─────────────────────────────────────────────────────────────────

/// Pre-trade risk engine handle.
///
/// Built from [`JsEngineBuilder`] via `Engine.builder()`. The handle drives the
/// two-stage pre-trade flow plus the post-trade and account-adjustment paths.
#[wasm_bindgen(js_name = Engine)]
pub struct JsEngine {
    inner: openpit::Engine<EngineTrait>,
}

#[wasm_bindgen(js_class = Engine)]
impl JsEngine {
    /// Returns a fresh staged engine builder.
    #[wasm_bindgen(js_name = builder)]
    pub fn builder() -> JsEngineBuilder {
        JsEngineBuilder
    }

    /// Runs start-stage checks and returns a [`JsStartResult`].
    ///
    /// `order` accepts an `Order` object or a plain `OrderInit` literal. On
    /// success the result carries a single-use `Request`; on rejection it
    /// carries the rejects. A wrapper `order` is not consumed, so the caller
    /// may reuse the same object afterwards; a field-equivalent clone is
    /// retained as the request payload for later policy callbacks.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when `order` is neither a valid `Order`
    /// nor a valid `OrderInit` literal. Re-throws the original error a custom
    /// policy callback threw, if any.
    #[wasm_bindgen(js_name = startPreTrade)]
    pub fn start_pre_trade(&self, order: OrderLike) -> Result<JsStartResult, JsValue> {
        let original: JsValue = order.into();
        let order = JsOrder::coerce(original.clone())?;
        let (request, lifecycle) = build_order_request(&order, &original)?;
        let callback_scope = CallbackErrorScope::capture();
        let result = match self.inner.start_pre_trade(request) {
            Ok(request) => JsStartResult::accepted(request, lifecycle.clone()),
            Err(rejects) => {
                lifecycle.invalidate();
                JsStartResult::rejected(&rejects)
            }
        };
        finish_callback_scope(callback_scope, JsValue::UNDEFINED)?;
        Ok(result)
    }

    /// Runs start- and main-stage checks in one step, returning a
    /// [`JsExecuteResult`].
    ///
    /// `order` accepts an `Order` object or a plain `OrderInit` literal. A
    /// wrapper `order` is not consumed, so the caller may reuse the same object
    /// afterwards.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when `order` is neither a valid `Order`
    /// nor a valid `OrderInit` literal. Re-throws the original error a custom
    /// policy callback threw, if any.
    #[wasm_bindgen(js_name = executePreTrade)]
    pub fn execute_pre_trade(&self, order: OrderLike) -> Result<JsExecuteResult, JsValue> {
        let original: JsValue = order.into();
        let order = JsOrder::coerce(original.clone())?;
        let (request, lifecycle) = build_order_request(&order, &original)?;
        let callback_scope = CallbackErrorScope::capture();
        let result = match self.inner.execute_pre_trade(request) {
            Ok(reservation) => JsExecuteResult::accepted(reservation, lifecycle.clone()),
            Err(rejects) => {
                lifecycle.invalidate();
                JsExecuteResult::rejected(&rejects)
            }
        };
        finish_callback_scope(callback_scope, JsValue::UNDEFINED)?;
        Ok(result)
    }

    /// Runs start-stage checks as a non-mutating dry-run.
    ///
    /// `order` accepts an `Order` object or a plain `OrderInit` literal. A
    /// wrapper `order` is not consumed, so the caller may reuse the same object
    /// afterwards.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when `order` is neither a valid `Order`
    /// nor a valid `OrderInit` literal. Re-throws the original error a custom
    /// policy callback threw, if any.
    #[wasm_bindgen(js_name = startPreTradeDryRun)]
    pub fn start_pre_trade_dry_run(&self, order: OrderLike) -> Result<JsDryRunReport, JsValue> {
        let original: JsValue = order.into();
        let order = JsOrder::coerce(original.clone())?;
        let (request, lifecycle) = build_order_request(&order, &original)?;
        let callback_scope = CallbackErrorScope::capture();
        let report = self.inner.start_pre_trade_dry_run(request);
        lifecycle.invalidate();
        finish_callback_scope(callback_scope, JsValue::UNDEFINED)?;
        Ok(JsDryRunReport::from_core(report))
    }

    /// Runs start- and main-stage checks as a non-mutating dry-run.
    ///
    /// `order` accepts an `Order` object or a plain `OrderInit` literal. A
    /// wrapper `order` is not consumed, so the caller may reuse the same object
    /// afterwards.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when `order` is neither a valid `Order`
    /// nor a valid `OrderInit` literal. Re-throws the original error a custom
    /// policy callback threw, if any.
    #[wasm_bindgen(js_name = executePreTradeDryRun)]
    pub fn execute_pre_trade_dry_run(&self, order: OrderLike) -> Result<JsDryRunReport, JsValue> {
        let original: JsValue = order.into();
        let order = JsOrder::coerce(original.clone())?;
        let (request, lifecycle) = build_order_request(&order, &original)?;
        let callback_scope = CallbackErrorScope::capture();
        let report = self.inner.execute_pre_trade_dry_run(request);
        lifecycle.invalidate();
        finish_callback_scope(callback_scope, JsValue::UNDEFINED)?;
        Ok(JsDryRunReport::from_core(report))
    }

    /// Applies an execution report across all policies and returns the
    /// aggregated post-trade result.
    ///
    /// `report` accepts an `ExecutionReport` object or a plain
    /// `ExecutionReportInit` literal. A wrapper `report` is not consumed, so
    /// the caller may reuse the same object afterwards.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when `report` is neither a valid
    /// `ExecutionReport` nor a valid `ExecutionReportInit` literal. Re-throws
    /// the original error a custom policy callback threw, if any.
    #[wasm_bindgen(js_name = applyExecutionReport)]
    pub fn apply_execution_report(
        &self,
        report: ExecutionReportLike,
    ) -> Result<JsPostTradeResult, JsValue> {
        let original: JsValue = report.into();
        let report = JsExecutionReport::coerce(original.clone())?;
        let request = build_report_request(&report, &original)?;
        let callback_scope = CallbackErrorScope::capture();
        let result = JsPostTradeResult::from_core(&self.inner.apply_execution_report(&request));
        if let Some(cause) = callback_scope.finish() {
            return Err(policy_callback_error(cause, JsValue::from(result)));
        }
        Ok(result)
    }

    /// Applies an account-adjustment batch for `accountId`.
    ///
    /// `accountId` accepts an `AccountId` object or a
    /// `number | bigint | string`.
    /// `adjustments` is any iterable of `AccountAdjustment` objects or plain
    /// `AccountAdjustmentInit` literals. On success the result carries the
    /// produced outcomes; on rejection it carries the failing index and the
    /// rejects.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError`/`AccountIdError` when `accountId` or an
    /// adjustment in the batch is invalid. Re-throws the original error a
    /// custom policy callback threw, if any.
    #[wasm_bindgen(js_name = applyAccountAdjustment)]
    pub fn apply_account_adjustment(
        &self,
        account_id: AccountIdLike,
        adjustments: AccountAdjustmentIterable,
    ) -> Result<JsAccountAdjustmentBatchResult, JsValue> {
        let account_id = resolve_account_id(account_id.into())?;
        let (batch, lifecycle) = collect_adjustments(adjustments.into())?;
        let callback_scope = CallbackErrorScope::capture();
        let result = match self.inner.apply_account_adjustment(account_id, &batch) {
            Ok(result) => JsAccountAdjustmentBatchResult::accepted(&result),
            Err(error) => JsAccountAdjustmentBatchResult::rejected(&error),
        };
        lifecycle.invalidate();
        if let Some(cause) = callback_scope.finish() {
            return Err(policy_callback_error(cause, JsValue::from(result)));
        }
        Ok(result)
    }

    /// Returns the engine's account registry and block facility.
    #[wasm_bindgen(js_name = accounts)]
    pub fn accounts(&self) -> JsAccounts {
        JsAccounts {
            inner: self.inner.accounts(),
        }
    }

    /// Returns the engine's runtime policy configurator.
    #[wasm_bindgen(js_name = configure)]
    pub fn configure(&self) -> JsConfigurator {
        JsConfigurator::from_inner(self.inner.configure())
    }
}

/// Builds an order request and its shared callback lifecycle token.
///
/// The argument is borrowed (not consumed), so the caller's `Order` stays
/// usable after the call. Custom policy callbacks receive a normalized public
/// `Order` decorated with the submitted order's custom metadata and prototype.
fn build_order_request(
    order: &JsOrder,
    original: &JsValue,
) -> Result<(Order, LifecycleToken), JsValue> {
    let lifecycle = LifecycleToken::new();
    let normalized = JsValue::from(order.clone());
    let payload = OrderPayload {
        snapshot: make_order_policy_payload(&normalized, original)?,
        lifecycle: lifecycle.clone(),
    };
    Ok((
        RequestWithPayload::new(order.to_interop(), payload),
        lifecycle,
    ))
}

/// Builds an execution-report request, retaining the report JS object as
/// payload.
///
/// The argument is borrowed (not consumed), so the caller's `ExecutionReport`
/// stays usable after the call. Custom policy callbacks receive a normalized
/// public `ExecutionReport` decorated with submitted custom metadata and
/// prototype.
fn build_report_request(
    report: &JsExecutionReport,
    original: &JsValue,
) -> Result<ExecutionReport, JsValue> {
    let normalized = JsValue::from(report.clone());
    let payload =
        ExecutionReportPayload(make_execution_report_policy_payload(&normalized, original)?);
    Ok(RequestWithPayload::new(report.to_interop(), payload))
}

/// Collects an iterable of `AccountAdjustment` objects (or
/// `AccountAdjustmentInit` literals) into a request batch.
///
/// Each element is converted to an engine-facing interop request with a
/// normalized public wrapper retained as its callback payload.
///
/// # Errors
///
/// Throws `TypeError` when `adjustments` is not iterable, or
/// `TypeError`/`ParamError`/`AssetError` when an element is neither a valid
/// `AccountAdjustment` nor a valid literal.
fn collect_adjustments(
    adjustments: JsValue,
) -> Result<(Vec<AccountAdjustment>, LifecycleToken), JsValue> {
    let iterator = js_sys::try_iter(&adjustments)?.ok_or_else(|| {
        make_error(
            ErrorKind::Type,
            "adjustments must be an iterable of AccountAdjustment",
            None,
        )
    })?;

    let lifecycle = LifecycleToken::new();
    let mut batch = Vec::new();
    for item in iterator {
        let item = item?;
        let adjustment = JsAccountAdjustment::coerce(item)?;
        let payload = AccountAdjustmentPayload {
            adjustment: adjustment.clone(),
            lifecycle: lifecycle.clone(),
        };
        batch.push(RequestWithPayload::new(adjustment.to_interop(), payload));
    }
    Ok((batch, lifecycle))
}

// ─── Accounts ────────────────────────────────────────────────────────────────

/// Handle to the engine's account-group registry, currency configuration, and
/// block controls.
///
/// Obtained from `Engine.accounts()`. It shares the engine's single account
/// control state, so changes made through it are visible to every other handle
/// and to running policies.
#[wasm_bindgen(js_name = Accounts)]
pub struct JsAccounts {
    inner: openpit::Accounts<StorageFactory>,
}

#[wasm_bindgen(js_class = Accounts)]
impl JsAccounts {
    /// Atomically registers every account in `accounts` into `group`.
    ///
    /// `accounts` is an iterable of `AccountId` or numeric/string identifiers;
    /// `group` accepts an `AccountGroupId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountGroupRegistrationError` on a conflict, or
    /// `ParamError`/`AccountIdError` on a bad input.
    #[wasm_bindgen(js_name = registerGroup)]
    pub fn register_group(
        &self,
        accounts: AccountIdIterable,
        group: AccountGroupIdLike,
    ) -> Result<(), JsValue> {
        let account_ids = collect_account_ids(accounts.into())?;
        let group = resolve_account_group_id(group.into())?;
        self.inner
            .register_group(&account_ids, group)
            .map_err(|error| account_group_error_to_js(&error))
    }

    /// Atomically removes every account in `accounts` from `group`.
    ///
    /// `accounts` is an iterable of `AccountId` or numeric/string identifiers;
    /// `group` accepts an `AccountGroupId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountGroupRegistrationError` on a conflict, or
    /// `ParamError`/`AccountIdError` on a bad input.
    #[wasm_bindgen(js_name = unregisterGroup)]
    pub fn unregister_group(
        &self,
        accounts: AccountIdIterable,
        group: AccountGroupIdLike,
    ) -> Result<(), JsValue> {
        let account_ids = collect_account_ids(accounts.into())?;
        let group = resolve_account_group_id(group.into())?;
        self.inner
            .unregister_group(&account_ids, group)
            .map_err(|error| account_group_error_to_js(&error))
    }

    /// Returns the group of `account`, or `undefined` when it is unregistered.
    ///
    /// `account` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid identifier.
    #[wasm_bindgen(js_name = groupOf)]
    pub fn group_of(&self, account: AccountIdLike) -> Result<Option<JsAccountGroupId>, JsValue> {
        let account = resolve_account_id(account.into())?;
        Ok(self
            .inner
            .group_of(account)
            .map(JsAccountGroupId::from_inner))
    }

    /// Sets the currency used by account-aware policies for `account`.
    ///
    /// This changes configuration only; it does not recompute holdings, average
    /// entry prices, or realized P&L.
    ///
    /// `account` accepts an `AccountId` or a numeric/string identifier. `asset`
    /// must be a valid asset identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid account identifier or `AssetError`
    /// on an invalid asset identifier.
    #[wasm_bindgen(js_name = setCurrency)]
    pub fn set_currency(&self, account: AccountIdLike, asset: &str) -> Result<(), JsValue> {
        let account = resolve_account_id(account.into())?;
        let asset = parse_asset(asset)?;
        self.inner.set_currency(account, asset);
        Ok(())
    }

    /// Clears the currency configured for `account`.
    ///
    /// This changes configuration only; it does not recompute holdings, average
    /// entry prices, or realized P&L.
    ///
    /// `account` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid account identifier.
    #[wasm_bindgen(js_name = clearCurrency)]
    pub fn clear_currency(&self, account: AccountIdLike) -> Result<(), JsValue> {
        let account = resolve_account_id(account.into())?;
        self.inner.clear_currency(account);
        Ok(())
    }

    /// Sets the fallback currency used by account-aware policies for `group`.
    ///
    /// `AccountGroupId.DEFAULT()` selects the global fallback tier. This changes
    /// configuration only; it does not recompute holdings, average entry prices,
    /// or realized P&L.
    ///
    /// `group` accepts an `AccountGroupId` or a numeric/string identifier.
    /// `asset` must be a valid asset identifier.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid group identifier or `AssetError` on an
    /// invalid asset identifier.
    #[wasm_bindgen(js_name = setGroupCurrency)]
    pub fn set_group_currency(
        &self,
        group: AccountGroupIdLike,
        asset: &str,
    ) -> Result<(), JsValue> {
        let group = resolve_account_group_id(group.into())?;
        let asset = parse_asset(asset)?;
        self.inner.set_group_currency(group, asset);
        Ok(())
    }

    /// Clears the fallback currency configured for `group`.
    ///
    /// `AccountGroupId.DEFAULT()` selects the global fallback tier. This changes
    /// configuration only; it does not recompute holdings, average entry prices,
    /// or realized P&L.
    ///
    /// `group` accepts an `AccountGroupId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid group identifier.
    #[wasm_bindgen(js_name = clearGroupCurrency)]
    pub fn clear_group_currency(&self, group: AccountGroupIdLike) -> Result<(), JsValue> {
        let group = resolve_account_group_id(group.into())?;
        self.inner.clear_group_currency(group);
        Ok(())
    }

    /// Blocks `account` out of band with the operator-supplied `reason`.
    ///
    /// `account` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid identifier.
    #[wasm_bindgen(js_name = block)]
    pub fn block(&self, account: AccountIdLike, reason: String) -> Result<(), JsValue> {
        let account = resolve_account_id(account.into())?;
        self.inner.block(account, reason);
        Ok(())
    }

    /// Unblocks `account`, clearing any block on it.
    ///
    /// `account` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid identifier.
    #[wasm_bindgen(js_name = unblock)]
    pub fn unblock(&self, account: AccountIdLike) -> Result<(), JsValue> {
        let account = resolve_account_id(account.into())?;
        self.inner.unblock(account);
        Ok(())
    }

    /// Replaces the stored reason for an already-blocked `account`.
    ///
    /// `account` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountBlockError` (`err.kind === "AccountNotBlocked"`) when
    /// `account` is not currently blocked, or `AccountIdError` on an invalid
    /// identifier.
    #[wasm_bindgen(js_name = replaceBlockReason)]
    pub fn replace_block_reason(
        &self,
        account: AccountIdLike,
        reason: String,
    ) -> Result<(), JsValue> {
        let account = resolve_account_id(account.into())?;
        self.inner
            .replace_block_reason(account, reason)
            .map_err(|error| account_block_error_to_js(&error))
    }

    /// Blocks every account in `group` with the operator-supplied `reason`.
    ///
    /// `group` accepts an `AccountGroupId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountBlockError` (`err.kind === "ReservedGroup"`) when `group`
    /// is the reserved default group, or `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = blockGroup)]
    pub fn block_group(&self, group: AccountGroupIdLike, reason: String) -> Result<(), JsValue> {
        let group = resolve_account_group_id(group.into())?;
        self.inner
            .block_group(group, reason)
            .map_err(|error| account_block_error_to_js(&error))
    }

    /// Unblocks every account in `group`.
    ///
    /// `group` accepts an `AccountGroupId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountBlockError` (`err.kind === "ReservedGroup"`) when `group`
    /// is the reserved default group, or `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = unblockGroup)]
    pub fn unblock_group(&self, group: AccountGroupIdLike) -> Result<(), JsValue> {
        let group = resolve_account_group_id(group.into())?;
        self.inner
            .unblock_group(group)
            .map_err(|error| account_block_error_to_js(&error))
    }

    /// Replaces the stored reason for an already-blocked `group`.
    ///
    /// `group` accepts an `AccountGroupId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountBlockError` when `group` is the reserved default group
    /// (`err.kind === "ReservedGroup"`) or is not currently blocked
    /// (`err.kind === "GroupNotBlocked"`), or `ParamError` on an invalid
    /// identifier.
    #[wasm_bindgen(js_name = replaceGroupBlockReason)]
    pub fn replace_group_block_reason(
        &self,
        group: AccountGroupIdLike,
        reason: String,
    ) -> Result<(), JsValue> {
        let group = resolve_account_group_id(group.into())?;
        self.inner
            .replace_group_block_reason(group, reason)
            .map_err(|error| account_block_error_to_js(&error))
    }
}

/// Collects an iterable of `AccountId` objects (or numeric/string identifiers)
/// into a vector.
fn collect_account_ids(accounts: JsValue) -> Result<Vec<openpit::param::AccountId>, JsValue> {
    let iterator = js_sys::try_iter(&accounts)?.ok_or_else(account_iter_error)?;

    let mut ids = Vec::new();
    for item in iterator {
        let item = item?;
        ids.push(resolve_account_id(item)?);
    }
    Ok(ids)
}

/// Builds the error raised when the accounts argument is not iterable.
fn account_iter_error() -> JsValue {
    make_error(
        ErrorKind::Type,
        "accounts must be an iterable of AccountId",
        None,
    )
}

// ─── Builders ────────────────────────────────────────────────────────────────

/// Initial stage of the engine builder: requires at least one policy.
///
/// The first `preTrade`/`builtin` call advances to [`JsReadyEngineBuilder`].
/// `marketData` opens a market-data builder without leaving this stage. The
/// engine always uses no-op locking for the single-threaded WASM runtime.
#[wasm_bindgen(js_name = EngineBuilder)]
pub struct JsEngineBuilder;

#[wasm_bindgen(js_class = EngineBuilder)]
impl JsEngineBuilder {
    /// Registers a custom pre-trade policy and advances to the ready builder.
    ///
    /// `policy` is any JS object implementing the `Policy` shape; it is adapted
    /// to a core `PreTradePolicy` immediately.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `policy` is not a valid `Policy` object.
    #[wasm_bindgen(js_name = preTrade)]
    pub fn pre_trade(&self, policy: PolicyLike) -> Result<JsReadyEngineBuilder, JsValue> {
        let builder = JsReadyEngineBuilder::new();
        builder.register_pre_trade(policy.into())?;
        Ok(builder)
    }

    /// Registers a builtin policy from its ready-builder token and advances to
    /// the ready builder.
    ///
    /// `readyBuilder` is an always-ready builtin builder or the result of a
    /// required barrier-stage configuration call.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `readyBuilder` is not a builtin ready-builder
    /// token, or `EngineBuildError` when the builtin's configuration is invalid.
    #[wasm_bindgen(js_name = builtin)]
    pub fn builtin(
        &self,
        ready_builder: BuiltinReadyBuilder,
    ) -> Result<JsReadyEngineBuilder, JsValue> {
        let builder = JsReadyEngineBuilder::new();
        builder.register_builtin(ready_builder.into())?;
        Ok(builder)
    }

    /// Opens a market-data builder with the given default TTL.
    ///
    /// The resulting service uses no-op locks.
    #[wasm_bindgen(js_name = marketData)]
    pub fn market_data(&self, default_ttl: &JsQuoteTtl) -> JsMarketDataBuilder {
        JsMarketDataBuilder::with_default_ttl(default_ttl.inner())
    }
}

/// Stage 3 of the staged engine builder: build-ready.
///
/// Accepts further `preTrade`/`builtin`/`marketData` calls and finalizes via
/// `build()`. The core builder is held in a `RefCell<Option<..>>` and advanced
/// with a take/replace pattern so failed operations can restore the prior
/// state.
#[wasm_bindgen(js_name = ReadyEngineBuilder)]
pub struct JsReadyEngineBuilder {
    state: RefCell<Option<BuilderState>>,
}

#[wasm_bindgen(js_class = ReadyEngineBuilder)]
impl JsReadyEngineBuilder {
    /// Registers an additional custom pre-trade policy.
    ///
    /// `policy` is any JS object implementing the `Policy` shape.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `policy` is not a valid `Policy` object, or
    /// `LifecycleError` when the builder has already been consumed.
    #[wasm_bindgen(js_name = preTrade)]
    pub fn pre_trade(&self, policy: PolicyLike) -> Result<(), JsValue> {
        self.register_pre_trade(policy.into())
    }

    /// Registers an additional builtin policy from its ready-builder token.
    ///
    /// `readyBuilder` is an always-ready builtin builder or the result of a
    /// required barrier-stage configuration call.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `readyBuilder` is not a builtin ready-builder
    /// token, `EngineBuildError` when the builtin's configuration is invalid, or
    /// `LifecycleError` when the builder has already been consumed.
    #[wasm_bindgen(js_name = builtin)]
    pub fn builtin(&self, ready_builder: BuiltinReadyBuilder) -> Result<(), JsValue> {
        self.register_builtin(ready_builder.into())
    }

    /// Opens a market-data builder with the given default TTL.
    #[wasm_bindgen(js_name = marketData)]
    pub fn market_data(&self, default_ttl: &JsQuoteTtl) -> JsMarketDataBuilder {
        JsMarketDataBuilder::with_default_ttl(default_ttl.inner())
    }

    /// Finalizes the configuration and builds the engine.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the builder has already been consumed, or
    /// `EngineBuildError` when no policy is registered or on a duplicate policy
    /// name or group id.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> Result<JsEngine, JsValue> {
        let state = self.take_state()?;
        match state {
            BuilderState::Ready(builder) => builder
                .build()
                .map(|inner| JsEngine { inner })
                .map_err(|error| engine_build_error_to_js(&error)),
            BuilderState::Synced(_) => {
                let payload = Object::new();
                Err(make_error_with(
                    ErrorKind::EngineBuild,
                    "no policies registered",
                    Some("NoPolicies"),
                    payload.into(),
                    JsValue::UNDEFINED,
                ))
            }
        }
    }
}

impl JsReadyEngineBuilder {
    /// Builds a fresh ready builder seeded with the no-sync core builder.
    fn new() -> Self {
        let builder = EngineBuilder::<Order, ExecutionReport, AccountAdjustment>::new()
            .sync(EngineLocking::new(SyncMode::None));
        Self {
            state: RefCell::new(Some(BuilderState::Synced(builder))),
        }
    }

    /// Removes and returns the current builder state.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the builder has already been consumed.
    fn take_state(&self) -> Result<BuilderState, JsValue> {
        self.state.borrow_mut().take().ok_or_else(|| {
            make_error(
                ErrorKind::Lifecycle,
                "engine builder is no longer available",
                None,
            )
        })
    }

    /// Adapts and registers a custom JS policy, advancing the builder state.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `policy` is invalid, or `LifecycleError` when
    /// the builder has already been consumed.
    fn register_pre_trade(&self, policy: JsValue) -> Result<(), JsValue> {
        // `from_js` must run before `take_state`: an invalid policy returns its
        // error while the builder state is still in place, so a rejected
        // registration never consumes the builder (mirrors the restore-on-
        // failure rationale in `register_builtin`).
        let adapter = JsPreTradePolicyAdapter::from_js(policy)?;
        let boxed = BoxedPreTradePolicy::new(Box::new(adapter));
        let state = self.take_state()?;
        *self.state.borrow_mut() = Some(state.pre_trade(boxed));
        Ok(())
    }

    /// Resolves a builtin ready-builder token and registers its core policy.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `token` is not a builtin ready-builder,
    /// `EngineBuildError` on an invalid builtin configuration, or
    /// `LifecycleError` on a consumed builder.
    fn register_builtin(&self, token: JsValue) -> Result<(), JsValue> {
        let state = self.take_state()?;
        let policy = match build_builtin_policy(&token, state.storage_builder()) {
            Ok(policy) => policy,
            Err(error) => {
                // Restore the state so a failed builtin does not consume the
                // builder; the caller may correct and retry.
                *self.state.borrow_mut() = Some(state);
                return Err(error);
            }
        };
        *self.state.borrow_mut() = Some(state.builtin(policy));
        Ok(())
    }
}

/// Resolves a builtin ready-builder token into its boxed core policy.
///
/// Downcasts `token` to each known builtin ready-builder type and builds the
/// corresponding core policy with the engine's `storage_builder`.
///
/// # Errors
///
/// Throws `TypeError` when `token` is not a recognized builtin ready-builder,
/// or `EngineBuildError`/`ParamError` when the builtin's configuration is
/// invalid.
fn build_builtin_policy(
    token: &JsValue,
    storage_builder: &StorageBuilderRef,
) -> Result<BuiltinPolicy, JsValue> {
    if let Some(builder) = extract_cloned_wrapper::<JsOrderValidationBuilder>(token)? {
        return Ok(BuiltinPolicy::OrderValidation(builder.build_policy()));
    }
    if let Some(builder) = extract_cloned_wrapper::<JsOrderSizeLimitBuilder>(token)? {
        return builder.build_policy().map(BuiltinPolicy::OrderSizeLimit);
    }
    if let Some(builder) = extract_cloned_wrapper::<JsRateLimitBuilder>(token)? {
        return builder
            .build_policy(storage_builder)
            .map(BuiltinPolicy::RateLimit);
    }
    if let Some(builder) = extract_cloned_wrapper::<JsPnlBoundsKillswitchBuilder>(token)? {
        return builder
            .build_policy(storage_builder)
            .map(BuiltinPolicy::PnlBoundsKillswitch);
    }
    if let Some(builder) = extract_cloned_wrapper::<JsSpotFundsBuilder>(token)? {
        return builder
            .build_policy(storage_builder)
            .map(BuiltinPolicy::SpotFunds);
    }
    if let Some(builder) = extract_cloned_wrapper::<JsSpotFundsPnlBoundsKillswitchBuilder>(token)? {
        return builder
            .build_policy(storage_builder)
            .map(BuiltinPolicy::SpotFunds);
    }
    Err(make_error(
        ErrorKind::Type,
        "builtin expects a builtin ready-builder token (build*() result)",
        None,
    ))
}

/// Finishes one callback scope and surfaces its first error.
///
/// # Errors
///
/// Throws `PolicyCallbackError` with the original value as `cause`.
fn finish_callback_scope(scope: CallbackErrorScope, result: JsValue) -> Result<(), JsValue> {
    match scope.finish() {
        Some(cause) => Err(policy_callback_error(cause, result)),
        None => Ok(()),
    }
}

/// Converts a core engine-build failure with its stable discriminant/payload.
fn engine_build_error_to_js(error: &openpit::EngineBuildError) -> JsValue {
    let payload = Object::new();
    let (kind, message) = match error {
        openpit::EngineBuildError::DuplicatePolicyName { name } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("name"),
                &JsValue::from_str(name),
            );
            ("DuplicatePolicyName", error.to_string())
        }
        openpit::EngineBuildError::DuplicatePolicyGroupId { policy_group_id } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("policyGroupId"),
                &JsValue::from_f64(f64::from(policy_group_id.value())),
            );
            ("DuplicatePolicyGroupId", error.to_string())
        }
        _ => ("InvalidConfiguration", error.to_string()),
    };
    make_error_with(
        ErrorKind::EngineBuild,
        &message,
        Some(kind),
        payload.into(),
        JsValue::UNDEFINED,
    )
}
