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

//! Custom JavaScript policy adapter.
//!
//! Turns a JS object implementing the `Policy` shape into an `openpit`
//! [`PreTradePolicy`]. The adapter reads fields off the JS object, calls its
//! hooks, and translates the returned decision/result/reject/mutation shapes
//! into core types.
//!
//! ## Callback-error bridge (critical under `panic = "abort"`)
//!
//! A JS callback may throw. Because `panic = "abort"` turns any Rust panic into
//! an unrecoverable wasm trap, a thrown `JsValue` must never become a Rust
//! panic. Instead the adapter captures the thrown value in a thread-local
//! operation-scoped stack, returns a benign sentinel reject into the engine
//! core, and lets the core unwind normally. The engine entry point then wraps
//! the first captured exception together with any reconciled result.
//!
//! The builtin policy builders live in the sibling modules and are re-exported
//! through this module; their JS surface is described in their own files.

use std::cell::RefCell;

use js_sys::{Function, Reflect};
use openpit::param::{AccountId, Price};
use openpit::pretrade::{
    AccountBlock, PolicyAccountAdjustmentResult, PolicyGroupId, PolicyPreTradeResult,
    PostTradeContext, PostTradeResult, PreTradeContext, PreTradePolicy, Reject, RejectCode,
    RejectScope, Rejects,
};
use openpit::{
    AccountAdjustmentContext, AccountOutcomeEntry, Mutation, Mutations, OutcomeAmount, PnlOutcome,
    PnlOutcomeAmount,
};
use wasm_bindgen::prelude::*;

use crate::context::{JsAccountAdjustmentContext, JsContext, JsPostTradeContext};
use crate::domain::{
    extract_cloned_wrapper, is_plain_object, parse_asset, resolve_pnl, resolve_position_size,
    resolve_price,
};
use crate::engine::{AccountAdjustment, EngineLocking, ExecutionReport, Order};
use crate::error::{make_error, ErrorKind};
use crate::outcome::{
    JsAccountOutcomeEntry, JsAccountPnlOutcome, JsOutcomeAmount, JsPnlHaltReason, JsPnlOutcome,
    JsPnlOutcomeAmount,
};
use crate::param::ids::JsAccountId;
use crate::reject::{parse_reject_code, JsAccountBlock};
use crate::result::JsPostTradeResult;

pub mod order_size_limit;
pub mod order_validation;
pub mod pnl_killswitch;
pub mod rate_limit;
pub mod spot_funds;

#[wasm_bindgen(inline_js = r#"
export function observeNativePromiseSettlement(value) {
  try {
    Promise.prototype.then.call(
      value,
      () => undefined,
      () => undefined,
    );
  } catch {
    // A foreign thenable has no native Promise slots. Do not invoke it.
  }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = observeNativePromiseSettlement)]
    fn observe_native_promise_settlement(value: &JsValue);
}

// TypeScript surface for the custom-policy SDK. The adapter above reads these
// shapes off the JS policy object field by field (it never instantiates a wasm
// class for them), so the result-bearing shapes are plain interfaces rather
// than exported classes. Only the names that exist as exported wasm classes
// (`Context`, `Order`, `PostTradeContext`, `ExecutionReport`,
// `PostTradeResult`, `AccountAdjustmentContext`, `AccountId`,
// `AccountAdjustment`, `AccountBlock`, `AccountOutcomeEntry`,
// `AccountPnlOutcome`, `AccountAdjustmentOutcome`, `PnlOutcome`,
// `PnlOutcomeAmount`, `PnlHaltReason`, `Price`) are referenced directly.
// `Mutation` is a JS-only class imported from `../tx/index.js`; the remaining
// result-bearing shapes are defined here. This inventory drifts easily as the
// SDK grows - keep it complete when adding a new referenced class.
#[wasm_bindgen(typescript_custom_section)]
const POLICY_TS: &'static str = r#"
/**
 * A reject raised by a custom {@link Policy}.
 *
 * `scope` selects an order-level reject (`"order"`, rejects this order) or an
 * account-level reject (`"account"`, escalates to the account) and defaults to
 * `"order"`; the exported `RejectScope` value set carries the same literals.
 * `userData` defaults to `0` when omitted.
 */
export interface PolicyReject {
  code: import("../types.js").RejectCode;
  reason: string;
  details: string;
  scope?: import("../types.js").RejectScope;
  userData?: number | bigint;
}

/**
 * A commit/rollback pair contributed by a policy decision. Either the
 * `Mutation` wrapper class (from `@openpit/engine/tx`) or a plain object with
 * both callbacks is accepted.
 */
export type PolicyMutation =
  | import("../tx/index.js").Mutation
  | {
      commit: import("../tx/index.js").MutationFn;
      rollback: import("../tx/index.js").MutationFn;
    };

/** Plain-object form of an account outcome amount. */
export interface PolicyOutcomeAmount {
  delta: PositionSize | string | number | bigint;
  absolute: PositionSize | string | number | bigint;
}

/** Plain-object form of a realized-P&L outcome amount. */
export interface PolicyPnlOutcomeAmount {
  delta: Pnl | string | number | bigint;
  absolute: Pnl | string | number | bigint;
}

/** Plain-object form of a realized-P&L outcome. */
export type PolicyPnlOutcome =
  | {
      pnl: PnlOutcomeAmount | PolicyPnlOutcomeAmount;
      haltReason?: never;
    }
  | {
      pnl?: never;
      haltReason: PnlHaltReason;
    };

/** Plain-object form accepted wherever a policy returns an account outcome. */
export interface PolicyAccountOutcomeEntry {
  asset: string;
  balance?: OutcomeAmount | PolicyOutcomeAmount;
  held?: OutcomeAmount | PolicyOutcomeAmount;
  incoming?: OutcomeAmount | PolicyOutcomeAmount;
  realizedPnl?: PnlOutcome | PolicyPnlOutcome;
  averageEntryPrice?: Price | string | number | bigint;
}

/**
 * A pre-trade decision: rejects to raise and/or mutations to stage. An empty
 * decision (no rejects, no mutations) accepts the order.
 */
export interface PolicyDecision {
  rejects?: Iterable<PolicyReject>;
  mutations?: Iterable<PolicyMutation>;
}

/**
 * The result of {@link Policy.performPreTradeCheck}. Carries optional rejects
 * and mutations (as in {@link PolicyDecision}) plus optional account
 * adjustments and lock prices. Returning `null`/`undefined` accepts the order
 * with no contribution.
 */
export interface PolicyPreTradeResult {
  rejects?: Iterable<PolicyReject>;
  mutations?: Iterable<PolicyMutation>;
  accountAdjustments?: Iterable<AccountOutcomeEntry | PolicyAccountOutcomeEntry>;
  lockPrices?: Iterable<Price | string | number | bigint>;
}

/**
 * Accepted result returned by {@link Policy.applyAccountAdjustment}.
 * Every field is optional; an empty object contributes no result. A nonempty
 * runtime object must contain at least one recognized field. Additional fields
 * are allowed when a recognized field is present.
 */
export interface PolicyAccountAdjustmentResult {
  rejects?: Iterable<PolicyReject>;
  mutations?: Iterable<PolicyMutation>;
  accountAdjustments?: Iterable<AccountOutcomeEntry | PolicyAccountOutcomeEntry>;
  accountBlocks?: Iterable<AccountBlock>;
}

/** Structural post-trade result accepted from a custom policy. */
export interface PolicyPostTradeResult {
  accountBlocks?: Iterable<AccountBlock>;
  accountPnls?: Iterable<AccountPnlOutcome>;
  accountAdjustments?: Iterable<AccountAdjustmentOutcome>;
}

/**
 * A custom pre-trade policy implemented in JavaScript. Pass an object
 * satisfying this shape to {@link EngineBuilder.preTrade} /
 * {@link ReadyEngineBuilder.preTrade}; the engine adapts it to a native policy
 * and invokes its hooks during the pre-trade, post-trade, and
 * account-adjustment flows.
 *
 * `checkPreTradeStart` and `performPreTradeCheck` are required. Their optional
 * dry-run counterparts let a stateful policy provide a read-only emulation;
 * when absent, the engine falls back to the corresponding normal hook.
 * `applyExecutionReport` and `applyAccountAdjustment` are optional. Each hook
 * is called with `this` bound to the policy object.
 */
export interface Policy<
  OrderModel extends object = Order,
  ExecutionReportModel extends object = ExecutionReport,
> {
  /** Unique policy name. */
  readonly name: string;
  /** Policy group id (`0..=65535`); defaults to `0` when omitted. */
  readonly policyGroupId?: number;
  /**
   * Start-stage check. Returns the rejects to raise (empty iterable to accept).
   */
  checkPreTradeStart(ctx: Context, order: OrderModel): Iterable<PolicyReject>;
  /**
   * Read-only start-stage hook used by `startPreTradeDryRun` and
   * `executePreTradeDryRun`. Falls back to {@link checkPreTradeStart} when
   * omitted.
   */
  checkPreTradeStartDryRun?(
    ctx: Context,
    order: OrderModel,
  ): Iterable<PolicyReject>;
  /**
   * Main-stage check. Returns a {@link PolicyPreTradeResult}, or
   * `null`/`undefined` to accept with no contribution.
   */
  performPreTradeCheck(
    ctx: Context,
    order: OrderModel,
  ): PolicyPreTradeResult | null | undefined;
  /**
   * Read-only main-stage hook used by `executePreTradeDryRun`. Falls back to
   * {@link performPreTradeCheck} when omitted.
   */
  performPreTradeCheckDryRun?(
    ctx: Context,
    order: OrderModel,
  ): PolicyPreTradeResult | null | undefined;
  /**
   * Post-trade hook applied to an execution report. Returns a
   * {@link PostTradeResult}, its structural equivalent, or `null`/`undefined`
   * for no contribution.
   */
  applyExecutionReport?(
    ctx: PostTradeContext,
    report: ExecutionReportModel,
  ): PostTradeResult | PolicyPostTradeResult | null | undefined;
  /**
   * Account-adjustment hook. Returns a
   * {@link PolicyAccountAdjustmentResult}, or `null`/`undefined` for no
   * contribution.
   */
  applyAccountAdjustment?(
    ctx: AccountAdjustmentContext,
    accountId: AccountId,
    adjustment: AccountAdjustment,
  ): PolicyAccountAdjustmentResult | null | undefined;
}

/** @internal Nominal marker carried by tokens accepted by `builtin()`. */
declare const builtinReadyBuilderBrand: unique symbol;

/** Always-ready order-validation builder token. */
export interface OrderValidationReadyBuilder extends OrderValidationBuilder {
  readonly [builtinReadyBuilderBrand]: true;
  withPolicyGroupId(
    ...args: Parameters<OrderValidationBuilder["withPolicyGroupId"]>
  ): OrderValidationReadyBuilder;
  clone(): OrderValidationReadyBuilder;
}

/** Order-size-limit builder with at least one barrier configuration call. */
export interface OrderSizeLimitReadyBuilder extends OrderSizeLimitBuilder {
  readonly [builtinReadyBuilderBrand]: true;
  brokerBarrier(
    ...args: Parameters<OrderSizeLimitBuilder["brokerBarrier"]>
  ): OrderSizeLimitReadyBuilder;
  assetBarriers(
    ...args: Parameters<OrderSizeLimitBuilder["assetBarriers"]>
  ): OrderSizeLimitReadyBuilder;
  accountAssetBarriers(
    ...args: Parameters<OrderSizeLimitBuilder["accountAssetBarriers"]>
  ): OrderSizeLimitReadyBuilder;
  withPolicyGroupId(
    ...args: Parameters<OrderSizeLimitBuilder["withPolicyGroupId"]>
  ): OrderSizeLimitReadyBuilder;
  clone(): OrderSizeLimitReadyBuilder;
}

/** Rate-limit builder with at least one barrier configuration call. */
export interface RateLimitReadyBuilder extends RateLimitBuilder {
  readonly [builtinReadyBuilderBrand]: true;
  brokerBarrier(
    ...args: Parameters<RateLimitBuilder["brokerBarrier"]>
  ): RateLimitReadyBuilder;
  assetBarriers(
    ...args: Parameters<RateLimitBuilder["assetBarriers"]>
  ): RateLimitReadyBuilder;
  accountBarriers(
    ...args: Parameters<RateLimitBuilder["accountBarriers"]>
  ): RateLimitReadyBuilder;
  accountAssetBarriers(
    ...args: Parameters<RateLimitBuilder["accountAssetBarriers"]>
  ): RateLimitReadyBuilder;
  withPolicyGroupId(
    ...args: Parameters<RateLimitBuilder["withPolicyGroupId"]>
  ): RateLimitReadyBuilder;
  clone(): RateLimitReadyBuilder;
}

/** P&L kill-switch builder with at least one barrier configuration call. */
export interface PnlBoundsKillswitchReadyBuilder
  extends PnlBoundsKillswitchBuilder {
  readonly [builtinReadyBuilderBrand]: true;
  brokerBarriers(
    ...args: Parameters<PnlBoundsKillswitchBuilder["brokerBarriers"]>
  ): PnlBoundsKillswitchReadyBuilder;
  accountBarriers(
    ...args: Parameters<PnlBoundsKillswitchBuilder["accountBarriers"]>
  ): PnlBoundsKillswitchReadyBuilder;
  withPolicyGroupId(
    ...args: Parameters<PnlBoundsKillswitchBuilder["withPolicyGroupId"]>
  ): PnlBoundsKillswitchReadyBuilder;
  clone(): PnlBoundsKillswitchReadyBuilder;
}

