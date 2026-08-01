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

use super::reject::Rejects;
use super::request::RequestHandle;
use super::reservation::{PreTradeReservation, ReservationHandle};
use crate::core::mutation::MutationFailureKillSwitch;
use crate::Mutations;
use std::marker::PhantomData;

type RequestExecutor = Box<dyn FnOnce() -> Result<PreTradeReservation, Rejects>>;

pub(crate) struct RequestHandleImpl<Order> {
    execute: RequestExecutor,
    marker: PhantomData<fn(Order)>,
}

impl<Order> RequestHandleImpl<Order> {
    pub(crate) fn new(execute: RequestExecutor) -> Self {
        Self {
            execute,
            marker: PhantomData,
        }
    }
}

impl<Order> RequestHandle<Order> for RequestHandleImpl<Order> {
    fn execute(self: Box<Self>) -> Result<PreTradeReservation, Rejects> {
        let this = *self;
        (this.execute)()
    }
}

/// Owner-driven finalization of one prepared mutation batch.
///
/// Finalization is void for the owner, so a failed finalizer cannot be
/// reported back to it. The kill switch is that report: it arms the engine's
/// blocked set with the reach the failing mutation's provenance implies. See
/// the finalizer contract on [`Mutation`](crate::Mutation).
pub(crate) struct ReservationHandleImpl {
    mutations: Mutations,
    kill_switch: MutationFailureKillSwitch,
}

impl ReservationHandleImpl {
    pub(crate) fn new(mutations: Mutations, kill_switch: MutationFailureKillSwitch) -> Self {
        Self {
            mutations,
            kill_switch,
        }
    }
}

impl ReservationHandle for ReservationHandleImpl {
    fn commit(self: Box<Self>) {
        let Self {
            mutations,
            kill_switch,
        } = *self;
        kill_switch.arm(&mutations.commit_all());
    }

    fn rollback(self: Box<Self>) {
        let Self {
            mutations,
            kill_switch,
        } = *self;
        kill_switch.arm(mutations.rollback_all().failure());
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{MutationFailureKillSwitch, RequestHandleImpl, ReservationHandleImpl};
    use crate::core::mutation::MutationFailureScope;
    use crate::pretrade::request::RequestHandle;
    use crate::pretrade::reservation::ReservationHandle;
    use crate::pretrade::{Reject, RejectCode, RejectScope, Rejects};
    use crate::{Mutation, Mutations};

    fn noop_action() {}

    fn always_fails() -> bool {
        false
    }

    fn always_succeeds() -> bool {
        true
    }

    type ScopeSink = Rc<RefCell<Vec<MutationFailureScope>>>;

    fn recording_kill_switch() -> (ScopeSink, MutationFailureKillSwitch) {
        let sink: ScopeSink = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&sink);
        (
            sink,
            MutationFailureKillSwitch::new(move |scope| recorded.borrow_mut().push(scope)),
        )
    }

    #[test]
    fn commit_arms_the_kill_switch_for_a_failed_finalizer() {
        let (sink, kill_switch) = recording_kill_switch();
        let mut mutations = Mutations::with_capacity(1);
        mutations.push(Mutation::new_fallible(always_fails, always_succeeds));

        Box::new(ReservationHandleImpl::new(mutations, kill_switch)).commit();

        assert_eq!(&*sink.borrow(), &[MutationFailureScope::Global]);
    }

    #[test]
    fn rollback_arms_the_kill_switch_for_a_failed_finalizer() {
        let (sink, kill_switch) = recording_kill_switch();
        let mut mutations = Mutations::with_capacity(1);
        mutations.push(Mutation::new_fallible(always_succeeds, always_fails));

        Box::new(ReservationHandleImpl::new(mutations, kill_switch)).rollback();

        assert_eq!(&*sink.borrow(), &[MutationFailureScope::Global]);
    }

    #[test]
    fn a_clean_batch_leaves_the_kill_switch_alone() {
        let (sink, kill_switch) = recording_kill_switch();
        let mut mutations = Mutations::with_capacity(1);
        mutations.push(Mutation::new(noop_action, noop_action));

        Box::new(ReservationHandleImpl::new(mutations, kill_switch)).commit();

        assert!(sink.borrow().is_empty());
    }

    #[test]
    fn commit_calls_commit_closures_in_order() {
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

        let handle = Box::new(ReservationHandleImpl::new(
            mutations,
            MutationFailureKillSwitch::inert(),
        ));
        handle.commit();

        assert_eq!(&*calls.borrow(), &["a", "b", "c"]);
    }

    #[test]
    fn rollback_calls_rollback_closures_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(3);
        for id in ["a", "b", "c"] {
            let r = Rc::clone(&calls);
            mutations.push(Mutation::new(noop_action, move || {
                r.borrow_mut().push(id);
            }));
        }

        let handle = Box::new(ReservationHandleImpl::new(
            mutations,
            MutationFailureKillSwitch::inert(),
        ));
        handle.rollback();

        assert_eq!(&*calls.borrow(), &["c", "b", "a"]);
    }

    #[test]
    fn request_handle_execute_calls_executor() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = Rc::clone(&called);
        let handle: Box<RequestHandleImpl<()>> =
            Box::new(RequestHandleImpl::new(Box::new(move || {
                *called_clone.borrow_mut() = true;
                Err(Rejects::new(vec![Reject::new(
                    "test",
                    RejectScope::Order,
                    RejectCode::Other,
                    "expected",
                    "expected execute error",
                )]))
            })));

        let result = handle.execute();
        assert!(result.is_err());
        assert!(*called.borrow());
    }
}
