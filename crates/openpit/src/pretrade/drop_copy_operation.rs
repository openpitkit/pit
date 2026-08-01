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

use super::reservation::ReservationHandle;
use super::{AccountBlock, PreTradeLock};
use crate::core::account_outcome::AccountAdjustmentOutcome;

/// Opaque capability object representing applied drop-copy bookkeeping.
///
/// `DropCopyOperation` is the result of successful drop-copy evaluation. It
/// owns the commit/rollback capability for the mutations prepared by policies
/// and carries the [`PreTradeLock`] produced while those mutations were built,
/// exactly like [`PreTradeReservation`](super::PreTradeReservation) does for
/// reserved state. The lock, the finalization contract, and the automatic
/// rollback on drop are the reservation contract; read
/// [`PreTradeReservation`](super::PreTradeReservation) for it, including why
/// the lock must be persisted when later reconciliation depends on the policy
/// context produced while the mutations were built.
///
/// Ordinary policy rejects are non-enforcing during drop copy and are used only
/// to derive policy effects and the optional account block. A fatal policy
/// evaluation reject produces no operation at all.
///
/// Effects that drop copy applies outside that finalization boundary stay
/// applied: deferred account-control operations are published before the
/// operation is returned, and rate-limit attempts are consumed exactly like an
/// ordinary pre-trade attempt.
///
/// # Finalization
///
/// [`commit`](Self::commit) and [`rollback`](Self::rollback) are void, and a
/// mutation finalizer that fails is never ignored: the engine raises a kill
/// switch instead, which the owner meets on its next pre-trade request. See the
/// finalizer contract on [`Mutation`](crate::Mutation).
///
/// # Lifecycle guidance
///
/// - Keep the `DropCopyOperation` alive until the historical order is stored.
/// - Call [`DropCopyOperation::commit`] only after that bookkeeping must become
///   durable engine state.
/// - Call [`DropCopyOperation::rollback`] if storing the historical order fails
///   and the applied state must be reverted immediately.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Price, Quantity, Side};
/// use openpit::{Engine, Instrument, OrderOperation};
/// use openpit::param::TradeAmount;
///
/// use openpit::pretrade::policies::OrderValidationPolicy;
/// let engine = Engine::builder::<OrderOperation, (), ()>()
///     .no_sync()
///     .pre_trade(OrderValidationPolicy::new())
///     .build()?;
/// let order = OrderOperation {
///     instrument: Instrument::new(
///         Asset::new("AAPL")?,
///         Asset::new("USD")?,
///     ),
///     account_id: openpit::param::AccountId::from_u64(99224416),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(
///         Quantity::from_str("10")?
///     ),
///     price: Some(Price::from_str("185")?),
/// };
/// let mut operation = engine.apply_drop_copy(order)?;
/// let lock = operation.lock().clone();
///
/// // Store the historical order. On success commit, on failure rollback.
/// operation.commit(); // or operation.rollback()
///
/// // If later reconciliation needs the policy context, persist `lock`
/// // together with the stored order until the final execution report.
/// let _ = lock;
/// # Ok(())
/// # }
/// ```
pub struct DropCopyOperation {
    account_block: Option<AccountBlock>,
    lock: PreTradeLock,
    account_adjustments: Vec<AccountAdjustmentOutcome>,
    inner: Option<Box<dyn ReservationHandle>>,
    account_blocked: bool,
}

impl std::fmt::Debug for DropCopyOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropCopyOperation").finish_non_exhaustive()
    }
}

impl DropCopyOperation {
    /// Finalizes by applying commit mutations.
    ///
    /// The operation owns its commit/rollback capability exactly once, with
    /// the same contract as
    /// [`PreTradeReservation::commit`](super::PreTradeReservation::commit),
    /// including the kill switch raised for a failed commit callback.
    ///
    /// # Panics
    ///
    /// Panics with `"drop-copy operation already consumed"` when `commit`
    /// is called after any finalization has consumed the operation. This
    /// mirrors [`PreTradeReservation::commit`](super::PreTradeReservation::commit).
    ///
    /// FFI carries a finalized flag and makes repeated pointer-level
    /// finalization a no-op, so bindings cannot reach this panic. Native double
    /// finalization is programmer misuse, so the native API reports it
    /// immediately.
    pub fn commit(&mut self) {
        self.inner
            .take()
            .expect("drop-copy operation already consumed")
            .commit();
    }