/** Always-ready spot-funds builder token. */
export interface SpotFundsReadyBuilder extends SpotFundsBuilder {
  readonly [builtinReadyBuilderBrand]: true;
  withPolicyGroupId(
    ...args: Parameters<SpotFundsBuilder["withPolicyGroupId"]>
  ): SpotFundsReadyBuilder;
  marketData(
    ...args: Parameters<SpotFundsBuilder["marketData"]>
  ): SpotFundsReadyBuilder;
  clone(): SpotFundsReadyBuilder;
}

/** Spot-funds P&L builder with at least one barrier configuration call. */
export interface SpotFundsPnlBoundsKillswitchReadyBuilder
  extends SpotFundsPnlBoundsKillswitchBuilder {
  readonly [builtinReadyBuilderBrand]: true;
  globalBarrier(
    ...args: Parameters<SpotFundsPnlBoundsKillswitchBuilder["globalBarrier"]>
  ): SpotFundsPnlBoundsKillswitchReadyBuilder;
  accountGroupBarriers(
    ...args: Parameters<
      SpotFundsPnlBoundsKillswitchBuilder["accountGroupBarriers"]
    >
  ): SpotFundsPnlBoundsKillswitchReadyBuilder;
  accountBarriers(
    ...args: Parameters<SpotFundsPnlBoundsKillswitchBuilder["accountBarriers"]>
  ): SpotFundsPnlBoundsKillswitchReadyBuilder;
  withPolicyGroupId(
    ...args: Parameters<
      SpotFundsPnlBoundsKillswitchBuilder["withPolicyGroupId"]
    >
  ): SpotFundsPnlBoundsKillswitchReadyBuilder;
  marketData(
    ...args: Parameters<SpotFundsPnlBoundsKillswitchBuilder["marketData"]>
  ): SpotFundsPnlBoundsKillswitchReadyBuilder;
  clone(): SpotFundsPnlBoundsKillswitchReadyBuilder;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// A JS object implementing the `Policy` shape.
    ///
    /// Carried opaquely as the underlying `JsValue`; the adapter reads its
    /// fields and hooks (see [`JsPreTradePolicyAdapter::from_js`]). The typed
    /// wrapper only sharpens the generated `.d.ts` signature from `any` to
    /// `Policy`.
    #[wasm_bindgen(typescript_type = "Policy<any, any>")]
    pub type PolicyLike;

    /// A builtin ready-builder token: an always-ready builder or a builder
    /// whose required barrier stage has been configured.
    ///
    /// This is the closed union consumed by
    /// [`build_builtin_policy`](crate::engine). Builders that require at least
    /// one barrier enter the union only through their branded ready type.
    #[wasm_bindgen(
        typescript_type = "OrderValidationReadyBuilder | OrderSizeLimitReadyBuilder | RateLimitReadyBuilder | PnlBoundsKillswitchReadyBuilder | SpotFundsReadyBuilder | SpotFundsPnlBoundsKillswitchReadyBuilder"
    )]
    pub type BuiltinReadyBuilder;
}

/// One nested binding operation's callback-error capture state.
enum CallbackErrorFrame {
    /// Capture the first error raised while this operation is active.
    Capture(Option<JsValue>),
    /// Ignore callback errors while an implicit destructor rollback runs.
    Suppress,
}

thread_local! {
    /// Operation-scoped callback-error stack.
    ///
    /// JavaScript is single-threaded here, but callbacks may synchronously
    /// re-enter another engine method. A stack keeps the nested method from
    /// draining or overwriting the outer method's error.
    static CALLBACK_ERRORS: RefCell<Vec<CallbackErrorFrame>> = const { RefCell::new(Vec::new()) };
}

/// RAII boundary around an engine operation that may invoke JS callbacks.
pub(crate) struct CallbackErrorScope {
    active: bool,
}

impl CallbackErrorScope {
    /// Starts an operation that captures its first callback error.
    pub(crate) fn capture() -> Self {
        CALLBACK_ERRORS.with(|frames| {
            frames.borrow_mut().push(CallbackErrorFrame::Capture(None));
        });
        Self { active: true }
    }

    /// Starts a nested scope that deliberately discards callback errors.
    ///
    /// Used only for implicit reservation rollback during destruction: there is
    /// no active JS call to receive the exception, and leaking it into a later
    /// unrelated operation would be worse than suppressing it.
    pub(crate) fn suppress() -> Self {
        CALLBACK_ERRORS.with(|frames| {
            frames.borrow_mut().push(CallbackErrorFrame::Suppress);
        });
        Self { active: true }
    }

    /// Finishes this operation and returns its first captured error.
    pub(crate) fn finish(mut self) -> Option<JsValue> {
        self.active = false;
        CALLBACK_ERRORS.with(|frames| match frames.borrow_mut().pop() {
            Some(CallbackErrorFrame::Capture(error)) => error,
            Some(CallbackErrorFrame::Suppress) | None => None,
        })
    }
}

impl Drop for CallbackErrorScope {
    fn drop(&mut self) {
        if self.active {
            CALLBACK_ERRORS.with(|frames| {
                frames.borrow_mut().pop();
            });
        }
    }
}

/// Records the first JS callback error in the innermost active operation.
///
/// When no operation is active (or an implicit destructor rollback is being
/// suppressed), the value is intentionally discarded so it cannot poison a
/// later unrelated engine call.
pub(crate) fn set_callback_error(error: JsValue) {
    CALLBACK_ERRORS.with(|frames| {
        let mut frames = frames.borrow_mut();
        if let Some(CallbackErrorFrame::Capture(pending)) = frames.last_mut() {
            if pending.is_none() {
                *pending = Some(error);
            }
        }
    });
}

/// Builds the sentinel reject returned into the core when a callback failed.
///
/// The captured `JsValue` carries the real error; this sentinel only lets the
/// core unwind cleanly so the entry point can re-throw the captured value.
fn callback_failure_reject(policy_name: &str) -> Reject {
    Reject::new(
        policy_name,
        RejectScope::Order,
        RejectCode::SystemUnavailable,
        "javascript policy callback failed",
        "javascript policy callback raised an exception",
    )
}

/// Builds the sentinel rejects collection for a failed callback.
fn callback_failure_rejects(policy_name: &str) -> Rejects {
    Rejects::from(callback_failure_reject(policy_name))
}

// ─── Adapter ─────────────────────────────────────────────────────────────────

