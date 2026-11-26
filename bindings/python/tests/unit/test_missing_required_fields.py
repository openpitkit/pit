# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import conftest
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
@pytest.mark.parametrize("mode", list(openpit.pretrade.RuleMode))
def test_start_pre_trade_pnl_kill_switch_without_operation_rejects(
    mode: openpit.pretrade.RuleMode,
) -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-500"),
                    mode=mode,
                )
            )
        )
        .build()
    )
    order = openpit.Order()
    result = engine.start_pre_trade(order=order)
    assert not result.ok
    assert result.reviews == []
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD
    assert result.rejects[0].mode == openpit.pretrade.RuleMode.REJECT


@pytest.mark.unit
@pytest.mark.parametrize("mode", list(openpit.pretrade.RuleMode))
def test_pnl_kill_switch_execution_report_without_pnl_blocks_account(
    mode: openpit.pretrade.RuleMode,
) -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-500"),
                    mode=mode,
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
    assert len(post.account_blocks) == 1
    assert post.account_blocks[0].code == "MissingRequiredField"
    assert post.account_blocks[0].reason == "failed to access required field 'P&L'"
    assert not engine.start_pre_trade(order=conftest.make_order()).ok


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
                    )
                )
            )
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_notional=openpit.param.Volume("50000"),
                    ),
                    asset=openpit.param.Asset("USD"),
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
