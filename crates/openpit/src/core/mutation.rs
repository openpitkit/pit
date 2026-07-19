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

#[derive(Default)]
pub(crate) struct MutationRollbackResult {
    #[cfg(test)]
    pub(crate) report: AccountAdjustmentRollbackReport,
}

impl MutationRollbackResult {
    #[cfg(test)]
    fn append(&mut self, mut other: Self) {
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

    #[cfg(not(test))]
    fn append(&mut self, _other: Self) {}
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
/// The engine applies commit actions in registration order on success,
/// and rollback actions in reverse registration order on failure.
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
/// # Examples
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
/// use openpit::Mutation;
///
/// let counter = Rc::new(RefCell::new(0i64));
///
/// let c = Rc::clone(&counter);
/// let r = Rc::clone(&counter);
/// let mutation = Mutation::new(
///     move || { *c.borrow_mut() += 100; },
///     move || { *r.borrow_mut() -= 100; },
/// );
/// ```
pub struct Mutation {
    commit: Box<dyn FnOnce()>,
    rollback: Box<dyn FnOnce() -> MutationRollbackResult>,
    lifetime_guard: Option<Box<dyn Any>>,
}

impl Mutation {
    /// Creates a mutation from commit and rollback closures.
    ///
    /// `commit` runs when the pipeline succeeds (reservation commit or
    /// account-adjustment batch acceptance).
    ///
    /// `rollback` runs when the pipeline fails (policy reject, reservation
    /// rollback, or reservation drop without explicit finalization).
    pub fn new(commit: impl FnOnce() + 'static, rollback: impl FnOnce() + 'static) -> Self {
        Self::new_reporting(commit, move || {
            rollback();
            MutationRollbackResult::default()
        })
    }

    pub(crate) fn new_reporting(
        commit: impl FnOnce() + 'static,
        rollback: impl FnOnce() -> MutationRollbackResult + 'static,
    ) -> Self {
        Self {
            commit: Box::new(commit),
            rollback: Box::new(rollback),
            lifetime_guard: None,
        }
    }

    pub(crate) fn new_reporting_with_guard<Guard>(
        commit: impl FnOnce() + 'static,
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

    fn commit(self) {
        let Self {
            commit,
            rollback,
            lifetime_guard,
        } = self;
        drop(rollback);
        commit();
        drop(lifetime_guard);
    }

    fn rollback(self) -> MutationRollbackResult {
        let Self {
            commit,
            rollback,
            lifetime_guard,
        } = self;
        drop(commit);
        let result = rollback();
        drop(lifetime_guard);
        result
    }
}

/// Collected mutations registered during pre-trade checks.
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
/// let c = Rc::clone(&state);
/// let r = Rc::clone(&state);
/// mutations.push(Mutation::new(
///     move || { *c.borrow_mut() = true; },
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

    pub(crate) fn owner_id(&self) -> u64 {
        self.owner_id
    }

    /// Applies all commit actions in registration order.
    pub(crate) fn commit_all(self) {
        for mutation in self.mutations {
            mutation.commit();
        }
    }

    /// Applies all rollback actions in reverse registration order.
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

    use super::{Mutation, Mutations};

    fn noop_action() {}

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

        mutations.commit_all();
        assert_eq!(&*calls.borrow(), &["a", "b", "c"]);
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
        Mutations::new().commit_all();
    }

    #[test]
    fn rollback_all_on_empty_is_noop() {
        let _ = Mutations::new().rollback_all();
    }
}