/// Adapts a JS `Policy` object to the core [`PreTradePolicy`] trait.
///
/// The JS object and its bound hook functions are stored as `JsValue`s. On
/// `wasm32-unknown-unknown` (no atomics) `JsValue` is `Send`, so the adapter
/// satisfies the `+ Send` bound the binding-layer policy object requires while
/// remaining correct for the single-threaded runtime.
pub(crate) struct JsPreTradePolicyAdapter {
    name: String,
    policy_group_id: PolicyGroupId,
    policy: JsValue,
    check_pre_trade_start: Function,
    check_pre_trade_start_dry_run: Option<Function>,
    perform_pre_trade_check: Function,
    perform_pre_trade_check_dry_run: Option<Function>,
    apply_execution_report: Option<Function>,
    apply_account_adjustment: Option<Function>,
}

impl JsPreTradePolicyAdapter {
    /// Builds an adapter from a JS policy object.
    ///
    /// Reads the required `name` and
    /// `checkPreTradeStart`/`performPreTradeCheck` hooks, the optional
    /// `policyGroupId`, and the optional
    /// `applyExecutionReport`/`applyAccountAdjustment` hooks.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` when `policy`/`name`/a hook has the wrong type,
    /// `RangeError` when `policyGroupId` is out of range, and rethrows a getter
    /// exception unchanged.
    pub(crate) fn from_js(policy: JsValue) -> Result<Self, JsValue> {
        if !policy.is_object() {
            return Err(policy_type_error("policy must be an object"));
        }
        let name = read_string(&policy, "name")?
            .ok_or_else(|| policy_type_error("policy.name must be a string"))?;
        let policy_group_id = read_policy_group_id(&policy, "policyGroupId")?;
        let check_pre_trade_start = read_function(&policy, "checkPreTradeStart")?;
        let check_pre_trade_start_dry_run =
            read_optional_function(&policy, "checkPreTradeStartDryRun")?;
        let perform_pre_trade_check = read_function(&policy, "performPreTradeCheck")?;
        let perform_pre_trade_check_dry_run =
            read_optional_function(&policy, "performPreTradeCheckDryRun")?;
        let apply_execution_report = read_optional_function(&policy, "applyExecutionReport")?;
        let apply_account_adjustment = read_optional_function(&policy, "applyAccountAdjustment")?;
        Ok(Self {
            name,
            policy_group_id,
            policy,
            check_pre_trade_start,
            check_pre_trade_start_dry_run,
            perform_pre_trade_check,
            perform_pre_trade_check_dry_run,
            apply_execution_report,
            apply_account_adjustment,
        })
    }
}

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment, EngineLocking>
    for JsPreTradePolicyAdapter
{
    fn name(&self) -> &str {
        &self.name
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.policy_group_id
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<crate::engine::StorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        let context = JsContext::from_parts(
            ctx.account_control.clone(),
            ctx.account_group(),
            order.payload.lifecycle(),
        );
        let context = JsValue::from(context);
        let payload = callback_payload(order.payload.fresh_js(), &self.name)?;
        let result = call_hook(
            &self.check_pre_trade_start,
            &self.policy,
            &[context, payload],
            &self.name,
        )?;
        let rejects = parse_policy_rejects(&result, &self.name).map_err(|error| {
            set_callback_error(error);
            callback_failure_rejects(&self.name)
        })?;
        if rejects.is_empty() {
            Ok(())
        } else {
            Err(Rejects::from(rejects))
        }
    }

    fn check_pre_trade_start_dry_run(
        &self,
        ctx: &PreTradeContext<crate::engine::StorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        let function = self
            .check_pre_trade_start_dry_run
            .as_ref()
            .unwrap_or(&self.check_pre_trade_start);
        let context = JsContext::from_parts(
            ctx.account_control.clone(),
            ctx.account_group(),
            order.payload.lifecycle(),
        );
        let payload = callback_payload(order.payload.fresh_js(), &self.name)?;
        let result = call_hook(
            function,
            &self.policy,
            &[JsValue::from(context), payload],
            &self.name,
        )?;
        let rejects = parse_policy_rejects(&result, &self.name).map_err(|error| {
            set_callback_error(error);
            callback_failure_rejects(&self.name)
        })?;
        if rejects.is_empty() {
            Ok(())
        } else {
            Err(Rejects::from(rejects))
        }
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<crate::engine::StorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        let context = JsContext::from_parts(
            ctx.account_control.clone(),
            ctx.account_group(),
            order.payload.lifecycle(),
        );
        let context = JsValue::from(context);
        let payload = callback_payload(order.payload.fresh_js(), &self.name)?;
        let decision = call_hook(
            &self.perform_pre_trade_check,
            &self.policy,
            &[context, payload],
            &self.name,
        )?;
        let mut rejects = Vec::new();
        let result =
            match apply_policy_pre_trade_result(&self.name, &decision, mutations, &mut rejects) {
                Ok(result) => result,
                Err(error) => {
                    set_callback_error(error);
                    return Err(callback_failure_rejects(&self.name));
                }
            };
        if ctx.is_drop_copy() {
            if let (Some(control), Some(reject)) = (
                ctx.account_control.as_ref(),
                rejects
                    .iter()
                    .find(|reject| reject.scope == RejectScope::Account),
            ) {
                control.block(reject.account_block_with_code(RejectCode::AccountBlocked));
            }
            Ok(result)
        } else if rejects.is_empty() {
            Ok(result)
        } else {
            Err(Rejects::from(rejects))
        }
    }

    fn perform_pre_trade_check_dry_run(
        &self,
        ctx: &PreTradeContext<crate::engine::StorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        let function = self
            .perform_pre_trade_check_dry_run
            .as_ref()
            .unwrap_or(&self.perform_pre_trade_check);
        let context = JsContext::from_parts(
            ctx.account_control.clone(),
            ctx.account_group(),
            order.payload.lifecycle(),
        );
        let payload = callback_payload(order.payload.fresh_js(), &self.name)?;
        let decision = call_hook(
            function,
            &self.policy,
            &[JsValue::from(context), payload],
            &self.name,
        )?;
        let mut rejects = Vec::new();
        let result =
            match apply_policy_pre_trade_result(&self.name, &decision, mutations, &mut rejects) {
                Ok(result) => result,
                Err(error) => {
                    set_callback_error(error);
                    return Err(callback_failure_rejects(&self.name));
                }
            };
        if rejects.is_empty() {
            Ok(result)
        } else {
            Err(Rejects::from(rejects))
        }
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<crate::engine::StorageFactory>,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        let hook = self.apply_execution_report.as_ref()?;
        let context = JsValue::from(JsPostTradeContext::from_group(ctx.account_group()));
        let payload = match report.payload.fresh_js() {
            Ok(payload) => payload,
            Err(error) => {
                set_callback_error(error);
                return None;
            }
        };
        let result = match call_function(hook, &self.policy, &[context, payload]) {
            Ok(result) => result,
            Err(error) => {
                set_callback_error(error);
                return None;
            }
        };
        if result.is_null() || result.is_undefined() {
            return None;
        }
        match parse_post_trade_result(&result) {
            Ok(core) => {
                if core.is_empty() {
                    None
                } else {
                    Some(core)
                }
            }
            Err(error) => {
                set_callback_error(error);
                None
            }
        }
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<crate::engine::StorageFactory>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
        let Some(hook) = self.apply_account_adjustment.as_ref() else {
            return Ok(PolicyAccountAdjustmentResult::default());
        };
        let context = JsValue::from(JsAccountAdjustmentContext::from_parts(
            ctx.account_control.clone(),
            ctx.account_group(),
            adjustment.payload.lifecycle(),
        ));
        let account = JsValue::from(JsAccountId::from_inner(account_id));
        let payload = callback_payload(adjustment.payload.fresh_js(), &self.name)?;
        let result =
            call_function(hook, &self.policy, &[context, account, payload]).map_err(|error| {
                set_callback_error(error);
                callback_failure_rejects(&self.name)
            })?;

        if result.is_null() || result.is_undefined() {
            return Ok(PolicyAccountAdjustmentResult::default());
        }

        match parse_account_adjustment_return(&self.name, &result, mutations) {
            Ok(AccountAdjustmentReturn::Result(result)) => Ok(result),
            Ok(AccountAdjustmentReturn::Rejects(rejects)) => Err(Rejects::from(rejects)),
            Err(error) => {
                set_callback_error(error);
                Err(callback_failure_rejects(&self.name))
            }
        }
    }
}

// ─── Callback invocation helpers ─────────────────────────────────────────────

/// Invokes a hook, mapping a thrown error onto a captured sentinel reject.
///
/// Used by the pre-trade hooks that return `Result<_, Rejects>`; a thrown JS
/// value is captured for re-throw and a sentinel `Rejects` is returned so the
/// core unwinds.
fn call_hook(
    function: &Function,
    this: &JsValue,
    args: &[JsValue],
    policy_name: &str,
) -> Result<JsValue, Rejects> {
    call_function(function, this, args).map_err(|error| {
        set_callback_error(error);
        callback_failure_rejects(policy_name)
    })
}

/// Maps payload-cloning failures onto the same sentinel path as a thrown hook.
fn callback_payload(
    payload: Result<JsValue, JsValue>,
    policy_name: &str,
) -> Result<JsValue, Rejects> {
    payload.map_err(|error| {
        set_callback_error(error);
        callback_failure_rejects(policy_name)
    })
}

/// Invokes a JS function with `this` bound and up to three positional args.
///
/// Returns the thrown `JsValue` unchanged on failure so the caller can capture
/// it for re-throw.
fn call_function(
    function: &Function,
    this: &JsValue,
    args: &[JsValue],
) -> Result<JsValue, JsValue> {
    let result = match args.len() {
        0 => function.call0(this),
        1 => function.call1(this, &args[0]),
        2 => function.call2(this, &args[0], &args[1]),
        _ => function.call3(this, &args[0], &args[1], &args[2]),
    }?;
    reject_thenable(&result)?;
    Ok(result)
}

/// Rejects asynchronous callback results from the synchronous engine boundary.
fn reject_thenable(value: &JsValue) -> Result<(), JsValue> {
    if !value.is_object() && !value.is_function() {
        return Ok(());
    }
    let then = Reflect::get(value, &JsValue::from_str("then"))?;
    if then.is_function() {
        observe_native_promise_settlement(value);
        return Err(policy_type_error(concat!(
            "policy callbacks must return synchronously; ",
            "Promise and thenable results are not supported"
        )));
    }
    Ok(())
}

// ─── JS object readers ───────────────────────────────────────────────────────

/// Builds a native `TypeError` for a malformed JavaScript shape.
fn policy_type_error(message: &str) -> JsValue {
    make_error(ErrorKind::Type, message, None)
}

/// Builds a native `RangeError` for an invalid numeric boundary value.
fn policy_range_error(message: &str) -> JsValue {
    make_error(ErrorKind::Range, message, None)
}

/// Reads a string property, returning `None` when absent or not a string.
/// A throwing getter is propagated unchanged.
fn read_string(object: &JsValue, key: &str) -> Result<Option<String>, JsValue> {
    Reflect::get(object, &JsValue::from_str(key)).map(|value| value.as_string())
}

/// Reads an optional `policyGroupId` property, defaulting to the default group.
///
/// # Errors
///
/// Throws `TypeError` when present but not a number, `RangeError` when it is not
/// an integer in `0..=65535`, and rethrows a getter exception unchanged.
fn read_policy_group_id(object: &JsValue, key: &str) -> Result<PolicyGroupId, JsValue> {
    let value = Reflect::get(object, &JsValue::from_str(key))?;
    if value.is_undefined() || value.is_null() {
        return Ok(openpit::DEFAULT_POLICY_GROUP_ID);
    }
    let number = value
        .as_f64()
        .ok_or_else(|| policy_type_error("policy.policyGroupId must be a number"))?;
    policy_group_id_from_f64(number)
}

/// Converts a JS number into a [`PolicyGroupId`], validating the `u16` range.
///
/// # Errors
///
/// Throws `RangeError` when the value is fractional, negative, non-finite, or
/// above 65535.
fn policy_group_id_from_f64(value: f64) -> Result<PolicyGroupId, JsValue> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(u16::MAX) {
        return Err(policy_range_error(
            "policyGroupId must be an integer in range 0..=65535",
        ));
    }
    Ok(PolicyGroupId::new(value as u16))
}

/// Reads a required callable property as a [`Function`].
///
/// # Errors
///
/// Throws `TypeError` when the property is missing or not callable, and
/// rethrows a getter exception unchanged.
fn read_function(object: &JsValue, key: &str) -> Result<Function, JsValue> {
    let value = Reflect::get(object, &JsValue::from_str(key))?;
    value
        .dyn_into::<Function>()
        .map_err(|_| policy_type_error(&format!("policy.{key} must be a function")))
}

/// Reads an optional callable property as a [`Function`].
///
/// Returns `None` when the property is absent, `null`, or `undefined`.
///
/// # Errors
///
/// Throws `TypeError` when present but not callable, and rethrows a getter
/// exception unchanged.
fn read_optional_function(object: &JsValue, key: &str) -> Result<Option<Function>, JsValue> {
    let value = Reflect::get(object, &JsValue::from_str(key))?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    value
        .dyn_into::<Function>()
        .map(Some)
        .map_err(|_| policy_type_error(&format!("policy.{key} must be a function")))
}

// ─── Decision/result parsing ─────────────────────────────────────────────────

/// Reads a property off a JS object, mapping a non-object to a `TypeError` and
/// preserving exceptions from user-defined getters.
fn get_field(object: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    if !object.is_object() {
        return Err(policy_type_error("policy return value must be an object"));
    }
    Reflect::get(object, &JsValue::from_str(key))
}

/// Returns `true` when `object` carries an own/inherited property named `key`.
fn has_field(object: &JsValue, key: &str) -> Result<bool, JsValue> {
    if !object.is_object() {
        return Ok(false);
    }
    Reflect::has(object, &JsValue::from_str(key))
}

/// Parses an iterable of `PolicyReject` shapes into core rejects.
///
/// A `null`/`undefined` value yields an empty list (mirroring the default empty
/// start-stage result).
///
/// # Errors
///
/// Throws `TypeError` when the value is not iterable or an element has the
/// wrong shape; semantic reject validation remains a `ParamError`.
fn parse_policy_rejects(value: &JsValue, policy_name: &str) -> Result<Vec<Reject>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(Vec::new());
    }
    let iterator = js_sys::try_iter(value)?.ok_or_else(|| {
        policy_type_error("checkPreTradeStart must return an iterable of rejects")
    })?;
    let mut rejects = Vec::new();
    for item in iterator {
        let item = item?;
        rejects.push(parse_policy_reject(&item, policy_name)?);
    }
    Ok(rejects)
}

/// Parses a single `PolicyReject` shape into a core [`Reject`].
///
/// # Errors
///
/// Throws `TypeError` when a field is missing or has the wrong type, and
/// `ParamError` when a reject code is unknown.
fn parse_policy_reject(value: &JsValue, policy_name: &str) -> Result<Reject, JsValue> {
    let code = read_string(value, "code")?
        .ok_or_else(|| policy_type_error("reject.code must be a string"))?;
    let code = parse_reject_code(&code)?;
    let reason = read_string(value, "reason")?
        .ok_or_else(|| policy_type_error("reject.reason must be a string"))?;
    let details = read_string(value, "details")?
        .ok_or_else(|| policy_type_error("reject.details must be a string"))?;
    let scope = match read_string(value, "scope")? {
        Some(scope) => parse_reject_scope(&scope)?,
        None => RejectScope::Order,
    };
    let user_data = read_user_data(value)?;
    Ok(Reject::new(policy_name, scope, code, reason, details).with_user_data(user_data))
}

/// Parses a reject-scope wire string (`"order"`/`"account"`, case-insensitive).
///
/// # Errors
///
/// Throws `RangeError` on any other value.
fn parse_reject_scope(value: &str) -> Result<RejectScope, JsValue> {
    match value.trim().to_ascii_lowercase().as_str() {
        "order" => Ok(RejectScope::Order),
        "account" => Ok(RejectScope::Account),
        _ => Err(policy_range_error(
            "reject.scope must be \"order\" or \"account\"",
        )),
    }
}

/// Reads an optional `userData` token, defaulting to `0`.
///
/// # Errors
///
/// Throws `TypeError` when present but not numeric, or `RangeError` when it is
/// not a non-negative integer that fits wasm32 `usize`.
fn read_user_data(value: &JsValue) -> Result<usize, JsValue> {
    let token = Reflect::get(value, &JsValue::from_str("userData"))?;
    if token.is_undefined() || token.is_null() {
        return Ok(0);
    }
    if let Some(number) = token.as_f64() {
        if number.is_finite()
            && number.fract() == 0.0
            && number >= 0.0
            && number <= usize::MAX as f64
        {
            return Ok(number as usize);
        }
        return Err(policy_range_error(
            "userData must be a non-negative integer within the supported token range",
        ));
    }
    if token.is_bigint() {
        let big: js_sys::BigInt = token.unchecked_into();
        let value = u64::try_from(big).map_err(|_| {
            policy_range_error("userData must be non-negative and within the supported token range")
        })?;
        return usize::try_from(value)
            .map_err(|_| policy_range_error("userData exceeds the supported token range"));
    }
    Err(policy_type_error(
        "userData must be a number or bigint integer token",
    ))
}

/// Applies a `PolicyDecision`-shaped value: collects rejects and mutations.
///
/// # Errors
///
/// Throws `TypeError` for malformed iterables/shapes and `ParamError` for an
/// unknown reject code.
fn apply_policy_decision(
    policy_name: &str,
    value: &JsValue,
    mutations: &mut Mutations,
    rejects: &mut Vec<Reject>,
) -> Result<(), JsValue> {
    let reject_items = get_field(value, "rejects")?;
    if !reject_items.is_null() && !reject_items.is_undefined() {
        for item in iterate(&reject_items, "decision.rejects must be iterable")? {
            rejects.push(parse_policy_reject(&item?, policy_name)?);
        }
    }
    let mutation_items = get_field(value, "mutations")?;
    if !mutation_items.is_null() && !mutation_items.is_undefined() {
        for item in iterate(&mutation_items, "decision.mutations must be iterable")? {
            mutations.push(parse_policy_mutation(&item?)?);
        }
    }
    Ok(())
}

/// Applies a `PolicyPreTradeResult`-shaped value returned by a main-stage hook.
///
/// Collects rejects/mutations from the decision part and the optional
/// `accountAdjustments`/`lockPrices` payloads. Returns `None` when the value is
/// `null`/`undefined` or carries no account adjustments or lock prices.
///
/// # Errors
///
/// Throws `TypeError` for malformed iterables/shapes, `ParamError` for invalid
/// domain values, or `AssetError` for an invalid outcome asset.
fn apply_policy_pre_trade_result(
    policy_name: &str,
    value: &JsValue,
    mutations: &mut Mutations,
    rejects: &mut Vec<Reject>,
) -> Result<Option<PolicyPreTradeResult>, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    if !value.is_object() {
        return Err(policy_type_error(
            "performPreTradeCheck must return a PolicyPreTradeResult, null, or undefined",
        ));
    }

    let has_rejects = has_field(value, "rejects")?;
    let has_mutations = has_field(value, "mutations")?;
    if has_rejects || has_mutations {
        apply_policy_decision(policy_name, value, mutations, rejects)?;
    }

    let has_account_adjustments = has_field(value, "accountAdjustments")?;
    let has_lock_prices = has_field(value, "lockPrices")?;
    if !has_rejects && !has_mutations && !has_account_adjustments && !has_lock_prices {
        if is_empty_record(value)? {
            return Ok(None);
        }
        return Err(policy_type_error(
            "performPreTradeCheck returned an object with no recognized result fields",
        ));
    }

    let mut result = PolicyPreTradeResult::empty();
    if has_account_adjustments {
        let items = get_field(value, "accountAdjustments")?;
        if !items.is_null() && !items.is_undefined() {
            for item in iterate(&items, "accountAdjustments must be iterable")? {
                result
                    .account_adjustments
                    .push(parse_account_outcome_entry(&item?)?);
            }
        }
    }
    if has_lock_prices {
        let items = get_field(value, "lockPrices")?;
        if !items.is_null() && !items.is_undefined() {
            for item in iterate(&items, "lockPrices must be iterable")? {
                result.lock_prices.push(parse_price(&item?)?);
            }
        }
    }
    if result.account_adjustments.is_empty() && result.lock_prices.is_empty() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

