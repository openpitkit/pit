# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://github.com/openpitkit and the OWNERS file for details.

import typing

import openpit
import pytest

# --- Shared helpers ---


def _aapl_usd_order(quantity: str, price: str) -> openpit.Order:
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Quantity(quantity),
            price=openpit.param.Price(price),
        ),
    )


def _aapl_usd_report(pnl: str, fee: str) -> openpit.ExecutionReport:
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl(pnl),
            fee=openpit.param.Fee(fee),
        ),
    )


# --- Policy-API: Rollback Safety Pattern ---


class ReservePolicy(openpit.pretrade.Policy):
    @typing.override
    def __init__(self) -> None:
        self._reserved = openpit.param.Volume("0")

    @property
    @typing.override
    def name(self) -> str:
        return "ReservePolicy"

    @typing.override
    def perform_pre_trade_check(
        self,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        assert context.order.operation is not None
        prev_reserved = self._reserved
        self._reserved = openpit.param.Volume("100")
        return openpit.pretrade.PolicyDecision.accept(
            mutations=[
                openpit.pretrade.Mutation(
                    commit=lambda: None,  # state already applied
                    rollback=lambda: setattr(self, "_reserved", prev_reserved),
                )
            ]
        )

    @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


class RejectingPolicy(openpit.pretrade.Policy):
    @property
    @typing.override
    def name(self) -> str:
        return "RejectingPolicy"

    @typing.override
    def perform_pre_trade_check(
        self,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        _ = context
        return openpit.pretrade.PolicyDecision.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="forced reject",
                    details="demonstrates rollback when a later policy fails",
                    scope=openpit.pretrade.RejectScope.ORDER,
                )
            ]
        )

    @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


# --- Policy-API: Custom Main-Stage Policy ---


class NotionalCapPolicy(openpit.pretrade.Policy):
    @typing.override
    def __init__(self, max_abs_notional: openpit.param.Volume) -> None:
        self._max_abs_notional = max_abs_notional

    @property
    @typing.override
    def name(self) -> str:
        return "NotionalCapPolicy"

    @typing.override
    def perform_pre_trade_check(
        self,
        context: openpit.pretrade.PolicyContext,
    ) -> openpit.pretrade.PolicyDecision:
        assert context.order.operation is not None
        trade_amount = context.order.operation.trade_amount
        if isinstance(trade_amount, openpit.param.Volume):
            requested_notional = trade_amount
        else:
            assert isinstance(trade_amount, openpit.param.Quantity)
            assert context.order.operation.price is not None
            requested_notional = context.order.operation.price.calculate_volume(
                trade_amount
            )

        if requested_notional > self._max_abs_notional:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="strategy cap exceeded",
                        details=(
                            "requested notional "
                            f"{requested_notional.value}, "
                            f"max allowed: {self._max_abs_notional.value}"
                        ),
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )

        return openpit.pretrade.PolicyDecision.accept()

    @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


# --- Account-Adjustments: CumulativeLimitPolicy ---


