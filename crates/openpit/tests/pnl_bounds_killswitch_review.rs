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

use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount};
use openpit::pretrade::policies::{
    PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy,
    PnlBoundsKillSwitchSettings,
};
use openpit::pretrade::{RejectCode, RejectScope, RuleMode};
use openpit::storage::NoLocking;
use openpit::{
    Engine, ExecutionReportOperation, FinancialImpact, HasAccountId, HasFee, HasInstrument, HasPnl,
    Instrument, OrderOperation, OrderReview, RequestFieldAccessError, WithExecutionReportOperation,
    WithFinancialImpact, WithOrderOperation, WithOrderReview,
};
use rust_decimal::Decimal;

type Order = WithOrderOperation<WithOrderReview<()>>;
type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;
type TestPnlPolicy = PnlBoundsKillSwitchPolicy<NoLocking>;

fn account_id() -> AccountId {
    AccountId::from_u64(99224416)
}

fn barrier(settlement_asset: Asset, lower_bound: &str, mode: RuleMode) -> PnlBoundsBrokerBarrier {
    PnlBoundsBrokerBarrier {
        settlement_asset,
        lower_bound: Some(pnl(lower_bound)),
        upper_bound: None,
        mode,
    }
}

fn pnl(value: &str) -> Pnl {
    Pnl::from_str(value).unwrap()
}

fn order(confirmed: bool) -> Order {
    WithOrderOperation {
        operation: OrderOperation {
            instrument: Instrument::new(Asset::new("AAPL").unwrap(), Asset::new("USD").unwrap()),
            account_id: account_id(),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("1").unwrap()),
            price: Some(Price::from_str("100").unwrap()),
        },
        inner: WithOrderReview {
            inner: (),
            review: OrderReview {
                confirmed,
                already_held: false,
            },
        },
    }
}

fn execution_report_spx_usd(pnl_value: &str) -> Report {
    WithExecutionReportOperation {
        operation: ExecutionReportOperation {
            instrument: Instrument::new(Asset::new("SPX").unwrap(), Asset::new("USD").unwrap()),
            account_id: account_id(),
            side: Side::Buy,
        },
        inner: WithFinancialImpact {
            inner: (),
            financial_impact: FinancialImpact {
                pnl: pnl(pnl_value),
                fee: Fee::ZERO,
            },
        },
    }
}

#[test]
fn pnl_both_review_modes_coexist_with_a_reject_barrier() {
    let builder = Engine::builder::<Order, Report, ()>()
        .with_order_review()
        .no_sync();
    let barriers = [
        RuleMode::Reject,
        RuleMode::ReviewWithHold,
        RuleMode::ReviewWithoutHold,
    ]
    .into_iter()
    .map(|mode| {
        barrier(
            Asset::new("USD").unwrap(),
            if mode == RuleMode::Reject {
                "-100"
            } else {
                "1"
            },
            mode,
        )
    });
    let policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(barriers, []).unwrap(),
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build().unwrap();
    let request = engine.start_pre_trade(order(false)).unwrap();
    assert_eq!(
        request.reviews().len(),
        2,
        "both review modes must remain visible"
    );
    assert_eq!(request.reviews()[0].mode, RuleMode::ReviewWithHold);
    assert_eq!(request.reviews()[1].mode, RuleMode::ReviewWithoutHold);
    drop(request);
    engine
        .configure()
        .set_account_pnl(
            TestPnlPolicy::NAME,
            account_id(),
            Asset::new("USD").unwrap(),
            pnl("-101"),
        )
        .unwrap();
    let rejects = engine
        .start_pre_trade(order(false))
        .expect_err("reject mode must still reject");
    assert_eq!(
        rejects.len(),
        1,
        "effective reject must suppress review marks"
    );
    assert_eq!(rejects[0].mode, RuleMode::Reject);
    assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
}

