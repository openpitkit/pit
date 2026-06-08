# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import openpit
import openpit.pretrade
import pytest


@pytest.mark.unit
def test_start_pre_trade_order_without_operation_produces_missing_field_reject() -> (
    None
):
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )
    order = openpit.Order()
    result = engine.start_pre_trade(order=order)
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD


@pytest.mark.unit
def test_start_pre_trade_pnl_kill_switch_without_operation_rejects() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset="USD",
                    lower_bound=openpit.param.Pnl("-500"),
                )
            )
        )
        .build()
    )
    order = openpit.Order()
    result = engine.start_pre_trade(order=order)
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD


@pytest.mark.unit
def test_apply_execution_report_without_financial_impact_does_not_panic() -> None:
    """
    Engine must not panic when financial_impact group is absent.

    Kill switch must not trigger.
    """
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset="USD",
                    lower_bound=openpit.param.Pnl("-500"),
                )
            )
        )
        .build()
    )
    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            account_id=openpit.param.AccountId.from_int(99224416),
        ),
    )
    post = engine.apply_execution_report(report=report)
    assert post.account_blocks
    assert post.account_blocks[0].code == "MissingRequiredField"


@pytest.mark.unit
def test_start_pre_trade_order_size_limit_without_operation_rejects() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(
                policies.OrderSizeBrokerBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("1000000"),
                        max_notional=openpit.param.Volume("1000000000"),
                    )
                )
            )
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("100"),
                        max_notional=openpit.param.Volume("50000"),
                    ),
                    settlement_asset="USD",
                )
            )
        )
        .build()
    )
    order = openpit.Order()
    result = engine.start_pre_trade(order=order)
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD
