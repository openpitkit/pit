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

use super::lock::PreTradeLock;
use super::reject::Rejects;
use super::request::RequestHandle;
use super::reservation::{PreTradeReservation, ReservationHandle};
use crate::Mutations;
use std::marker::PhantomData;

type RequestExecutor = Box<dyn FnOnce() -> Result<PreTradeReservation, Rejects>>;

pub(crate) struct RequestHandleImpl<O> {
    execute: RequestExecutor,
    marker: PhantomData<fn(O)>,
}

impl<O> RequestHandleImpl<O> {
    pub(crate) fn new(execute: RequestExecutor) -> Self {
        Self {
            execute,
            marker: PhantomData,
        }
    }
}

impl<O> RequestHandle<O> for RequestHandleImpl<O> {
    fn execute(self: Box<Self>) -> Result<PreTradeReservation, Rejects> {
        let this = *self;
        (this.execute)()
    }
}

pub(crate) struct ReservationHandleImpl {
    mutations: Option<Mutations>,
}

impl ReservationHandleImpl {
    pub(crate) fn new(mutations: Mutations) -> Self {
        Self {
            mutations: Some(mutations),
        }
    }
}

impl ReservationHandle for ReservationHandleImpl {
    fn commit(mut self: Box<Self>) {
        if let Some(mutations) = self.mutations.take() {
            mutations.commit_all();
        }
    }

    fn rollback(mut self: Box<Self>) {
        if let Some(mutations) = self.mutations.take() {
            mutations.rollback_all();
        }
    }

    fn lock(&self) -> PreTradeLock {
        PreTradeLock::default()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{RequestHandleImpl, ReservationHandleImpl};
    use crate::pretrade::request::RequestHandle;
    use crate::pretrade::reservation::ReservationHandle;
    use crate::pretrade::{Reject, RejectCode, RejectScope, Rejects};
    use crate::{Mutation, Mutations};

    fn noop_action() {}

    #[test]
    fn commit_calls_commit_closures_in_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        for id in ["a", "b", "c"] {
            let c = Rc::clone(&calls);
            mutations.push(Mutation::new(
                move || {
                    c.borrow_mut().push(id);
                },
                noop_action,
            ));
        }

        let handle = Box::new(ReservationHandleImpl::new(mutations));
        handle.commit();

        assert_eq!(&*calls.borrow(), &["a", "b", "c"]);
    }

    #[test]
    fn rollback_calls_rollback_closures_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::new();
        for id in ["a", "b", "c"] {
            let r = Rc::clone(&calls);
            mutations.push(Mutation::new(noop_action, move || {
                r.borrow_mut().push(id);
            }));
        }

        let handle = Box::new(ReservationHandleImpl::new(mutations));
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

    #[test]
    fn lock_returns_default_lock() {
        noop_action();
        let handle = ReservationHandleImpl::new(Mutations::new());
        assert_eq!(handle.lock(), crate::pretrade::PreTradeLock::default());
    }
}