/// Parses a `Mutation`-shaped value (a `{ commit, rollback }` pair).
///
/// Both callbacks are bound and invoked with no `this`; a throw inside either
/// is captured for re-throw rather than panicking.
///
/// # Errors
///
/// Throws `TypeError` when `commit` or `rollback` is not callable.
fn parse_policy_mutation(value: &JsValue) -> Result<Mutation, JsValue> {
    let commit = read_function(value, "commit")?;
    let rollback = read_function(value, "rollback")?;
    let commit_undefined = JsValue::UNDEFINED;
    let rollback_undefined = JsValue::UNDEFINED;
    Ok(Mutation::new(
        move || {
            let result = commit
                .call0(&commit_undefined)
                .and_then(|value| reject_thenable(&value));
            if let Err(error) = result {
                set_callback_error(error);
            }
        },
        move || {
            let result = rollback
                .call0(&rollback_undefined)
                .and_then(|value| reject_thenable(&value));
            if let Err(error) = result {
                set_callback_error(error);
            }
        },
    ))
}

/// Parses an `AccountOutcomeEntry` shape (binding object or plain object).
///
/// # Errors
///
/// Throws `TypeError` when the shape is malformed, `ParamError` for invalid
/// numeric values, or `AssetError` for an invalid asset.
fn parse_account_outcome_entry(value: &JsValue) -> Result<AccountOutcomeEntry, JsValue> {
    if let Some(entry) = extract_cloned_wrapper::<JsAccountOutcomeEntry>(value)? {
        return entry.to_core();
    }
    if !value.is_object() {
        return Err(policy_type_error(
            "account adjustment outcome must be an AccountOutcomeEntry or object",
        ));
    }

    let asset = get_field(value, "asset")?
        .as_string()
        .ok_or_else(|| policy_type_error("account outcome asset must be a string"))?;
    Ok(AccountOutcomeEntry {
        asset: parse_asset(&asset)?,
        balance: parse_optional_outcome_amount(value, "balance")?,
        held: parse_optional_outcome_amount(value, "held")?,
        incoming: parse_optional_outcome_amount(value, "incoming")?,
        realized_pnl: parse_optional_pnl_outcome(value, "realizedPnl")?,
        average_entry_price: match optional_field(value, "averageEntryPrice")? {
            Some(price) => Some(resolve_price(price)?),
            None => None,
        },
    })
}

/// Reads an optional nullable object field while preserving getter errors.
fn optional_field(value: &JsValue, field: &str) -> Result<Option<JsValue>, JsValue> {
    let field = get_field(value, field)?;
    if field.is_null() || field.is_undefined() {
        Ok(None)
    } else {
        Ok(Some(field))
    }
}

/// Parses a position-size outcome amount from a wrapper or plain object.
fn parse_optional_outcome_amount(
    value: &JsValue,
    field: &str,
) -> Result<Option<OutcomeAmount>, JsValue> {
    let Some(amount) = optional_field(value, field)? else {
        return Ok(None);
    };
    if let Some(wrapped) = extract_cloned_wrapper::<JsOutcomeAmount>(&amount)? {
        return Ok(Some(wrapped.to_core()));
    }
    if !amount.is_object() {
        return Err(policy_type_error(&format!(
            "account outcome {field} must be an OutcomeAmount or object"
        )));
    }
    Ok(Some(OutcomeAmount {
        delta: resolve_position_size(get_field(&amount, "delta")?)?,
        absolute: resolve_position_size(get_field(&amount, "absolute")?)?,
    }))
}

/// Parses a realized-P&L outcome from a wrapper or plain object.
fn parse_optional_pnl_outcome(value: &JsValue, field: &str) -> Result<Option<PnlOutcome>, JsValue> {
    let Some(outcome) = optional_field(value, field)? else {
        return Ok(None);
    };
    if let Some(wrapped) = extract_cloned_wrapper::<JsPnlOutcome>(&outcome)? {
        return wrapped.to_core().map(Some);
    }
    if !outcome.is_object() {
        return Err(policy_type_error(&format!(
            "account outcome {field} must be a PnlOutcome or object"
        )));
    }
    let pnl = optional_field(&outcome, "pnl")?
        .map(|amount| {
            if let Some(wrapped) = extract_cloned_wrapper::<JsPnlOutcomeAmount>(&amount)? {
                return Ok(wrapped.to_core());
            }
            if !amount.is_object() {
                return Err(policy_type_error(
                    "account outcome pnl must be a PnlOutcomeAmount or object",
                ));
            }
            Ok(PnlOutcomeAmount {
                delta: resolve_pnl(get_field(&amount, "delta")?)?,
                absolute: resolve_pnl(get_field(&amount, "absolute")?)?,
            })
        })
        .transpose()?;
    let halt_reason = optional_field(&outcome, "haltReason")?
        .map(|reason| {
            extract_cloned_wrapper::<JsPnlHaltReason>(&reason)?.ok_or_else(|| {
                policy_type_error("account outcome haltReason must be a PnlHaltReason")
            })
        })
        .transpose()?;
    match (pnl, halt_reason) {
        (Some(amount), None) => Ok(Some(Ok(amount))),
        (None, Some(reason)) => Ok(Some(Err(reason.to_core()))),
        _ => Err(policy_type_error(
            "account outcome realizedPnl requires exactly one of pnl or haltReason",
        )),
    }
}

/// Parses a `Price` shape (binding `Price` object or `DecimalInput`).
///
/// # Errors
///
/// Throws the value-type boundary's `TypeError`/`RangeError`/`ParamError` on an
/// invalid value.
fn parse_price(value: &JsValue) -> Result<Price, JsValue> {
    resolve_price(value.clone())
}

