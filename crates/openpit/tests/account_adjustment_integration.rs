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

use std::cell::RefCell;
use std::rc::Rc;

use openpit::param::AccountId;
use openpit::pretrade::{
    AccountAdjustmentPolicy, Mutation, Mutations, Reject, RejectCode, RejectScope,
};
use openpit::{AccountAdjustmentBalanceOperation, Engine, HasBalanceAsset};

type TestAdjustment = AccountAdjustmentBalanceOperation;
type TestEngine = Engine<(), (), TestAdjustment>;
type EngineWithRecorders = (
    TestEngine,
    Rc<RefCell<Vec<AccountId>>>,
    Rc<RefCell<Vec<String>>>,
);
type RollbackEngine = (
    TestEngine,
    Rc<RefCell<Vec<String>>>,
    Rc<RefCell<Vec<String>>>,
);

// Policy that records every call. Does not push mutations.
struct RecordingAdjustmentPolicy {
    name: &'static str,
    seen_account_ids: Rc<RefCell<Vec<AccountId>>>,
    seen_asset_codes: Rc<RefCell<Vec<String>>>,
    reject_on_asset: Option<String>,
}

impl AccountAdjustmentPolicy<TestAdjustment> for RecordingAdjustmentPolicy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn apply_account_adjustment(
        &self,
        account_id: AccountId,
        adjustment: &TestAdjustment,
        _mutations: &mut Mutations,
    ) -> Result<(), Reject> {
        self.seen_account_ids.borrow_mut().push(account_id);
        let asset_code = adjustment
            .balance_asset()
            .expect("balance_asset must be accessible")
            .to_string();
        self.seen_asset_codes.borrow_mut().push(asset_code.clone());
        if self.reject_on_asset.as_deref() == Some(&asset_code) {
            return Err(Reject::new(
                self.name,
                RejectScope::Order,
                RejectCode::Other,
                "test reject",
                format!("asset {} blocked by test policy", asset_code),
            ));
        }
        Ok(())
    }
}

// Policy that applies state immediately and registers commit/rollback closures.
// committed_assets reflects the post-commit state: populated on success, empty after rollback.
// rollback_order records each asset name in the order its rollback closure ran,
// allowing tests to verify both that rollback occurred and that it ran in reverse.
struct MutatingRecordingPolicy {
    name: &'static str,
    committed_assets: Rc<RefCell<Vec<String>>>,
    rollback_order: Rc<RefCell<Vec<String>>>,
    reject_on_asset: Option<String>,
}

impl AccountAdjustmentPolicy<TestAdjustment> for MutatingRecordingPolicy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn apply_account_adjustment(
        &self,
        _account_id: AccountId,
        adjustment: &TestAdjustment,
        mutations: &mut Mutations,
    ) -> Result<(), Reject> {
        let asset = adjustment
            .balance_asset()
            .expect("balance_asset must be accessible")
            .to_string();

        if self.reject_on_asset.as_deref() == Some(&asset) {
            return Err(Reject::new(
                self.name,
                RejectScope::Order,
                RejectCode::Other,
                "test reject",
                format!("asset {} blocked by test policy", asset),
            ));
        }

        // Apply immediately. Safe because account adjustments run within a single
        // engine borrow and no external system sees intermediate batch state.
        self.committed_assets.borrow_mut().push(asset.clone());

        let committed = Rc::clone(&self.committed_assets);
        let rollback_order = Rc::clone(&self.rollback_order);
        mutations.push(Mutation::new(
            || {},
            move || {
                committed.borrow_mut().pop();
                rollback_order.borrow_mut().push(asset);
            },
        ));

        Ok(())
    }
}

fn balance_adjustment(asset_code: &str) -> AccountAdjustmentBalanceOperation {
    AccountAdjustmentBalanceOperation {
        asset: openpit::param::Asset::new(asset_code).expect("asset must be valid"),
        average_entry_price: None,
    }
}

fn make_engine(reject_on_asset: Option<&str>) -> EngineWithRecorders {
    let seen_ids = Rc::new(RefCell::new(Vec::new()));
    let seen_assets = Rc::new(RefCell::new(Vec::new()));
    let engine = Engine::<(), (), TestAdjustment>::builder()
        .account_adjustment_policy(RecordingAdjustmentPolicy {
            name: "RecordingAdjustmentPolicy",
            seen_account_ids: Rc::clone(&seen_ids),
            seen_asset_codes: Rc::clone(&seen_assets),
            reject_on_asset: reject_on_asset.map(str::to_owned),
        })
        .build()
        .expect("engine must build");
    (engine, seen_ids, seen_assets)
}

