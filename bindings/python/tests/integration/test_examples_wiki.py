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


import openpit
import pytest

# --- Shared helpers ---


def _aapl_usd_order(quantity: str, price: str) -> openpit.Order:
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(quantity),
            price=openpit.param.Price(price),
        ),
    )


def _aapl_usd_report(pnl: str, fee: str) -> openpit.ExecutionReport:
    return openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
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


class ReserveThenValidatePolicy(openpit.pretrade.PreTradePolicy):
    # @typing.override
    def __init__(self) -> None:
        self._reserved = openpit.param.Volume(0.0)
        self._limit = openpit.param.Volume(50.0)

    # @typing.override
    @property
    def name(self) -> str:
        return "ReserveThenValidatePolicy"

    # @typing.override
    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx
        assert order.operation is not None
        prev_reserved = self._reserved
        next_reserved = openpit.param.Volume(100.0)
        self._reserved = next_reserved
        rollback = openpit.Mutation(
            commit=lambda: None,  # Commit is empty: state was applied eagerly.
            rollback=lambda: setattr(self, "_reserved", prev_reserved),
        )
        if next_reserved > self._limit:
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="temporary reservation exceeds limit",
                        details=(f"reserved {next_reserved}, " f"limit: {self._limit}"),
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ],
                mutations=[rollback],
            )

        return openpit.pretrade.PolicyDecision.accept(mutations=[rollback])

    # @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


# --- Policy-API: Custom Main-Stage Policy ---


class NotionalCapPolicy(openpit.pretrade.PreTradePolicy):
    # @typing.override
    def __init__(self, max_abs_notional: openpit.param.Volume) -> None:
        # Policy-local config: reject any order above this absolute notional.
        self._max_abs_notional = max_abs_notional

    @property
    # @typing.override
    def name(self) -> str:
        return "NotionalCapPolicy"

    # @typing.override
    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx
        assert order.operation is not None

        # Translate the public order surface into one number that this policy
        # can reason about: requested notional.
        trade_amount = order.operation.trade_amount
        if trade_amount.is_volume:
            requested_notional = trade_amount.as_volume
        else:
            assert trade_amount.is_quantity
            assert order.operation.price is not None
            requested_notional = order.operation.price.calculate_volume(
                trade_amount.as_quantity
            )

        if requested_notional > self._max_abs_notional:
            # Business validation failures should become explicit rejects,
            # not exceptions.
            return openpit.pretrade.PolicyDecision.reject(
                rejects=[
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="strategy cap exceeded",
                        details=(
                            "requested notional "
                            f"{requested_notional}, "
                            f"max allowed: {self._max_abs_notional}"
                        ),
                        scope=openpit.pretrade.RejectScope.ORDER,
                    )
                ]
            )

        # This policy only validates. It does not reserve mutable state.
        return openpit.pretrade.PolicyDecision.accept()

    # @typing.override
    def apply_execution_report(
        self,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


# --- Account-Adjustments: CumulativeLimitPolicy ---


class CumulativeLimitPolicy(openpit.AccountAdjustmentPolicy):
    """Tracks cumulative totals per asset, rejects batch on limit breach."""

    def __init__(self, max_cumulative: openpit.param.Volume) -> None:
        self._max = max_cumulative
        self._totals: dict[str, openpit.param.Volume] = {}

    @property
    def name(self) -> str:
        return "CumulativeLimitPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> list[openpit.pretrade.PolicyReject] | tuple[openpit.Mutation, ...] | None:
        del ctx, account_id
        # Use the asset as the aggregation key for the cumulative limit.
        asset_id = adjustment.operation.asset

        prev = self._totals.get(asset_id, openpit.param.Volume("0"))
        # Simplified - real code would add delta to prev.
        new_total = prev

        # Reject if limit breached.
        if new_total > self._max:
            return [
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="cumulative limit exceeded",
                    details=f"{asset_id}: {new_total} > {self._max}",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                )
            ]

        # Apply immediately so later adjustments in the same batch see the updated
        # total.
        self._totals[asset_id] = new_total

        # Rollback by absolute value - safe in account adjustment pipeline
        # because no external system sees intermediate batch state.
        prev_value = prev
        asset_key = asset_id
        return (
            openpit.Mutation(
                # Commit is empty: state was applied eagerly.
                commit=lambda: None,
                rollback=lambda: self._totals.__setitem__(asset_key, prev_value),
            ),
        )


# --- Tests ---


@pytest.mark.integration
def test_example_wiki_domain_types_create_validated_values() -> None:
    # Used in: pit.wiki/Domain-Types.md — Create Validated Values
    from decimal import Decimal

    import openpit

    # Build validated value objects at the integration boundary.
    asset = "AAPL"
    quantity = openpit.param.Quantity("10.5")
    price = openpit.param.Price(185)
    pnl = openpit.param.Pnl(-12.5)

    # Domain types with exact decimal semantics.
    assert asset == "AAPL"
    assert str(quantity) == "10.5"
    assert str(price) == "185"
    assert str(pnl) == "-12.5"
    assert quantity.decimal == Decimal("10.5")
    assert isinstance(price.decimal, Decimal)


@pytest.mark.integration
def test_example_wiki_domain_types_directional_types() -> None:
    # Used in: pit.wiki/Domain-Types.md — Work With Directional Types
    import openpit

    # Directional helpers keep side logic explicit instead of comparing raw strings.
    side = openpit.param.Side.BUY
    position_side = openpit.param.PositionSide.LONG

    assert side.opposite().value == "sell"
    assert side.sign() == 1
    assert position_side.opposite().value == "short"


@pytest.mark.integration
def test_example_wiki_domain_types_leverage() -> None:
    # Used in: pit.wiki/Domain-Types.md — Create Leverage
    import openpit

    # Leverage is a plain multiplier with direct int/float constructors.
    from_multiplier = openpit.param.Leverage(100)
    from_float = openpit.param.Leverage(100.5)

    # Both constructors end up with the same strongly typed leverage wrapper.
    assert from_multiplier.value == 100.0
    assert from_float.value == 100.5


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

    # Start stage returns either a reject or a deferred request handle.
    start_result = engine.start_pre_trade(order=order)
    if not start_result:
        for reject in start_result.rejects:
            print(
                f"rejected by {reject.policy} "
                f"[{reject.code}]: {reject.reason}: {reject.details}"
            )
    else:
        # Keep the request object if later code wants to enter the main stage.
        request = start_result.request
        _ = request


@pytest.mark.integration
def test_example_wiki_pipeline_main_stage_finalize() -> None:
    # Used in: pit.wiki/Pre-trade-Pipeline.md — Execute the Main Stage and Finalize the
    # PreTradeReservation
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy()
        )
        .build()
    )
    order = _aapl_usd_order("100", "185")

    start_result = engine.start_pre_trade(order=order)
    # Main stage consumes the deferred request and returns reservation or rejects.
    execute_result = start_result.request.execute()

    if execute_result:
        # Commit only after the caller knows the reservation should become durable.
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

    # The shortcut runs start stage and main stage as one convenience call.
    execute_result = engine.execute_pre_trade(order=order)
    if execute_result:
        # Finalization is still explicit even when the two stages are composed.
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
    # Build one batch that mixes balance and position adjustments.
    account_id = openpit.param.AccountId.from_u64(99224416)

    adjustments = [
        openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentBalanceOperation(
                asset="USD",
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
                    "SPX",
                    "USD",
                ),
                collateral_asset="USD",
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

    # The engine validates the whole batch atomically.
    engine = openpit.Engine.builder().build()
    result = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert result.ok


@pytest.mark.integration
def test_example_wiki_account_adjustments_cumulative_limit() -> None:
    # Used in: pit.wiki/Account-Adjustments.md — Example: Balance Limit Policy
    policy = CumulativeLimitPolicy(max_cumulative=openpit.param.Volume("1000000"))
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

    adjustments = [
        openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentBalanceOperation(
                asset="USD",
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
    # Used in: pit.wiki/Policy-API.md — Example: Rollback Safety Pattern
    reserve_policy = ReserveThenValidatePolicy()
    engine = openpit.Engine.builder().pre_trade_policy(policy=reserve_policy).build()

    start_result = engine.start_pre_trade(order=_aapl_usd_order("10", "25"))
    assert start_result.ok
    execute_result = start_result.request.execute()
    assert execute_result.ok is False
    assert (
        execute_result.rejects[0].code
        == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    )
    assert reserve_policy._reserved == openpit.param.Volume("0")


@pytest.mark.integration
def test_example_wiki_policy_notional_cap() -> None:
    # Used in: pit.wiki/Policy-API.md — Example: Custom Main-Stage Policy
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


# --- Policy-API: Custom Order and Execution Report Models ---


class StrategyOrder(openpit.Order):
    def __init__(
        self,
        *,
        operation: openpit.OrderOperation,
        strategy_tag: str,
    ) -> None:
        super().__init__(operation=operation)
        self.strategy_tag = strategy_tag


class StrategyReport(openpit.ExecutionReport):
    def __init__(
        self,
        *,
        operation: openpit.ExecutionReportOperation,
        financial_impact: openpit.FinancialImpact,
        venue_exec_id: str,
    ) -> None:
        super().__init__(operation=operation, financial_impact=financial_impact)
        self.venue_exec_id = venue_exec_id


class StrategyTagPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    @property
    def name(self) -> str:
        return "StrategyTagPolicy"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> list[openpit.pretrade.PolicyReject]:
        import typing

        strategy_order = typing.cast(StrategyOrder, order)
        if strategy_order.strategy_tag == "blocked":
            return [
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
                    reason="strategy blocked",
                    details=(
                        f"strategy tag {strategy_order.strategy_tag!r} is not allowed"
                    ),
                    scope=openpit.pretrade.RejectScope.ORDER,
                )
            ]
        return []

    def apply_execution_report(self, report: openpit.ExecutionReport) -> bool:
        _ = report
        return False


def _make_strategy_order(strategy_tag: str) -> StrategyOrder:
    return StrategyOrder(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_u64(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(10),
            price=openpit.param.Price(25),
        ),
        strategy_tag=strategy_tag,
    )


@pytest.mark.integration
def test_example_wiki_custom_python_models() -> None:
    # Used in: pit.wiki/Policy-API.md — Example: Python Custom Models
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=StrategyTagPolicy())
        .build()
    )

    # Allowed order must pass both stages.
    order = _make_strategy_order("alpha")
    start_result = engine.start_pre_trade(order=order)
    assert start_result.ok
    assert start_result.request is not None

    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.commit()

    # Blocked order must be rejected at the start stage.
    blocked = _make_strategy_order("blocked")
    blocked_start = engine.start_pre_trade(order=blocked)
    assert not blocked_start.ok
    assert len(blocked_start.rejects) == 1
    assert (
        blocked_start.rejects[0].code
        == openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION
    )
    assert blocked_start.rejects[0].policy == "StrategyTagPolicy"