/// Parses a `PostTradeResult` shape into the core type.
///
/// Accepts the binding `PostTradeResult` object or a plain object with
/// `accountBlocks`/`accountPnls`/`accountAdjustments` arrays.
///
/// # Errors
///
/// Throws `TypeError` when a field/iterable shape is invalid and `ParamError`
/// for invalid nested domain values.
fn parse_post_trade_result(value: &JsValue) -> Result<PostTradeResult, JsValue> {
    if let Some(result) = extract_cloned_wrapper::<JsPostTradeResult>(value)? {
        return result.to_core();
    }

    let has_account_blocks = has_field(value, "accountBlocks")?;
    let has_account_pnls = has_field(value, "accountPnls")?;
    let has_account_adjustments = has_field(value, "accountAdjustments")?;
    if !has_account_blocks && !has_account_pnls && !has_account_adjustments {
        if is_empty_record(value)? {
            return Ok(PostTradeResult::default());
        }
        return Err(policy_type_error(
            "applyExecutionReport returned an object with no recognized result fields",
        ));
    }

    let mut account_blocks = Vec::new();
    if has_account_blocks {
        let blocks = get_field(value, "accountBlocks")?;
        if !blocks.is_null() && !blocks.is_undefined() {
            for item in iterate(&blocks, "accountBlocks must be iterable")? {
                account_blocks.push(parse_account_block(&item?)?);
            }
        }
    }
    let mut account_pnls = Vec::new();
    if has_account_pnls {
        let pnls = get_field(value, "accountPnls")?;
        if !pnls.is_null() && !pnls.is_undefined() {
            for item in iterate(&pnls, "accountPnls must be iterable")? {
                account_pnls.push(parse_account_pnl_outcome(&item?)?);
            }
        }
    }
    let mut account_adjustments = Vec::new();
    if has_account_adjustments {
        let adjustments = get_field(value, "accountAdjustments")?;
        if !adjustments.is_null() && !adjustments.is_undefined() {
            for item in iterate(&adjustments, "accountAdjustments must be iterable")? {
                account_adjustments.push(parse_account_adjustment_outcome(&item?)?);
            }
        }
    }
    Ok(PostTradeResult {
        account_blocks,
        account_pnls,
        account_adjustments,
    })
}

/// Parses an `AccountBlock` shape into the core type.
///
/// # Errors
///
/// Throws `TypeError` when the value is not an `AccountBlock`, or `ParamError`
/// when its reject code is invalid.
fn parse_account_block(value: &JsValue) -> Result<AccountBlock, JsValue> {
    let block = extract_cloned_wrapper::<JsAccountBlock>(value)?
        .ok_or_else(|| policy_type_error("account block must be an AccountBlock"))?;
    block.to_core()
}

/// Parses an `AccountPnlOutcome` shape into the core type.
///
/// # Errors
///
/// Throws `TypeError` when the value is not an `AccountPnlOutcome`.
fn parse_account_pnl_outcome(value: &JsValue) -> Result<openpit::AccountPnlOutcome, JsValue> {
    let outcome = extract_cloned_wrapper::<JsAccountPnlOutcome>(value)?
        .ok_or_else(|| policy_type_error("account PnL outcome must be an AccountPnlOutcome"))?;
    outcome.to_core()
}

/// Parses an `AccountAdjustmentOutcome` shape into the core type.
///
/// # Errors
///
/// Throws `TypeError` when the value is not an `AccountAdjustmentOutcome`.
fn parse_account_adjustment_outcome(
    value: &JsValue,
) -> Result<openpit::AccountAdjustmentOutcome, JsValue> {
    let outcome = extract_cloned_wrapper::<crate::outcome::JsAccountAdjustmentOutcome>(value)?
        .ok_or_else(|| {
            policy_type_error("account adjustment outcome must be an AccountAdjustmentOutcome")
        })?;
    outcome.to_core()
}

/// Result of parsing the polymorphic `applyAccountAdjustment` return value.
enum AccountAdjustmentReturn {
    /// Accepted account-adjustment result to surface to the engine.
    Result(PolicyAccountAdjustmentResult),
    /// Account-level rejects raised by the policy.
    Rejects(Vec<Reject>),
}

/// Parses a `PolicyAccountAdjustmentResult` returned by a custom policy.
///
/// # Errors
///
/// Throws `TypeError` for malformed shapes/iterables, `RangeError` for invalid
/// boundary integers, and the nested `ParamError`/`AssetError` variants for
/// invalid domain values.
fn parse_account_adjustment_return(
    policy_name: &str,
    value: &JsValue,
    mutations: &mut Mutations,
) -> Result<AccountAdjustmentReturn, JsValue> {
    if !value.is_object() {
        return Err(policy_type_error(
            "applyAccountAdjustment must return a PolicyAccountAdjustmentResult",
        ));
    }

    let has_rejects = has_field(value, "rejects")?;
    let has_mutations = has_field(value, "mutations")?;
    let has_account_adjustments = has_field(value, "accountAdjustments")?;
    let has_account_blocks = has_field(value, "accountBlocks")?;
    if !has_rejects && !has_mutations && !has_account_adjustments && !has_account_blocks {
        if is_empty_record(value)? {
            return Ok(AccountAdjustmentReturn::Result(
                PolicyAccountAdjustmentResult::default(),
            ));
        }
        return Err(policy_type_error(
            "applyAccountAdjustment returned an object with no recognized result fields",
        ));
    }

    let mut rejects = Vec::new();
    if has_rejects || has_mutations {
        apply_policy_decision(policy_name, value, mutations, &mut rejects)?;
    }
    if !rejects.is_empty() {
        return Ok(AccountAdjustmentReturn::Rejects(rejects));
    }

    let mut parsed_adjustments = Vec::new();
    if has_account_adjustments {
        let account_adjustments = get_field(value, "accountAdjustments")?;
        if !account_adjustments.is_null() && !account_adjustments.is_undefined() {
            for entry in iterate(&account_adjustments, "accountAdjustments must be iterable")? {
                parsed_adjustments.push(parse_account_outcome_entry(&entry?)?);
            }
        }
    }

    let mut parsed_blocks = Vec::new();
    if has_account_blocks {
        let account_blocks = get_field(value, "accountBlocks")?;
        if !account_blocks.is_null() && !account_blocks.is_undefined() {
            for block in iterate(&account_blocks, "accountBlocks must be iterable")? {
                parsed_blocks.push(parse_account_block(&block?)?);
            }
        }
    }

    Ok(AccountAdjustmentReturn::Result(
        PolicyAccountAdjustmentResult {
            account_adjustments: parsed_adjustments,
            account_blocks: parsed_blocks,
        },
    ))
}

/// Builds a JS iterator over an iterable value, mapping a non-iterable to a
/// `TypeError` carrying `message` and preserving iterator getter exceptions.
///
/// # Errors
///
/// Throws `TypeError` when the value is not iterable.
fn iterate(value: &JsValue, message: &str) -> Result<js_sys::IntoIter, JsValue> {
    js_sys::try_iter(value)?.ok_or_else(|| policy_type_error(message))
}

/// Returns whether `value` is a record-like object with no own keys.
/// Proxy/reflect errors are propagated unchanged.
fn is_empty_record(value: &JsValue) -> Result<bool, JsValue> {
    if !is_plain_object(value) {
        return Ok(false);
    }
    Reflect::own_keys(value).map(|keys| keys.length() == 0)
}