    /// Finalizes by applying rollback mutations.
    ///
    /// Rollback is void, and calling it after the operation has
    /// already been finalized is a no-op, with the same contract as
    /// [`PreTradeReservation::rollback`](super::PreTradeReservation::rollback).
    ///
    /// Account-control operations published by the drop-copy pipeline and
    /// consumed rate-limit attempts are outside this boundary and are not
    /// reverted. A rollback callback that fails does not fail this call; it
    /// arms the engine kill switch described by the finalizer contract on
    /// [`Mutation`](crate::Mutation).
    ///
    /// # Panics
    ///
    /// This method does not panic on its own. Panics can only originate from
    /// inside individual rollback mutation closures registered by policies.
    pub fn rollback(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.rollback();
        }
    }

    /// Returns the lock context attached to the operation.
    ///
    /// Persist this value if post-trade reconciliation for the stored order
    /// needs the policy context produced while the mutations were built.
    pub fn lock(&self) -> &PreTradeLock {
        &self.lock
    }

    /// Returns account position modifications grouped by [`super::PolicyGroupId`].
    ///
    /// Contains zero or more entries. Policies that share a group tag contribute
    /// to the same entry; policies that report nothing do not create an entry.
    /// Order within a group follows policy registration order.
    pub fn account_adjustments(&self) -> &[AccountAdjustmentOutcome] {
        &self.account_adjustments
    }

    /// Returns the first account block requested by this policy pipeline.
    ///
    /// This request-local history can differ from the apply-time registry
    /// snapshot. A pre-existing block may remain the stored cause, and a
    /// deferred unblock from this same request may remove the requested block.
    /// Use [`Self::is_account_blocked`] for the snapshot captured before
    /// [`crate::Engine::apply_drop_copy`] returned.
    pub fn account_block(&self) -> Option<&AccountBlock> {
        self.account_block.as_ref()
    }

    /// Returns the apply-time blocked-state snapshot for the order account.
    ///
    /// This is true both when this request established the block and when the
    /// account or its group was already blocked before drop-copy ran. The
    /// snapshot does not track later registry changes.
    pub fn is_account_blocked(&self) -> bool {
        self.account_blocked
    }

    pub(crate) fn from_handle(
        inner: Box<dyn ReservationHandle>,
        lock: PreTradeLock,
        account_adjustments: Vec<AccountAdjustmentOutcome>,
        account_block: Option<AccountBlock>,
        account_blocked: bool,
    ) -> Self {
        Self {
            account_block,
            lock,
            account_adjustments,
            inner: Some(inner),
            account_blocked,
        }
    }
}

impl Drop for DropCopyOperation {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.rollback();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{AccountBlock, DropCopyOperation, PreTradeLock};
    use crate::core::mutation::MutationFailureKillSwitch;
    use crate::core::DEFAULT_POLICY_GROUP_ID;
    use crate::param::Price;
    use crate::pretrade::handle::ReservationHandleImpl;
    use crate::pretrade::RejectCode;
    use crate::{Mutation, Mutations};

    fn noop_action() {}

    fn operation(mutations: Mutations, lock: PreTradeLock) -> DropCopyOperation {
        DropCopyOperation::from_handle(
            Box::new(ReservationHandleImpl::new(
                mutations,
                MutationFailureKillSwitch::inert(),
            )),
            lock,
            Vec::new(),
            None,
            false,
        )
    }

    #[test]
    fn commit_executes_commit_mutations() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(1);
        let commit_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(
            move || {
                commit_calls.borrow_mut().push("commit");
            },
            noop_action,
        ));

        let mut operation = operation(mutations, PreTradeLock::default());
        operation.commit();

        assert_eq!(&*calls.borrow(), &["commit"]);
    }

    #[test]
    fn rollback_executes_rollback_mutations_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(2);
        for id in ["first", "second"] {
            let rollback_calls = Rc::clone(&calls);
            mutations.push(Mutation::new(noop_action, move || {
                rollback_calls.borrow_mut().push(id);
            }));
        }

        let mut operation = operation(mutations, PreTradeLock::default());
        operation.rollback();

        assert_eq!(&*calls.borrow(), &["second", "first"]);
    }

    #[test]
    fn drop_without_explicit_finalize_rolls_back() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(2);
        for id in ["first", "second"] {
            let rollback_calls = Rc::clone(&calls);
            mutations.push(Mutation::new(noop_action, move || {
                rollback_calls.borrow_mut().push(id);
            }));
        }

        drop(operation(mutations, PreTradeLock::default()));

        assert_eq!(&*calls.borrow(), &["second", "first"]);
    }

    #[test]
    fn drop_after_commit_does_not_roll_back() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(1);
        let rollback_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            rollback_calls.borrow_mut().push("rollback");
        }));

        let mut operation = operation(mutations, PreTradeLock::default());
        operation.commit();
        drop(operation);

        assert!(calls.borrow().is_empty());
    }

    #[test]
    #[should_panic(expected = "drop-copy operation already consumed")]
    fn commit_panics_for_finalized_operation() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(1);
        let commit_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(
            move || commit_calls.borrow_mut().push("commit"),
            noop_action,
        ));

        let mut operation = operation(mutations, PreTradeLock::default());
        operation.commit();
        operation.commit();
    }

    #[test]
    fn rollback_is_noop_for_finalized_operation() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut mutations = Mutations::with_capacity(1);
        let rollback_calls = Rc::clone(&calls);
        mutations.push(Mutation::new(noop_action, move || {
            rollback_calls.borrow_mut().push("rollback");
        }));

        let mut operation = operation(mutations, PreTradeLock::default());
        operation.rollback();
        operation.rollback();

        assert_eq!(&*calls.borrow(), &["rollback"]);
    }

    #[test]
    fn accessors_report_the_evaluated_bookkeeping() {
        let price = Price::from_str("185").expect("price must be valid");
        let block = AccountBlock::new(
            "policy",
            RejectCode::PnlKillSwitchTriggered,
            "pnl kill switch triggered",
            "drop-copy block",
        );
        let mut operation = DropCopyOperation::from_handle(
            Box::new(ReservationHandleImpl::new(
                Mutations::new(),
                MutationFailureKillSwitch::inert(),
            )),
            PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, price)]),
            Vec::new(),
            Some(block),
            true,
        );

        assert_eq!(
            operation
                .lock()
                .prices_of(DEFAULT_POLICY_GROUP_ID)
                .collect::<Vec<_>>(),
            vec![price]
        );
        assert!(operation.account_adjustments().is_empty());
        assert_eq!(
            operation
                .account_block()
                .expect("the requested block must be reported")
                .reason,
            "pnl kill switch triggered"
        );
        assert!(operation.is_account_blocked());
        operation.commit();
    }
}
