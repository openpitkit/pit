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

use std::fmt::{Display, Formatter};

use super::account_control::{AccountBlockHandle, AccountControl};
use super::account_groups::AccountGroupsHandle;
use super::account_outcome::{AccountAdjustmentBatchResult, AccountAdjustmentOutcome};
use super::accounts::Accounts;
use super::engine_builder::EngineBuilder;
use super::engine_trait::{EngineTrait, EngineTraitOf};
use super::mutation::{MutationFailure, MutationFailureKillSwitch};
use super::sync_mode::{AccountSync, FullSync, LocalSync, SyncMode};
use super::{
    AccountCurrencies, AccountGroups, BlockedAccounts, ConfigRegistry, Configurator, HasAccountId,
};
use crate::param::AccountId;
use crate::pretrade::handle::{RequestHandleImpl, ReservationHandleImpl};
use crate::pretrade::start_pre_trade_time::with_start_pre_trade_now;
use crate::pretrade::PostTradeContext;
use crate::pretrade::PreTradePolicy;
use crate::pretrade::{
    AccountBlock, DropCopyOperation, PolicyAccountAdjustmentResult, PolicyPreTradeResult,
    PostTradeResult, PreTradeContext, PreTradeDryRunReport, PreTradeLock, PreTradeRequest,
    PreTradeReservation, Reject, RejectCode, RejectScope, Rejects,
};
use crate::time::Instant;
use crate::{AccountAdjustmentContext, Mutations};

pub(crate) struct EngineInner<Trait: EngineTrait> {
    #[allow(clippy::type_complexity)]
    pub(crate) pre_trade_policies: Vec<
        Box<
            <Trait::Sync as SyncMode>::PreTradePolicyObject<
                Trait::Order,
                Trait::ExecutionReport,
                Trait::AccountAdjustment,
            >,
        >,
    >,
    pub(crate) blocked_accounts: <<Trait::Sync as SyncMode>::StorageLockingPolicyFactory
        as crate::storage::LockingPolicyFactory>::Shared<
        BlockedAccounts<<Trait::Sync as SyncMode>::StorageLockingPolicyFactory>,
    >,
    pub(crate) account_groups: <<Trait::Sync as SyncMode>::StorageLockingPolicyFactory
        as crate::storage::LockingPolicyFactory>::Shared<
        AccountGroups<<Trait::Sync as SyncMode>::StorageLockingPolicyFactory>,
    >,
    pub(crate) account_currencies: <<Trait::Sync as SyncMode>::StorageLockingPolicyFactory
        as crate::storage::LockingPolicyFactory>::Shared<
        AccountCurrencies<<Trait::Sync as SyncMode>::StorageLockingPolicyFactory>,
    >,
    pub(crate) config_registry: <<Trait::Sync as SyncMode>::StorageLockingPolicyFactory
        as crate::storage::LockingPolicyFactory>::Shared<
        ConfigRegistry<<Trait::Sync as SyncMode>::StorageLockingPolicyFactory>,
    >,
}

/// Error returned when account-adjustment batch validation fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountAdjustmentBatchError {
    /// Rejects produced by the policy.
    pub rejects: Rejects,
    /// Zero-based index of the failing adjustment.
    pub failed_adjustment_index: usize,
}

impl Display for AccountAdjustmentBatchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "account adjustment batch rejected at index {}: {}",
            self.failed_adjustment_index, self.rejects
        )
    }
}

impl std::error::Error for AccountAdjustmentBatchError {}

/// Risk engine orchestrating start-stage and main-stage pre-trade checks.
///
/// Build the engine once during platform initialization using
/// [`EngineBuilder::new`], then share it across order submissions.
///
/// Generic parameters:
/// - `Trait`: aggregate of order, execution-report, account-adjustment, and
///   synchronization-mode choices.
///
/// # Threading
///
/// The engine handle's thread-safety is determined by `Trait::Sync`:
///
/// - [`LocalSync`] (default, produced by `no_sync`): the handle
///   is `!Send + !Sync`. Keep it on the OS thread that created it. Concurrent
///   invocation is not supported.
/// - [`FullSync`] (produced by `full_sync`): the handle is
///   `Send + Sync` when all registered policies are `Send + Sync`. It can be
///   wrapped in `Arc<FullSyncEngine<...>>` and shared across threads. With
///   `FullLocking` storage, concurrent invocation from multiple threads is
///   safe.
/// - [`AccountSync`] (produced by `account_sync`): the handle
///   is `Send + !Sync`. Ownership may move between OS threads sequentially, but
///   concurrent invocation on the same handle is not supported.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Price, Quantity, Side, TradeAmount};
/// use openpit::{Engine, Instrument, OrderOperation, WithOrderOperation};
/// use openpit::{FinancialImpact, ExecutionReportOperation, WithFinancialImpact, WithExecutionReportOperation};
/// use openpit::pretrade::policies::OrderValidationPolicy;
///
/// type MyOrder = WithOrderOperation<()>;
/// type MyReport = WithExecutionReportOperation<WithFinancialImpact<()>>;
///
/// let engine = Engine::builder::<MyOrder, MyReport, ()>()
///     .no_sync()
///     .pre_trade(OrderValidationPolicy::new())
///     .build()?;
///
/// let order = WithOrderOperation {
///     inner: (),
///     operation: OrderOperation {
///         instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
///         account_id: openpit::param::AccountId::from_u64(12345),
///         side: Side::Buy,
///         trade_amount: TradeAmount::Quantity(Quantity::from_f64(100.0)?),
///         price: Some(Price::from_str("185")?),
///     },
/// };
///
/// let request = engine.start_pre_trade(order)?;
/// let mut reservation = request.execute()?;
/// reservation.commit();
/// # Ok(())
/// # }
/// ```
pub struct Engine<Trait: EngineTrait = EngineTraitOf<(), (), (), LocalSync>> {
    pub(crate) inner: <Trait::Sync as SyncMode>::Strong<EngineInner<Trait>>,
}

impl<Trait: EngineTrait> Engine<Trait> {
    pub(crate) fn from_inner(inner: <Trait::Sync as SyncMode>::Strong<EngineInner<Trait>>) -> Self {
        Self { inner }
    }

    /// Returns a handle to the engine's account registry.
    ///
    /// The returned [`Accounts`] handle shares the engine's single account-group
    /// registry and its single blocked-accounts set; register or unregister
    /// account-group membership and block or unblock accounts and groups through
    /// it. The handle is cloneable and inherits the engine's synchronization
    /// mode.
    pub fn accounts(&self) -> Accounts<<Trait::Sync as SyncMode>::StorageLockingPolicyFactory> {
        Accounts::new(
            AccountGroupsHandle::from_inner(self.inner.account_groups.clone()),
            AccountBlockHandle::from_inner(self.inner.blocked_accounts.clone()),
            self.inner.account_currencies.clone(),
            self.inner.config_registry.clone(),
        )
    }

    /// Returns a handle for retuning supported built-in policies at runtime.
    ///
    /// The returned [`Configurator`] shares the engine's settings registry; an
    /// update published through it is observed by the running policy on its
    /// next hot-path read. The handle is cloneable and inherits the engine's
    /// synchronization mode. Custom-policy runtime reconfiguration is planned
    /// for a later release.
    pub fn configure(&self) -> Configurator<Trait> {
        Configurator::from_inner(self.inner.clone())
    }
}

/// Single-threaded engine type alias.
///
/// Equivalent to
/// `Engine<EngineTraitOf<Order, ExecutionReport, AccountAdjustment, LocalSync>>`.
/// Produced by [`EngineBuilder::no_sync`] chains.
pub type LocalEngine<Order, ExecutionReport = (), AccountAdjustment = ()> =
    Engine<EngineTraitOf<Order, ExecutionReport, AccountAdjustment, LocalSync>>;

/// Account-sharded engine type alias.
///
/// Equivalent to
/// `Engine<EngineTraitOf<Order, ExecutionReport, AccountAdjustment, AccountSync>>`.
/// Produced by [`EngineBuilder::account_sync`] chains. Engine handle is
/// `Send + !Sync`.
pub type AccountSyncEngine<Order, ExecutionReport = (), AccountAdjustment = ()> =
    Engine<EngineTraitOf<Order, ExecutionReport, AccountAdjustment, AccountSync>>;

/// Multi-threaded engine type alias.
///
/// Equivalent to
/// `Engine<EngineTraitOf<Order, ExecutionReport, AccountAdjustment, FullSync>>`.
/// Produced by [`EngineBuilder::full_sync`] chains. The resulting engine handle is
/// `Send + Sync` and may be wrapped in `Arc<FullSyncEngine<...>>`.
pub type FullSyncEngine<Order, ExecutionReport = (), AccountAdjustment = ()> =
    Engine<EngineTraitOf<Order, ExecutionReport, AccountAdjustment, FullSync>>;

impl Engine {
    /// Creates an engine builder.
    pub fn builder<Order, ExecutionReport, AccountAdjustment>(
    ) -> EngineBuilder<Order, ExecutionReport, AccountAdjustment>
    where
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
    {
        EngineBuilder::new()
    }
}

/// # Threading
///
/// See [`Engine`]'s `# Threading` section for the threading contract.
impl<Trait: EngineTrait> Engine<Trait> {
    /// Executes start-stage checks and creates a deferred [`PreTradeRequest`].
    ///
    /// Start-stage policies run in registration order. The engine collects
    /// reject lists returned by all start-stage policies before deciding
    /// whether to create a deferred request.
    ///
    /// The engine does not enforce optional order extensions (for example
    /// `instrument` or `side`). Policies that depend on extension fields must
    /// validate their presence.
    ///
    /// # Account blocks
    ///
    /// The blocked set is consulted first: an order whose account is blocked
    /// individually, belongs to a blocked group, or arrives while a global block
    /// is active is rejected before any policy runs. While anything is blocked,
    /// an order whose account ID cannot be read is rejected too, because the
    /// engine cannot clear it against the blocked set.
    ///
    /// A start-stage reject with [`RejectScope::Account`] latches a block for
    /// the order's account. The block persists until it is lifted, so later
    /// requests for the account are rejected up front. The block is recorded
    /// only for a readable account: an unreadable one records nothing, since a
    /// rejected request created no exposure that would justify the global
    /// block. The order is rejected either way.
    ///
    /// # Errors
    ///
    /// Returns [`Rejects`] when the account is blocked or when any start-stage
    /// policy rejects the order.
    pub fn start_pre_trade(
        &self,
        order: Trait::Order,
    ) -> Result<PreTradeRequest<Trait::Order>, Rejects>
    where
        Trait::Order: HasAccountId,
    {
        if let Some(rejects) = self.inner.blocked_accounts.check(
            &self.inner.account_groups,
            &order,
            RejectScope::Order,
        ) {
            return Err(rejects);
        }

        let now: Instant = Instant::now();
        let account = order.account_id().ok();
        let account_control = account.map(|id| {
            let handle = AccountBlockHandle::from_inner(self.inner.blocked_accounts.clone());
            AccountControl::new(handle, id)
        });
        let account_groups = AccountGroupsHandle::from_inner(self.inner.account_groups.clone());
        let ctx = PreTradeContext::with_accounts(
            account_control,
            self.accounts(),
            account_groups,
            account,
        );
        let (start_rejects, account_block) = with_start_pre_trade_now(now, || {
            run_pre_trade_start_stage::<Trait, _>(
                &self.inner,
                &ctx,
                &order,
                |policy, ctx, order| policy.check_pre_trade_start(ctx, order),
            )
        });

        debug_assert!(account_block.is_none() || start_rejects.is_some());
        if let Some(rejects) = start_rejects {
            if let Some(block) = account_block {
                self.inner.blocked_accounts.record_pre_trade(&order, block);
            }
            return Err(rejects);
        }

        let engine = <Trait::Sync as SyncMode>::downgrade(&self.inner);
        let request_handle = RequestHandleImpl::<Trait::Order>::new(Box::new(move || {
            execute_pre_trade_request::<Trait>(engine, ctx, order)
        }));

        Ok(PreTradeRequest::from_handle(Box::new(request_handle)))
    }

    /// Runs start-stage checks and executes main-stage checks immediately.
    ///
    /// This is a convenience shortcut equivalent to
    /// `engine.start_pre_trade(order)?.execute()`, including its account-block
    /// behavior: see [`Self::start_pre_trade`] and
    /// [`PreTradeRequest::execute`].
    ///
    /// # Errors
    ///
    /// Returns [`Rejects`] for both stages.
    pub fn execute_pre_trade(&self, order: Trait::Order) -> Result<PreTradeReservation, Rejects>
    where
        Trait::Order: HasAccountId,
    {
        self.start_pre_trade(order)
            .and_then(PreTradeRequest::execute)
    }

    /// Applies a drop-copy operation without enforcing policy rejects.
    ///
    /// Policies run in registration order and keep their normal mutations,
    /// locks, account adjustments, and account blocks. Ordinary policy rejects
    /// do not stop the pipeline or appear in the operation. Rejects that mean a
    /// policy could not evaluate or apply the historical order fail the whole
    /// operation. On such a failure, collected mutations are rolled back and
    /// deferred start-stage effects and account blocks are discarded. Rate-limit
    /// attempts are the deliberate exception: they consume budget exactly like
    /// an ordinary pre-trade request, including when a later policy fails.
    /// Existing account and account-group blocks are ignored for admission.
    ///
    /// The returned [`DropCopyOperation`] is finalized by its owner exactly
    /// like the [`PreTradeReservation`] returned by [`Self::execute_pre_trade`]:
    /// it retains the prepared mutations, [`DropCopyOperation::commit`] applies
    /// them, and an explicit or implicit rollback compensates them. Deferred
    /// account-control operations and consumed rate-limit attempts are applied
    /// before this method returns and stay outside that boundary.
    ///
    /// A mutation finalizer that fails is never ignored, whether it fails while
    /// this method compensates a fatal exit or later, when the owner finalizes
    /// the returned operation. Both raise the engine kill switch described by
    /// the finalizer contract on [`Mutation`](crate::Mutation): the account for
    /// an engine-owned mutation, every account for a custom-policy one.
    ///
    /// Order fields are requested by policies when needed; the engine does not
    /// prevalidate a common field set. The account ID is the exception because
    /// it is the engine's routing and account-control key: when it cannot be
    /// read, the operation fails immediately with
    /// [`RejectCode::MissingRequiredField`], before any policy runs and before
    /// any other order field is touched. [`FullSync`](crate::FullSync) permits
    /// concurrent calls for the same account, but their individual storage
    /// accesses may interleave. Callers that need whole-pipeline isolation must
    /// serialize those calls externally.
    ///
    /// # Errors
    ///
    /// Returns [`RejectCode::MissingRequiredField`] when the account ID is
    /// unreadable, or the fatal evaluation rejects produced by the policy
    /// pipeline.
    ///
    /// When compensating a fatal pipeline exit fails, an engine
    /// [`RejectCode::SystemUnavailable`] reject is **appended** after the fatal
    /// policy rejects, and the kill switch is armed. The first reject stays the
    /// policy cause, so a caller reading `rejects[0]` always sees why the
    /// operation failed rather than how the cleanup failed.
    pub fn apply_drop_copy(&self, order: Trait::Order) -> Result<DropCopyOperation, Rejects>
    where
        Trait::Order: HasAccountId,
    {
        let now = Instant::now();
        let account = order
            .account_id()
            .map_err(|_| new_drop_copy_unreadable_account_rejects())?;
        let account_control = AccountControl::new(
            AccountBlockHandle::from_inner(self.inner.blocked_accounts.clone()),
            account,
        );
        let account_groups = AccountGroupsHandle::from_inner(self.inner.account_groups.clone());
        let ctx = PreTradeContext::with_accounts_and_drop_copy(
            account_control,
            self.accounts(),
            account_groups,
            account,
        );

        let (start_rejects, _) = with_start_pre_trade_now(now, || {
            run_pre_trade_start_stage::<Trait, _>(
                &self.inner,
                &ctx,
                &order,
                |policy, ctx, order| policy.check_pre_trade_start(ctx, order),
            )
        });
        if let Some(rejects) = fatal_drop_copy_rejects(start_rejects) {
            ctx.abandon_drop_copy_account_operations();
            let rollback = ctx.take_drop_copy_start_mutations().rollback_all();
            if rollback.callback_failed() {
                let details = rollback.callback_failure_details();
                self.record_mutation_failure(Some(account), rollback.failure());
                return Err(append_drop_copy_mutation_callback_rejects(rejects, details));
            }
            return Err(rejects);
        }

        let (rejects, _, mutations, lock, outcomes) = run_pre_trade_main_stage::<Trait, _>(
            &self.inner,
            &ctx,
            &order,
            |policy, ctx, order, mutations| policy.perform_pre_trade_check(ctx, order, mutations),
        );
        if let Some(rejects) = fatal_drop_copy_rejects(rejects) {
            ctx.abandon_drop_copy_account_operations();
            let mut rollback = mutations.rollback_all();
            rollback.append(ctx.take_drop_copy_start_mutations().rollback_all());
            if rollback.callback_failed() {
                let details = rollback.callback_failure_details();
                self.record_mutation_failure(Some(account), rollback.failure());
                return Err(append_drop_copy_mutation_callback_rejects(rejects, details));
            }
            return Err(rejects);
        }

        let mut all_mutations = ctx.take_drop_copy_start_mutations();
        all_mutations.append(mutations);
        let account_block = ctx.drop_copy_account_block();
        ctx.apply_drop_copy_account_operations(&self.inner.blocked_accounts);
        let account_blocked = self
            .inner
            .blocked_accounts
            .is_blocked(&self.inner.account_groups, account);
        Ok(DropCopyOperation::from_handle(
            Box::new(ReservationHandleImpl::new(
                all_mutations,
                self.mutation_failure_kill_switch(Some(account)),
            )),
            lock,
            outcomes,
            account_block,
            account_blocked,
        ))
    }

    /// Builds the kill switch handed to an owner-finalized mutation batch.
    ///
    /// The engine is the only place that can name its storage factory, so the
    /// blocked-set access is captured here and type-erased for the
    /// finalization path. See the finalizer contract on
    /// [`Mutation`](crate::Mutation).
    fn mutation_failure_kill_switch(
        &self,
        account: Option<AccountId>,
    ) -> MutationFailureKillSwitch {
        let handle = self.block_handle();
        MutationFailureKillSwitch::new(move |scope| handle.record_mutation_failure(account, scope))
    }

    /// Arms the kill switch for a batch the engine finalized itself.
    fn record_mutation_failure(&self, account: Option<AccountId>, failure: &MutationFailure) {
        if let Some(scope) = failure.scope() {
            self.block_handle().record_mutation_failure(account, scope);
        }
    }

    fn block_handle(&self) -> AccountBlockHandleOf<Trait> {
        AccountBlockHandle::from_inner(self.inner.blocked_accounts.clone())
    }

    /// Runs start-stage checks as a non-mutating dry-run.
    ///
    /// Returns a [`PreTradeDryRunReport`] describing the verdict the start stage
    /// *would* return for `order`, with zero effect on engine state: no policy
    /// side effect is applied (a rate-limit budget is not spent) and no account
    /// block is recorded, even when an account-scope reject *would* latch one.
    /// A repeated dry-run never moves engine state.
    ///
    /// The reported lock and account adjustments are always empty: those are
    /// produced by the main stage, which this method does not run. See
    /// [`Self::execute_pre_trade_dry_run`] for a full-pipeline dry-run.
    ///
    /// The initial blocked-account read still applies, so an already-blocked
    /// account is reported as rejected.
    pub fn start_pre_trade_dry_run(&self, order: Trait::Order) -> PreTradeDryRunReport
    where
        Trait::Order: HasAccountId,
    {
        if let Some(rejects) = self.inner.blocked_accounts.check(
            &self.inner.account_groups,
            &order,
            RejectScope::Order,
        ) {
            return PreTradeDryRunReport::new(Some(rejects), PreTradeLock::new(), Vec::new(), None);
        }

        let now: Instant = Instant::now();
        let account = order.account_id().ok();
        let account_control = account.map(|id| {
            let handle = AccountBlockHandle::from_inner(self.inner.blocked_accounts.clone());
            AccountControl::new(handle, id)
        });
        let account_groups = AccountGroupsHandle::from_inner(self.inner.account_groups.clone());
        let ctx = PreTradeContext::with_accounts(
            account_control,
            self.accounts(),
            account_groups,
            account,
        );
        let (rejects, account_block) = with_start_pre_trade_now(now, || {
            run_pre_trade_start_stage::<Trait, _>(
                &self.inner,
                &ctx,
                &order,
                |policy, ctx, order| policy.check_pre_trade_start_dry_run(ctx, order),
            )
        });

        PreTradeDryRunReport::new(rejects, PreTradeLock::new(), Vec::new(), account_block)
    }

    /// Runs the full pre-trade pipeline as a non-mutating dry-run.
    ///
    /// Returns a [`PreTradeDryRunReport`] describing the verdict, the lock, and
    /// the account adjustments both stages *would* produce for `order`, with
    /// zero effect on engine state: no rate-limit budget is spent, no
    /// reservation or hold is applied, and no account block is recorded. Any
    /// mutations a main-stage policy registers on the dry-run path are dropped -
    /// neither committed nor rolled back - so a repeated dry-run never moves
    /// engine state.
    ///
    /// The start stage runs first; if it would reject, the report carries those
    /// rejects (and the would-be account block) and the main stage is skipped,
    /// exactly as the real pipeline short-circuits. Otherwise the main stage
    /// runs and the report carries its pass/reject verdict together with the
    /// would-be lock and account adjustments, whose numbers match what a real
    /// reservation reports for the same order and engine state.
    pub fn execute_pre_trade_dry_run(&self, order: Trait::Order) -> PreTradeDryRunReport
    where
        Trait::Order: HasAccountId,
    {
        if let Some(rejects) = self.inner.blocked_accounts.check(
            &self.inner.account_groups,
            &order,
            RejectScope::Order,
        ) {
            return PreTradeDryRunReport::new(Some(rejects), PreTradeLock::new(), Vec::new(), None);
        }

        let now: Instant = Instant::now();
        let account = order.account_id().ok();
        let account_control = account.map(|id| {
            let handle = AccountBlockHandle::from_inner(self.inner.blocked_accounts.clone());
            AccountControl::new(handle, id)
        });
        let account_groups = AccountGroupsHandle::from_inner(self.inner.account_groups.clone());
        let ctx = PreTradeContext::with_accounts(
            account_control,
            self.accounts(),
            account_groups,
            account,
        );

        let (start_rejects, start_account_block) = with_start_pre_trade_now(now, || {
            run_pre_trade_start_stage::<Trait, _>(
                &self.inner,
                &ctx,
                &order,
                |policy, ctx, order| policy.check_pre_trade_start_dry_run(ctx, order),
            )
        });
        if let Some(rejects) = start_rejects {
            return PreTradeDryRunReport::new(
                Some(rejects),
                PreTradeLock::new(),
                Vec::new(),
                start_account_block,
            );
        }

        // The collected mutations are intentionally discarded: a dry-run never
        // commits or rolls back. The throwaway `Mutations` only satisfies the
        // hook signature for any read-only policy that still appends inert
        // commit/rollback pairs through the default delegation.
        let (rejects, account_block, _mutations, lock, outcomes) =
            run_pre_trade_main_stage::<Trait, _>(
                &self.inner,
                &ctx,
                &order,
                |policy, ctx, order, mutations| {
                    policy.perform_pre_trade_check_dry_run(ctx, order, mutations)
                },
            );

        PreTradeDryRunReport::new(rejects, lock, outcomes, account_block)
    }

    /// Applies post-trade updates across all policies and returns aggregated result.
    ///
    /// Each policy applies its state changes immediately and directly to storage.
    /// Processing is **not atomic**: if one policy sets an account block, the
    /// state changes already applied by earlier policies are not rolled back.
    ///
    /// Reject scope is not consulted on this path; this method does not derive
    /// an account block from a reject.
    ///
    /// A non-empty [`PostTradeResult::account_blocks`] means at least one policy entered a
    /// blocked state after the report was applied. This does **not** imply that
    /// [`PostTradeResult::account_adjustments`] were undone - they reflect storage
    /// that has already been mutated and must be propagated by the caller.
    ///
    /// [`PostTradeResult::account_adjustments`] contains zero or more account position
    /// modifications in policy registration order. A single asset may appear more than once;
    /// the exact content depends on which policies the engine was configured with and how
    /// those policies choose to report.
    pub fn apply_execution_report(&self, report: &Trait::ExecutionReport) -> PostTradeResult
    where
        Trait::ExecutionReport: HasAccountId,
    {
        let inner: &EngineInner<Trait> = &self.inner;
        let mut blocks: Vec<AccountBlock> = Vec::new();
        let mut account_pnls = Vec::new();
        let mut account_adjustments = Vec::new();

        let account_groups = AccountGroupsHandle::from_inner(inner.account_groups.clone());
        let accounts = self.accounts();
        let ctx =
            PostTradeContext::with_accounts(accounts, account_groups, report.account_id().ok());

        for policy in &inner.pre_trade_policies {
            let Some(result) = policy.apply_execution_report(&ctx, report) else {
                continue;
            };
            blocks.extend(result.account_blocks);
            account_pnls.extend(result.account_pnls);
            account_adjustments.extend(result.account_adjustments);
        }

        if let Some(first) = blocks.first() {
            inner
                .blocked_accounts
                .record_execution_report(report, first.clone());
        }

        PostTradeResult {
            account_blocks: blocks,
            account_pnls,
            account_adjustments,
        }
    }

    /// Applies an account-adjustment batch as a sequence with compensation
    /// rollback on failure: each adjustment is applied through policy storage
    /// immediately, so concurrent readers may observe partial batch state
    /// between adjustments. On rejection, applied mutations are rolled back
    /// through inverse deltas (best-effort).
    ///
    /// Policies are evaluated in registration order for each adjustment, and
    /// adjustments are traversed in slice order.
    ///
    /// Reject scope is not consulted on this path. `AccountControl::block`
    /// writes through immediately, including from a policy's commit or rollback
    /// closure. A rejected batch discards
    /// `PolicyAccountAdjustmentResult::account_blocks`, but its rollback can
    /// still block through that control or a failed mutation finalizer.
    ///
    /// On success returns [`crate::AccountAdjustmentBatchResult`]. Its
    /// `outcomes` field is a flat list of
    /// [`crate::AccountAdjustmentOutcome`] in policy registration order. Each
    /// entry carries the [`crate::PolicyGroupId`] of the policy that produced
    /// it. A single asset may appear more than once. Policies that report
    /// nothing contribute no entries.
    ///
    /// The engine records each block from accepted
    /// `PolicyAccountAdjustmentResult::account_blocks` for `account_id` only
    /// after every batch mutation commits, then returns the same blocks in
    /// `account_blocks`. The first reported block remains the stored cause if
    /// more than one block is reported for the account.
    ///
    /// A commit or rollback callback that fails is never ignored: it arms the
    /// engine kill switch described by the finalizer contract on
    /// [`Mutation`](crate::Mutation) before the policy-reported blocks are
    /// recorded, so the more severe cause is the one that wins the account.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAdjustmentBatchError`] for the first rejected element.
    /// The `index` field points to the failing adjustment in `adjustments`.
    pub fn apply_account_adjustment(
        &self,
        account_id: AccountId,
        adjustments: &[Trait::AccountAdjustment],
    ) -> Result<AccountAdjustmentBatchResult, AccountAdjustmentBatchError> {
        if adjustments.is_empty() {
            return Ok(AccountAdjustmentBatchResult::default());
        }

        let inner: &EngineInner<Trait> = &self.inner;
        let mut mutations = Mutations::with_capacity(adjustments.len());
        let mut batch_error: Option<AccountAdjustmentBatchError> = None;
        let mut outcomes: Vec<AccountAdjustmentOutcome> = Vec::new();
        let mut account_blocks: Vec<AccountBlock> = Vec::new();
        let handle = AccountBlockHandle::from_inner(inner.blocked_accounts.clone());
        let account_control = AccountControl::new(handle, account_id);
        let account_groups = AccountGroupsHandle::from_inner(inner.account_groups.clone());
        let ctx = AccountAdjustmentContext::with_accounts(
            account_control,
            self.accounts(),
            account_groups,
            account_id,
        );

        'outer: for (index, adjustment) in adjustments.iter().enumerate() {
            for policy in &inner.pre_trade_policies {
                match policy.apply_account_adjustment(&ctx, account_id, adjustment, &mut mutations)
                {
                    Ok(result) => {
                        let PolicyAccountAdjustmentResult {
                            account_adjustments,
                            account_blocks: reported_blocks,
                        } = result;
                        let policy_group_id = policy.policy_group_id();
                        outcomes.extend(account_adjustments.into_iter().map(|e| {
                            AccountAdjustmentOutcome {
                                policy_group_id,
                                entry: e,
                            }
                        }));
                        account_blocks.extend(reported_blocks);
                    }
                    Err(rejects) => {
                        debug_assert!(
                            !rejects.is_empty(),
                            "policy returned Err with empty Rejects"
                        );
                        batch_error = Some(AccountAdjustmentBatchError {
                            failed_adjustment_index: index,
                            rejects,
                        });
                        break 'outer;
                    }
                }
            }
        }

        if let Some(err) = batch_error {
            let rollback = mutations.rollback_all();
            self.record_mutation_failure(Some(account_id), rollback.failure());
            return Err(err);
        }

        // A failed commit finalizer is raised before the policy-reported
        // blocks, so the more severe cause is the one that wins the account.
        self.record_mutation_failure(Some(account_id), &mutations.commit_all());
        for block in &account_blocks {
            inner
                .blocked_accounts
                .block_account(account_id, block.clone());
        }
        Ok(AccountAdjustmentBatchResult {
            outcomes,
            account_blocks,
        })
    }
}

/// Concrete pre-trade policy-object shape registered in an engine of `Trait`.
type PreTradePolicyObjectOf<Trait> =
    <<Trait as EngineTrait>::Sync as SyncMode>::PreTradePolicyObject<
        <Trait as EngineTrait>::Order,
        <Trait as EngineTrait>::ExecutionReport,
        <Trait as EngineTrait>::AccountAdjustment,
    >;

/// Pre-trade context type for an engine of `Trait`.
type PreTradeContextOf<Trait> =
    PreTradeContext<<<Trait as EngineTrait>::Sync as SyncMode>::StorageLockingPolicyFactory>;

/// Blocked-accounts handle type for an engine of `Trait`.
type AccountBlockHandleOf<Trait> =
    AccountBlockHandle<<<Trait as EngineTrait>::Sync as SyncMode>::StorageLockingPolicyFactory>;

/// Runs the start stage over every policy and reports the merged verdict
/// without recording an account block in the engine registry.
///
/// `hook` selects the per-policy entry point (the normal
/// [`check_pre_trade_start`](PreTradePolicy::check_pre_trade_start) or its
/// dry-run variant); the loop, reject merge, and first-account-block selection
/// are identical for both. The caller decides whether to record the returned
/// [`AccountBlock`] - this function never touches the blocked-accounts registry.
fn run_pre_trade_start_stage<Trait, Hook>(
    inner: &EngineInner<Trait>,
    ctx: &PreTradeContextOf<Trait>,
    order: &Trait::Order,
    hook: Hook,
) -> (Option<Rejects>, Option<AccountBlock>)
where
    Trait: EngineTrait,
    Hook: Fn(
        &PreTradePolicyObjectOf<Trait>,
        &PreTradeContextOf<Trait>,
        &Trait::Order,
    ) -> Result<(), Rejects>,
{
    let mut rejects_collection = Vec::new();
    let mut total_rejects_len = 0;
    let mut account_block: Option<AccountBlock> = None;
    for policy in &inner.pre_trade_policies {
        let result = hook(&**policy, ctx, order);
        if let Err(rejects) = result {
            debug_assert!(
                !rejects.is_empty(),
                "policy returned Err with empty Rejects"
            );
            total_rejects_len += rejects.len();
            let reject_account_block = rejects
                .iter()
                .find(|r| r.scope == RejectScope::Account)
                .map(|r| r.account_block_with_code(RejectCode::AccountBlocked));
            if ctx.is_drop_copy() {
                if let Some(block) = &reject_account_block {
                    ctx.record_drop_copy_account_block(block.clone());
                }
            }
            if account_block.is_none() {
                account_block = reject_account_block;
            }
            rejects_collection.push(rejects);
        }
    }
    debug_assert!(account_block.is_none() || total_rejects_len > 0);
    (
        merge_reject_lists(rejects_collection, total_rejects_len),
        account_block,
    )
}

/// Runs the main stage over every policy and reports the merged verdict, the
/// collected mutations, the assembled lock, and the account outcomes
/// **without** committing, rolling back, or recording anything.
///
/// `hook` selects the per-policy entry point (the normal
/// [`perform_pre_trade_check`](PreTradePolicy::perform_pre_trade_check) or its
/// dry-run variant); the loop, reject merge, first-account-block selection, lock
/// assembly, and outcome tagging are identical for both. The caller decides
/// what to do with the returned [`Mutations`] (commit, roll back, or drop) and
/// whether to record the returned reject-derived [`AccountBlock`]. Drop-copy
/// blocks reported directly through the context remain there until the caller
/// applies or abandons its deferred operations.
fn run_pre_trade_main_stage<Trait, Hook>(
    inner: &EngineInner<Trait>,
    ctx: &PreTradeContextOf<Trait>,
    order: &Trait::Order,
    hook: Hook,
) -> (
    Option<Rejects>,
    Option<AccountBlock>,
    Mutations,
    PreTradeLock,
    Vec<AccountAdjustmentOutcome>,
)
where
    Trait: EngineTrait,
    Hook: Fn(
        &PreTradePolicyObjectOf<Trait>,
        &PreTradeContextOf<Trait>,
        &Trait::Order,
        &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects>,
{
    let policy_count = inner.pre_trade_policies.len();
    let mut mutations = Mutations::with_capacity(policy_count);
    let mut rejects_collection = Vec::new();
    let mut total_rejects_len = 0;
    let mut outcomes: Vec<AccountAdjustmentOutcome> = Vec::new();
    let mut lock = PreTradeLock::new();
    let mut first_account_block: Option<AccountBlock> = None;
    for policy in &inner.pre_trade_policies {
        let result = hook(&**policy, ctx, order, &mut mutations);
        match result {
            Ok(None) => {}
            Ok(Some(outcome)) => {
                append_pre_trade_policy_result(
                    outcome,
                    policy.policy_group_id(),
                    &mut lock,
                    &mut outcomes,
                );
            }
            Err(rejects) => {
                let (rejects, recorded_result) = rejects.into_parts();
                if ctx.is_drop_copy()
                    && !rejects
                        .iter()
                        .any(|reject| reject.code.is_evaluation_failure())
                {
                    if let Some(outcome) = recorded_result {
                        append_pre_trade_policy_result(
                            outcome,
                            policy.policy_group_id(),
                            &mut lock,
                            &mut outcomes,
                        );
                    }
                }
                debug_assert!(
                    !rejects.is_empty(),
                    "policy returned Err with empty Rejects"
                );
                total_rejects_len += rejects.len();
                let reject_account_block = rejects
                    .iter()
                    .find(|r| r.scope == RejectScope::Account)
                    .map(|r| r.account_block_with_code(RejectCode::AccountBlocked));
                if ctx.is_drop_copy() {
                    if let Some(block) = &reject_account_block {
                        ctx.record_drop_copy_account_block(block.clone());
                    }
                }
                if first_account_block.is_none() {
                    first_account_block = reject_account_block;
                }
                rejects_collection.push(Rejects::new(rejects));
            }
        }
    }

    debug_assert!(first_account_block.is_none() || total_rejects_len > 0);
    (
        merge_reject_lists(rejects_collection, total_rejects_len),
        first_account_block,
        mutations,
        lock,
        outcomes,
    )
}

/// Rejects returned when a drop-copy order carries no readable account ID.
///
/// Drop-copy ignores existing blocks for admission, so this is not a failed
/// blocked-set lookup: the operation has no routing and account-control key at
/// all, and the caller must fix the order rather than look for a stuck block.
fn new_drop_copy_unreadable_account_rejects() -> Rejects {
    Reject::new(
        "Engine",
        RejectScope::Order,
        RejectCode::MissingRequiredField,
        "drop-copy requires a readable account ID",
        "the account ID routes the drop-copy pipeline and its account control".to_owned(),
    )
    .into()
}

fn fatal_drop_copy_rejects(rejects: Option<Rejects>) -> Option<Rejects> {
    let fatal = rejects?
        .into_vec()
        .into_iter()
        .filter(|reject| reject.code.is_evaluation_failure())
        .collect::<Vec<_>>();
    (!fatal.is_empty()).then(|| Rejects::new(fatal))
}

fn drop_copy_mutation_callback_rejects(details: String) -> Rejects {
    Reject::new(
        "Engine",
        RejectScope::Order,
        RejectCode::SystemUnavailable,
        "mutation callback failed",
        details,
    )
    .into()
}

fn append_drop_copy_mutation_callback_rejects(rejects: Rejects, details: String) -> Rejects {
    let mut rejects = rejects.into_vec();
    rejects.extend(drop_copy_mutation_callback_rejects(details).into_vec());
    Rejects::new(rejects)
}

fn append_pre_trade_policy_result(
    outcome: PolicyPreTradeResult,
    policy_group_id: crate::core::PolicyGroupId,
    lock: &mut PreTradeLock,
    outcomes: &mut Vec<AccountAdjustmentOutcome>,
) {
    let PolicyPreTradeResult {
        account_adjustments,
        lock_prices,
    } = outcome;
    lock.push_many(policy_group_id, lock_prices);
    outcomes.extend(
        account_adjustments
            .into_iter()
            .map(|entry| AccountAdjustmentOutcome {
                policy_group_id,
                entry,
            }),
    );
}

fn execute_pre_trade_request<Trait: EngineTrait>(
    engine: <Trait::Sync as SyncMode>::Weak<EngineInner<Trait>>,
    ctx: PreTradeContext<<Trait::Sync as SyncMode>::StorageLockingPolicyFactory>,
    order: Trait::Order,
) -> Result<PreTradeReservation, Rejects>
where
    Trait::Order: HasAccountId,
{
    let Some(engine_ref) = <Trait::Sync as SyncMode>::upgrade(&engine) else {
        return Err(Rejects::new(vec![Reject::new(
            "Engine",
            RejectScope::Order,
            RejectCode::SystemUnavailable,
            "engine is no longer available",
            "request handle outlived engine instance".to_owned(),
        )]));
    };
    let inner: &EngineInner<Trait> = &engine_ref;

    if let Some(rejects) =
        inner
            .blocked_accounts
            .check(&inner.account_groups, &order, RejectScope::Order)
    {
        return Err(rejects);
    }

    let (rejects, first_account_block, mutations, lock, outcomes) =
        run_pre_trade_main_stage::<Trait, _>(
            inner,
            &ctx,
            &order,
            |policy, ctx, order, mutations| policy.perform_pre_trade_check(ctx, order, mutations),
        );

    let account = order.account_id().ok();
    let block_handle: AccountBlockHandleOf<Trait> =
        AccountBlockHandle::from_inner(inner.blocked_accounts.clone());

    if let Some(rejects) = rejects {
        let rollback = mutations.rollback_all();
        if let Some(scope) = rollback.failure().scope() {
            block_handle.record_mutation_failure(account, scope);
        }
        if let Some(block) = first_account_block {
            // The first block for an account wins. A rollback failure therefore
            // keeps its account-scoped cause, while a global failure still
            // records the policy block for this account.
            inner.blocked_accounts.record_pre_trade(&order, block);
        }
        return Err(rejects);
    }

    let reservation_handle = ReservationHandleImpl::new(
        mutations,
        MutationFailureKillSwitch::new(move |scope| {
            block_handle.record_mutation_failure(account, scope)
        }),
    );
    Ok(PreTradeReservation::from_handle(
        Box::new(reservation_handle),
        lock,
        outcomes,
    ))
}

fn merge_reject_lists(lists: Vec<Rejects>, len: usize) -> Option<Rejects> {
    if len == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(len);
    for rejects in lists {
        out.extend(rejects.into_vec());
    }
    Some(Rejects::new(out))
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use crate::core::mutation::MutationRollbackResult;
    use crate::core::{
        ExecutionReportOperation, FinancialImpact, Instrument, OrderOperation,
        WithExecutionReportOperation, WithFinancialImpact, WithOrderOperation,
    };
    use crate::param::{
        AccountGroupId, AccountId, Asset, Fee, Pnl, PositionSize, Price, Quantity, Side,
        TradeAmount, Volume,
    };
    use crate::pretrade::policies::{
        OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitSettings,
        OrderValidationPolicy, RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier,
        RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
    };
    use crate::pretrade::{
        AccountBlock, PolicyAccountAdjustmentResult, PolicyPreTradeResult, PostTradeResult,
        PreTradeContext, PreTradeDryRunReport, PreTradePolicy, Reject, RejectCode, RejectScope,
        Rejects, DEFAULT_POLICY_GROUP_ID,
    };
    use crate::storage::NoLocking;
    use crate::{
        AccountAdjustmentContext, AccountOutcomeEntry, HasAccountId, HasOrderPrice, Mutation,
        Mutations, OutcomeAmount, RequestFieldAccessError,
    };

    use super::{AccountAdjustmentBatchError, Engine, FullSyncEngine, LocalEngine};
    use crate::EngineBuildError;

    type TestOrder = WithOrderOperation<()>;
    type TestReport = WithFinancialImpact<WithExecutionReportOperation<()>>;
    type TestAdjustment = MockAdjustment;
    type MutationHook = Rc<dyn Fn(&mut Mutations)>;
    type AdjustmentHook = Box<dyn Fn(&mut Mutations)>;

    /// Minimal order stub for tests that don't require order fields.
    /// Returns `Err` for `account_id()` - only global-block check applies.
    #[derive(Clone)]
    struct NoAccountOrder;

    impl HasAccountId for NoAccountOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("account_id"))
        }
    }

    struct SingleReadAccountOrder {
        calls: Rc<Cell<u32>>,
    }

    impl HasAccountId for SingleReadAccountOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            let calls = self.calls.get();
            self.calls.set(calls + 1);
            if calls == 0 {
                Ok(AccountId::from_u64(99224416))
            } else {
                Err(RequestFieldAccessError::new("account_id"))
            }
        }
    }

    impl HasOrderPrice for NoAccountOrder {
        fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
            Ok(Some(Price::from_str("1").expect("price must be valid")))
        }
    }

    /// Minimal order stub whose price cannot be read.
    #[derive(Clone)]
    struct PriceAccessErrorOrder;

    impl HasAccountId for PriceAccessErrorOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Ok(AccountId::from_u64(99224416))
        }
    }

    impl HasOrderPrice for PriceAccessErrorOrder {
        fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("price"))
        }
    }

    /// Minimal execution-report stub for tests that don't require report fields.
    #[derive(Clone)]
    struct NoAccountReport;

    impl HasAccountId for NoAccountReport {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("account_id"))
        }
    }

    struct NoopPolicy {
        name: &'static str,
        calls: Option<Rc<Cell<usize>>>,
        group_id: crate::core::PolicyGroupId,
    }

    impl NoopPolicy {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                calls: None,
                group_id: crate::core::DEFAULT_POLICY_GROUP_ID,
            }
        }

        fn with_policy_group_id(mut self, group_id: crate::core::PolicyGroupId) -> Self {
            self.group_id = group_id;
            self
        }

        fn with_calls(mut self, calls: Rc<Cell<usize>>) -> Self {
            self.calls = Some(calls);
            self
        }
    }

    impl<Order, ExecutionReport, AccountAdjustment, Sync: crate::core::SyncMode>
        PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync> for NoopPolicy
    {
        fn name(&self) -> &str {
            self.name
        }

        fn policy_group_id(&self) -> crate::core::PolicyGroupId {
            self.group_id
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &Order,
        ) -> Result<(), Rejects> {
            if let Some(calls) = &self.calls {
                calls.set(calls.get() + 1);
            }
            Ok(())
        }
    }

    struct DropCopyRejectedResultPolicy;

    fn drop_copy_policy_result(delta: &str, absolute: &str, lock: &str) -> PolicyPreTradeResult {
        let mut result = PolicyPreTradeResult::with_capacity(1, 1);
        result.account_adjustments.push(AccountOutcomeEntry {
            asset: Asset::new("USD").expect("asset must be valid"),
            balance: Some(OutcomeAmount {
                delta: PositionSize::from_str(delta).expect("delta must be valid"),
                absolute: PositionSize::from_str(absolute).expect("amount must be valid"),
            }),
            held: None,
            incoming: None,
            realized_pnl: None,
            average_entry_price: None,
        });
        result
            .lock_prices
            .push(Price::from_str(lock).expect("price must be valid"));
        result
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyRejectedResultPolicy
    {
        fn name(&self) -> &str {
            "drop_copy_rejected_result"
        }

        fn policy_group_id(&self) -> crate::core::PolicyGroupId {
            crate::core::PolicyGroupId::new(7)
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            Err(Rejects::from(Reject::new(
                "drop_copy_rejected_result",
                RejectScope::Account,
                RejectCode::Other,
                "rejected with output",
                "drop-copy must preserve output attached to an ignored reject",
            ))
            .with_policy_result(drop_copy_policy_result("-5", "95", "13")))
        }
    }

    struct DropCopySuccessfulRecordedResultPolicy;

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopySuccessfulRecordedResultPolicy
    {
        fn name(&self) -> &str {
            "successful_recorded_result"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            Ok(Some(drop_copy_policy_result("-2", "98", "12")))
        }
    }

    struct DropCopyRejectPolicy {
        name: &'static str,
        code: RejectCode,
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyRejectPolicy
    {
        fn name(&self) -> &str {
            self.name
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
        ) -> Result<(), Rejects> {
            Ok(())
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            Err(Rejects::from(Reject::new(
                self.name,
                RejectScope::Order,
                self.code,
                "drop-copy test reject",
                "drop-copy test policy rejected the order",
            )))
        }
    }

    struct DropCopyFatalStartPolicy;

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyFatalStartPolicy
    {
        fn name(&self) -> &str {
            "fatal_start"
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
        ) -> Result<(), Rejects> {
            Err(Rejects::from(Reject::new(
                "fatal_start",
                RejectScope::Order,
                RejectCode::MissingRequiredField,
                "missing start field",
                "start field is unavailable",
            )))
        }
    }

    struct DropCopyDirectBlockPolicy;

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyDirectBlockPolicy
    {
        fn name(&self) -> &str {
            "direct_block"
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            ctx.account_control
                .as_ref()
                .expect("test order has an account")
                .block(AccountBlock::new(
                    "direct_block",
                    RejectCode::AccountBlocked,
                    "direct block",
                    "direct block requested before a later fatal evaluation",
                ));
            Ok(None)
        }
    }

    struct DropCopyDirectBlockAndRejectPolicy;

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyDirectBlockAndRejectPolicy
    {
        fn name(&self) -> &str {
            "direct_block_and_reject"
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            ctx.account_control
                .as_ref()
                .expect("test order has an account")
                .block(AccountBlock::new(
                    "direct_block_and_reject",
                    RejectCode::AccountBlocked,
                    "first direct block",
                    "direct block requested before returning a later reject",
                ));
            Err(Rejects::from(Reject::new(
                "direct_block_and_reject",
                RejectScope::Account,
                RejectCode::RiskLimitExceeded,
                "later account reject",
                "ordinary reject emitted after the direct block",
            )))
        }
    }

    #[derive(Clone)]
    enum TestDeferredAccountOperation {
        Block {
            reason: &'static str,
            provenance: u64,
        },
        InvalidateProvenance(u64),
    }

    struct DropCopyAccountOperationsPolicy {
        operations: Rc<RefCell<Vec<TestDeferredAccountOperation>>>,
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyAccountOperationsPolicy
    {
        fn name(&self) -> &str {
            "drop_copy_account_operations"
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            let control = ctx
                .account_control
                .as_ref()
                .expect("test order has an account");
            for operation in self.operations.borrow().iter().cloned() {
                match operation {
                    TestDeferredAccountOperation::Block { reason, provenance } => {
                        control.block(
                            AccountBlock::new(
                                "drop_copy_account_operations",
                                RejectCode::AccountBlocked,
                                reason,
                                "ordered drop-copy account operation",
                            )
                            .with_provenance(Some(provenance)),
                        );
                    }
                    TestDeferredAccountOperation::InvalidateProvenance(provenance) => {
                        let _ = control.invalidate_provenance(provenance);
                    }
                }
            }
            Ok(None)
        }
    }

    struct ConditionalDropCopyFatalPolicy {
        fatal: Rc<Cell<bool>>,
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for ConditionalDropCopyFatalPolicy
    {
        fn name(&self) -> &str {
            "conditional_drop_copy_fatal"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            if self.fatal.get() {
                return Err(Rejects::from(Reject::new(
                    "conditional_drop_copy_fatal",
                    RejectScope::Order,
                    RejectCode::SystemUnavailable,
                    "fatal evaluation",
                    "failure after a deferred account operation",
                )));
            }
            Ok(None)
        }
    }

    struct DropCopyEagerStartMutationPolicy {
        value: Rc<Cell<usize>>,
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyEagerStartMutationPolicy
    {
        fn name(&self) -> &str {
            "drop_copy_eager_start_mutation"
        }

        fn check_pre_trade_start(
            &self,
            ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
        ) -> Result<(), Rejects> {
            let previous = self.value.get();
            self.value.set(previous + 1);
            let rollback_value = Rc::clone(&self.value);
            assert!(ctx
                .record_drop_copy_start_mutation(Mutation::new(
                    || {},
                    move || rollback_value.set(previous),
                ))
                .is_ok());
            Ok(())
        }
    }

    struct DropCopyFailingStartRollbackPolicy;

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyFailingStartRollbackPolicy
    {
        fn name(&self) -> &str {
            "failing_start_rollback"
        }

        fn check_pre_trade_start(
            &self,
            ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
        ) -> Result<(), Rejects> {
            assert!(ctx
                .record_drop_copy_start_mutation(Mutation::new_fallible_with_error(
                    || Ok(()),
                    || Err("start rollback failed".to_owned()),
                ))
                .is_ok());
            Ok(())
        }
    }

    struct DropCopyFatalMainWithFailingRollbackPolicy;

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyFatalMainWithFailingRollbackPolicy
    {
        fn name(&self) -> &str {
            "failing_main_rollback"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            mutations.push(Mutation::new_fallible_with_error(
                || Ok(()),
                || Err("main rollback failed".to_owned()),
            ));
            Err(Rejects::from(Reject::new(
                "failing_main_rollback",
                RejectScope::Order,
                RejectCode::MissingRequiredField,
                "required field is unavailable",
                "the main policy cannot evaluate the historical order",
            )))
        }
    }

    struct DropCopyRollbackBlockPolicy;

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for DropCopyRollbackBlockPolicy
    {
        fn name(&self) -> &str {
            "drop_copy_rollback_block"
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            let control = ctx
                .account_control
                .clone()
                .expect("drop-copy test context must carry account control");
            mutations.push(Mutation::new(
                || {},
                move || {
                    control.block(AccountBlock::new(
                        "drop_copy_rollback_block",
                        RejectCode::SystemUnavailable,
                        "rollback could not restore policy state",
                        "policy requested a safety block during compensation",
                    ));
                },
            ));
            Err(Reject::new(
                "drop_copy_rollback_block",
                RejectScope::Order,
                RejectCode::MissingRequiredField,
                "historical order cannot be evaluated",
                "forced fatal reject",
            )
            .into())
        }
    }

    #[test]
    fn build_rejects_duplicate_policy_names_across_stages() {
        let result = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("dup"))
            .pre_trade(MainPolicyMock::pass("dup"))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name }) if name == "dup"
        ));
    }

    #[test]
    fn build_rejects_duplicate_policy_names_within_start_stage() {
        let result = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("dup"))
            .pre_trade(StartPolicyMock::pass("dup"))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name }) if name == "dup"
        ));
    }

    #[test]
    fn build_rejects_duplicate_non_default_group_ids() {
        let group_id = crate::core::PolicyGroupId::new(7);
        let result = Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(NoopPolicy::new("a").with_policy_group_id(group_id))
            .pre_trade(NoopPolicy::new("b").with_policy_group_id(group_id))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyGroupId { policy_group_id: gid }) if gid == group_id
        ));
    }

    #[test]
    fn build_allows_multiple_policies_sharing_default_group_id() {
        let result = Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(NoopPolicy::new("a"))
            .pre_trade(NoopPolicy::new("b"))
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn apply_account_adjustment_passes_adjustment_to_policy_for_single_element() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::pass("adj", Rc::clone(&seen)))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 7, amount: 120 }];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(result.is_ok());
        assert_eq!(*seen.borrow(), vec![MockAdjustment { id: 7, amount: 120 }]);
    }

    #[test]
    fn apply_account_adjustment_returns_failing_index() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::reject_on_id(
                "adj_reject",
                Rc::clone(&seen),
                1,
            ))
            .build()
            .expect("engine must build");

        let batch = [
            MockAdjustment { id: 0, amount: 10 },
            MockAdjustment { id: 1, amount: 20 },
            MockAdjustment { id: 2, amount: 30 },
        ];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(matches!(
            result,
            Err(AccountAdjustmentBatchError {
                failed_adjustment_index: 1,
                rejects,
            }) if rejects[0].policy == "adj_reject"
        ));
        assert_eq!(
            *seen.borrow(),
            vec![
                MockAdjustment { id: 0, amount: 10 },
                MockAdjustment { id: 1, amount: 20 },
            ]
        );
    }

    #[test]
    fn apply_account_adjustment_does_not_apply_partial_batch_on_reject() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut applied = Vec::new();
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::reject_on_id(
                "adj_reject",
                Rc::clone(&seen),
                2,
            ))
            .build()
            .expect("engine must build");

        let batch = [
            MockAdjustment { id: 1, amount: 10 },
            MockAdjustment { id: 2, amount: 20 },
        ];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);
        if result.is_ok() {
            applied.extend(batch.iter().map(|adjustment| adjustment.id));
        }

        assert!(result.is_err());
        assert!(applied.is_empty());
        assert_eq!(
            *seen.borrow(),
            vec![
                MockAdjustment { id: 1, amount: 10 },
                MockAdjustment { id: 2, amount: 20 },
            ]
        );
    }

    #[test]
    fn apply_account_adjustment_accepts_multi_element_batch_when_all_policies_pass() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::pass("first", Rc::clone(&first_seen)))
            .pre_trade(AdjustmentPolicyMock::pass(
                "second",
                Rc::clone(&second_seen),
            ))
            .build()
            .expect("engine must build");

        let batch = [
            MockAdjustment { id: 10, amount: 1 },
            MockAdjustment { id: 11, amount: 2 },
            MockAdjustment { id: 12, amount: 3 },
        ];

        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_ok());
        assert_eq!(*first_seen.borrow(), batch);
        assert_eq!(*second_seen.borrow(), batch);
    }

    #[test]
    fn apply_account_adjustment_empty_batch_skips_policy_calls() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::pass("adj", Rc::clone(&seen)))
            .build()
            .expect("engine must build");

        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &[])
            .is_ok());
        assert!(seen.borrow().is_empty());
    }

    #[test]
    fn apply_account_adjustment_returns_reject_from_second_policy() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::pass("first", Rc::clone(&first_seen)))
            .pre_trade(AdjustmentPolicyMock::reject_on_id(
                "second",
                Rc::clone(&second_seen),
                9,
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 9, amount: 99 }];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(matches!(
            result,
            Err(AccountAdjustmentBatchError {
                failed_adjustment_index: 0,
                rejects,
            }) if rejects[0].policy == "second"
        ));
        assert_eq!(*first_seen.borrow(), batch);
        assert_eq!(*second_seen.borrow(), batch);
    }

    #[test]
    fn apply_account_adjustment_respects_policy_registration_order_for_rejects() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::reject_on_id(
                "first",
                Rc::clone(&first_seen),
                77,
            ))
            .pre_trade(AdjustmentPolicyMock::reject_on_id(
                "second",
                Rc::clone(&second_seen),
                77,
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 77, amount: 1 }];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(matches!(
            result,
            Err(AccountAdjustmentBatchError {
                failed_adjustment_index: 0,
                rejects,
            }) if rejects[0].policy == "first"
        ));
        assert_eq!(*first_seen.borrow(), batch);
        assert!(second_seen.borrow().is_empty());
    }

    #[test]
    fn build_rejects_duplicate_policy_names_between_start_and_account_adjustment() {
        let result = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("dup"))
            .pre_trade(AdjustmentPolicyMock::pass(
                "dup",
                Rc::new(RefCell::new(Vec::new())),
            ))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name }) if name == "dup"
        ));
    }

    #[test]
    fn apply_account_adjustment_commits_mutations_on_success() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::with_side_effect(
                "adj_mutation",
                Rc::clone(&seen),
                shared_kill_switch_mutation(Rc::clone(&state), "adj_mutation", true, false),
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 1, amount: 10 }];
        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_ok());
        assert_eq!(*state.borrow(), Some(true));
    }

    #[test]
    fn apply_account_adjustment_rolls_back_mutations_on_reject() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::with_side_effect(
                "adj_mutation",
                Rc::clone(&first_seen),
                shared_kill_switch_mutation(Rc::clone(&state), "adj_mutation", true, false),
            ))
            .pre_trade(AdjustmentPolicyMock::reject_on_id(
                "adj_rejecter",
                Rc::clone(&second_seen),
                1,
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 1, amount: 10 }];
        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_err());
        assert_eq!(*state.borrow(), Some(false));
    }

    #[test]
    fn apply_account_adjustment_without_mutations_leaves_state_unchanged() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(AdjustmentPolicyMock::pass("adj", Rc::clone(&seen)))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 1, amount: 10 }];
        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_ok());
    }

    #[test]
    fn engine_builder_with_default_report_type_remains_operational() {
        let engine: LocalEngine<TestOrder, NoAccountReport> = Engine::builder()
            .no_sync()
            .pre_trade(NoopPolicy::new("noop"))
            .build()
            .expect("engine must build with default report type");
        let request = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("start stage must pass");
        let mut reservation = request.execute().expect("execute must pass");
        reservation.rollback();

        let post_trade = engine.apply_execution_report(&NoAccountReport);
        assert!(post_trade.account_blocks.is_empty());
    }

    #[test]
    fn execute_pre_trade_shortcut_returns_reservation_when_both_stages_pass() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::pass("main"))
            .build()
            .expect("engine must build");

        let mut reservation = engine
            .execute_pre_trade(order_with_settlement("USD"))
            .expect("shortcut must pass");
        reservation.rollback();
    }

    #[test]
    fn execute_pre_trade_wraps_start_stage_reject_into_single_element_rejects() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::new(
                "start_reject",
                Rc::new(Cell::new(0)),
                true,
                false,
                None,
                None,
            ))
            .build()
            .expect("engine must build");

        let rejects = match engine.execute_pre_trade(order_with_settlement("USD")) {
            Ok(_) => panic!("start stage must reject"),
            Err(rejects) => rejects,
        };
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "start_reject");
        assert_eq!(rejects[0].code, RejectCode::Other);
        assert_eq!(rejects[0].reason, "start reject");
    }

    #[test]
    fn execute_pre_trade_returns_main_stage_rejects_in_original_order() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::with_mutation_and_optional_reject(
                "main_first",
                "m1",
                true,
                RejectScope::Order,
            ))
            .pre_trade(MainPolicyMock::with_mutation_and_optional_reject(
                "main_second",
                "m2",
                true,
                RejectScope::Account,
            ))
            .build()
            .expect("engine must build");

        let rejects = match engine.execute_pre_trade(order_with_settlement("USD")) {
            Ok(_) => panic!("main stage must reject"),
            Err(rejects) => rejects,
        };
        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].policy, "main_first");
        assert_eq!(rejects[1].policy, "main_second");
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(rejects[1].scope, RejectScope::Account);
    }

    #[test]
    fn execute_pre_trade_commit_applies_mutations_on_success() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "main",
                shared_kill_switch_mutation(Rc::clone(&state), "shortcut_commit", true, false),
                false,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let mut reservation = engine
            .execute_pre_trade(order_with_settlement("USD"))
            .expect("shortcut must pass");
        reservation.commit();

        assert_eq!(*state.borrow(), Some(true));
    }

    #[test]
    fn execute_pre_trade_reject_does_not_apply_commit_mutations() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "rejecting_main",
                shared_kill_switch_mutation(Rc::clone(&state), "shortcut_reject", true, false),
                true,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let result = engine.execute_pre_trade(order_with_settlement("USD"));
        assert!(result.is_err(), "shortcut must reject");
        assert_eq!(*state.borrow(), Some(false));
    }

    #[test]
    fn accepts_order_without_operation_fields_when_no_policy_requires_them() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(CoreStartPolicyMock {
                name: "core_start",
                reject: false,
                reject_scope: RejectScope::Order,
            })
            .pre_trade(CoreMainPolicyMock {
                name: "core_main",
                on_apply: None,
                reject: false,
                reject_scope: RejectScope::Order,
            })
            .build()
            .expect("engine must build");
        let order = NoAccountOrder;
        let mut reservation = engine
            .start_pre_trade(order)
            .expect("start stage must pass")
            .execute()
            .expect("main stage must pass");
        reservation.commit();

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(post_trade.account_blocks.is_empty());
    }

    #[test]
    fn order_trade_input_build_rejects_duplicate_policy_names_across_stages() {
        let result = Engine::builder()
            .no_sync()
            .pre_trade(CoreStartPolicyMock {
                name: "dup",
                reject: false,
                reject_scope: RejectScope::Order,
            })
            .pre_trade(CoreMainPolicyMock {
                name: "dup",
                on_apply: None,
                reject: false,
                reject_scope: RejectScope::Order,
            })
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name }) if name == "dup"
        ));
    }

    #[test]
    fn order_trade_input_build_rejects_duplicate_policy_names_within_start_stage() {
        let result = Engine::builder()
            .no_sync()
            .pre_trade(CoreStartPolicyMock {
                name: "dup",
                reject: false,
                reject_scope: RejectScope::Order,
            })
            .pre_trade(CoreStartPolicyMock {
                name: "dup",
                reject: false,
                reject_scope: RejectScope::Order,
            })
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name }) if name == "dup"
        ));
    }

    #[test]
    fn order_trade_input_start_pre_trade_rejects_before_request_is_created() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(CoreStartPolicyMock {
                name: "core_start_reject",
                reject: true,
                reject_scope: RejectScope::Order,
            })
            .build()
            .expect("engine must build");
        let order = NoAccountOrder;

        let result = engine.start_pre_trade(order);
        let Err(rejects) = result else {
            panic!("start stage must reject");
        };
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "core_start_reject");
        assert_eq!(rejects[0].code, RejectCode::Other);

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(post_trade.account_blocks.is_empty());
    }

    // An account-scope reject blocks only a known account. A recorded block is
    // latched until it is lifted. With an unreadable account ID, nothing is
    // recorded: a rejected request created no exposure that would justify the
    // global block.
    #[test]
    fn accountless_start_stage_account_reject_records_no_block() {
        let engine = Engine::builder::<NoAccountOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(CoreStartPolicyMock {
                name: "core_start_account_reject",
                reject: true,
                reject_scope: RejectScope::Account,
            })
            .build()
            .expect("engine must build");

        let Err(rejects) = engine.start_pre_trade(NoAccountOrder) else {
            panic!("start stage must reject");
        };
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].scope, RejectScope::Account);

        assert!(!engine.inner.blocked_accounts.is_all_blocked());
        assert!(engine
            .inner
            .blocked_accounts
            .check(
                &engine.inner.account_groups,
                &NoAccountOrder,
                RejectScope::Order,
            )
            .is_none());
    }

    #[test]
    fn accountless_main_stage_account_reject_records_no_block() {
        let engine = Engine::builder::<NoAccountOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(CoreMainPolicyMock {
                name: "core_main_account_reject",
                on_apply: None,
                reject: true,
                reject_scope: RejectScope::Account,
            })
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(NoAccountOrder)
            .expect("start stage must pass");
        let Err(rejects) = request.execute() else {
            panic!("main stage must reject");
        };
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].scope, RejectScope::Account);

        assert!(!engine.inner.blocked_accounts.is_all_blocked());
        assert!(engine
            .inner
            .blocked_accounts
            .check(
                &engine.inner.account_groups,
                &NoAccountOrder,
                RejectScope::Order,
            )
            .is_none());
    }

    #[test]
    fn order_core_execute_rejects_and_rolls_back_mutations() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(CoreStartPolicyMock {
                name: "core_start",
                reject: false,
                reject_scope: RejectScope::Order,
            })
            .pre_trade(CoreMainPolicyMock {
                name: "core_main",
                on_apply: Some(Rc::new(shared_kill_switch_mutation(
                    Rc::clone(&state),
                    "core_order_mutation",
                    true,
                    false,
                ))),
                reject: true,
                reject_scope: RejectScope::Order,
            })
            .build()
            .expect("engine must build");
        let order = NoAccountOrder;

        let request = engine
            .start_pre_trade(order)
            .expect("start stage must create request");
        let result = request.execute();
        assert!(result.is_err(), "main stage must reject");
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "core_main");
        assert_eq!(rejects[0].code, RejectCode::Other);
        assert_eq!(*state.borrow(), Some(false));

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(post_trade.account_blocks.is_empty());
    }

    #[test]
    fn order_core_execute_commit_and_rollback_apply_mutation_callback() {
        let cases = [
            (FinalizeAction::Commit, true),
            (FinalizeAction::Rollback, false),
        ];

        for (action, expected_state) in cases {
            let state = Rc::new(RefCell::new(None));
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(CoreStartPolicyMock {
                    name: "core_start",
                    reject: false,
                    reject_scope: RejectScope::Order,
                })
                .pre_trade(CoreMainPolicyMock {
                    name: "core_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "core_finalize_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                    reject_scope: RejectScope::Order,
                })
                .build()
                .expect("engine must build");
            let order = NoAccountOrder;

            let mut reservation = engine
                .start_pre_trade(order)
                .expect("start stage must create request")
                .execute()
                .expect("main stage must pass");

            match action {
                FinalizeAction::Commit => reservation.commit(),
                FinalizeAction::Rollback => reservation.rollback(),
            }

            assert_eq!(*state.borrow(), Some(expected_state));

            let post_trade = engine.apply_execution_report(&execution_report("USD"));
            assert!(post_trade.account_blocks.is_empty());
        }
    }

    #[test]
    fn start_pre_trade_table_cases_follow_registration_order_and_collect_rejects() {
        struct Case {
            reject_index: Option<usize>,
            expected_calls: [usize; 3],
            expected_main_calls: usize,
            expected_ok: bool,
        }

        let cases = [
            Case {
                reject_index: None,
                expected_calls: [1, 1, 1],
                expected_main_calls: 0,
                expected_ok: true,
            },
            Case {
                reject_index: Some(1),
                expected_calls: [1, 1, 1],
                expected_main_calls: 0,
                expected_ok: false,
            },
        ];

        for case in cases {
            let calls_0 = Rc::new(Cell::new(0));
            let calls_1 = Rc::new(Cell::new(0));
            let calls_2 = Rc::new(Cell::new(0));
            let main_calls = Rc::new(Cell::new(0));

            let start_0 = StartPolicyMock::new("s0", Rc::clone(&calls_0), false, false, None, None);
            let start_1 = StartPolicyMock::new(
                "s1",
                Rc::clone(&calls_1),
                case.reject_index == Some(1),
                false,
                None,
                None,
            );
            let start_2 = StartPolicyMock::new("s2", Rc::clone(&calls_2), false, false, None, None);

            let engine = Engine::builder()
                .no_sync()
                .pre_trade(start_0)
                .pre_trade(start_1)
                .pre_trade(start_2)
                .pre_trade(MainPolicyMock::with_calls(
                    "m0",
                    Rc::clone(&main_calls),
                    false,
                    false,
                    None,
                ))
                .build()
                .expect("engine must build");

            let result = engine.start_pre_trade(order_with_settlement("USD"));
            assert_eq!(result.is_ok(), case.expected_ok);
            assert_eq!(calls_0.get(), case.expected_calls[0]);
            assert_eq!(calls_1.get(), case.expected_calls[1]);
            assert_eq!(calls_2.get(), case.expected_calls[2]);
            assert_eq!(main_calls.get(), case.expected_main_calls);
        }
    }

    #[test]
    fn execute_table_cases_cover_success_commit_and_reject_rollback() {
        struct Case {
            fail_first: bool,
            fail_second: bool,
            expected_rejects: usize,
            expected_kill_switch: bool,
        }

        let cases = [
            Case {
                fail_first: false,
                fail_second: false,
                expected_rejects: 0,
                expected_kill_switch: true,
            },
            Case {
                fail_first: true,
                fail_second: true,
                expected_rejects: 2,
                expected_kill_switch: false,
            },
        ];

        for case in cases {
            let state = Rc::new(RefCell::new(None));
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(StartPolicyMock::pass("start"))
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "m1_policy",
                    shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "shared_kill_switch",
                        false,
                        false,
                    ),
                    case.fail_first,
                    RejectScope::Order,
                ))
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "m2_policy",
                    shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "shared_kill_switch",
                        true,
                        true,
                    ),
                    case.fail_second,
                    RejectScope::Account,
                ))
                .build()
                .expect("engine must build");

            let request = engine
                .start_pre_trade(order_with_settlement("USD"))
                .expect("start stage must pass");
            let execute_result = request.execute();

            if case.expected_rejects == 0 {
                let mut reservation = execute_result.expect("execute must pass");
                reservation.commit();
            } else {
                assert!(execute_result.is_err(), "execute must reject");
                let rejects = execute_result.err().expect("rejects must be present");
                assert_eq!(rejects.len(), case.expected_rejects);
                assert_eq!(rejects[0].code, RejectCode::Other);
                assert_eq!(rejects[0].scope, RejectScope::Order);
                assert_eq!(rejects[1].code, RejectCode::Other);
                assert_eq!(rejects[1].scope, RejectScope::Account);
            }

            assert_eq!(*state.borrow(), Some(case.expected_kill_switch));
        }
    }

    #[test]
    fn light_stage_changes_are_not_rolled_back_when_execute_rejects() {
        let light_counter = Rc::new(Cell::new(0));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::with_counter(
                "start",
                Rc::clone(&light_counter),
            ))
            .pre_trade(MainPolicyMock::with_mutation_and_optional_reject(
                "rejecting_main",
                "m1",
                true,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("start stage must pass");
        assert!(request.execute().is_err(), "execute must reject");

        assert_eq!(light_counter.get(), 1);
    }

    #[test]
    fn reservation_drop_triggers_rollback_in_reverse_order() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "m1_policy",
                shared_kill_switch_mutation(Rc::clone(&state), "shared_kill_switch", false, false),
                false,
                RejectScope::Order,
            ))
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "m2_policy",
                shared_kill_switch_mutation(Rc::clone(&state), "shared_kill_switch", true, true),
                false,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("start stage must pass");
        let reservation = request.execute().expect("execute must pass");
        drop(reservation);

        assert_eq!(*state.borrow(), Some(false));
    }

    #[test]
    fn apply_execution_report_aggregates_account_blocks() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::new(
                "start_false",
                Rc::new(Cell::new(0)),
                false,
                false,
                None,
                None,
            ))
            .pre_trade(MainPolicyMock::with_calls(
                "main_true",
                Rc::new(Cell::new(0)),
                false,
                true,
                None,
            ))
            .build()
            .expect("engine must build");

        let result = engine.apply_execution_report(&execution_report("USD"));
        assert!(!result.account_blocks.is_empty());
    }

    #[test]
    fn apply_execution_report_exposes_report_account_group_to_policy() {
        use crate::param::AccountGroupId;
        use crate::pretrade::PostTradeContext;

        struct GroupCapturePolicy {
            seen: Rc<Cell<Option<AccountGroupId>>>,
        }

        impl<Order, ExecutionReport, AccountAdjustment, Sync: crate::core::SyncMode>
            PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync> for GroupCapturePolicy
        {
            fn name(&self) -> &str {
                "GroupCapturePolicy"
            }

            fn apply_execution_report(
                &self,
                ctx: &PostTradeContext<
                    <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
                >,
                _report: &ExecutionReport,
            ) -> Option<PostTradeResult> {
                self.seen.set(ctx.account_group());
                None
            }
        }

        let seen = Rc::new(Cell::new(None));
        let engine = Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(GroupCapturePolicy {
                seen: Rc::clone(&seen),
            })
            .build()
            .expect("engine must build");

        // The execution_report helper carries account 99224416.
        let group = AccountGroupId::from_u32(42).expect("account group id must be valid");
        engine
            .accounts()
            .register_group(&[AccountId::from_u64(99224416)], group)
            .expect("registration must succeed");
        engine
            .accounts()
            .block(AccountId::from_u64(99224416), "account halt".to_owned());
        engine
            .accounts()
            .block_group(group, "group halt".to_owned())
            .expect("group block must succeed");

        engine.apply_execution_report(&execution_report("USD"));
        assert_eq!(seen.get(), Some(group));
    }

    #[test]
    fn request_returns_system_unavailable_when_engine_is_dropped() {
        let request = {
            let engine: LocalEngine<TestOrder> = Engine::builder()
                .no_sync()
                .pre_trade(NoopPolicy::new("noop"))
                .build()
                .expect("engine must build");
            engine
                .start_pre_trade(order_with_settlement("USD"))
                .expect("start stage must pass")
        };

        let result = request.execute();
        assert!(
            result.is_err(),
            "request must fail when engine is unavailable"
        );
        let rejects = result
            .err()
            .expect("rejects must be present when engine is unavailable");
        assert_eq!(rejects.len(), 1);

        let reject = &rejects[0];
        assert_eq!(reject.policy, "Engine");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::SystemUnavailable);
        assert_eq!(reject.reason, "engine is no longer available");
        assert_eq!(reject.details, "request handle outlived engine instance");
    }

    #[test]
    fn order_core_request_returns_system_unavailable_when_engine_is_dropped() {
        let request = {
            let engine: LocalEngine<NoAccountOrder> = Engine::builder()
                .no_sync()
                .pre_trade(NoopPolicy::new("noop"))
                .build()
                .expect("engine must build");
            engine
                .start_pre_trade(NoAccountOrder)
                .expect("start stage must pass")
        };

        let result = request.execute();
        assert!(
            result.is_err(),
            "request must fail when engine is unavailable"
        );
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
    }

    #[test]
    fn reservation_mutation_callback_is_noop_when_engine_is_dropped() {
        let state = Rc::new(RefCell::new(None));
        let mut reservation = {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(StartPolicyMock::pass("start"))
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "main",
                    shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "shared_kill_switch",
                        false,
                        false,
                    ),
                    false,
                    RejectScope::Order,
                ))
                .build()
                .expect("engine must build");

            let request = engine
                .start_pre_trade(order_with_settlement("USD"))
                .expect("start stage must pass");
            request.execute().expect("main stage must pass")
        };

        reservation.commit();
    }

    #[test]
    fn order_core_reservation_mutation_callback_is_noop_when_engine_is_dropped() {
        let state = Rc::new(RefCell::new(None));
        let mut reservation = {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(CoreStartPolicyMock {
                    name: "core_start",
                    reject: false,
                    reject_scope: RejectScope::Order,
                })
                .pre_trade(CoreMainPolicyMock {
                    name: "core_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "core_drop_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                    reject_scope: RejectScope::Order,
                })
                .build()
                .expect("engine must build");

            engine
                .start_pre_trade(NoAccountOrder)
                .expect("start stage must pass")
                .execute()
                .expect("main stage must pass")
        };

        reservation.commit();
    }

    #[test]
    fn build_error_display_is_stable() {
        let err = EngineBuildError::DuplicatePolicyName {
            name: "dup".to_string(),
        };
        assert_eq!(err.to_string(), "duplicate policy name: dup");
    }

    #[test]
    fn account_adjustment_batch_error_display_is_stable() {
        let err = AccountAdjustmentBatchError {
            failed_adjustment_index: 2,
            rejects: Rejects::from(Reject::new(
                "adj_policy",
                RejectScope::Order,
                RejectCode::Other,
                "account adjustment rejected",
                "mock account adjustment policy rejected the adjustment",
            )),
        };
        assert_eq!(
            err.to_string(),
            "account adjustment batch rejected at index 2: [adj_policy] account adjustment rejected: mock account adjustment policy rejected the adjustment"
        );
    }

    #[test]
    fn main_stage_observes_settlement_assets_independently() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::with_calls(
                "collector",
                Rc::new(Cell::new(0)),
                false,
                false,
                Some(Rc::clone(&seen)),
            ))
            .build()
            .expect("engine must build");

        let request_usd = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("USD order must pass start stage");
        let mut reservation_usd = request_usd.execute().expect("USD order must pass");
        reservation_usd.commit();

        let request_eur = engine
            .start_pre_trade(order_with_settlement("EUR"))
            .expect("EUR order must pass start stage");
        let mut reservation_eur = request_eur.execute().expect("EUR order must pass");
        reservation_eur.commit();

        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(
            seen[0],
            Asset::new("USD").expect("asset code must be valid")
        );
        assert_eq!(
            seen[1],
            Asset::new("EUR").expect("asset code must be valid")
        );
    }

    #[test]
    fn account_scoped_reject_from_start_stage_permanently_blocks_account() {
        let blocked = Rc::new(Cell::new(true));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::with_block_flag(
                "toggle",
                Rc::clone(&blocked),
            ))
            .build()
            .expect("engine must build");

        let first = engine.start_pre_trade(order_with_settlement("USD"));
        assert!(first.is_err());

        blocked.set(false);

        let second = engine.start_pre_trade(order_with_settlement("USD"));
        let Err(rejects) = second else {
            panic!("account must stay blocked");
        };
        assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
        assert_eq!(rejects[0].policy, "toggle");
    }

    #[test]
    fn drop_copy_runs_all_stages_and_applies_rejecting_mutations() {
        let start_calls = Rc::new(Cell::new(0));
        let mutation_state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::new(
                "rejecting_start",
                Rc::clone(&start_calls),
                true,
                false,
                None,
                None,
            ))
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "rejecting_main",
                shared_kill_switch_mutation(
                    Rc::clone(&mutation_state),
                    "drop_copy_mutation",
                    true,
                    false,
                ),
                true,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let mut operation = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("limit drop-copy must be admitted");
        operation.commit();
        assert_eq!(start_calls.get(), 1);
        assert_eq!(*mutation_state.borrow(), Some(true));
    }

    #[test]
    fn drop_copy_preserves_owned_rejected_result_with_policy_group() {
        let group = crate::core::PolicyGroupId::new(7);
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyRejectedResultPolicy)
            .build()
            .expect("engine must build");

        let result = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("limit drop-copy must be admitted");

        assert_eq!(
            result.lock().prices_of(group).collect::<Vec<_>>(),
            vec![Price::from_str("13").expect("price must be valid")]
        );
        assert_eq!(result.account_adjustments().len(), 1);
        assert_eq!(result.account_adjustments()[0].policy_group_id, group);
        assert_eq!(
            result.account_block().map(|block| block.reason.as_str()),
            Some("rejected with output")
        );
    }

    #[test]
    fn ordinary_dry_run_ignores_policy_result_attached_to_reject() {
        let group = crate::core::PolicyGroupId::new(7);
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyRejectedResultPolicy)
            .build()
            .expect("engine must build");

        let report = engine.execute_pre_trade_dry_run(order_with_settlement("USD"));

        assert!(!report.is_pass());
        assert!(report.account_adjustments().is_empty());
        assert_eq!(report.lock().prices_of(group).count(), 0);
    }

    #[test]
    fn drop_copy_uses_successful_policy_result() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopySuccessfulRecordedResultPolicy)
            .build()
            .expect("engine must build");

        let result = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("successful drop-copy policy must pass");

        assert_eq!(result.account_adjustments().len(), 1);
        assert_eq!(
            result.account_adjustments()[0]
                .entry
                .balance
                .expect("balance outcome must be present")
                .delta,
            PositionSize::from_str("-2").expect("delta must be valid")
        );
        assert_eq!(
            result
                .lock()
                .prices_of(crate::pretrade::DEFAULT_POLICY_GROUP_ID)
                .collect::<Vec<_>>(),
            vec![Price::from_str("12").expect("price must be valid")]
        );
    }

    #[test]
    fn drop_copy_does_not_require_price_when_policies_do_not_need_it() {
        let start_calls = Rc::new(Cell::new(0));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::new(
                "start",
                Rc::clone(&start_calls),
                false,
                false,
                None,
                None,
            ))
            .build()
            .expect("engine must build");
        let mut order = order_with_settlement("USD");
        order.operation.price = None;

        engine
            .apply_drop_copy(order)
            .expect("a policy-independent market order must pass");
        assert_eq!(start_calls.get(), 1);
    }

    #[test]
    fn drop_copy_does_not_read_price_for_a_policy_that_does_not_need_it() {
        let calls = Rc::new(Cell::new(0));
        let engine = Engine::builder::<PriceAccessErrorOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(NoopPolicy::new("noop").with_calls(Rc::clone(&calls)))
            .build()
            .expect("engine must build");

        engine
            .apply_drop_copy(PriceAccessErrorOrder)
            .expect("unused price access must not fail the request");
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn drop_copy_requires_a_readable_account_before_running_policies() {
        let calls = Rc::new(Cell::new(0));
        let engine = Engine::builder::<NoAccountOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(NoopPolicy::new("noop").with_calls(Rc::clone(&calls)))
            .build()
            .expect("engine must build");

        engine.inner.blocked_accounts.block_account(
            AccountId::from_u64(77),
            AccountBlock::new(
                "unrelated",
                RejectCode::AccountBlocked,
                "unrelated account block",
                "must not affect an order without an account",
            ),
        );

        let rejects = engine
            .apply_drop_copy(NoAccountOrder)
            .expect_err("drop-copy must reject without a routing account");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        // Drop-copy ignores blocks for admission, so the guard must describe the
        // unreadable account and never send the integrator after a stuck block.
        assert_eq!(
            rejects[0].reason,
            "drop-copy requires a readable account ID"
        );
        assert_eq!(
            rejects[0].details,
            "the account ID routes the drop-copy pipeline and its account control"
        );
        assert_eq!(calls.get(), 0);
        assert!(!engine.inner.blocked_accounts.is_all_blocked());
    }

    #[test]
    fn drop_copy_without_an_account_does_not_activate_a_global_block() {
        let engine = Engine::builder::<NoAccountOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(CoreMainPolicyMock {
                name: "account_reject",
                on_apply: None,
                reject: true,
                reject_scope: RejectScope::Account,
            })
            .build()
            .expect("engine must build");

        let rejects = engine
            .apply_drop_copy(NoAccountOrder)
            .expect_err("drop-copy must reject before running the policy");

        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        assert!(!engine.inner.blocked_accounts.is_all_blocked());
    }

    #[test]
    fn drop_copy_reads_the_engine_account_key_once() {
        let calls = Rc::new(Cell::new(0));
        // NoopPolicy never touches order accessors, so every read of
        // `account_id()` still comes solely from the engine's own cached key.
        let engine = Engine::builder::<SingleReadAccountOrder, (), ()>()
            .no_sync()
            .pre_trade(NoopPolicy::new("noop"))
            .build()
            .expect("engine must build");

        let result = engine
            .apply_drop_copy(SingleReadAccountOrder {
                calls: Rc::clone(&calls),
            })
            .expect("the cached account key must remain readable");

        assert!(!result.is_account_blocked());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn drop_copy_commit_applies_mutations_and_drop_rolls_back_implicitly() {
        let state = Rc::new(Cell::new(0));
        let commits = Rc::new(Cell::new(0));
        let rollbacks = Rc::new(Cell::new(0));
        let state_for_policy = Rc::clone(&state);
        let commits_for_policy = Rc::clone(&commits);
        let rollbacks_for_policy = Rc::clone(&rollbacks);
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "caller_finalized_mutation",
                move |mutations| {
                    state_for_policy.set(1);
                    let commits = Rc::clone(&commits_for_policy);
                    let state = Rc::clone(&state_for_policy);
                    let rollbacks = Rc::clone(&rollbacks_for_policy);
                    mutations.push(Mutation::new(
                        move || commits.set(commits.get() + 1),
                        move || {
                            state.set(0);
                            rollbacks.set(rollbacks.get() + 1);
                        },
                    ));
                },
                false,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        // Policies apply tentative state eagerly, so the commit callback runs
        // only when the caller finalizes the operation.
        let operation = engine
            .apply_drop_copy(order.clone())
            .expect("limit drop-copy must be admitted");
        assert_eq!(state.get(), 1);
        assert_eq!(commits.get(), 0);
        assert_eq!(rollbacks.get(), 0);
        drop(operation);
        assert_eq!(state.get(), 0);
        assert_eq!(commits.get(), 0);
        assert_eq!(rollbacks.get(), 1);

        let mut operation = engine
            .apply_drop_copy(order)
            .expect("limit drop-copy must be admitted");
        operation.commit();
        drop(operation);
        assert_eq!(state.get(), 1);
        assert_eq!(commits.get(), 1);
        assert_eq!(rollbacks.get(), 1);
    }

    #[test]
    fn drop_copy_explicit_rollback_runs_mutations_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let calls_for_policy = Rc::clone(&calls);
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "ordered_finalization",
                move |mutations| {
                    let commit = Rc::clone(&calls_for_policy);
                    let rollback = Rc::clone(&calls_for_policy);
                    mutations.push(Mutation::new(
                        move || commit.borrow_mut().push("commit-first"),
                        move || rollback.borrow_mut().push("rollback-first"),
                    ));
                    let commit = Rc::clone(&calls_for_policy);
                    let rollback = Rc::clone(&calls_for_policy);
                    mutations.push(Mutation::new(
                        move || commit.borrow_mut().push("commit-second"),
                        move || rollback.borrow_mut().push("rollback-second"),
                    ));
                },
                false,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let mut operation = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("limit drop-copy must be admitted");
        operation.rollback();

        assert_eq!(&*calls.borrow(), &["rollback-second", "rollback-first"]);
    }

    #[test]
    fn drop_copy_caller_rollback_kills_the_engine_when_a_callback_fails() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let calls_for_policy = Rc::clone(&calls);
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "failing_caller_rollback",
                move |mutations| {
                    let rollback = Rc::clone(&calls_for_policy);
                    mutations.push(Mutation::new_fallible_with_error(
                        || Ok(()),
                        move || {
                            rollback.borrow_mut().push("rollback-first");
                            Ok(())
                        },
                    ));
                    let rollback = Rc::clone(&calls_for_policy);
                    mutations.push(Mutation::new_fallible_with_error(
                        || Ok(()),
                        move || {
                            rollback.borrow_mut().push("rollback-second");
                            Err("second rollback failed".to_owned())
                        },
                    ));
                },
                false,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let mut operation = engine
            .apply_drop_copy(order.clone())
            .expect("limit drop-copy must be admitted");
        operation.rollback();

        // Every rollback still runs; the failure is reported to the engine,
        // not to the owner, whose `rollback` stays void.
        assert_eq!(&*calls.borrow(), &["rollback-second", "rollback-first"]);
        let rejects = engine
            .start_pre_trade(order)
            .expect_err("a failed finalizer must arm the kill switch");
        assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
    }

    // ─── Mutation finalizer failure: kill-switch scope by provenance ──────
    //
    // A finalizer has no right to fail. When one does, the engine blocks: the
    // account when the mutation is engine-owned, everything when it comes from
    // a custom policy, whose state reach the engine cannot bound. Each case
    // therefore probes a second, untouched account to pin the reach down.

    const PIPELINE_ACCOUNT: u64 = 99224416;
    const OTHER_ACCOUNT: u64 = 11223344;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum FailingFinalizer {
        Commit,
        Rollback,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum MutationOwner {
        EngineOwned,
        CustomPolicy,
    }

    const OWNERS: [MutationOwner; 2] = [MutationOwner::EngineOwned, MutationOwner::CustomPolicy];

    fn order_for_account(account: u64) -> TestOrder {
        let mut order = order_with_settlement("USD");
        order.operation.account_id = AccountId::from_u64(account);
        order
    }

    fn failing_mutation(owner: MutationOwner, finalizer: FailingFinalizer) -> Mutation {
        let commit_succeeds = finalizer != FailingFinalizer::Commit;
        let rollback_succeeds = finalizer != FailingFinalizer::Rollback;
        match owner {
            MutationOwner::EngineOwned => Mutation::new_reporting(
                move || commit_succeeds,
                move || {
                    if rollback_succeeds {
                        MutationRollbackResult::default()
                    } else {
                        MutationRollbackResult::engine_owned_callback_failure(
                            "engine-owned rollback callback failed",
                        )
                    }
                },
            ),
            MutationOwner::CustomPolicy => {
                Mutation::new_fallible(move || commit_succeeds, move || rollback_succeeds)
            }
        }
    }

    fn failing_mutation_hook(
        owner: MutationOwner,
        finalizer: FailingFinalizer,
    ) -> impl Fn(&mut Mutations) + 'static {
        move |mutations: &mut Mutations| mutations.push(failing_mutation(owner, finalizer))
    }

    /// Asserts that the kill switch fired with the reach `owner` implies.
    fn assert_kill_switch(
        owner: MutationOwner,
        pipeline_account: Option<Rejects>,
        other_account: Option<Rejects>,
    ) {
        let rejects = pipeline_account.expect("a failed finalizer must block its own account");
        assert_mutation_failure_reject(&rejects[0]);

        match owner {
            MutationOwner::EngineOwned => assert!(
                other_account.is_none(),
                "an engine-owned mutation has a known reach: only its account is blocked"
            ),
            MutationOwner::CustomPolicy => {
                let rejects = other_account
                    .expect("a custom-policy mutation has an unknown reach: block everything");
                assert_mutation_failure_reject(&rejects[0]);
            }
        }
    }

    fn assert_mutation_failure_reject(reject: &Reject) {
        assert_eq!(reject.code, RejectCode::SystemUnavailable);
        assert_eq!(reject.reason, "mutation finalizer failed");
        // A finalizer failure never names an account or a group.
        assert!(!reject.details.contains(&PIPELINE_ACCOUNT.to_string()));
        assert!(!reject.reason.contains(&PIPELINE_ACCOUNT.to_string()));
    }

    #[test]
    fn reservation_commit_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "finalizer",
                    failing_mutation_hook(owner, FailingFinalizer::Commit),
                    false,
                    RejectScope::Order,
                ))
                .build()
                .expect("engine must build");

            let mut reservation = engine
                .execute_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                .expect("pipeline must pass");
            reservation.commit();

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn reservation_rollback_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "finalizer",
                    failing_mutation_hook(owner, FailingFinalizer::Rollback),
                    false,
                    RejectScope::Order,
                ))
                .build()
                .expect("engine must build");

            let mut reservation = engine
                .execute_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                .expect("pipeline must pass");
            reservation.rollback();

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn pre_trade_account_reject_rollback_failure_prioritizes_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "finalizer",
                    failing_mutation_hook(owner, FailingFinalizer::Rollback),
                    false,
                    RejectScope::Order,
                ))
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "rejecting",
                    |_| {},
                    true,
                    RejectScope::Account,
                ))
                .build()
                .expect("engine must build");

            let rejects = engine
                .execute_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                .err()
                .expect("the second policy must reject");
            assert_eq!(rejects[0].reason, "main reject");

            let pipeline_rejects = engine
                .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                .expect_err("the pipeline account must remain blocked");
            let other_rejects = engine
                .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                .err();

            match owner {
                MutationOwner::EngineOwned => {
                    assert!(!engine.inner.blocked_accounts.is_all_blocked());
                    assert_mutation_failure_reject(&pipeline_rejects[0]);
                    assert!(
                        other_rejects.is_none(),
                        "an engine-owned rollback failure must not block another account"
                    );
                }
                MutationOwner::CustomPolicy => {
                    assert!(engine.inner.blocked_accounts.is_all_blocked());
                    assert_eq!(pipeline_rejects[0].reason, "main reject");

                    let other_rejects = other_rejects
                        .expect("a custom-policy rollback failure must block all accounts");
                    assert_mutation_failure_reject(&other_rejects[0]);

                    engine.accounts().unblock_all();

                    let pipeline_rejects = engine
                        .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                        .expect_err("the policy account block must survive global unblock");
                    assert_eq!(pipeline_rejects[0].reason, "main reject");
                    assert!(
                        engine
                            .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                            .is_ok(),
                        "global unblock must release unrelated accounts"
                    );
                }
            }
        }
    }

    #[test]
    fn drop_copy_commit_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "finalizer",
                    failing_mutation_hook(owner, FailingFinalizer::Commit),
                    false,
                    RejectScope::Order,
                ))
                .build()
                .expect("engine must build");

            let mut operation = engine
                .apply_drop_copy(order_for_account(PIPELINE_ACCOUNT))
                .expect("drop copy must be admitted");
            operation.commit();

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn drop_copy_explicit_rollback_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "finalizer",
                    failing_mutation_hook(owner, FailingFinalizer::Rollback),
                    false,
                    RejectScope::Order,
                ))
                .build()
                .expect("engine must build");

            let mut operation = engine
                .apply_drop_copy(order_for_account(PIPELINE_ACCOUNT))
                .expect("drop copy must be admitted");
            operation.rollback();

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn drop_copy_rollback_on_drop_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "finalizer",
                    failing_mutation_hook(owner, FailingFinalizer::Rollback),
                    false,
                    RejectScope::Order,
                ))
                .build()
                .expect("engine must build");

            drop(
                engine
                    .apply_drop_copy(order_for_account(PIPELINE_ACCOUNT))
                    .expect("drop copy must be admitted"),
            );

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn drop_copy_fatal_compensation_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "finalizer",
                    failing_mutation_hook(owner, FailingFinalizer::Rollback),
                    false,
                    RejectScope::Order,
                ))
                .pre_trade(DropCopyRejectPolicy {
                    name: "fatal_main",
                    code: RejectCode::ReferenceDataUnavailable,
                })
                .build()
                .expect("engine must build");

            let rejects = engine
                .apply_drop_copy(order_for_account(PIPELINE_ACCOUNT))
                .expect_err("a fatal evaluation reject must fail the operation");
            // The policy cause stays first; the cleanup failure is appended.
            assert_eq!(rejects[0].code, RejectCode::ReferenceDataUnavailable);
            assert_eq!(
                rejects[rejects.len() - 1].code,
                RejectCode::SystemUnavailable
            );

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn account_adjustment_commit_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(AdjustmentPolicyMock::with_side_effect(
                    "finalizer",
                    Rc::new(RefCell::new(Vec::new())),
                    failing_mutation_hook(owner, FailingFinalizer::Commit),
                ))
                .build()
                .expect("engine must build");

            engine
                .apply_account_adjustment(
                    AccountId::from_u64(PIPELINE_ACCOUNT),
                    &[MockAdjustment { id: 1, amount: 10 }],
                )
                .expect("the batch itself must be accepted");

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn account_adjustment_rollback_failure_arms_the_kill_switch() {
        for owner in OWNERS {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(AdjustmentPolicyMock::with_side_effect(
                    "finalizer",
                    Rc::new(RefCell::new(Vec::new())),
                    failing_mutation_hook(owner, FailingFinalizer::Rollback),
                ))
                .pre_trade(AdjustmentPolicyMock::reject_on_id(
                    "rejecting",
                    Rc::new(RefCell::new(Vec::new())),
                    1,
                ))
                .build()
                .expect("engine must build");

            engine
                .apply_account_adjustment(
                    AccountId::from_u64(PIPELINE_ACCOUNT),
                    &[MockAdjustment { id: 1, amount: 10 }],
                )
                .expect_err("the second policy must reject the batch");

            assert_kill_switch(
                owner,
                engine
                    .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
                    .err(),
                engine
                    .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                    .err(),
            );
        }
    }

    #[test]
    fn a_global_finalizer_block_is_cleared_through_the_admin_surface() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "finalizer",
                failing_mutation_hook(MutationOwner::CustomPolicy, FailingFinalizer::Commit),
                false,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let mut reservation = engine
            .execute_pre_trade(order_for_account(PIPELINE_ACCOUNT))
            .expect("pipeline must pass");
        reservation.commit();
        assert!(engine
            .start_pre_trade(order_for_account(OTHER_ACCOUNT))
            .is_err());

        engine.accounts().unblock_all();

        assert!(
            engine
                .start_pre_trade(order_for_account(OTHER_ACCOUNT))
                .is_ok(),
            "the operator must be able to return the engine to service"
        );
        // The account the pipeline ran for keeps its own block: the global
        // block and the per-account one are separate causes.
        engine
            .accounts()
            .unblock(AccountId::from_u64(PIPELINE_ACCOUNT));
        assert!(engine
            .start_pre_trade(order_for_account(PIPELINE_ACCOUNT))
            .is_ok());
    }

    #[test]
    fn drop_copy_rollback_does_not_revert_account_control_effects() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyDirectBlockAndRejectPolicy)
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let mut operation = engine
            .apply_drop_copy(order.clone())
            .expect("ordinary account reject must not abort drop-copy");
        operation.rollback();

        let blocked = engine
            .start_pre_trade(order)
            .expect_err("drop-copy account-control effects stay published");
        assert_eq!(blocked[0].reason, "first direct block");
    }

    #[test]
    fn drop_copy_rollback_does_not_refund_rate_limit_attempt() {
        let builder = Engine::builder::<TestOrder, TestReport, TestAdjustment>().no_sync();
        let settings = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }),
            [],
            [],
            [],
        )
        .expect("rate-limit settings must be valid");
        let rate_limit = RateLimitPolicy::new(settings, builder.storage_builder());
        let engine = builder
            .pre_trade(rate_limit)
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let mut operation = engine
            .apply_drop_copy(order.clone())
            .expect("limit drop-copy must be admitted");
        operation.rollback();

        let rejects = engine
            .start_pre_trade(order)
            .expect_err("drop-copy must still spend the rate-limit attempt");
        assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
    }

    #[test]
    fn drop_copy_fatal_start_reject_skips_main_stage() {
        let main_calls = Rc::new(Cell::new(0));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyFatalStartPolicy)
            .pre_trade(MainPolicyMock::with_calls(
                "must_not_run",
                Rc::clone(&main_calls),
                false,
                false,
                None,
            ))
            .build()
            .expect("engine must build");

        let rejects = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect_err("missing start field must fail drop-copy");

        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "fatal_start");
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        assert_eq!(main_calls.get(), 0);
    }

    #[test]
    fn drop_copy_fatal_main_reject_reports_start_and_main_rollback_failures() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyFailingStartRollbackPolicy)
            .pre_trade(DropCopyFatalMainWithFailingRollbackPolicy)
            .build()
            .expect("engine must build");

        let rejects = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect_err("fatal main reject with failed rollbacks must reject drop-copy");

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        assert_eq!(rejects[1].code, RejectCode::SystemUnavailable);
        assert_eq!(
            rejects[1].details,
            "main rollback failed; start rollback failed"
        );
    }

    #[test]
    fn drop_copy_fatal_main_reject_rolls_back_collected_mutations() {
        let mutation_state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "mutating",
                shared_kill_switch_mutation(
                    Rc::clone(&mutation_state),
                    "drop_copy_rollback",
                    true,
                    false,
                ),
                false,
                RejectScope::Order,
            ))
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::MissingRequiredField,
            })
            .build()
            .expect("engine must build");

        let rejects = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect_err("missing main field must fail drop-copy");

        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "fatal_main");
        assert_eq!(*mutation_state.borrow(), Some(false));
    }

    #[test]
    fn drop_copy_rollback_failure_preserves_the_fatal_reject() {
        let rollback_calls = Rc::new(Cell::new(0));
        let rollback_calls_for_policy = Rc::clone(&rollback_calls);
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "rollback_failure",
                move |mutations| {
                    let rollback = Rc::clone(&rollback_calls_for_policy);
                    mutations.push(Mutation::new_fallible(
                        || true,
                        move || {
                            rollback.set(rollback.get() + 1);
                            false
                        },
                    ));
                },
                false,
                RejectScope::Order,
            ))
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::MissingRequiredField,
            })
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let rejects = engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal drop-copy exit must reject");

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].policy, "fatal_main");
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        assert_eq!(rejects[1].policy, "Engine");
        assert_eq!(rejects[1].code, RejectCode::SystemUnavailable);
        assert_eq!(rollback_calls.get(), 1);
        assert_eq!(
            engine
                .start_pre_trade(order)
                .expect_err("rollback failure must safety-block the account")[0]
                .code,
            RejectCode::SystemUnavailable
        );
    }

    #[test]
    fn drop_copy_fatal_start_reject_rolls_back_eager_start_mutation() {
        let value = Rc::new(Cell::new(7));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyEagerStartMutationPolicy {
                value: Rc::clone(&value),
            })
            .pre_trade(DropCopyFatalStartPolicy)
            .build()
            .expect("engine must build");

        engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect_err("fatal start evaluation must reject drop-copy");

        assert_eq!(value.get(), 7);
    }

    #[test]
    fn drop_copy_fatal_start_rollback_failure_safety_blocks_account() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyFailingStartRollbackPolicy)
            .pre_trade(DropCopyFatalStartPolicy)
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let rejects = engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal start exit with failed rollback must reject");

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].policy, "fatal_start");
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
        assert_eq!(rejects[1].policy, "Engine");
        assert_eq!(rejects[1].code, RejectCode::SystemUnavailable);
        assert_eq!(rejects[1].details, "start rollback failed");
        assert_eq!(
            engine
                .start_pre_trade(order)
                .expect_err("start rollback failure must safety-block the account")[0]
                .code,
            RejectCode::SystemUnavailable
        );
    }

    #[test]
    fn drop_copy_fatal_main_reject_rolls_back_eager_start_mutation() {
        let value = Rc::new(Cell::new(11));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyEagerStartMutationPolicy {
                value: Rc::clone(&value),
            })
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::SystemUnavailable,
            })
            .build()
            .expect("engine must build");

        engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect_err("fatal main evaluation must reject drop-copy");

        assert_eq!(value.get(), 11);
    }

    #[test]
    fn drop_copy_fatal_reject_discards_earlier_account_block() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_mutation_and_optional_reject(
                "blocking_main",
                "discarded_block",
                true,
                RejectScope::Account,
            ))
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::MissingRequiredField,
            })
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal evaluation must reject the whole operation");

        assert!(engine.start_pre_trade(order).is_ok());
    }

    #[test]
    fn drop_copy_fatal_reject_discards_direct_account_block() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyDirectBlockPolicy)
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::SystemUnavailable,
            })
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        engine
            .apply_drop_copy(order.clone())
            .expect_err("callback-style failure must reject the whole operation");

        assert!(engine.start_pre_trade(order).is_ok());
    }

    #[test]
    fn drop_copy_rollback_callback_can_safety_block_account() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyRollbackBlockPolicy)
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal evaluation must roll back the policy mutation");

        let rejects = engine
            .start_pre_trade(order)
            .expect_err("rollback-requested safety block must be effective");
        assert_eq!(rejects[0].reason, "rollback could not restore policy state");
    }

    #[test]
    fn drop_copy_direct_block_precedes_later_account_reject_block() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyDirectBlockAndRejectPolicy)
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let result = engine
            .apply_drop_copy(order.clone())
            .expect("ordinary account reject must not abort drop-copy");

        assert_eq!(
            result
                .account_block()
                .expect("the first request block must be reported")
                .reason,
            "first direct block"
        );
        assert!(result.is_account_blocked());
        let rejects = engine
            .start_pre_trade(order)
            .expect_err("the direct block must be effective");
        assert_eq!(rejects[0].reason, "first direct block");
    }

    #[test]
    fn drop_copy_fatal_reject_discards_direct_account_unblock() {
        let operations = Rc::new(RefCell::new(vec![TestDeferredAccountOperation::Block {
            reason: "initial block",
            provenance: 41,
        }]));
        let fatal = Rc::new(Cell::new(false));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyAccountOperationsPolicy {
                operations: Rc::clone(&operations),
            })
            .pre_trade(ConditionalDropCopyFatalPolicy {
                fatal: Rc::clone(&fatal),
            })
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        engine
            .apply_drop_copy(order.clone())
            .expect("initial block must be applied");
        *operations.borrow_mut() = vec![TestDeferredAccountOperation::InvalidateProvenance(41)];
        fatal.set(true);

        engine
            .apply_drop_copy(order.clone())
            .expect_err("later fatal evaluation must discard the unblock");
        let rejects = engine
            .start_pre_trade(order)
            .expect_err("the original block must remain published");
        assert_eq!(rejects[0].reason, "initial block");
    }

    #[test]
    fn drop_copy_applies_block_then_unblock_in_recorded_order() {
        let operations = Rc::new(RefCell::new(vec![
            TestDeferredAccountOperation::Block {
                reason: "transient block",
                provenance: 51,
            },
            TestDeferredAccountOperation::InvalidateProvenance(51),
        ]));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyAccountOperationsPolicy { operations })
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let result = engine
            .apply_drop_copy(order.clone())
            .expect("ordered operations must apply");

        assert_eq!(
            result
                .account_block()
                .expect("request-produced block must be reported")
                .reason,
            "transient block"
        );
        assert!(!result.is_account_blocked());
        assert!(engine.start_pre_trade(order).is_ok());
    }

    #[test]
    fn drop_copy_preserves_first_block_result_across_ordered_unblock_and_reblock() {
        let operations = Rc::new(RefCell::new(vec![
            TestDeferredAccountOperation::Block {
                reason: "first request block",
                provenance: 61,
            },
            TestDeferredAccountOperation::InvalidateProvenance(61),
            TestDeferredAccountOperation::Block {
                reason: "effective replacement block",
                provenance: 62,
            },
        ]));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyAccountOperationsPolicy { operations })
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        let result = engine
            .apply_drop_copy(order.clone())
            .expect("ordered operations must apply");

        assert_eq!(
            result
                .account_block()
                .expect("the first request block must win the result")
                .reason,
            "first request block"
        );
        assert!(result.is_account_blocked());
        let rejects = engine
            .start_pre_trade(order)
            .expect_err("the replacement block must be effective");
        assert_eq!(rejects[0].reason, "effective replacement block");
    }

    #[test]
    fn drop_copy_fatal_reject_consumes_rate_limit_like_ordinary_pre_trade() {
        let builder = Engine::builder::<TestOrder, TestReport, TestAdjustment>().no_sync();
        let settings = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }),
            [],
            [],
            [],
        )
        .expect("rate-limit settings must be valid");
        let rate_limit = RateLimitPolicy::new(settings, builder.storage_builder());
        let engine = builder
            .pre_trade(rate_limit)
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::MissingRequiredField,
            })
            .build()
            .expect("engine must build");
        let order = order_with_settlement("USD");

        engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal evaluation must reject the whole operation");

        let rejects = engine
            .start_pre_trade(order)
            .expect_err("fatal drop-copy must still spend the rate-limit attempt");
        assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
    }

    #[test]
    fn drop_copy_fatal_reject_consumes_per_account_rate_limit() {
        let builder = Engine::builder::<TestOrder, TestReport, TestAdjustment>().no_sync();
        let order = order_with_settlement("USD");
        let settings = RateLimitSettings::new(
            None,
            [],
            [RateLimitAccountBarrier {
                account_id: order.account_id().expect("test order has an account"),
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }],
            [],
        )
        .expect("rate-limit settings must be valid");
        let rate_limit = RateLimitPolicy::new(settings, builder.storage_builder());
        let engine = builder
            .pre_trade(rate_limit)
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::MissingRequiredField,
            })
            .build()
            .expect("engine must build");

        engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal evaluation must reject the whole operation");
        let rejects = engine
            .start_pre_trade(order)
            .expect_err("fatal drop-copy must spend the account slot");
        assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
    }

    #[test]
    fn drop_copy_fatal_reject_consumes_per_account_asset_rate_limit() {
        let builder = Engine::builder::<TestOrder, TestReport, TestAdjustment>().no_sync();
        let order = order_with_settlement("USD");
        let settings = RateLimitSettings::new(
            None,
            [],
            [],
            [RateLimitAccountAssetBarrier {
                account_id: order.account_id().expect("test order has an account"),
                settlement_asset: Asset::new("USD").expect("asset must be valid"),
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }],
        )
        .expect("rate-limit settings must be valid");
        let rate_limit = RateLimitPolicy::new(settings, builder.storage_builder());
        let engine = builder
            .pre_trade(rate_limit)
            .pre_trade(DropCopyRejectPolicy {
                name: "fatal_main",
                code: RejectCode::MissingRequiredField,
            })
            .build()
            .expect("engine must build");

        engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal evaluation must reject the whole operation");
        let rejects = engine
            .start_pre_trade(order)
            .expect_err("fatal drop-copy must spend the account-asset slot");
        assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
    }

    #[test]
    fn drop_copy_skips_order_size_limit() {
        let settings = OrderSizeLimitSettings::new(
            Some(OrderSizeBrokerBarrier {
                limit: OrderSizeLimit {
                    max_quantity: Some(Quantity::from_str("100").expect("quantity must be valid")),
                    max_notional: Some(Volume::from_str("10000").expect("volume must be valid")),
                },
            }),
            [],
            [],
        )
        .expect("order-size settings must be valid");
        let engine = Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(OrderSizeLimitPolicy::new(settings))
            .build()
            .expect("engine must build");
        let mut order = order_with_settlement("USD");
        order.operation.price = None;

        engine
            .apply_drop_copy(order)
            .expect("historical orders are not subject to order-size admission limits");
    }

    #[test]
    fn drop_copy_skips_order_validation() {
        let engine = Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .no_sync()
            .pre_trade(OrderValidationPolicy::new())
            .build()
            .expect("engine must build");
        let mut order = order_with_settlement("USD");
        order.operation.trade_amount = TradeAmount::Quantity(Quantity::ZERO);

        engine
            .apply_drop_copy(order)
            .expect("historical orders are not subject to order validation");
    }

    #[test]
    fn drop_copy_filters_fatal_rejects_in_policy_order() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(DropCopyRejectPolicy {
                name: "ordinary",
                code: RejectCode::RiskLimitExceeded,
            })
            .pre_trade(DropCopyRejectPolicy {
                name: "missing_one",
                code: RejectCode::MissingRequiredField,
            })
            .pre_trade(DropCopyRejectPolicy {
                name: "missing_two",
                code: RejectCode::MissingRequiredField,
            })
            .build()
            .expect("engine must build");

        let rejects = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect_err("missing fields must fail drop-copy");

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].policy, "missing_one");
        assert_eq!(rejects[1].policy, "missing_two");
    }

    #[test]
    fn drop_copy_bypasses_existing_account_and_group_blocks() {
        let account = AccountId::from_u64(99224416);
        let group = AccountGroupId::from_u32(42).expect("account group id must be valid");
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::pass("main"))
            .build()
            .expect("engine must build");
        let accounts = engine.accounts();
        accounts
            .register_group(&[account], group)
            .expect("registration must succeed");
        accounts.block(account, "account halt".to_owned());
        accounts
            .block_group(group, "group halt".to_owned())
            .expect("group block must succeed");

        let result = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("limit drop-copy must be admitted");
        assert!(result.is_account_blocked());

        let account_rejects = match engine.start_pre_trade(order_with_settlement("USD")) {
            Ok(_) => panic!("account block must remain"),
            Err(rejects) => rejects,
        };
        assert_eq!(account_rejects[0].reason, "account halt");
        accounts.unblock(account);
        let group_rejects = match engine.start_pre_trade(order_with_settlement("USD")) {
            Ok(_) => panic!("group block must remain"),
            Err(rejects) => rejects,
        };
        assert_eq!(group_rejects[0].reason, "group halt");
        accounts
            .unblock_group(group)
            .expect("group unblock must succeed");
        assert!(engine.start_pre_trade(order_with_settlement("USD")).is_ok());
    }

    #[test]
    fn drop_copy_reports_its_account_block_when_account_is_already_blocked() {
        let account = AccountId::from_u64(99224416);
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_mutation_and_optional_reject(
                "blocking_main",
                "preblocked_drop_copy",
                true,
                RejectScope::Account,
            ))
            .build()
            .expect("engine must build");
        let accounts = engine.accounts();
        accounts.block(account, "existing account halt".to_owned());

        let result = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("limit drop-copy must be admitted");
        let block = result
            .account_block()
            .expect("drop-copy must expose its account block");
        assert!(result.is_account_blocked());
        assert_eq!(block.policy, "blocking_main");
        assert_eq!(block.reason, "main reject");
        let rejects = match engine.start_pre_trade(order_with_settlement("USD")) {
            Ok(_) => panic!("existing account block must remain"),
            Err(rejects) => rejects,
        };
        assert_eq!(rejects[0].reason, "existing account halt");
    }

    #[test]
    fn drop_copy_records_policy_block_and_continues_into_main_stage() {
        let start_calls = Rc::new(Cell::new(0));
        let start_block = Rc::new(Cell::new(true));
        let main_calls = Rc::new(Cell::new(0));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::new(
                "blocking_start",
                Rc::clone(&start_calls),
                false,
                false,
                None,
                Some(Rc::clone(&start_block)),
            ))
            .pre_trade(MainPolicyMock {
                name: "blocking_main",
                calls: Rc::clone(&main_calls),
                reject: true,
                reject_scope: RejectScope::Account,
                on_apply: None,
                post_trade_trigger: false,
                seen_settlement: None,
            })
            .build()
            .expect("engine must build");

        let result = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("limit drop-copy must be admitted");
        let block = result
            .account_block()
            .expect("the first account block must win");
        assert!(result.is_account_blocked());
        assert_eq!(block.policy, "blocking_start");
        assert_eq!(block.reason, "pnl kill switch triggered");
        assert_eq!(start_calls.get(), 1);
        assert_eq!(main_calls.get(), 1);
        start_block.set(false);
        let rejects = match engine.start_pre_trade(order_with_settlement("USD")) {
            Ok(_) => panic!("policy block must be recorded"),
            Err(rejects) => rejects,
        };
        assert_eq!(rejects[0].policy, "blocking_start");
        assert_eq!(rejects[0].reason, "pnl kill switch triggered");
    }

    #[test]
    fn drop_copy_records_main_stage_policy_block_and_applies_mutation() {
        let mutation_state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "blocking_main",
                shared_kill_switch_mutation(
                    Rc::clone(&mutation_state),
                    "drop_copy_account_reject",
                    true,
                    false,
                ),
                true,
                RejectScope::Account,
            ))
            .build()
            .expect("engine must build");

        let mut operation = engine
            .apply_drop_copy(order_with_settlement("USD"))
            .expect("limit drop-copy must be admitted");
        operation.commit();
        assert_eq!(
            operation.account_block().map(|block| block.policy.as_str()),
            Some("blocking_main")
        );
        assert_eq!(*mutation_state.borrow(), Some(true));
        let rejects = match engine.start_pre_trade(order_with_settlement("USD")) {
            Ok(_) => panic!("main-stage policy block must be recorded"),
            Err(rejects) => rejects,
        };
        assert_eq!(rejects[0].policy, "blocking_main");
        assert_eq!(rejects[0].reason, "main reject");
    }

    #[test]
    fn tagged_build_rejects_duplicate_policy_names_across_stages() {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let seen_orders = Rc::new(RefCell::new(Vec::new()));
        let seen_reports = Rc::new(RefCell::new(Vec::new()));

        let result = Engine::builder()
            .no_sync()
            .pre_trade(CaptureTaggedStartPolicy::new(
                "dup",
                Rc::clone(&journal),
                Rc::clone(&seen_orders),
                Rc::clone(&seen_reports),
            ))
            .pre_trade(CaptureTaggedMainPolicy::new(
                "dup",
                Rc::clone(&journal),
                Rc::clone(&seen_orders),
                Rc::clone(&seen_reports),
            ))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name }) if name == "dup"
        ));
    }

    #[test]
    fn tagged_build_rejects_duplicate_policy_names_within_start_stage() {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let seen_orders = Rc::new(RefCell::new(Vec::new()));
        let seen_reports = Rc::new(RefCell::new(Vec::new()));

        let result = Engine::builder()
            .no_sync()
            .pre_trade(CaptureTaggedStartPolicy::new(
                "dup",
                Rc::clone(&journal),
                Rc::clone(&seen_orders),
                Rc::clone(&seen_reports),
            ))
            .pre_trade(SequenceFenceStartPolicy::new("dup", Rc::clone(&journal)))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name }) if name == "dup"
        ));
    }

    #[test]
    fn tagged_start_pre_trade_rejects_before_request_is_created() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(RejectTaggedStartPolicyMock {
                name: "tagged_start_reject",
            })
            .build()
            .expect("engine must build");

        let result = engine.start_pre_trade(tagged_order("ord-reject", "AAPL", "1", "10"));
        let Err(rejects) = result else {
            panic!("start stage must reject");
        };
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "tagged_start_reject");
        assert_eq!(rejects[0].code, RejectCode::Other);

        let post_trade =
            engine.apply_execution_report(&tagged_execution_report("rep-reject", "AAPL", "1", "1"));
        assert!(post_trade.account_blocks.is_empty());
    }

    #[test]
    fn tagged_request_returns_system_unavailable_when_engine_is_dropped() {
        let request = {
            let engine: LocalEngine<TaggedOrder> = Engine::builder()
                .no_sync()
                .pre_trade(NoopPolicy::new("noop"))
                .build()
                .expect("engine must build");
            engine
                .start_pre_trade(tagged_order("ord-dropped", "AAPL", "1", "10"))
                .expect("start stage must pass")
        };

        let result = request.execute();
        assert!(
            result.is_err(),
            "request must fail when engine is unavailable"
        );
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
    }

    #[test]
    fn tagged_execute_rejects_and_rolls_back_mutations() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(TaggedMutationPolicyMock {
                name: "tagged_main",
                on_apply: Some(Rc::new(shared_kill_switch_mutation(
                    Rc::clone(&state),
                    "tagged_reject_mutation",
                    true,
                    false,
                ))),
                reject: true,
            })
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(tagged_order("ord-tagged-reject", "AAPL", "2", "11"))
            .expect("start stage must create request");
        let result = request.execute();
        assert!(result.is_err(), "main stage must reject");
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "tagged_main");
        assert_eq!(rejects[0].code, RejectCode::Other);
        assert_eq!(*state.borrow(), Some(false));

        let post_trade = engine.apply_execution_report(&tagged_execution_report(
            "rep-tagged-reject",
            "AAPL",
            "2",
            "1",
        ));
        assert!(post_trade.account_blocks.is_empty());
    }

    #[test]
    fn tagged_execute_commit_and_rollback_apply_mutation_callback() {
        let cases = [
            (FinalizeAction::Commit, true),
            (FinalizeAction::Rollback, false),
        ];

        for (action, expected_state) in cases {
            let state = Rc::new(RefCell::new(None));
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(TaggedMutationPolicyMock {
                    name: "tagged_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "tagged_finalize_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                })
                .build()
                .expect("engine must build");

            let mut reservation = engine
                .start_pre_trade(tagged_order("ord-tagged-finalize", "MSFT", "3", "12"))
                .expect("start stage must create request")
                .execute()
                .expect("main stage must pass");

            match action {
                FinalizeAction::Commit => reservation.commit(),
                FinalizeAction::Rollback => reservation.rollback(),
            }

            assert_eq!(*state.borrow(), Some(expected_state));

            let post_trade = engine.apply_execution_report(&tagged_execution_report(
                "rep-tagged-finalize",
                "MSFT",
                "3",
                "1",
            ));
            assert!(post_trade.account_blocks.is_empty());
        }
    }

    #[test]
    fn tagged_reservation_mutation_callback_is_noop_when_engine_is_dropped() {
        let state = Rc::new(RefCell::new(None));
        let mut reservation = {
            let engine = Engine::builder()
                .no_sync()
                .pre_trade(TaggedMutationPolicyMock {
                    name: "tagged_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "tagged_drop_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                })
                .build()
                .expect("engine must build");

            engine
                .start_pre_trade(tagged_order("ord-tagged-drop", "AAPL", "1", "10"))
                .expect("start stage must pass")
                .execute()
                .expect("main stage must pass")
        };

        reservation.commit();
    }

    #[test]
    fn interleaved_requests_and_reports_preserve_original_tags_across_all_policies() {
        struct Case {
            execute_order: [usize; 3],
            finalize_actions: [FinalizeAction; 3],
            report_order: [usize; 3],
        }

        let cases = [
            Case {
                execute_order: [2, 0, 1],
                finalize_actions: [
                    FinalizeAction::Rollback,
                    FinalizeAction::Commit,
                    FinalizeAction::Rollback,
                ],
                report_order: [1, 2, 0],
            },
            Case {
                execute_order: [1, 2, 0],
                finalize_actions: [
                    FinalizeAction::Commit,
                    FinalizeAction::Rollback,
                    FinalizeAction::Commit,
                ],
                report_order: [2, 0, 1],
            },
            Case {
                execute_order: [0, 2, 1],
                finalize_actions: [
                    FinalizeAction::Commit,
                    FinalizeAction::Commit,
                    FinalizeAction::Rollback,
                ],
                report_order: [0, 1, 2],
            },
        ];

        for case in cases {
            let journal = Rc::new(RefCell::new(Vec::new()));
            let start_seen_orders = Rc::new(RefCell::new(Vec::new()));
            let start_seen_reports = Rc::new(RefCell::new(Vec::new()));
            let main_seen_orders = Rc::new(RefCell::new(Vec::new()));
            let main_seen_reports = Rc::new(RefCell::new(Vec::new()));

            let engine = Engine::builder()
                .no_sync()
                .pre_trade(CaptureTaggedStartPolicy::new(
                    "capture_start",
                    Rc::clone(&journal),
                    Rc::clone(&start_seen_orders),
                    Rc::clone(&start_seen_reports),
                ))
                .pre_trade(SequenceFenceStartPolicy::new(
                    "sequence_start",
                    Rc::clone(&journal),
                ))
                .pre_trade(CaptureTaggedMainPolicy::new(
                    "capture_main",
                    Rc::clone(&journal),
                    Rc::clone(&main_seen_orders),
                    Rc::clone(&main_seen_reports),
                ))
                .pre_trade(SequenceFenceMainPolicy::new(
                    "sequence_main",
                    Rc::clone(&journal),
                ))
                .build()
                .expect("engine must build");

            let orders = [
                tagged_order("ord-a", "AAPL", "10", "25"),
                tagged_order("ord-b", "MSFT", "11", "26"),
                tagged_order("ord-c", "TSLA", "12", "27"),
            ];
            let reports = [
                tagged_execution_report("rep-a", "AAPL", "5", "1"),
                tagged_execution_report("rep-b", "MSFT", "6", "1"),
                tagged_execution_report("rep-c", "TSLA", "7", "1"),
            ];

            let mut requests: Vec<_> = orders
                .iter()
                .cloned()
                .map(|order| {
                    Some(
                        engine
                            .start_pre_trade(order)
                            .expect("start stage must pass for tagged order"),
                    )
                })
                .collect();

            for (request_index, action) in
                case.execute_order.iter().zip(case.finalize_actions.iter())
            {
                let request = requests[*request_index]
                    .take()
                    .expect("request must be available exactly once");
                let mut reservation = request
                    .execute()
                    .expect("main stage must pass for tagged order");

                match action {
                    FinalizeAction::Commit => reservation.commit(),
                    FinalizeAction::Rollback => reservation.rollback(),
                }
                journal.borrow_mut().push(format!(
                    "finalize:{}:{}",
                    action.as_str(),
                    orders[*request_index].tag
                ));
            }

            for report_index in case.report_order {
                let post_trade = engine.apply_execution_report(&reports[report_index]);
                assert!(post_trade.account_blocks.is_empty());
            }

            assert_eq!(*start_seen_orders.borrow(), vec!["ord-a", "ord-b", "ord-c"]);
            assert_eq!(
                *main_seen_orders.borrow(),
                case.execute_order
                    .iter()
                    .map(|index| orders[*index].tag)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                *start_seen_reports.borrow(),
                case.report_order
                    .iter()
                    .map(|index| reports[*index].tag)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                *main_seen_reports.borrow(),
                case.report_order
                    .iter()
                    .map(|index| reports[*index].tag)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                *journal.borrow(),
                expected_interleaving_journal(
                    &case.execute_order,
                    &case.finalize_actions,
                    &case.report_order,
                )
            );
        }
    }

    #[test]
    fn start_pre_trade_allows_extreme_price_without_notional_precompute() {
        let engine: LocalEngine<TestOrder> = Engine::builder()
            .no_sync()
            .pre_trade(NoopPolicy::new("noop"))
            .build()
            .expect("engine must build");
        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("1").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("100").expect("price must be valid")),
            },
        };

        let mut reservation = engine
            .start_pre_trade(order)
            .expect("request must be created without notional precompute")
            .execute()
            .expect("execute without policies must pass");
        reservation.rollback();
    }

    #[test]
    fn sell_order_can_reserve_notional_without_engine_notional_cache() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let reserved_amount = Volume::from_str("20000").expect("volume must be valid");
        let reserved_notional = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(ReserveNotionalPolicy {
                settlement: usd.clone(),
                amount: reserved_amount,
                reserved_notional: Rc::clone(&reserved_notional),
            })
            .build()
            .expect("engine must build");

        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    usd.clone(),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Sell,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("100").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("200").expect("price must be valid")),
            },
        };

        let mut reservation = engine
            .start_pre_trade(order)
            .expect("sell order must pass start stage")
            .execute()
            .expect("sell order must pass execute");
        reservation.commit();
        assert_eq!(*reserved_notional.borrow(), Some(reserved_amount));

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(post_trade.account_blocks.is_empty());
    }

    #[test]
    fn sell_order_reservation_rollback_resets_reserved_notional_to_zero() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let reserved_amount = Volume::from_str("20000").expect("volume must be valid");
        let reserved_notional = Rc::new(RefCell::new(None));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(ReserveNotionalPolicy {
                settlement: usd.clone(),
                amount: reserved_amount,
                reserved_notional: Rc::clone(&reserved_notional),
            })
            .build()
            .expect("engine must build");

        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    usd,
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Sell,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("100").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("200").expect("price must be valid")),
            },
        };

        let mut reservation = engine
            .start_pre_trade(order)
            .expect("sell order must pass start stage")
            .execute()
            .expect("sell order must pass execute");
        reservation.rollback();

        assert_eq!(*reserved_notional.borrow(), Some(Volume::ZERO));
    }

    #[test]
    #[should_panic(expected = "quantity-based order expected")]
    fn reserve_notional_policy_panics_when_volume_order_is_passed() {
        let policy = ReserveNotionalPolicy {
            settlement: Asset::new("USD").expect("asset code must be valid"),
            amount: Volume::from_str("100").expect("volume must be valid"),
            reserved_notional: Rc::new(RefCell::new(None)),
        };
        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Sell,
                trade_amount: TradeAmount::Volume(
                    Volume::from_str("100").expect("volume must be valid"),
                ),
                price: Some(Price::from_str("200").expect("price must be valid")),
            },
        };
        let mut mutations = Mutations::default();
        let _ = <ReserveNotionalPolicy as PreTradePolicy<
            TestOrder,
            TestReport,
            TestAdjustment,
            crate::core::LocalSync,
        >>::perform_pre_trade_check(
            &policy,
            &PreTradeContext::<NoLocking>::new(None),
            &order,
            &mut mutations,
        );
    }

    fn order_with_settlement(settlement: &str) -> TestOrder {
        WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new(settlement).expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("1").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("100").expect("price must be valid")),
            },
        }
    }

    fn execution_report(settlement: &str) -> TestReport {
        WithFinancialImpact {
            inner: WithExecutionReportOperation {
                inner: (),
                operation: ExecutionReportOperation {
                    instrument: Instrument::new(
                        Asset::new("AAPL").expect("asset code must be valid"),
                        Asset::new(settlement).expect("asset code must be valid"),
                    ),
                    account_id: AccountId::from_u64(99224416),
                    side: Side::Buy,
                },
            },
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str("-10").expect("pnl must be valid"),
                fee: Fee::from_str("1").expect("fee must be valid"),
            },
        }
    }

    #[derive(Clone, Copy)]
    enum FinalizeAction {
        Commit,
        Rollback,
    }

    impl FinalizeAction {
        fn as_str(self) -> &'static str {
            match self {
                Self::Commit => "commit",
                Self::Rollback => "rollback",
            }
        }
    }

    #[derive(Clone)]
    struct TaggedOrder {
        tag: &'static str,
    }

    impl HasAccountId for TaggedOrder {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("account_id"))
        }
    }

    #[derive(Clone)]
    struct TaggedReport {
        tag: &'static str,
    }

    impl HasAccountId for TaggedReport {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("account_id"))
        }
    }

    struct CaptureTaggedStartPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
        seen_orders: Rc<RefCell<Vec<&'static str>>>,
        seen_reports: Rc<RefCell<Vec<&'static str>>>,
    }

    impl CaptureTaggedStartPolicy {
        fn new(
            name: &'static str,
            journal: Rc<RefCell<Vec<String>>>,
            seen_orders: Rc<RefCell<Vec<&'static str>>>,
            seen_reports: Rc<RefCell<Vec<&'static str>>>,
        ) -> Self {
            Self {
                name,
                journal,
                seen_orders,
                seen_reports,
            }
        }
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<TaggedOrder, TaggedReport, TestAdjustment, Sync>
        for CaptureTaggedStartPolicy
    {
        fn name(&self) -> &str {
            self.name
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            order: &TaggedOrder,
        ) -> Result<(), Rejects> {
            self.seen_orders.borrow_mut().push(order.tag);
            self.journal
                .borrow_mut()
                .push(format!("start:{}:{}", self.name, order.tag));
            Ok(())
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            report: &TaggedReport,
        ) -> Option<PostTradeResult> {
            self.seen_reports.borrow_mut().push(report.tag);
            self.journal
                .borrow_mut()
                .push(format!("report-start:{}:{}", self.name, report.tag));
            None
        }
    }

    struct SequenceFenceStartPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
    }

    impl SequenceFenceStartPolicy {
        fn new(name: &'static str, journal: Rc<RefCell<Vec<String>>>) -> Self {
            Self { name, journal }
        }
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<TaggedOrder, TaggedReport, TestAdjustment, Sync>
        for SequenceFenceStartPolicy
    {
        fn name(&self) -> &str {
            self.name
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            order: &TaggedOrder,
        ) -> Result<(), Rejects> {
            self.journal
                .borrow_mut()
                .push(format!("start:{}:{}", self.name, order.tag));
            Ok(())
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            report: &TaggedReport,
        ) -> Option<PostTradeResult> {
            self.journal
                .borrow_mut()
                .push(format!("report-start:{}:{}", self.name, report.tag));
            None
        }
    }

    struct CaptureTaggedMainPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
        seen_orders: Rc<RefCell<Vec<&'static str>>>,
        seen_reports: Rc<RefCell<Vec<&'static str>>>,
    }

    impl CaptureTaggedMainPolicy {
        fn new(
            name: &'static str,
            journal: Rc<RefCell<Vec<String>>>,
            seen_orders: Rc<RefCell<Vec<&'static str>>>,
            seen_reports: Rc<RefCell<Vec<&'static str>>>,
        ) -> Self {
            Self {
                name,
                journal,
                seen_orders,
                seen_reports,
            }
        }
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<TaggedOrder, TaggedReport, TestAdjustment, Sync>
        for CaptureTaggedMainPolicy
    {
        fn name(&self) -> &str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            order: &TaggedOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            self.seen_orders.borrow_mut().push(order.tag);
            self.journal
                .borrow_mut()
                .push(format!("execute:{}:{}", self.name, order.tag));
            Ok(None)
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            report: &TaggedReport,
        ) -> Option<PostTradeResult> {
            self.seen_reports.borrow_mut().push(report.tag);
            self.journal
                .borrow_mut()
                .push(format!("report-main:{}:{}", self.name, report.tag));
            None
        }
    }

    struct SequenceFenceMainPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
    }

    impl SequenceFenceMainPolicy {
        fn new(name: &'static str, journal: Rc<RefCell<Vec<String>>>) -> Self {
            Self { name, journal }
        }
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<TaggedOrder, TaggedReport, TestAdjustment, Sync>
        for SequenceFenceMainPolicy
    {
        fn name(&self) -> &str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            order: &TaggedOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            self.journal
                .borrow_mut()
                .push(format!("execute:{}:{}", self.name, order.tag));
            Ok(None)
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            report: &TaggedReport,
        ) -> Option<PostTradeResult> {
            self.journal
                .borrow_mut()
                .push(format!("report-main:{}:{}", self.name, report.tag));
            None
        }
    }

    struct RejectTaggedStartPolicyMock {
        name: &'static str,
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<TaggedOrder, TaggedReport, TestAdjustment, Sync>
        for RejectTaggedStartPolicyMock
    {
        fn name(&self) -> &str {
            self.name
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TaggedOrder,
        ) -> Result<(), Rejects> {
            Err(Rejects::from(Reject::new(
                self.name,
                RejectScope::Order,
                RejectCode::Other,
                "tagged start reject",
                "tagged start policy rejected the order",
            )))
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _report: &TaggedReport,
        ) -> Option<PostTradeResult> {
            None
        }
    }

    struct TaggedMutationPolicyMock {
        name: &'static str,
        on_apply: Option<MutationHook>,
        reject: bool,
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<TaggedOrder, TaggedReport, TestAdjustment, Sync>
        for TaggedMutationPolicyMock
    {
        fn name(&self) -> &str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TaggedOrder,
            mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            if let Some(on_apply) = &self.on_apply {
                on_apply(mutations);
            }

            if self.reject {
                return Err(Rejects::from(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "tagged main reject",
                    "tagged mutation policy rejected the order",
                )));
            }
            Ok(None)
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _report: &TaggedReport,
        ) -> Option<PostTradeResult> {
            None
        }
    }

    struct CoreStartPolicyMock {
        name: &'static str,
        reject: bool,
        reject_scope: RejectScope,
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<NoAccountOrder, TestReport, TestAdjustment, Sync> for CoreStartPolicyMock
    {
        fn name(&self) -> &str {
            self.name
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &NoAccountOrder,
        ) -> Result<(), Rejects> {
            if self.reject {
                return Err(Rejects::from(Reject::new(
                    self.name,
                    self.reject_scope.clone(),
                    RejectCode::Other,
                    "core start reject",
                    "order core start policy rejected the order",
                )));
            }
            Ok(())
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _report: &TestReport,
        ) -> Option<PostTradeResult> {
            None
        }
    }

    struct CoreMainPolicyMock {
        name: &'static str,
        on_apply: Option<MutationHook>,
        reject: bool,
        reject_scope: RejectScope,
    }

    impl<Sync: crate::core::SyncMode>
        PreTradePolicy<NoAccountOrder, TestReport, TestAdjustment, Sync> for CoreMainPolicyMock
    {
        fn name(&self) -> &str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &NoAccountOrder,
            mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            if let Some(on_apply) = &self.on_apply {
                on_apply(mutations);
            }

            if self.reject {
                return Err(Rejects::from(Reject::new(
                    self.name,
                    self.reject_scope.clone(),
                    RejectCode::Other,
                    "core main reject",
                    "order core main policy rejected the order",
                )));
            }
            Ok(None)
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _report: &TestReport,
        ) -> Option<PostTradeResult> {
            None
        }
    }

    fn tagged_order(
        tag: &'static str,
        _underlying: &'static str,
        _quantity: &'static str,
        _price: &'static str,
    ) -> TaggedOrder {
        TaggedOrder { tag }
    }

    fn tagged_execution_report(
        tag: &'static str,
        _underlying: &'static str,
        _pnl: &'static str,
        _fee: &'static str,
    ) -> TaggedReport {
        TaggedReport { tag }
    }

    fn expected_interleaving_journal(
        execute_order: &[usize; 3],
        finalize_actions: &[FinalizeAction; 3],
        report_order: &[usize; 3],
    ) -> Vec<String> {
        let order_tags = ["ord-a", "ord-b", "ord-c"];
        let report_tags = ["rep-a", "rep-b", "rep-c"];
        let mut expected = Vec::new();

        for order_tag in order_tags {
            expected.push(format!("start:capture_start:{order_tag}"));
            expected.push(format!("start:sequence_start:{order_tag}"));
        }

        for (request_index, action) in execute_order.iter().zip(finalize_actions.iter()) {
            let order_tag = order_tags[*request_index];
            expected.push(format!("execute:capture_main:{order_tag}"));
            expected.push(format!("execute:sequence_main:{order_tag}"));
            expected.push(format!("finalize:{}:{order_tag}", action.as_str()));
        }

        for report_index in report_order {
            let report_tag = report_tags[*report_index];
            expected.push(format!("report-start:capture_start:{report_tag}"));
            expected.push(format!("report-start:sequence_start:{report_tag}"));
            expected.push(format!("report-main:capture_main:{report_tag}"));
            expected.push(format!("report-main:sequence_main:{report_tag}"));
        }

        expected
    }

    struct StartPolicyMock {
        name: &'static str,
        calls: Rc<Cell<usize>>,
        reject: bool,
        post_trade_trigger: bool,
        light_counter: Option<Rc<Cell<usize>>>,
        block_flag: Option<Rc<Cell<bool>>>,
    }

    impl StartPolicyMock {
        fn new(
            name: &'static str,
            calls: Rc<Cell<usize>>,
            reject: bool,
            post_trade_trigger: bool,
            light_counter: Option<Rc<Cell<usize>>>,
            block_flag: Option<Rc<Cell<bool>>>,
        ) -> Self {
            Self {
                name,
                calls,
                reject,
                post_trade_trigger,
                light_counter,
                block_flag,
            }
        }

        fn pass(name: &'static str) -> Self {
            Self::new(name, Rc::new(Cell::new(0)), false, false, None, None)
        }

        fn with_counter(name: &'static str, counter: Rc<Cell<usize>>) -> Self {
            Self::new(
                name,
                Rc::new(Cell::new(0)),
                false,
                false,
                Some(counter),
                None,
            )
        }

        fn with_block_flag(name: &'static str, block_flag: Rc<Cell<bool>>) -> Self {
            Self::new(
                name,
                Rc::new(Cell::new(0)),
                false,
                false,
                None,
                Some(block_flag),
            )
        }
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for StartPolicyMock
    {
        fn name(&self) -> &str {
            self.name
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
        ) -> Result<(), Rejects> {
            self.calls.set(self.calls.get() + 1);
            if let Some(counter) = &self.light_counter {
                counter.set(counter.get() + 1);
            }
            if let Some(block_flag) = &self.block_flag {
                if block_flag.get() {
                    return Err(Rejects::from(Reject::new(
                        self.name,
                        RejectScope::Account,
                        RejectCode::PnlKillSwitchTriggered,
                        "pnl kill switch triggered",
                        "mock policy blocked the account",
                    )));
                }
            }
            if self.reject {
                return Err(Rejects::from(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "start reject",
                    "mock start policy rejected the order",
                )));
            }
            Ok(())
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _report: &TestReport,
        ) -> Option<PostTradeResult> {
            if self.post_trade_trigger {
                Some(PostTradeResult::blocks_only(vec![
                    crate::pretrade::AccountBlock::new(
                        self.name,
                        crate::pretrade::RejectCode::PnlKillSwitchTriggered,
                        "kill switch triggered",
                        "",
                    ),
                ]))
            } else {
                None
            }
        }
    }

    struct MainPolicyMock {
        name: &'static str,
        calls: Rc<Cell<usize>>,
        reject: bool,
        reject_scope: RejectScope,
        on_apply: Option<MutationHook>,
        post_trade_trigger: bool,
        seen_settlement: Option<Rc<RefCell<Vec<Asset>>>>,
    }

    impl MainPolicyMock {
        fn pass(name: &'static str) -> Self {
            Self {
                name,
                calls: Rc::new(Cell::new(0)),
                reject: false,
                reject_scope: RejectScope::Order,
                on_apply: None,
                post_trade_trigger: false,
                seen_settlement: None,
            }
        }

        fn with_calls(
            name: &'static str,
            calls: Rc<Cell<usize>>,
            reject: bool,
            post_trade_trigger: bool,
            seen_settlement: Option<Rc<RefCell<Vec<Asset>>>>,
        ) -> Self {
            Self {
                name,
                calls,
                reject,
                reject_scope: RejectScope::Order,
                on_apply: None,
                post_trade_trigger,
                seen_settlement,
            }
        }

        fn with_mutation_and_optional_reject(
            name: &'static str,
            mutation_id: &'static str,
            reject: bool,
            reject_scope: RejectScope,
        ) -> Self {
            let state = Rc::new(RefCell::new(None));
            Self::with_custom_mutation_and_optional_reject(
                name,
                shared_kill_switch_mutation(state, mutation_id, true, false),
                reject,
                reject_scope,
            )
        }

        fn with_custom_mutation_and_optional_reject(
            name: &'static str,
            on_apply: impl Fn(&mut Mutations) + 'static,
            reject: bool,
            reject_scope: RejectScope,
        ) -> Self {
            Self {
                name,
                calls: Rc::new(Cell::new(0)),
                reject,
                reject_scope,
                on_apply: Some(Rc::new(on_apply)),
                post_trade_trigger: false,
                seen_settlement: None,
            }
        }
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for MainPolicyMock
    {
        fn name(&self) -> &str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            order: &TestOrder,
            mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            self.calls.set(self.calls.get() + 1);
            if let Some(seen_settlement) = &self.seen_settlement {
                seen_settlement
                    .borrow_mut()
                    .push(order.operation.instrument.settlement_asset().clone());
            }

            if let Some(on_apply) = &self.on_apply {
                on_apply(mutations);
            }

            if self.reject {
                return Err(Rejects::from(Reject::new(
                    self.name,
                    self.reject_scope.clone(),
                    RejectCode::Other,
                    "main reject",
                    "mock main-stage policy rejected the order",
                )));
            }
            Ok(None)
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _report: &TestReport,
        ) -> Option<PostTradeResult> {
            if self.post_trade_trigger {
                Some(PostTradeResult::blocks_only(vec![
                    crate::pretrade::AccountBlock::new(
                        self.name,
                        crate::pretrade::RejectCode::PnlKillSwitchTriggered,
                        "kill switch triggered",
                        "",
                    ),
                ]))
            } else {
                None
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MockAdjustment {
        id: u32,
        amount: i64,
    }

    struct AdjustmentPolicyMock {
        name: &'static str,
        seen: Rc<RefCell<Vec<MockAdjustment>>>,
        reject_on_id: Option<u32>,
        on_apply: Option<AdjustmentHook>,
    }

    impl AdjustmentPolicyMock {
        fn pass(name: &'static str, seen: Rc<RefCell<Vec<MockAdjustment>>>) -> Self {
            Self {
                name,
                seen,
                reject_on_id: None,
                on_apply: None,
            }
        }

        fn reject_on_id(
            name: &'static str,
            seen: Rc<RefCell<Vec<MockAdjustment>>>,
            reject_on_id: u32,
        ) -> Self {
            Self {
                name,
                seen,
                reject_on_id: Some(reject_on_id),
                on_apply: None,
            }
        }

        fn with_side_effect(
            name: &'static str,
            seen: Rc<RefCell<Vec<MockAdjustment>>>,
            on_apply: impl Fn(&mut Mutations) + 'static,
        ) -> Self {
            Self {
                name,
                seen,
                reject_on_id: None,
                on_apply: Some(Box::new(on_apply)),
            }
        }
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for AdjustmentPolicyMock
    {
        fn name(&self) -> &str {
            self.name
        }

        fn apply_account_adjustment(
            &self,
            _ctx: &AccountAdjustmentContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _account_id: AccountId,
            adjustment: &MockAdjustment,
            mutations: &mut Mutations,
        ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
            self.seen.borrow_mut().push(*adjustment);
            if let Some(ref on_apply) = self.on_apply {
                on_apply(mutations);
            }
            if self.reject_on_id == Some(adjustment.id) {
                return Err(Rejects::from(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "account adjustment rejected",
                    "mock account adjustment policy rejected the adjustment",
                )));
            }
            Ok(PolicyAccountAdjustmentResult::default())
        }
    }

    struct ReserveNotionalPolicy {
        settlement: Asset,
        amount: Volume,
        reserved_notional: Rc<RefCell<Option<Volume>>>,
    }

    fn shared_kill_switch_mutation(
        state: Rc<RefCell<Option<bool>>>,
        _id: &'static str,
        commit_value: bool,
        rollback_value: bool,
    ) -> impl Fn(&mut Mutations) {
        move |mutations: &mut Mutations| {
            let c = Rc::clone(&state);
            let r = Rc::clone(&state);
            mutations.push(Mutation::new(
                move || {
                    *c.borrow_mut() = Some(commit_value);
                },
                move || {
                    *r.borrow_mut() = Some(rollback_value);
                },
            ));
        }
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for ReserveNotionalPolicy
    {
        fn name(&self) -> &str {
            "ReserveNotionalPolicy"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            order: &TestOrder,
            mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            use crate::core::{HasOrderPrice, HasTradeAmount};
            assert_eq!(order.operation.side, Side::Sell);
            assert_eq!(
                order.operation.instrument.settlement_asset(),
                &self.settlement
            );
            let calculated_amount = order
                .price()
                .expect("price must be present")
                .expect("price value must be Some")
                .calculate_volume(
                    match order.trade_amount().expect("trade_amount must be present") {
                        TradeAmount::Quantity(value) => value,
                        TradeAmount::Volume(_) => panic!("quantity-based order expected"),
                    },
                )
                .expect("volume must be calculable");
            assert_eq!(calculated_amount, self.amount);
            let commit_store = Rc::clone(&self.reserved_notional);
            let rollback_store = Rc::clone(&self.reserved_notional);
            let commit_amount = self.amount;
            mutations.push(Mutation::new(
                move || {
                    *commit_store.borrow_mut() = Some(commit_amount);
                },
                move || {
                    *rollback_store.borrow_mut() = Some(Volume::ZERO);
                },
            ));
            Ok(None)
        }

        fn apply_execution_report(
            &self,
            _ctx: &crate::pretrade::PostTradeContext<
                <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
            >,
            _report: &TestReport,
        ) -> Option<PostTradeResult> {
            None
        }
    }

    // ── Send/Sync type assertions ─────────────────────────────────────────────

    #[test]
    fn synced_engine_with_full_storage_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        type SyncedEngineWithFull = FullSyncEngine<OrderOperation>;
        assert_send::<SyncedEngineWithFull>();
        assert_sync::<SyncedEngineWithFull>();
    }

    /// Confirms that `LocalSync` builders accept `!Send` policies.
    /// The `!Send + !Sync` property of `LocalEngine<...>` is enforced by `Rc`
    /// auto-deriving `!Send + !Sync`; see the compile_fail doctest on
    /// `LocalSync` for explicit proof.
    #[test]
    fn local_engine_accepts_not_send_policies() {
        // StartPolicyMock contains Rc<...> (!Send). Confirms the local builder
        // compiles with !Send policies.
        let _engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("local_policy"))
            .build()
            .expect("engine with !Send policy must build in local mode");
    }

    #[test]
    fn account_sync_engine_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<super::AccountSyncEngine<OrderOperation>>();
    }

    // ── pre-trade dry-run ──────────────────────────────────────────────────────

    fn ps(value: &str) -> PositionSize {
        PositionSize::from_str(value).expect("position size must be valid")
    }

    /// Main-stage mock whose normal hook mutates immediately (bumping a shared
    /// counter and registering a commit/rollback pair), while its dry-run hook is
    /// read-only. Both hooks report the identical lock price and outcome entry, so
    /// a dry-run report can be compared field-for-field against a real
    /// reservation.
    struct ImmediateSideEffectMainPolicy {
        name: &'static str,
        immediate_calls: Rc<Cell<usize>>,
        committed: Rc<Cell<bool>>,
    }

    impl ImmediateSideEffectMainPolicy {
        fn result() -> PolicyPreTradeResult {
            let mut result = PolicyPreTradeResult::with_capacity(1, 1);
            result.account_adjustments.push(AccountOutcomeEntry {
                asset: Asset::new("USD").expect("asset must be valid"),
                balance: Some(OutcomeAmount {
                    delta: ps("-5"),
                    absolute: ps("95"),
                }),
                held: Some(OutcomeAmount {
                    delta: ps("5"),
                    absolute: ps("5"),
                }),
                incoming: None,
                realized_pnl: None,
                average_entry_price: None,
            });
            result
                .lock_prices
                .push(Price::from_str("100").expect("price must be valid"));
            result
        }
    }

    impl<Sync: crate::core::SyncMode> PreTradePolicy<TestOrder, TestReport, TestAdjustment, Sync>
        for ImmediateSideEffectMainPolicy
    {
        fn name(&self) -> &str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            self.immediate_calls.set(self.immediate_calls.get() + 1);
            let committed = Rc::clone(&self.committed);
            mutations.push(Mutation::new(move || committed.set(true), || {}));
            Ok(Some(Self::result()))
        }

        fn perform_pre_trade_check_dry_run(
            &self,
            _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            Ok(Some(Self::result()))
        }
    }

    #[test]
    fn execute_dry_run_does_not_trigger_immediate_side_effects_or_commit() {
        let immediate_calls = Rc::new(Cell::new(0));
        let committed = Rc::new(Cell::new(false));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(ImmediateSideEffectMainPolicy {
                name: "immediate_main",
                immediate_calls: Rc::clone(&immediate_calls),
                committed: Rc::clone(&committed),
            })
            .build()
            .expect("engine must build");

        let report = engine.execute_pre_trade_dry_run(order_with_settlement("USD"));
        engine.execute_pre_trade_dry_run(order_with_settlement("USD"));

        assert!(report.is_pass());
        // The dry-run hook is read-only: the immediate side effect never fired
        // and no commit ran.
        assert_eq!(immediate_calls.get(), 0);
        assert!(!committed.get());
    }

    #[test]
    fn execute_dry_run_report_matches_real_reservation_lock_and_adjustments() {
        let build_engine = || {
            Engine::builder()
                .no_sync()
                .pre_trade(StartPolicyMock::pass("start"))
                .pre_trade(ImmediateSideEffectMainPolicy {
                    name: "immediate_main",
                    immediate_calls: Rc::new(Cell::new(0)),
                    committed: Rc::new(Cell::new(false)),
                })
                .build()
                .expect("engine must build")
        };

        let dry_run_engine = build_engine();
        let report = dry_run_engine.execute_pre_trade_dry_run(order_with_settlement("USD"));

        let real_engine = build_engine();
        let reservation = real_engine
            .execute_pre_trade(order_with_settlement("USD"))
            .expect("real execute must pass");

        assert!(report.is_pass());
        assert_eq!(report.rejects(), None);
        assert_eq!(report.account_block(), None);
        assert_eq!(report.lock(), reservation.lock());
        assert_eq!(
            report.account_adjustments(),
            reservation.account_adjustments()
        );

        let prices: Vec<_> = report.lock().prices_of(DEFAULT_POLICY_GROUP_ID).collect();
        assert_eq!(prices, vec![Price::from_str("100").expect("price valid")]);
        assert_eq!(report.account_adjustments().len(), 1);
        drop(reservation);
    }

    #[test]
    fn start_dry_run_reports_account_block_without_recording_it() {
        let blocked = Rc::new(Cell::new(true));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::with_block_flag(
                "toggle",
                Rc::clone(&blocked),
            ))
            .build()
            .expect("engine must build");

        let report = engine.start_pre_trade_dry_run(order_with_settlement("USD"));
        assert!(!report.is_pass());
        let rejects = report.rejects().expect("dry-run must report rejects");
        assert_eq!(rejects[0].scope, RejectScope::Account);
        assert_eq!(rejects[0].policy, "toggle");
        let block = report
            .account_block()
            .expect("account-scope reject must surface a would-be block");
        assert_eq!(block.code, RejectCode::AccountBlocked);
        assert_eq!(block.policy, "toggle");

        // The dry-run must NOT latch the block: with the flag cleared, a real
        // start passes. A normal call would latch the block until it is lifted.
        blocked.set(false);
        assert!(engine.start_pre_trade(order_with_settlement("USD")).is_ok());
    }

    #[test]
    fn execute_dry_run_reports_main_stage_reject_without_recording_block() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .pre_trade(MainPolicyMock::with_mutation_and_optional_reject(
                "rejecting_main",
                "m1",
                true,
                RejectScope::Account,
            ))
            .build()
            .expect("engine must build");

        let report = engine.execute_pre_trade_dry_run(order_with_settlement("USD"));
        assert!(!report.is_pass());
        let rejects = report.rejects().expect("must report rejects");
        assert_eq!(rejects[0].policy, "rejecting_main");
        assert_eq!(rejects[0].scope, RejectScope::Account);
        assert!(report.account_block().is_some());

        // No block was recorded: the start stage (which only consults the
        // blocked-accounts set before any main-stage policy runs) still admits
        // the same account. A real account-scope main-stage reject would have
        // latched a block that fails this check.
        assert!(engine.start_pre_trade(order_with_settlement("USD")).is_ok());
    }

    #[test]
    fn start_dry_run_on_blocked_account_reports_rejects() {
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(StartPolicyMock::pass("start"))
            .build()
            .expect("engine must build");

        engine
            .accounts()
            .block(AccountId::from_u64(99224416), "blocked".to_string());

        let report: PreTradeDryRunReport =
            engine.start_pre_trade_dry_run(order_with_settlement("USD"));
        assert!(!report.is_pass());
    }
}
