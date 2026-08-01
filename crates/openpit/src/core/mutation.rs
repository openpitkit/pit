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

use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
use crate::param::{AccountId, Pnl};
#[cfg(test)]
use crate::pretrade::AccountBlock;

static NEXT_MUTATION_OWNER_ID: AtomicU64 = AtomicU64::new(1);

/// One accepted fill contribution that was deliberately discarded while a
/// rejected account-PnL re-arm was rolled back to its prior halted state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct AccountPnlReconciliation {
    /// Account whose PnL assertion was rolled back.
    pub(crate) account_id: AccountId,
    /// Numeric contribution accepted while the provisional value was active.
    pub(crate) discarded_delta: Option<Pnl>,
}

/// Observable effects produced while compensating a rejected adjustment
/// batch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct AccountAdjustmentRollbackReport {
    /// Blocks that are required by the final, rolled-back account-PnL state.
    pub(crate) account_blocks: Vec<AccountBlock>,
    /// Provisional blocks removed because their originating assertion was
    /// rejected.
    pub(crate) invalidated_account_blocks: Vec<AccountBlock>,
    /// Accepted contributions discarded when rollback restored a prior halt.
    pub(crate) reconciliations: Vec<AccountPnlReconciliation>,
}

/// Who registered a [`Mutation`], which decides how far the engine must
/// reach when that mutation's finalizer fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MutationProvenance {
    /// Registered by engine-owned code: a built-in policy or the engine
    /// itself. Its state reach is known and bounded by the account the
    /// pipeline runs for.
    EngineOwned,
    /// Registered through a public constructor, so by a custom policy or a
    /// binding trampoline standing in for one. Its state reach is unknown and
    /// may include engine-wide state such as a broker-level rate-limit
    /// barrier or global P&L bounds.
    CustomPolicy,
}

/// Reach of the kill switch raised for a failed mutation finalizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MutationFailureScope {
    /// Block the account the pipeline ran for.
    Account,
    /// Block every account: the failing mutation's reach is unknown.
    Global,
}

/// Finalizer failures collected while a batch of mutations was applied.
///
/// A finalizer has no right to fail, so any failure recorded here arms the
/// engine kill switch; see [`Mutation`] for the full contract. The widest
/// scope wins, because one custom-policy failure is enough to make the
/// engine's global state suspect.
#[derive(Default)]
#[must_use = "mutation finalizer failures must be inspected"]
pub(crate) struct MutationFailure {
    details: Vec<String>,
    scope: Option<MutationFailureScope>,
}

impl MutationFailure {
    fn record(&mut self, provenance: MutationProvenance, details: String) {
        let scope = match provenance {
            MutationProvenance::EngineOwned => MutationFailureScope::Account,
            MutationProvenance::CustomPolicy => MutationFailureScope::Global,
        };
        self.widen(scope);
        self.details.push(details);
    }

    fn widen(&mut self, scope: MutationFailureScope) {
        if scope == MutationFailureScope::Global || self.scope.is_none() {
            self.scope = Some(scope);
        }
    }

    pub(crate) fn append(&mut self, mut other: Self) {
        if let Some(scope) = other.scope {
            self.widen(scope);
        }
        self.details.append(&mut other.details);
    }

    /// Returns the kill-switch reach, or `None` when every finalizer
    /// succeeded.
    pub(crate) fn scope(&self) -> Option<MutationFailureScope> {
        self.scope
    }

    pub(crate) fn failed(&self) -> bool {
        self.scope.is_some()
    }

    /// Returns the joined callback error texts, for the reject the engine
    /// hands back on the paths that still have a caller to answer.
    pub(crate) fn details(&self) -> String {
        self.details.join("; ")
    }
}

#[derive(Default)]
#[must_use = "mutation rollback failures must be inspected"]
pub(crate) struct MutationRollbackResult {
    #[cfg(test)]
    pub(crate) report: AccountAdjustmentRollbackReport,
    failure: MutationFailure,
}

impl MutationRollbackResult {
    pub(crate) fn append(&mut self, other: Self) {
        #[cfg(test)]
        let mut other = other;
        self.failure.append(other.failure);
        #[cfg(test)]
        {
            self.report
                .account_blocks
                .append(&mut other.report.account_blocks);
            self.report
                .invalidated_account_blocks
                .append(&mut other.report.invalidated_account_blocks);
            self.report
                .reconciliations
                .append(&mut other.report.reconciliations);
        }
    }

    /// Reports a failed engine-owned compensation.
    ///
    /// Built-in policies surface a failed compensation through their own
    /// account-control block today, so this exists for the engine's coverage
    /// of the provenance rule rather than for a production caller.
    #[cfg(test)]
    pub(crate) fn engine_owned_callback_failure(details: impl Into<String>) -> Self {
        Self::callback_failure(MutationProvenance::EngineOwned, details.into())
    }

    fn callback_failure(provenance: MutationProvenance, details: String) -> Self {
        let mut failure = MutationFailure::default();
        failure.record(provenance, details);
        Self {
            failure,
            #[cfg(test)]
            report: AccountAdjustmentRollbackReport::default(),
        }
    }

    #[must_use = "mutation rollback failures must arm the engine kill switch"]
    pub(crate) fn failure(&self) -> &MutationFailure {
        &self.failure
    }

    pub(crate) fn callback_failed(&self) -> bool {
        self.failure.failed()
    }

    pub(crate) fn callback_failure_details(&self) -> String {
        self.failure.details()
    }
}

/// Engine kill switch armed when a mutation finalizer fails on a path that has
/// no caller left to answer.
///
/// The finalization paths - [`PreTradeReservation`](crate::PreTradeReservation)
/// and [`DropCopyOperation`](crate::pretrade::DropCopyOperation) - are void for
/// their owner, so the engine reports a failed finalizer to itself by blocking.
/// The closure is built by the engine, which alone can name its storage
/// factory, and captures the account the pipeline ran for.
pub(crate) struct MutationFailureKillSwitch(Box<dyn Fn(MutationFailureScope)>);

impl MutationFailureKillSwitch {
    pub(crate) fn new(arm: impl Fn(MutationFailureScope) + 'static) -> Self {
        Self(Box::new(arm))
    }

    /// Kill switch with no engine behind it, for unit tests that assert only
    /// the mutation callbacks themselves.
    #[cfg(test)]
    pub(crate) fn inert() -> Self {
        Self::new(|_| {})
    }

    /// Arms the kill switch when `failure` carries one; a clean batch is a
    /// no-op.
    pub(crate) fn arm(&self, failure: &MutationFailure) {
        if let Some(scope) = failure.scope() {
            (self.0)(scope);
        }
    }
}

pub(crate) fn next_mutation_owner_id() -> u64 {
    loop {
        let id = NEXT_MUTATION_OWNER_ID.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

/// Commit/rollback pair produced by a policy.
///
/// Commit/rollback action pair registered by a policy during checks.
///
/// The engine applies commit actions in registration order on success and
/// rollback actions in reverse registration order on failure. Policies apply
/// their tentative state before registration, so a rollback also runs for
/// mutations whose commit callback was never reached. Commit callbacks
/// finalize that already-applied state; they are not the place to apply it for
/// the first time.
///
/// # Rollback safety by pipeline
///
/// Account adjustment pipeline: operations may interleave under
/// [`FullSync`](crate::FullSync). Rollbacks must therefore compensate deltas
/// or use an operation-owned assertion/lease when restoring absolute state;
/// blindly restoring a snapshot can overwrite a concurrent accepted update.
///
/// Pre-trade pipeline: rollback by absolute value can break
/// consistency. Between reservation creation and finalization, external
/// systems (venues, risk aggregators) may observe or depend on reserved
/// state. Policies in this pipeline should prefer delta-based rollback
/// or use values captured at registration time.
///
/// # Finalizer contract
///
/// A finalizer - the commit or the rollback callback - has **no right to
/// fail**. By the time it runs, the decision is already made and the state it
/// finalizes was applied eagerly, so there is nothing left to compensate and no
/// caller left to answer: `commit` and `rollback` are void on every surface.
///
/// A finalizer that fails anyway leaves the engine's own bookkeeping in an
/// unknown state, so the engine never ignores it. It raises a kill switch with
/// [`RejectCode::SystemUnavailable`](crate::pretrade::RejectCode::SystemUnavailable)
/// whose reach follows who registered the mutation:
///
/// - a mutation registered by engine-owned code (a built-in policy or the
///   engine itself) has a known reach, so the account the pipeline ran for is
///   blocked;
/// - a mutation registered through a public constructor - any custom policy,
///   including every binding trampoline - has an unknown reach, because such a
///   policy may hold engine-wide state such as a broker-level rate-limit
///   barrier, so **every** account is blocked.
///
/// The block is engine-owned and carries no account or account-group
/// identifier. The owner of a reservation or drop-copy operation is not told
/// directly; it learns about the kill switch the ordinary way, when the next
/// pre-trade request is rejected. A global block is cleared through
/// [`Accounts::unblock_all`](crate::Accounts::unblock_all).
///
/// This holds on every pipeline: pre-trade reservation finalization, drop-copy
/// operation finalization (explicit and on drop), compensation of a fatal
/// drop-copy evaluation exit, and the account-adjustment batch.
///
/// # Examples
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
/// use openpit::Mutation;
///
/// let counter = Rc::new(RefCell::new(0i64));
///
/// // Tentative state is applied before the pair is registered.
/// *counter.borrow_mut() += 100;
///
/// let r = Rc::clone(&counter);
/// let mutation = Mutation::new(
///     || {
///         // Commit is empty: state was applied eagerly.
///     },
///     move || { *r.borrow_mut() -= 100; },
/// );
/// ```
pub struct Mutation {
    commit: Option<Box<dyn FnOnce() -> Result<(), String>>>,
    rollback: Box<dyn FnOnce() -> MutationRollbackResult>,
    lifetime_guard: Option<Box<dyn Any>>,
    provenance: MutationProvenance,
}

impl Mutation {
    /// Creates a mutation from commit and rollback closures.
    ///
    /// `commit` runs when the pipeline succeeds (reservation commit or
    /// account-adjustment batch acceptance).
    ///
    /// `rollback` runs when the pipeline fails (policy reject) and when the
    /// caller-owned handle is rolled back or dropped without explicit
    /// finalization. Policies must therefore apply tentative state before
    /// registering the pair and make `rollback` reverse that tentative state.
    ///
    /// Neither callback may fail; see the finalizer contract on [`Mutation`].
    pub fn new(commit: impl FnOnce() + 'static, rollback: impl FnOnce() + 'static) -> Self {
        Self::new_infallible(MutationProvenance::CustomPolicy, commit, rollback)
    }

    /// Creates a mutation whose callbacks can report boundary callback failure.
    ///
    /// This is binding infrastructure. Returning `false` from either callback
    /// marks that finalizer as failed. The engine never ignores such a failure:
    /// compensation of a fatal drop-copy evaluation exit also reports it to the
    /// caller, and every path arms the kill switch described by the finalizer
    /// contract on [`Mutation`].
    #[doc(hidden)]
    pub fn new_fallible(
        commit: impl FnOnce() -> bool + 'static,
        rollback: impl FnOnce() -> bool + 'static,
    ) -> Self {
        Self::new_fallible_with_error(
            move || {
                commit()
                    .then_some(())
                    .ok_or_else(|| "mutation commit callback failed".to_owned())
            },
            move || {
                rollback()
                    .then_some(())
                    .ok_or_else(|| "mutation rollback callback failed".to_owned())
            },
        )
    }

    /// Creates a mutation whose callbacks return boundary failure details.
    ///
    /// This is binding infrastructure. The error text is preserved in the fatal
    /// drop-copy reject; on every path the failure arms the kill switch
    /// described by the finalizer contract on [`Mutation`].
    #[doc(hidden)]
    pub fn new_fallible_with_error(
        commit: impl FnOnce() -> Result<(), String> + 'static,
        rollback: impl FnOnce() -> Result<(), String> + 'static,
    ) -> Self {
        Self::new_reporting_with_error(MutationProvenance::CustomPolicy, commit, move || {
            match rollback() {
                Ok(()) => MutationRollbackResult::default(),
                Err(details) => MutationRollbackResult::callback_failure(
                    MutationProvenance::CustomPolicy,
                    details,
                ),
            }
        })
    }

    /// Engine-owned counterpart of [`Self::new`], for built-in policies and
    /// engine internals whose state reach the engine knows.
    pub(crate) fn new_engine_owned(
        commit: impl FnOnce() + 'static,
        rollback: impl FnOnce() + 'static,
    ) -> Self {
        Self::new_infallible(MutationProvenance::EngineOwned, commit, rollback)
    }

    fn new_infallible(
        provenance: MutationProvenance,
        commit: impl FnOnce() + 'static,
        rollback: impl FnOnce() + 'static,
    ) -> Self {
        Self::new_reporting_with_error(
            provenance,
            move || {
                commit();
                Ok(())
            },
            move || {
                rollback();
                MutationRollbackResult::default()
            },
        )
    }

    pub(crate) fn new_reporting(
        commit: impl FnOnce() -> bool + 'static,
        rollback: impl FnOnce() -> MutationRollbackResult + 'static,
    ) -> Self {
        Self::new_reporting_with_error(
            MutationProvenance::EngineOwned,
            move || {
                commit()
                    .then_some(())
                    .ok_or_else(|| "mutation commit callback failed".to_owned())
            },
            rollback,
        )
    }

    fn new_reporting_with_error(
        provenance: MutationProvenance,
        commit: impl FnOnce() -> Result<(), String> + 'static,
        rollback: impl FnOnce() -> MutationRollbackResult + 'static,
    ) -> Self {
        Self {
            commit: Some(Box::new(commit)),
            rollback: Box::new(rollback),
            lifetime_guard: None,
            provenance,
        }
    }

    pub(crate) fn new_reporting_with_guard<Guard>(
        commit: impl FnOnce() -> bool + 'static,
        rollback: impl FnOnce() -> MutationRollbackResult + 'static,
        guard: Guard,
    ) -> Self
    where
        Guard: 'static,
    {
        let mut mutation = Self::new_reporting(commit, rollback);
        mutation.lifetime_guard = Some(Box::new(guard));
        mutation
    }

    fn commit(&mut self) -> Result<(), String> {
        match self.commit.take() {
            Some(commit) => commit(),
            None => Ok(()),
        }
    }

    fn rollback(self) -> MutationRollbackResult {
        let Self {
            commit,
            rollback,
            lifetime_guard,
            provenance: _,
        } = self;
        drop(commit);
        let result = rollback();
        drop(lifetime_guard);
        result
    }
}

/// Collected mutations registered during pre-trade checks.
///
/// The engine applies every commit action in registration order on success and
/// every rollback action in reverse registration order on failure. A failing
/// callback never stops the batch: the remaining finalizers still run, and the
/// failures are reported together to the engine kill switch described by the
/// finalizer contract on [`Mutation`].
///
/// # Examples
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
/// use openpit::{Mutation, Mutations};
///
/// let state = Rc::new(RefCell::new(false));
/// let mut mutations = Mutations::with_capacity(2);
///
/// // Tentative state is applied before the pair is registered.
/// *state.borrow_mut() = true;
///
/// let r = Rc::clone(&state);
/// mutations.push(Mutation::new(
///     || {
///         // Commit is empty: state was applied eagerly.
///     },
///     move || { *r.borrow_mut() = false; },
/// ));
/// ```
pub struct Mutations {
    mutations: Vec<Mutation>,
    owner_id: u64,
}

impl Default for Mutations {
    fn default() -> Self {
        Self::new()
    }
}

impl Mutations {
    /// Creates an empty collector with no pre-allocated capacity.
    pub fn new() -> Self {
        Self {
            mutations: Vec::new(),
            owner_id: next_mutation_owner_id(),
        }
    }

    /// Creates a collector pre-allocated for `capacity` mutations.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            mutations: Vec::with_capacity(capacity),
            owner_id: next_mutation_owner_id(),
        }
    }

    /// Appends a mutation pair.
    pub fn push(&mut self, mutation: Mutation) {
        self.mutations.push(mutation);
    }

    pub(crate) fn append(&mut self, mut other: Self) {
        self.mutations.append(&mut other.mutations);
    }

    pub(crate) fn owner_id(&self) -> u64 {
        self.owner_id
    }

    /// Applies all commit actions in registration order and reports every
    /// failed finalizer.
    #[must_use = "mutation finalizer failures must arm the engine kill switch"]
    pub(crate) fn commit_all(self) -> MutationFailure {
        let mut failure = MutationFailure::default();
        for mut mutation in self.mutations {
            let provenance = mutation.provenance;
            if let Err(details) = mutation.commit() {
                failure.record(provenance, details);
            }
        }
        failure
    }

    /// Applies all rollback actions in reverse registration order.
    #[must_use = "mutation rollback failures must arm the engine kill switch"]
    pub(crate) fn rollback_all(self) -> MutationRollbackResult {
        let mut result = MutationRollbackResult::default();
        for mutation in self.mutations.into_iter().rev() {
            result.append(mutation.rollback());
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{Mutation, MutationFailureScope, Mutations};

    fn noop_action() {}

    fn always_fails() -> bool {
        false
    }

    fn always_succeeds() -> bool {
        true
    }

    #[test]
    fn commit_all_applies_in_registration_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(3);
        for id in ["a", "b", "c"] {
            let c = Rc::clone(&calls);
            mutations.push(Mutation::new(
                move || {
                    c.borrow_mut().push(id);
                },
                noop_action,
            ));
        }

        let failure = mutations.commit_all();
        assert!(!failure.failed());
        assert_eq!(&*calls.borrow(), &["a", "b", "c"]);
    }

    #[test]
    fn commit_all_reports_failure_and_keeps_applying() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(2);
        for (id, succeeds) in [("a", false), ("b", true)] {
            let commit_calls = Rc::clone(&calls);
            mutations.push(Mutation::new_fallible(
                move || {
                    commit_calls.borrow_mut().push(id);
                    succeeds
                },
                always_succeeds,
            ));
        }

        let failure = mutations.commit_all();
        assert_eq!(&*calls.borrow(), &["a", "b"]);
        assert_eq!(failure.scope(), Some(MutationFailureScope::Global));
        assert!(failure.details().contains("commit callback failed"));
    }

    #[test]
    fn engine_owned_finalizer_failure_stays_account_scoped() {
        let mut mutations = Mutations::with_capacity(1);
        mutations.push(Mutation::new_reporting(
            always_fails,
            super::MutationRollbackResult::default,
        ));

        let failure = mutations.commit_all();
        assert_eq!(failure.scope(), Some(MutationFailureScope::Account));
    }

    #[test]
    fn custom_policy_failure_widens_an_engine_owned_one() {
        let mut mutations = Mutations::with_capacity(2);
        mutations.push(Mutation::new_reporting(
            always_fails,
            super::MutationRollbackResult::default,
        ));
        mutations.push(Mutation::new_fallible(always_fails, always_succeeds));

        let failure = mutations.commit_all();
        assert_eq!(failure.scope(), Some(MutationFailureScope::Global));
    }

    #[test]
    fn rollback_all_reports_custom_policy_failure_globally() {
        let mut mutations = Mutations::with_capacity(1);
        mutations.push(Mutation::new_fallible(always_succeeds, always_fails));

        let result = mutations.rollback_all();
        assert!(result.callback_failed());
        assert_eq!(result.failure().scope(), Some(MutationFailureScope::Global));
        assert!(result
            .callback_failure_details()
            .contains("rollback callback failed"));
    }

    #[test]
    fn rollback_all_applies_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(3);
        for id in ["a", "b", "c"] {
            let r = Rc::clone(&calls);
            mutations.push(Mutation::new(noop_action, move || {
                r.borrow_mut().push(id);
            }));
        }

        let _ = mutations.rollback_all();
        assert_eq!(&*calls.borrow(), &["c", "b", "a"]);
    }

    #[test]
    fn default_creates_empty_mutations() {
        let mutations = Mutations::default();
        assert!(mutations.is_empty());
    }

    #[test]
    fn new_creates_empty_mutations() {
        let mutations = Mutations::new();
        assert!(mutations.is_empty());
    }

    #[test]
    fn commit_all_on_empty_is_noop() {
        noop_action();
        assert!(!Mutations::new().commit_all().failed());
    }

    #[test]
    fn rollback_all_on_empty_is_noop() {
        let _ = Mutations::new().rollback_all();
    }
}