#[test]
fn pnl_confirmed_keeps_reject_barrier_beside_review() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        for account_axis in [false, true] {
            let builder = Engine::builder::<Order, Report, ()>()
                .with_order_review()
                .no_sync();
            let settlement_asset = Asset::new("USD").unwrap();
            let barriers = [
                barrier(settlement_asset.clone(), "-100", mode),
                barrier(settlement_asset.clone(), "-200", RuleMode::Reject),
            ];
            let settings = if account_axis {
                PnlBoundsKillSwitchSettings::new(
                    [],
                    barriers.map(|barrier| PnlBoundsAccountAssetBarrier {
                        barrier,
                        account_id: account_id(),
                        initial_pnl: Pnl::ZERO,
                    }),
                )
            } else {
                PnlBoundsKillSwitchSettings::new(barriers, [])
            }
            .unwrap();
            let policy = PnlBoundsKillSwitchPolicy::new(settings, builder.storage_builder());
            let engine = builder.pre_trade(policy).build().unwrap();
            assert!(engine
                .apply_execution_report(&execution_report_spx_usd("-150"))
                .account_blocks
                .is_empty());

            let request = engine.start_pre_trade(order(false)).unwrap();
            assert_eq!(request.reviews().len(), 1);
            assert_eq!(request.reviews()[0].mode, mode);
            assert_eq!(
                request.reviews()[0].code,
                RejectCode::PnlKillSwitchTriggered
            );
            assert_eq!(request.reviews()[0].scope, RejectScope::Account);
            drop(request);
            let request = engine.start_pre_trade(order(true)).unwrap();
            assert!(
                request.reviews().is_empty(),
                "confirmed drops the breached review barrier mark"
            );
            drop(request);

            engine
                .configure()
                .set_account_pnl(
                    TestPnlPolicy::NAME,
                    account_id(),
                    settlement_asset.clone(),
                    pnl("-250"),
                )
                .unwrap();
            let rejects = engine.start_pre_trade(order(true)).unwrap_err();
            assert_eq!(rejects.len(), 1);
            assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
            assert_eq!(rejects[0].mode, RuleMode::Reject);
            assert_eq!(rejects[0].scope, RejectScope::Account);
            engine
                .configure()
                .set_account_pnl(
                    TestPnlPolicy::NAME,
                    account_id(),
                    settlement_asset,
                    Pnl::ZERO,
                )
                .unwrap();
            let rejects = engine.start_pre_trade(order(true)).unwrap_err();
            assert_eq!(rejects.len(), 1);
            assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
        }
    }
}

#[test]
fn pnl_review_pretrade_confirmed_rerun() {
    for mode in [
        RuleMode::Reject,
        RuleMode::ReviewWithHold,
        RuleMode::ReviewWithoutHold,
    ] {
        for account_axis in [false, true] {
            let builder = Engine::builder::<Order, Report, ()>()
                .with_order_review()
                .no_sync();
            let account_id = account_id();
            let settlement_asset = Asset::new("USD").unwrap();
            let barrier = barrier(settlement_asset.clone(), "1", mode);
            let settings = if account_axis {
                PnlBoundsKillSwitchSettings::new(
                    [],
                    [PnlBoundsAccountAssetBarrier {
                        barrier,
                        account_id,
                        initial_pnl: Pnl::ZERO,
                    }],
                )
            } else {
                PnlBoundsKillSwitchSettings::new([barrier], [])
            }
            .unwrap();
            let policy = PnlBoundsKillSwitchPolicy::new(settings, builder.storage_builder());
            let engine = builder.pre_trade(policy).build().unwrap();

            if mode == RuleMode::Reject {
                let rejects = engine
                    .start_pre_trade(order(true))
                    .expect_err("Reject barrier must reject a confirmed order");
                assert_eq!(rejects.len(), 1);
                assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
                assert_eq!(rejects[0].scope, RejectScope::Account);
                assert_eq!(rejects[0].mode, RuleMode::Reject);

                engine
                    .configure()
                    .set_account_pnl(TestPnlPolicy::NAME, account_id, settlement_asset, pnl("2"))
                    .unwrap();
                let rejects = engine
                    .start_pre_trade(order(true))
                    .expect_err("Reject breach must latch an account block after P&L recovers");
                assert_eq!(rejects.len(), 1);
                assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
                assert_eq!(rejects[0].scope, RejectScope::Account);
            } else {
                let mut order = order(false);
                let request = engine
                    .start_pre_trade(order.clone())
                    .expect("unconfirmed order must go to review");
                assert_eq!(request.reviews().len(), 1);
                let mark = &request.reviews()[0];
                assert_eq!(mark.mode, mode);
                assert_eq!(mark.code, RejectCode::PnlKillSwitchTriggered);
                assert_eq!(mark.scope, RejectScope::Account);
                drop(request);

                order.inner.review.confirmed = true;
                let request = engine
                    .start_pre_trade(order)
                    .expect("confirmed order must pass after a review breach");
                assert!(request.reviews().is_empty());
            }
        }
    }
}

#[test]
fn pnl_review_evaluation_failures_reject_live_and_abort_drop_copy() {
    #[derive(Clone)]
    struct MissingInstrument;

    impl HasAccountId for MissingInstrument {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Ok(account_id())
        }
    }

    impl HasInstrument for MissingInstrument {
        fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("instrument"))
        }
    }

    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        let builder = Engine::builder::<WithOrderReview<MissingInstrument>, Report, ()>()
            .with_order_review()
            .no_sync();
        let barrier = barrier(Asset::new("USD").unwrap(), "1", mode);
        let policy = PnlBoundsKillSwitchPolicy::new(
            PnlBoundsKillSwitchSettings::new([barrier], []).unwrap(),
            builder.storage_builder(),
        );
        let engine = builder.pre_trade(policy).build().unwrap();
        let order = WithOrderReview {
            inner: MissingInstrument,
            review: OrderReview {
                confirmed: true,
                already_held: false,
            },
        };
        let live = engine
            .start_pre_trade(order.clone())
            .expect_err("evaluation failure must reject a confirmed order");
        let Err(drop_copy) = engine.apply_drop_copy(order) else {
            panic!("evaluation failure must abort drop copy")
        };
        for rejects in [live, drop_copy] {
            assert_eq!(rejects.len(), 1);
            assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
            assert_eq!(rejects[0].mode, RuleMode::Reject);
            assert_eq!(rejects[0].scope, RejectScope::Order);
            assert_eq!(
                rejects[0].reason,
                "failed to access required field 'instrument'"
            );
        }
    }
}

#[test]
fn pnl_review_report_breach_does_not_block_but_overflow_does() {
    for mode in [
        RuleMode::Reject,
        RuleMode::ReviewWithHold,
        RuleMode::ReviewWithoutHold,
    ] {
        let builder = Engine::builder::<Order, Report, ()>()
            .with_order_review()
            .no_sync();
        let barrier = barrier(Asset::new("USD").unwrap(), "-100", mode);
        let policy = PnlBoundsKillSwitchPolicy::new(
            PnlBoundsKillSwitchSettings::new([barrier], []).unwrap(),
            builder.storage_builder(),
        );
        let engine = builder.pre_trade(policy).build().unwrap();
        let result = engine.apply_execution_report(&execution_report_spx_usd("-101"));
        assert_eq!(
            result.account_blocks.len(),
            usize::from(mode == RuleMode::Reject),
            "review execution report breach must never block"
        );
        engine
            .configure()
            .set_account_pnl(
                TestPnlPolicy::NAME,
                account_id(),
                Asset::new("USD").unwrap(),
                pnl("0"),
            )
            .unwrap();
        let result = engine.start_pre_trade(order(true));
        if mode == RuleMode::Reject {
            let rejects = result.expect_err("execution report block must survive P&L recovery");
            assert_eq!(rejects.len(), 1);
            assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
        } else {
            drop(result.expect("review execution report must not block"));
            let mut report = execution_report_spx_usd("0");
            report.inner.financial_impact.pnl = Pnl::new(Decimal::MAX);
            assert!(engine
                .apply_execution_report(&report)
                .account_blocks
                .is_empty());
            let result = engine.apply_execution_report(&report);
            assert_eq!(result.account_blocks.len(), 1);
            assert_eq!(
                result.account_blocks[0].code,
                RejectCode::OrderValueCalculationFailed
            );
            let rejects = engine
                .start_pre_trade(order(true))
                .expect_err("overflow must latch an account block with review-only barriers");
            assert_eq!(rejects.len(), 1);
            assert_eq!(rejects[0].code, RejectCode::OrderValueCalculationFailed);
        }
    }
}

#[test]
fn pnl_missing_report_field_blocks_with_review_only_barriers() {
    struct NoPnl;
    impl HasPnl for NoPnl {
        fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
            Err(RequestFieldAccessError::new("pnl"))
        }
    }
    impl HasFee for NoPnl {
        fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
            Ok(Fee::ZERO)
        }
    }
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        let builder = Engine::builder::<Order, WithExecutionReportOperation<NoPnl>, ()>()
            .with_order_review()
            .no_sync();
        let barrier = barrier(Asset::new("USD").unwrap(), "-100", mode);
        let policy = PnlBoundsKillSwitchPolicy::new(
            PnlBoundsKillSwitchSettings::new([barrier], []).unwrap(),
            builder.storage_builder(),
        );
        let engine = builder.pre_trade(policy).build().unwrap();
        let result = engine.apply_execution_report(&WithExecutionReportOperation {
            inner: NoPnl,
            operation: ExecutionReportOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").unwrap(),
                    Asset::new("USD").unwrap(),
                ),
                account_id: account_id(),
                side: Side::Buy,
            },
        });
        assert_eq!(result.account_blocks.len(), 1);
        assert_eq!(
            result.account_blocks[0].code,
            RejectCode::MissingRequiredField
        );
        let rejects = engine
            .start_pre_trade(order(true))
            .expect_err("missing report P&L must latch an account block even in review mode");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
    }
}
