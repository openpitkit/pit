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

//! Policy callback contexts and the per-account block handle.
//!
//! The engine hands these read-only contexts to custom policy callbacks
//! (wired in a later milestone). Each context exposes an `accountGroup`
//! getter, which also satisfies the market-data `accountInfo` shape so a
//! context can be passed straight into a quote read.
//!
//! [`JsAccountControl`] records a kill-switch block against the account bound
//! to a context. It is valid only within the pre-trade transaction that
//! produced it; a `used`/invalidated flag guards against use afterwards so a
//! stale handle fails loudly instead of recording against a finished request.

use std::cell::Cell;
use std::rc::Rc;

use openpit::param::AccountGroupId;
use wasm_bindgen::prelude::*;

use crate::engine::StorageFactory;
use crate::error::{make_error, ErrorKind};
use crate::param::ids::JsAccountGroupId;
use crate::reject::JsAccountBlock;

/// Shared validity flag for every binding handle created for one engine
/// operation.
///
/// JavaScript callbacks may retain a context (or an `AccountControl` obtained
/// from it) after the callback returns.  The core account-control handle is
/// deliberately cloneable, so the binding adds an operation-scoped token that
/// all those clones share.  Request/reservation wrappers keep the token alive
/// through deferred execution and invalidate it exactly when the operation is
/// rejected, committed, rolled back, or dropped.
#[derive(Clone)]
pub(crate) struct LifecycleToken(Rc<Cell<bool>>);

impl LifecycleToken {
    /// Creates a fresh valid token.
    pub(crate) fn new() -> Self {
        Self(Rc::new(Cell::new(true)))
    }

    /// Returns whether handles associated with this operation remain usable.
    fn is_valid(&self) -> bool {
        self.0.get()
    }

    /// Invalidates every context/control clone sharing this token.
    pub(crate) fn invalidate(&self) {
        self.0.set(false);
    }
}

/// Per-account kill-switch block handle scoped to one pre-trade transaction.
///
/// Obtained from a context's `accountControl`. Calling [`block`](Self::block)
/// records a block against the bound account. The handle is valid only within
/// the owning request's pre-trade processing (through commit or rollback); once
/// invalidated, further calls throw a `LifecycleError` rather than recording
/// against a completed transaction.
#[wasm_bindgen(js_name = AccountControl)]
pub struct JsAccountControl {
    inner: openpit::AccountControl<StorageFactory>,
    lifecycle: LifecycleToken,
}

#[wasm_bindgen(js_class = AccountControl)]
impl JsAccountControl {
    /// Records `block` against the account bound to this control.
    ///
    /// The first cause for an account wins, so recording a block against an
    /// already-blocked account is a no-op.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the owning transaction has already been
    /// finalized, or `ParamError` when the block code is not recognized.
    #[wasm_bindgen(js_name = block)]
    pub fn block(&self, block: &JsAccountBlock) -> Result<(), JsValue> {
        if !self.lifecycle.is_valid() {
            return Err(make_error(
                ErrorKind::Lifecycle,
                "account control is no longer valid for its transaction",
                None,
            ));
        }
        let native = block.to_core()?;
        self.inner.block(native);
        Ok(())
    }
}

impl JsAccountControl {
    /// Wraps a core account control handle as a fresh, valid binding handle.
    pub(crate) fn from_inner(
        inner: openpit::AccountControl<StorageFactory>,
        lifecycle: LifecycleToken,
    ) -> Self {
        Self { inner, lifecycle }
    }
}

/// Returns the group id wrapper for an optional core group.
fn group_getter(group: Option<AccountGroupId>) -> Option<JsAccountGroupId> {
    group.map(JsAccountGroupId::from_inner)
}

/// Pre-trade policy context: optional block handle and optional account group.
///
/// `accountControl` is present only when the request carries an account id;
/// `accountGroup` is present only when that account is registered to a group.
#[wasm_bindgen(js_name = Context)]
pub struct JsContext {
    account_control: Option<openpit::AccountControl<StorageFactory>>,
    group: Option<AccountGroupId>,
    lifecycle: LifecycleToken,
}

#[wasm_bindgen(js_class = Context)]
impl JsContext {
    /// The per-account block handle, or `undefined` when the request carries no
    /// account id.
    #[wasm_bindgen(getter, js_name = accountControl)]
    pub fn account_control(&self) -> Option<JsAccountControl> {
        self.account_control
            .as_ref()
            .map(|inner| JsAccountControl::from_inner(inner.clone(), self.lifecycle.clone()))
    }

    /// The account group, or `undefined`. Also serves as the market-data
    /// `accountInfo`.
    #[wasm_bindgen(getter, js_name = accountGroup)]
    pub fn account_group(&self) -> Option<JsAccountGroupId> {
        group_getter(self.group)
    }
}

impl JsContext {
    /// Builds a pre-trade context from its parts.
    pub(crate) fn from_parts(
        account_control: Option<openpit::AccountControl<StorageFactory>>,
        group: Option<AccountGroupId>,
        lifecycle: LifecycleToken,
    ) -> Self {
        Self {
            account_control,
            group,
            lifecycle,
        }
    }
}

/// Account-adjustment policy context: an always-present block handle and an
/// optional account group.
#[wasm_bindgen(js_name = AccountAdjustmentContext)]
pub struct JsAccountAdjustmentContext {
    account_control: openpit::AccountControl<StorageFactory>,
    group: Option<AccountGroupId>,
    lifecycle: LifecycleToken,
}

#[wasm_bindgen(js_class = AccountAdjustmentContext)]
impl JsAccountAdjustmentContext {
    /// The per-account block handle (always present in this context).
    #[wasm_bindgen(getter, js_name = accountControl)]
    pub fn account_control(&self) -> JsAccountControl {
        JsAccountControl::from_inner(self.account_control.clone(), self.lifecycle.clone())
    }

    /// The account group, or `undefined`. Also serves as the market-data
    /// `accountInfo`.
    #[wasm_bindgen(getter, js_name = accountGroup)]
    pub fn account_group(&self) -> Option<JsAccountGroupId> {
        group_getter(self.group)
    }
}

impl JsAccountAdjustmentContext {
    /// Builds an account-adjustment context from its parts.
    pub(crate) fn from_parts(
        account_control: openpit::AccountControl<StorageFactory>,
        group: Option<AccountGroupId>,
        lifecycle: LifecycleToken,
    ) -> Self {
        Self {
            account_control,
            group,
            lifecycle,
        }
    }
}

/// Post-trade policy context: an optional account group only.
#[wasm_bindgen(js_name = PostTradeContext)]
pub struct JsPostTradeContext {
    group: Option<AccountGroupId>,
}

#[wasm_bindgen(js_class = PostTradeContext)]
impl JsPostTradeContext {
    /// The account group, or `undefined`. Also serves as the market-data
    /// `accountInfo`.
    #[wasm_bindgen(getter, js_name = accountGroup)]
    pub fn account_group(&self) -> Option<JsAccountGroupId> {
        group_getter(self.group)
    }
}

impl JsPostTradeContext {
    /// Builds a post-trade context from its account group.
    pub(crate) fn from_group(group: Option<AccountGroupId>) -> Self {
        Self { group }
    }
}