fn make_rollback_engine(reject_on_asset: Option<&str>) -> RollbackEngine {
    let committed = Rc::new(RefCell::new(Vec::new()));
    let rollback_order = Rc::new(RefCell::new(Vec::new()));
    let engine = Engine::<(), (), TestAdjustment>::builder()
        .account_adjustment_policy(MutatingRecordingPolicy {
            name: "MutatingRecordingPolicy",
            committed_assets: Rc::clone(&committed),
            rollback_order: Rc::clone(&rollback_order),
            reject_on_asset: reject_on_asset.map(str::to_owned),
        })
        .build()
        .expect("engine must build");
    (engine, committed, rollback_order)
}

#[test]
fn account_adjustment_integration_successful_batch() {
    let account_id = AccountId::from_u64(99224416);
    let (engine, seen_ids, seen_assets) = make_engine(None);

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    let result = engine.apply_account_adjustment(account_id, &adjustments);

    assert!(result.is_ok());
    assert_eq!(
        *seen_ids.borrow(),
        vec![
            AccountId::from_u64(99224416),
            AccountId::from_u64(99224416),
            AccountId::from_u64(99224416),
        ]
    );
    assert_eq!(*seen_assets.borrow(), vec!["USD", "EUR", "GBP"]);
}

#[test]
fn account_adjustment_integration_reject_on_first() {
    let account_id = AccountId::from_u64(99224416);
    let (engine, _seen_ids, seen_assets) = make_engine(Some("USD"));

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    let result = engine.apply_account_adjustment(account_id, &adjustments);

    let error = result.expect_err("must reject");
    assert_eq!(error.index, 0);
    assert_eq!(*seen_assets.borrow(), vec!["USD"]);
}

#[test]
fn account_adjustment_integration_reject_on_last() {
    let account_id = AccountId::from_u64(99224416);
    let (engine, _seen_ids, seen_assets) = make_engine(Some("GBP"));

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    let result = engine.apply_account_adjustment(account_id, &adjustments);

    let error = result.expect_err("must reject");
    assert_eq!(error.index, 2);
    assert_eq!(*seen_assets.borrow(), vec!["USD", "EUR", "GBP"]);
}

#[test]
fn account_adjustment_integration_reject_on_middle() {
    // engine stops on first reject; GBP must not be seen
    let account_id = AccountId::from_u64(99224416);
    let (engine, _seen_ids, seen_assets) = make_engine(Some("EUR"));

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    let result = engine.apply_account_adjustment(account_id, &adjustments);

    let error = result.expect_err("must reject");
    assert_eq!(error.index, 1);
    assert_eq!(*seen_assets.borrow(), vec!["USD", "EUR"]);
}

#[test]
fn account_adjustment_integration_rollback_commits_on_success() {
    let account_id = AccountId::from_u64(99224416);
    let (engine, committed, rollback_order) = make_rollback_engine(None);

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    assert!(engine
        .apply_account_adjustment(account_id, &adjustments)
        .is_ok());

    assert_eq!(*committed.borrow(), vec!["USD", "EUR", "GBP"]);
    // no rollbacks on success
    assert!(rollback_order.borrow().is_empty());
}

#[test]
fn account_adjustment_integration_rollback_reverts_on_reject_first() {
    // USD is rejected before any mutation is registered, so rollback_order stays empty.
    // This distinguishes "nothing was applied" from "something was applied then rolled back".
    let account_id = AccountId::from_u64(99224416);
    let (engine, committed, rollback_order) = make_rollback_engine(Some("USD"));

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    assert!(engine
        .apply_account_adjustment(account_id, &adjustments)
        .is_err());

    assert!(committed.borrow().is_empty());
    assert!(rollback_order.borrow().is_empty());
}

#[test]
fn account_adjustment_integration_rollback_reverts_on_reject_last() {
    // USD and EUR were applied; GBP rejected. Rollback must undo EUR then USD (reverse order).
    let account_id = AccountId::from_u64(99224416);
    let (engine, committed, rollback_order) = make_rollback_engine(Some("GBP"));

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    assert!(engine
        .apply_account_adjustment(account_id, &adjustments)
        .is_err());

    assert!(committed.borrow().is_empty());
    assert_eq!(*rollback_order.borrow(), vec!["EUR", "USD"]);
}

#[test]
fn account_adjustment_integration_rollback_reverts_on_reject_middle() {
    // USD was applied; EUR rejected. Rollback must undo USD. GBP was never seen.
    let account_id = AccountId::from_u64(99224416);
    let (engine, committed, rollback_order) = make_rollback_engine(Some("EUR"));

    let adjustments = [
        balance_adjustment("USD"),
        balance_adjustment("EUR"),
        balance_adjustment("GBP"),
    ];
    assert!(engine
        .apply_account_adjustment(account_id, &adjustments)
        .is_err());

    assert!(committed.borrow().is_empty());
    assert_eq!(*rollback_order.borrow(), vec!["USD"]);
}