class CumulativeLimitPolicy(openpit.pretrade.AccountAdjustmentPolicy):
    """Tracks cumulative totals per asset, rejects batch on limit breach."""

    def __init__(self, max_cumulative: openpit.param.Volume) -> None:
        self._max = max_cumulative
        self._totals: dict[str, openpit.param.Volume] = {}

    @property
    def name(self) -> str:
        return "CumulativeLimitPolicy"

    def apply_account_adjustment(
        self,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyReject | tuple[openpit.pretrade.Mutation, ...] | None:
        _ = account_id
        asset_id = adjustment.operation.asset.value

        prev = self._totals.get(asset_id, openpit.param.Volume("0"))
        # Simplified - real code would add delta to prev.
        new_total = prev

        # Reject if limit breached.
        if new_total > self._max:
            return openpit.pretrade.PolicyReject(
                code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                reason="cumulative limit exceeded",
                details=f"{asset_id}: {new_total.value} > {self._max.value}",
                scope=openpit.pretrade.RejectScope.ACCOUNT,
            )

        # Apply immediately and register rollback.
        self._totals[asset_id] = new_total

        # Rollback by absolute value - safe in account adjustment pipeline
        # because no external system sees intermediate batch state.
        prev_value = prev
        asset_key = asset_id
        return (
            openpit.pretrade.Mutation(
                commit=lambda: None,  # state already applied
                rollback=lambda: self._totals.__setitem__(asset_key, prev_value),
            ),
        )


# --- Tests ---


@pytest.mark.integration
def test_example_wiki_domain_types_create_validated_values() -> None:
    # Used in: pit.wiki/Domain-Types.md — Create Validated Values
    import openpit

    asset = openpit.param.Asset("AAPL")
    quantity = openpit.param.Quantity("10.5")
    price = openpit.param.Price(185)
    pnl = openpit.param.Pnl(-12.5)

    assert asset.value == "AAPL"
    assert quantity.value == "10.5"
    assert price.value == "185"
    assert pnl.value == "-12.5"


@pytest.mark.integration
def test_example_wiki_domain_types_directional_types() -> None:
    # Used in: pit.wiki/Domain-Types.md — Work With Directional Types
    import openpit

    side = openpit.param.Side.BUY
    position_side = openpit.param.PositionSide.LONG

    assert side.opposite().value == "sell"
    assert side.sign() == 1
    assert position_side.opposite().value == "short"


@pytest.mark.integration
def test_example_wiki_domain_types_leverage() -> None:
    # Used in: pit.wiki/Domain-Types.md — Create Leverage
    import openpit

    from_multiplier = openpit.param.Leverage.from_u16(100)
    from_float = openpit.param.Leverage.from_f64(100.5)

    assert from_multiplier.value == 100.0
    assert from_float.value == 100.5


@pytest.mark.integration
def test_example_wiki_getting_started() -> None:
    # Used in: pit.wiki/Getting-Started.md — Build an Engine + Run an Order Through the
    # Engine + Apply Post-Trade Feedback
    import openpit

    pnl_policy = openpit.pretrade.policies.PnlKillSwitchPolicy(
        settlement_asset=openpit.param.Asset("USD"),
        barrier=openpit.param.Pnl("1000"),
    )

    rate_limit_policy = openpit.pretrade.policies.RateLimitPolicy(
        max_orders=100,
        window_seconds=1,
    )

    order_size_policy = openpit.pretrade.policies.OrderSizeLimitPolicy(
        limit=openpit.pretrade.policies.OrderSizeLimit(
            settlement_asset=openpit.param.Asset("USD"),
            max_quantity=openpit.param.Quantity("500"),
            max_notional=openpit.param.Volume("100000"),
        )
    )

    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .check_pre_trade_start_policy(policy=pnl_policy)
        .check_pre_trade_start_policy(policy=rate_limit_policy)
        .check_pre_trade_start_policy(policy=order_size_policy)
        .build()
    )

    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.Quantity("100"),
            price=openpit.param.Price("185"),
        ),
    )

    start_result = engine.start_pre_trade(order=order)
    if not start_result:
        reject = start_result.reject
        raise RuntimeError(
            f"{reject.policy} [{reject.code}]: {reject.reason}: {reject.details}"
        )

    execute_result = start_result.request.execute()
    if not execute_result:
        messages = ", ".join(
            f"{reject.policy} [{reject.code}]: {reject.reason}: {reject.details}"
            for reject in execute_result.rejects
        )
        raise RuntimeError(messages)

    reservation = execute_result.reservation
    reservation.commit()

    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                openpit.param.Asset("AAPL"),
                openpit.param.Asset("USD"),
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl("-50"),
            fee=openpit.param.Fee("3"),
        ),
    )

    result = engine.apply_execution_report(report=report)
    if result.kill_switch_triggered:
        print("halt new orders until the blocked state is cleared")


@pytest.mark.integration
def test_example_wiki_pipeline_start_stage_reject() -> None:
    # Used in: pit.wiki/Pre-trade-Pipeline.md — Handle a Start-Stage Reject
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy()
        )
        .build()
    )
    order = _aapl_usd_order("100", "185")

    start_result = engine.start_pre_trade(order=order)
    if not start_result:
        reject = start_result.reject
        print(
            f"rejected by {reject.policy} "
            f"[{reject.code}]: {reject.reason}: {reject.details}"
        )
    else:
        request = start_result.request
        _ = request


@pytest.mark.integration
def test_example_wiki_pipeline_main_stage_finalize() -> None:
    # Used in: pit.wiki/Pre-trade-Pipeline.md — Execute the Main Stage and Finalize the
    # Reservation
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy()
        )
        .build()
    )
    order = _aapl_usd_order("100", "185")

    start_result = engine.start_pre_trade(order=order)
    execute_result = start_result.request.execute()

    if execute_result:
        execute_result.reservation.commit()
    else:
        for reject in execute_result.rejects:
            print(
                f"rejected by {reject.policy} "
                f"[{reject.code}]: {reject.reason}: {reject.details}"
            )


@pytest.mark.integration
def test_example_wiki_pipeline_shortcut_start_and_main() -> None:
    # Used in: pit.wiki/Pre-trade-Pipeline.md — Shortcut for Start + Main Stages
    # Used in: pit.wiki/Getting-Started.md — Shortcut for Start + Main Stages
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy()
        )
        .build()
    )
    order = _aapl_usd_order("100", "185")

    execute_result = engine.execute_pre_trade(order=order)
    if execute_result:
        execute_result.reservation.commit()
    else:
        for reject in execute_result.rejects:
            print(
                f"rejected by {reject.policy} "
                f"[{reject.code}]: {reject.reason}: {reject.details}"
            )


@pytest.mark.integration
def test_example_wiki_account_adjustments() -> None:
    # Used in: pit.wiki/Account-Adjustments.md — Examples → Python
    account_id = openpit.param.AccountId.from_u64(99224416)

    adjustments = [
        openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentBalanceOperation(
                asset=openpit.param.Asset("USD"),
            ),
            amount=openpit.AccountAdjustmentAmount(
                total=openpit.param.AdjustmentAmount.absolute(
                    openpit.param.PositionSize(10000)
                )
            ),
        ),
        openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentPositionOperation(
                instrument=openpit.Instrument(
                    openpit.param.Asset("SPX"),
                    openpit.param.Asset("USD"),
                ),
                collateral_asset=openpit.param.Asset("USD"),
                average_entry_price=openpit.param.Price(95000),
                mode=openpit.param.PositionMode.HEDGED,
            ),
            amount=openpit.AccountAdjustmentAmount(
                total=openpit.param.AdjustmentAmount.absolute(
                    openpit.param.PositionSize(-3)
                )
            ),
        ),
    ]

    engine = openpit.Engine.builder().build()
    result = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert result.ok


@pytest.mark.integration
def test_example_wiki_account_adjustments_cumulative_limit() -> None:
    # Used in: pit.wiki/Account-Adjustments.md — CumulativeLimitPolicy → Python
    policy = CumulativeLimitPolicy(max_cumulative=openpit.param.Volume("1000000"))
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

    adjustments = [
        openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentBalanceOperation(
                asset=openpit.param.Asset("USD"),
            ),
            amount=openpit.AccountAdjustmentAmount(
                total=openpit.param.AdjustmentAmount.absolute(
                    openpit.param.PositionSize(100)
                )
            ),
        ),
    ]

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_u64(99224416),
        adjustments=adjustments,
    )
    assert result.ok


@pytest.mark.integration
def test_example_wiki_policy_rollback_safety() -> None:
    # Used in: pit.wiki/Policy-API.md — Rollback Safety Pattern → Python
    reserve_policy = ReservePolicy()
    engine = (
        openpit.Engine.builder()
        .pre_trade_policy(policy=reserve_policy)
        .pre_trade_policy(policy=RejectingPolicy())
        .build()
    )

    start_result = engine.start_pre_trade(order=_aapl_usd_order("10", "25"))
    assert start_result.ok
    execute_result = start_result.request.execute()
    assert execute_result.ok is False
    assert (
        execute_result.rejects[0].code
        == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    )


@pytest.mark.integration
def test_example_wiki_policy_notional_cap() -> None:
    # Used in: pit.wiki/Policy-API.md — Custom Main-Stage Policy → Python
    engine = (
        openpit.Engine.builder()
        .pre_trade_policy(
            policy=NotionalCapPolicy(
                max_abs_notional=openpit.param.Volume("1000"),
            )
        )
        .build()
    )

    start_result = engine.start_pre_trade(order=_aapl_usd_order("10", "25"))
    assert start_result.ok

    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.commit()

    blocked_result = engine.start_pre_trade(order=_aapl_usd_order("100", "25"))
    assert blocked_result.ok

    blocked_execute_result = blocked_result.request.execute()
    assert blocked_execute_result.ok is False
    assert (
        blocked_execute_result.rejects[0].code
        == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    )
