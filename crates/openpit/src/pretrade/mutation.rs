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

/// Commit/rollback pair produced by a policy.
///
/// Commit/rollback action pair registered by a policy during checks.
///
/// The engine applies commit actions in registration order on success,
/// and rollback actions in reverse registration order on failure.
///
/// # Rollback safety by pipeline
///
/// Account adjustment pipeline: rollback by absolute value is safe.
/// The entire batch runs within a single engine borrow. No external
/// system observes intermediate state, so restoring a previous absolute
/// value is always consistent.
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
/// use openpit::pretrade::Mutation;
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
    rollback: Box<dyn FnOnce()>,
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
        Self {
            commit: Box::new(commit),
            rollback: Box::new(rollback),
        }
    }
}

/// Collected mutations registered during pre-trade checks.
///
/// # Examples
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
/// use openpit::pretrade::{Mutation, Mutations};
///
/// let state = Rc::new(RefCell::new(false));
/// let mut mutations = Mutations::new();
///
/// let c = Rc::clone(&state);
/// let r = Rc::clone(&state);
/// mutations.push(Mutation::new(
///     move || { *c.borrow_mut() = true; },
///     move || { *r.borrow_mut() = false; },
/// ));
/// ```
#[derive(Default)]
pub struct Mutations {
    mutations: Vec<Mutation>,
}

impl Mutations {
    /// Creates an empty collector.
    pub fn new() -> Self {
        Self {
            mutations: Vec::new(),
        }
    }

    /// Appends a mutation pair.
    pub fn push(&mut self, mutation: Mutation) {
        self.mutations.push(mutation);
    }

    /// Applies all commit actions in registration order.
    pub(crate) fn commit_all(self) {
        for mutation in self.mutations {
            (mutation.commit)();
        }
    }

    /// Applies all rollback actions in reverse registration order.
    pub(crate) fn rollback_all(self) {
        for mutation in self.mutations.into_iter().rev() {
            (mutation.rollback)();
        }
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

    #[test]
    fn commit_all_applies_in_registration_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        for id in ["a", "b", "c"] {
            let c = Rc::clone(&calls);
            mutations.push(Mutation::new(
                move || {
                    c.borrow_mut().push(id);
                },
                || {},
            ));
        }

        mutations.commit_all();
        assert_eq!(&*calls.borrow(), &["a", "b", "c"]);
    }

    #[test]
    fn rollback_all_applies_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        for id in ["a", "b", "c"] {
            let r = Rc::clone(&calls);
            mutations.push(Mutation::new(
                || {},
                move || {
                    r.borrow_mut().push(id);
                },
            ));
        }

        mutations.rollback_all();
        assert_eq!(&*calls.borrow(), &["c", "b", "a"]);
    }

    #[test]
    fn default_creates_empty_mutations() {
        let mutations = Mutations::default();
        assert!(mutations.is_empty());
    }

    #[test]
    fn commit_all_on_empty_is_noop() {
        Mutations::new().commit_all();
    }

    #[test]
    fn rollback_all_on_empty_is_noop() {
        Mutations::new().rollback_all();
    }
}
