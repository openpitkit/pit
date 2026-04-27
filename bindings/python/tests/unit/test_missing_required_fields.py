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
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy()
        )
        .build()
    )
    order = openpit.Order()
    result = engine.start_pre_trade(order=order)
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD


@pytest.mark.unit
def test_start_pre_trade_pnl_kill_switch_without_operation_rejects() -> None:
    policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset="USD",
        barrier=openpit.param.Pnl("500"),
    )
    engine = (
        openpit.Engine.builder().check_pre_trade_start_policy(policy=policy).build()
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
    policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset="USD",
        barrier=openpit.param.Pnl("500"),
    )
    engine = (
        openpit.Engine.builder().check_pre_trade_start_policy(policy=policy).build()
    )
    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            side=openpit.param.Side.BUY,
            account_id=openpit.param.AccountId.from_u64(99224416),
        ),
    )
    post = engine.apply_execution_report(report=report)
    assert not post.kill_switch_triggered


@pytest.mark.unit
def test_start_pre_trade_order_size_limit_without_operation_rejects() -> None:
    limit = openpit.pretrade.policies.OrderSizeLimit(
        settlement_asset="USD",
        max_quantity=openpit.param.Quantity("100"),
        max_notional=openpit.param.Volume("50000"),
    )
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderSizeLimitPolicy(limit=limit)
        )
        .build()
    )
    order = openpit.Order()
    result = engine.start_pre_trade(order=order)
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD
