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
# Please see https://openpit.dev and the OWNERS file for details.


import openpit
import pytest

# Mirrors public Python examples from:
# - ../pit.wiki/Account-Adjustments.md
# - ../pit.wiki/Account-Blocking.md
# - ../pit.wiki/Account-Groups.md
# - ../pit.wiki/Balance-Reconciliation.md
# - ../pit.wiki/Domain-Types.md
# - ../pit.wiki/Dynamic-Policy-Reconfiguration.md
# - ../pit.wiki/Getting-Started.md
# - ../pit.wiki/Non-Mutating-Dry-Run.md
# - ../pit.wiki/Policies.md
# - ../pit.wiki/Policy-API.md
# - ../pit.wiki/Pre-trade-Pipeline.md
# - ../pit.wiki/Pre-Trade-Lock.md
# - ../pit.wiki/Spot-Funds.md
# If this file changes, update every linked documentation snippet.

# --- Shared helpers ---


def _aapl_usd_order(quantity: str, price: str) -> openpit.Order:
    return openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument(
                "AAPL",
                "USD",
            ),
            account_id=openpit.param.AccountId.from_int(99224416),
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
            account_id=openpit.param.AccountId.from_int(99224416),
            side=openpit.param.Side.BUY,
        ),
        financial_impact=openpit.FinancialImpact(
            pnl=openpit.param.Pnl(pnl),
            fee=openpit.param.Fee(fee),
        ),
    )


# --- Policy-API: Rollback Safety Pattern ---


class ReserveThenValidatePolicy(openpit.pretrade.Policy):
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
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
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
            return openpit.pretrade.PolicyPreTradeResult.reject(
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

        return openpit.pretrade.PolicyPreTradeResult.accept(mutations=[rollback])

    # @typing.override
    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> openpit.pretrade.PostTradeResult | None:
        _ = ctx, report
        return None


# --- Policy-API: Custom Main-Stage Check ---


class NotionalCapPolicy(openpit.pretrade.Policy):
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
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
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
            return openpit.pretrade.PolicyPreTradeResult.reject(
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
        return openpit.pretrade.PolicyPreTradeResult.accept()

    # @typing.override
    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> openpit.pretrade.PostTradeResult | None:
        _ = ctx, report
        return None


# --- Account-Adjustments: CumulativeLimitPolicy ---


class CumulativeLimitPolicy(openpit.pretrade.Policy):
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


# --- Policy-API: Block an Account from an Adjustment Callback ---


class BlockOnAdjustmentPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "BlockOnAdjustmentPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> None:
        del account_id, adjustment
        # The adjustment context always exposes the account-block facility.
        control: openpit.AccountControl = ctx.account_control
        control.block(
            openpit.pretrade.AccountBlock(
                policy=self.name,
                code=openpit.pretrade.RejectCode.ACCOUNT_BLOCKED,
                reason="blocked via account_control",
                details="custom policy blocked the account from a callback",
            )
        )
        return None


# --- Tests ---


@pytest.mark.integration
def test_example_wiki_domain_types_create_validated_values() -> None:
    # Used in: pit.wiki/Domain-Types.md - Create Validated Values
    from decimal import Decimal

    import openpit

    # Build validated value objects at the integration boundary.
    asset = openpit.param.Asset("AAPL")
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
    # Used in: pit.wiki/Domain-Types.md - Work With Directional Types
    import openpit

    # Directional helpers keep side logic explicit instead of comparing raw strings.
    side = openpit.param.Side.BUY
    position_side = openpit.param.PositionSide.LONG

    assert side.opposite().value == "SELL"
    assert side.sign() == 1
    assert position_side.opposite().value == "SHORT"


@pytest.mark.integration
def test_example_wiki_domain_types_leverage() -> None:
    # Used in: pit.wiki/Domain-Types.md - Create Leverage
    import openpit

    # Leverage is a plain multiplier with direct int/float constructors.
    from_multiplier = openpit.param.Leverage(100)
    from_float = openpit.param.Leverage(100.5)

    # Both constructors end up with the same strongly typed leverage wrapper.
    assert from_multiplier.value == 100.0
    assert from_float.value == 100.5


@pytest.mark.integration
def test_example_wiki_pipeline_start_stage_reject() -> None:
    # Used in: pit.wiki/Pre-trade-Pipeline.md - Handle a Start-Stage Reject
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
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
    # Used in: pit.wiki/Pre-trade-Pipeline.md - Execute the Main Stage and Finalize the
    # Reservation
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
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
    # Used in: pit.wiki/Pre-trade-Pipeline.md - Shortcut for Start + Main Stages
    # Used in: pit.wiki/Getting-Started.md - Shortcut for Start + Main Stages
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
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
    # Used in: pit.wiki/Account-Adjustments.md - Examples → Python
    # Build one batch that mixes balance and position adjustments.
    account_id = openpit.param.AccountId.from_int(99224416)

    adjustments = [
        openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentBalanceOperation(
                asset="USD",
            ),
            amount=openpit.AccountAdjustmentAmount(
                balance=openpit.param.AdjustmentAmount.absolute(
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
                balance=openpit.param.AdjustmentAmount.absolute(
                    openpit.param.PositionSize(-3)
                )
            ),
        ),
    ]

    # The engine validates the whole batch atomically.
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )
    result = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert result.ok


@pytest.mark.integration
def test_example_wiki_account_adjustments_cumulative_limit() -> None:
    # Used in: pit.wiki/Account-Adjustments.md - Example: Balance Limit Policy
    policy = CumulativeLimitPolicy(max_cumulative=openpit.param.Volume("1000000"))
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    adjustments = [
        openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentBalanceOperation(
                asset="USD",
            ),
            amount=openpit.AccountAdjustmentAmount(
                balance=openpit.param.AdjustmentAmount.absolute(
                    openpit.param.PositionSize(100)
                )
            ),
        ),
    ]

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_int(99224416),
        adjustments=adjustments,
    )
    assert result.ok


@pytest.mark.integration
def test_example_wiki_account_control_block() -> None:
    # Used in: pit.wiki/Policy-API.md - Block an Account from an Adjustment Callback
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=BlockOnAdjustmentPolicy())
        .build()
    )

    # Driving an adjustment triggers the block.
    engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_int(99224416),
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD")
            )
        ],
    )

    # A later order on the same account is rejected with ACCOUNT_BLOCKED, without
    # any start-check involvement.
    blocked = engine.start_pre_trade(
        order=openpit.Order(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument("AAPL", "USD"),
                account_id=openpit.param.AccountId.from_int(99224416),
                side=openpit.param.Side.BUY,
                trade_amount=openpit.param.TradeAmount.quantity(10),
                price=openpit.param.Price(25),
            ),
        )
    )
    assert not blocked.ok
    assert blocked.rejects[0].code == openpit.pretrade.RejectCode.ACCOUNT_BLOCKED


@pytest.mark.integration
def test_example_wiki_policy_rollback_safety() -> None:
    # Used in: pit.wiki/Policy-API.md - Example: Rollback Safety Pattern
    reserve_policy = ReserveThenValidatePolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=reserve_policy).build()

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
    # Used in: pit.wiki/Policy-API.md - Example: Custom Main-Stage Check
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(
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


class StrategyTagPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "StrategyTagPolicy"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
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


def _make_strategy_order(strategy_tag: str) -> StrategyOrder:
    return StrategyOrder(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_int(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(10),
            price=openpit.param.Price(25),
        ),
        strategy_tag=strategy_tag,
    )


@pytest.mark.integration
def test_example_wiki_custom_python_models() -> None:
    # Used in: pit.wiki/Policy-API.md - Example: Python Custom Models
    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=StrategyTagPolicy()).build()
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


@pytest.mark.integration
def test_example_wiki_policies_order_validation() -> None:
    # Used in: pit.wiki/Policies.md - OrderValidationPolicy
    # Keep this example in sync with the matching wiki example.
    import openpit
    import openpit.pretrade.policies

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_order_validation(),
        )
        .build()
    )

    order = _aapl_usd_order("100", "185")
    start_result = engine.start_pre_trade(order=order)
    assert start_result.ok
    start_result.request.execute().reservation.commit()


@pytest.mark.integration
def test_example_wiki_policies_rate_limit() -> None:
    # Used in: pit.wiki/Policies.md - RateLimitPolicy
    # Keep this example in sync with the matching wiki example.
    import datetime

    import openpit
    import openpit.pretrade.policies

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_rate_limit().broker_barrier(
                openpit.pretrade.policies.RateLimitBrokerBarrier(
                    limit=openpit.pretrade.policies.RateLimit(
                        max_orders=100,
                        window=datetime.timedelta(seconds=1),
                    ),
                ),
            )
        )
        .build()
    )

    order = _aapl_usd_order("1", "100")
    start_result = engine.start_pre_trade(order=order)
    assert start_result.ok
    start_result.request.execute().reservation.commit()


@pytest.mark.integration
def test_example_wiki_policies_order_size_limit() -> None:
    # Used in: pit.wiki/Policies.md - OrderSizeLimitPolicy
    # Keep this example in sync with the matching wiki example.
    import openpit
    import openpit.pretrade.policies

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_order_size_limit()
            .asset_barriers(
                openpit.pretrade.policies.OrderSizeAssetBarrier(
                    limit=openpit.pretrade.policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity(100),
                        max_notional=openpit.param.Volume(50000),
                    ),
                    settlement_asset="USD",
                ),
            )
            .broker_barrier(
                openpit.pretrade.policies.OrderSizeBrokerBarrier(
                    limit=openpit.pretrade.policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity(10000),
                        max_notional=openpit.param.Volume(5000000),
                    ),
                ),
            )
        )
        .build()
    )

    order = _aapl_usd_order("10", "100")
    start_result = engine.start_pre_trade(order=order)
    assert start_result.ok
    start_result.request.execute().reservation.commit()


@pytest.mark.integration
def test_example_wiki_pipeline_apply_post_trade_feedback() -> None:
    # Used in: pit.wiki/Pre-trade-Pipeline.md - Apply Post-Trade Feedback
    # Used in: pit.wiki/Getting-Started.md - Apply Post-Trade Feedback
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )
    report = _aapl_usd_report("-50", "3.4")

    # Execution reports feed realized outcomes back into cumulative policy state.
    result = engine.apply_execution_report(report=report)
    if result.account_blocks:
        print("halt new orders until the blocked state is cleared")

    assert not result.account_blocks


@pytest.mark.integration
def test_example_wiki_getting_started_run_order() -> None:
    # Used in: pit.wiki/Getting-Started.md - Run an Order Through the Engine
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )
    order = _aapl_usd_order("100", "185")

    start_result = engine.start_pre_trade(order=order)
    assert start_result.ok

    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.commit()


@pytest.mark.integration
def test_example_wiki_policies_pnl_bounds_killswitch() -> None:
    # Used in: pit.wiki/Policies.md - PnlBoundsKillSwitchPolicy
    # Keep this example in sync with the matching wiki example.
    import openpit
    import openpit.pretrade.policies

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_pnl_bounds_killswitch().broker_barriers(
                openpit.pretrade.policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl(-1000),
                    upper_bound=openpit.param.Pnl(500),
                ),
            )
        )
        .build()
    )

    order = _aapl_usd_order("1", "100")
    start_result = engine.start_pre_trade(order=order)
    assert start_result.ok
    start_result.request.execute().reservation.commit()


@pytest.mark.integration
def test_example_wiki_policies_spot_funds() -> None:
    # Used in: pit.wiki/Policies.md - SpotFundsPolicy → Python
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
    )
    assert engine is not None


@pytest.mark.integration
def test_example_wiki_spot_funds_limit_only() -> None:
    # Used in: pit.wiki/Spot-Funds.md - Limit-Only Mode → Python
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
    )

    account_id = openpit.param.AccountId.from_int(99224416)

    # Seed 10000 USD of available funds through the account-adjustment pipeline.
    seed = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(10000)
            )
        ),
    )
    seed_result = engine.apply_account_adjustment(
        account_id=account_id, adjustments=[seed]
    )
    assert seed_result.ok

    # Buy 10 AAPL @ 200 holds 2000 USD; available drops to 8000.
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("200"),
        ),
    )
    result = engine.execute_pre_trade(order=order)
    assert result.ok
    result.reservation.commit()


@pytest.mark.integration
def test_example_wiki_spot_funds_pnl_kill_switch_builder() -> None:
    # Used in: pit.wiki/Spot-Funds.md - Self-Computed PnL Kill Switch /
    # Configuring Barriers → Python
    account_id = openpit.param.AccountId.from_int(99224416)

    # The PnL kill switch is a distinct spot-funds builder entry point; it
    # produces the same SpotFundsPolicy, registered under the same name.
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_spot_funds_pnl_bounds_killswitch()
            .global_barriers(
                openpit.pretrade.policies.SpotFundsPnlBoundsBarrier(
                    account_currency=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl(-1000),
                ),
            )
            .account_barriers(
                openpit.pretrade.policies.SpotFundsPnlBoundsAccountBarrier(
                    account_id=account_id,
                    barrier=openpit.pretrade.policies.SpotFundsPnlBoundsBarrier(
                        account_currency=openpit.param.Asset("USD"),
                        lower_bound=openpit.param.Pnl(-250),
                    ),
                    initial_pnl=openpit.param.Pnl(0),
                ),
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.integration
def test_example_wiki_spot_funds_pnl_kill_switch_reconfigure() -> None:
    # Used in: pit.wiki/Spot-Funds.md - Self-Computed PnL Kill Switch /
    # Runtime Reconfiguration → Python
    # Harness scaffolding: a spot-funds engine with a per-account barrier the
    # snippet then retunes and force-sets.
    seed_account = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_spot_funds_pnl_bounds_killswitch().account_barriers(
                openpit.pretrade.policies.SpotFundsPnlBoundsAccountBarrier(
                    account_id=seed_account,
                    barrier=openpit.pretrade.policies.SpotFundsPnlBoundsBarrier(
                        account_currency=openpit.param.Asset("USD"),
                        lower_bound=openpit.param.Pnl(-250),
                    ),
                    initial_pnl=openpit.param.Pnl(0),
                ),
            )
        )
        .build()
    )

    account_id = openpit.param.AccountId.from_int(99224416)

    # Retune the account-currency PnL barriers; live accumulated PnL is untouched.
    engine.configure().spot_funds_pnl_bounds_killswitch(
        openpit.pretrade.policies.SpotFundsPnlBoundsKillswitchBuilder.NAME,
        global_barriers=[
            openpit.pretrade.policies.SpotFundsPnlBoundsBarrier(
                account_currency=openpit.param.Asset("USD"),
                lower_bound=openpit.param.Pnl(-500),
            ),
        ],
    )

    # Force-set the live accumulated PnL for one (account, account currency).
    engine.configure().set_spot_funds_account_pnl(
        openpit.pretrade.policies.SpotFundsPnlBoundsKillswitchBuilder.NAME,
        account=account_id,
        account_currency=openpit.param.Asset("USD"),
        pnl=openpit.param.Pnl(-120),
    )


@pytest.mark.integration
def test_example_wiki_spot_funds_market_orders() -> None:
    # Used in: pit.wiki/Spot-Funds.md - Market Orders → Python
    builder = openpit.Engine.builder().no_sync()

    # A shared market-data service feeds the policy's market-order pricing.
    market_data = builder.market_data(openpit.marketdata.QuoteTtl.infinite()).build()
    aapl = openpit.Instrument("AAPL", "USD")
    aapl_id = market_data.register(aapl)
    market_data.push(aapl_id, openpit.marketdata.Quote(mark="200"))

    # Spot funds with market orders enabled at 1500 bps worst-case slippage.
    engine = builder.builtin(
        openpit.pretrade.policies.build_spot_funds().market_data(
            market_data,
            global_slippage_bps=1500,
            pricing_source=openpit.pretrade.policies.SpotFundsPricingSource.MARK,
        )
    ).build()

    account_id = openpit.param.AccountId.from_int(99224416)
    seed = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(10000)
            )
        ),
    )
    engine.apply_account_adjustment(account_id=account_id, adjustments=[seed])

    # Market buy (no price): priced at mark 200 + 15% = 230 per unit worst case.
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=aapl,
            account_id=account_id,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity("5"),
            price=None,
        ),
    )
    result = engine.execute_pre_trade(order=order)
    assert result.ok
    result.reservation.commit()


@pytest.mark.integration
def test_example_wiki_pre_trade_lock_persistence() -> None:
    # Used in: pit.wiki/Pre-Trade-Lock.md - Persisting and Restoring a Lock → Python
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
    )

    account_id = openpit.param.AccountId.from_int(99224416)

    # Seed 10000 USD so the buy can be reserved.
    seed = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(10000)
            )
        ),
    )
    engine.apply_account_adjustment(account_id=account_id, adjustments=[seed])

    # Buy 10 AAPL @ 200 holds 2000 USD and records the lock price (200).
    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("200"),
        ),
    )
    result = engine.execute_pre_trade(order=order)

    # Persist the lock with its built-in JSON serialization before committing.
    payload = result.reservation.lock().to_json()
    result.reservation.commit()

    # --- After a process restart, rebuild the lock from your store. ---
    restored = openpit.pretrade.Lock.from_json(payload)

    # The final fill must carry the restored lock so the policy reconciles the
    # 2000 USD it held against the real fill instead of blocking the account.
    report = openpit.ExecutionReport(
        operation=openpit.ExecutionReportOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.BUY,
        ),
        fill=openpit.ExecutionReportFillDetails(
            last_trade=openpit.param.Trade(
                price=openpit.param.Price("200"),
                quantity=openpit.param.Quantity("10"),
            ),
            leaves_quantity=openpit.param.Quantity("0"),
            lock=restored,
            is_final=True,
        ),
    )
    post = engine.apply_execution_report(report=report)
    assert not post.account_blocks


@pytest.mark.integration
def test_example_wiki_balance_reconciliation_delta_absolute() -> None:
    # Used in: pit.wiki/Balance-Reconciliation.md - Delta Versus Absolute → Python
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
    )
    account_id = openpit.param.AccountId.from_int(99224416)

    def seed(amount: int) -> openpit.AccountAdjustment:
        return openpit.AccountAdjustment(
            operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
            amount=openpit.AccountAdjustmentAmount(
                balance=openpit.param.AdjustmentAmount.absolute(
                    openpit.param.PositionSize(amount)
                )
            ),
        )

    # First seed: available USD goes from 0 to 10000.
    first = engine.apply_account_adjustment(
        account_id=account_id, adjustments=[seed(10000)]
    )
    assert first.ok
    usd = first.outcomes[0].entry.balance
    assert usd.delta == openpit.param.PositionSize(10000)
    assert usd.absolute == openpit.param.PositionSize(10000)

    # Second seed: available USD goes from 10000 to 15000.
    second = engine.apply_account_adjustment(
        account_id=account_id, adjustments=[seed(15000)]
    )
    assert second.ok
    usd = second.outcomes[0].entry.balance
    # delta is the change to add to your own ledger; absolute is just a snapshot.
    assert usd.delta == openpit.param.PositionSize(5000)
    assert usd.absolute == openpit.param.PositionSize(15000)


@pytest.mark.integration
def test_example_wiki_account_block_unblock() -> None:
    # Used in: pit.wiki/Account-Blocking.md - Examples → Python
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    accounts = engine.accounts()

    # Block account 99224416 - all subsequent pre-trade orders are rejected.
    accounts.block(openpit.param.AccountId.from_int(99224416), "compliance hold")

    # Unblock account 99224416 - pre-trade orders are allowed again.
    accounts.unblock(openpit.param.AccountId.from_int(99224416))

    # Block every current and future member of a group in one call.
    desk = openpit.param.AccountGroupId.from_int(7)
    accounts.block_group(desk, "desk suspended")
    accounts.unblock_group(desk)


@pytest.mark.integration
def test_example_wiki_account_groups_register_and_read() -> None:
    # Used in: pit.wiki/Account-Groups.md - Examples → Python
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    # Group two accounts under one compact identifier.
    hedge_book = openpit.param.AccountGroupId.from_int(7)
    accounts = [
        openpit.param.AccountId.from_int(10),
        openpit.param.AccountId.from_int(11),
    ]
    engine.accounts().register_group(accounts, hedge_book)

    # Membership is readable by id, without enumerating the accounts.
    assert (
        engine.accounts().group_of(openpit.param.AccountId.from_int(10)) == hedge_book
    )
    assert engine.accounts().group_of(openpit.param.AccountId.from_int(99)) is None

    # Removing the group is atomic too: every listed account must be a member.
    engine.accounts().unregister_group(accounts, hedge_book)
    assert engine.accounts().group_of(openpit.param.AccountId.from_int(10)) is None


@pytest.mark.integration
def test_example_wiki_spot_funds_global_limit_mode() -> None:
    # Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md
    # Section: Spot Funds: Global Limit Mode
    # This mirror is intentionally wider than the wiki snippet: it adds the test
    # harness (seed adjustment via helper) so the example runs. Keep the shared
    # user-code flow in sync with the wiki.
    import openpit
    import openpit.pretrade.policies

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
    )

    account_id = openpit.param.AccountId.from_int(99224416)
    # Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
    seed = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(1000)
            )
        ),
    )
    engine.apply_account_adjustment(account_id=account_id, adjustments=[seed])

    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("200"),
        ),
    )

    # Default Enforce: 2 000 notional exceeds 1 000 available - rejected.
    result = engine.execute_pre_trade(order=order)
    assert not result.ok
    assert result.rejects[0].reason == "spot funds insufficient"

    # Switch to TrackOnly: the same order now passes and reserves against deficit.
    engine.configure().spot_funds(
        openpit.pretrade.policies.SpotFundsBuilder.NAME,
        global_limit_mode=openpit.pretrade.policies.SpotFundsLimitMode.TRACK_ONLY,
    )
    result = engine.execute_pre_trade(order=order)
    assert result.ok
    result.reservation.commit()  # available: 1 000 - 2 000 = -1 000

    # Restore Enforce: available is negative - still rejected.
    engine.configure().spot_funds(
        openpit.pretrade.policies.SpotFundsBuilder.NAME,
        global_limit_mode=openpit.pretrade.policies.SpotFundsLimitMode.ENFORCE,
    )
    result = engine.execute_pre_trade(order=order)
    assert not result.ok
    assert result.rejects[0].reason == "spot funds insufficient"


@pytest.mark.integration
def test_example_wiki_spot_funds_per_account_limit_mode() -> None:
    # Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md
    # Section: Spot Funds: Per-Account Limit Mode
    # This mirror is intentionally wider than the wiki snippet: it adds the test
    # harness (seed adjustment via helper) so the example runs. Keep the shared
    # user-code flow in sync with the wiki.
    import openpit
    import openpit.pretrade.policies

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_spot_funds())
        .build()
    )

    account_id = openpit.param.AccountId.from_int(99224416)
    # Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
    seed = openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
        amount=openpit.AccountAdjustmentAmount(
            balance=openpit.param.AdjustmentAmount.absolute(
                openpit.param.PositionSize(1000)
            )
        ),
    )
    engine.apply_account_adjustment(account_id=account_id, adjustments=[seed])

    order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=account_id,
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("200"),
        ),
    )

    # Global Enforce: under-funded buy is rejected.
    result = engine.execute_pre_trade(order=order)
    assert not result.ok
    assert result.rejects[0].reason == "spot funds insufficient"

    # Pin this account to TrackOnly: per-account override wins over global Enforce.
    engine.configure().spot_funds(
        openpit.pretrade.policies.SpotFundsBuilder.NAME,
        account_limit_modes=[
            openpit.pretrade.policies.SpotFundsLimitModeAccountEntry(
                account_id=account_id,
                mode=openpit.pretrade.policies.SpotFundsLimitMode.TRACK_ONLY,
            )
        ],
    )
    result = engine.execute_pre_trade(order=order)
    assert result.ok
    result.reservation.commit()  # reservation recorded despite insufficient funds

    # Clear the per-account override: cascade falls back to global Enforce.
    engine.configure().spot_funds(
        openpit.pretrade.policies.SpotFundsBuilder.NAME,
        account_limit_modes=[
            openpit.pretrade.policies.SpotFundsLimitModeAccountEntry(
                account_id=account_id,
                mode=None,
            )
        ],
    )
    result = engine.execute_pre_trade(order=order)
    assert not result.ok
    assert result.rejects[0].reason == "spot funds insufficient"


@pytest.mark.integration
def test_example_wiki_dynamic_policy_reconfiguration_rate_limit() -> None:
    # Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Retune a Built-in Policy
    # This mirror is intentionally wider than the wiki snippet: it adds the test
    # harness (`order` built via `_aapl_usd_order`) so the example runs. Keep
    # the shared user-code flow in sync with the wiki.
    import datetime

    import openpit
    import openpit.pretrade.policies

    # Harness: the AAPL/USD order from Getting Started.
    order = _aapl_usd_order("1", "100")

    # Register the rate-limit policy through builtin so the engine keeps a
    # handle to its settings; built-in policies are configurable by name.
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_rate_limit().broker_barrier(
                openpit.pretrade.policies.RateLimitBrokerBarrier(
                    limit=openpit.pretrade.policies.RateLimit(
                        max_orders=5,
                        window=datetime.timedelta(seconds=60),
                    ),
                ),
            )
        )
        .build()
    )

    # The generous limit of 5 admits the first three orders.
    for _ in range(3):
        execute_result = engine.execute_pre_trade(order=order)
        assert execute_result.ok
        execute_result.reservation.commit()

    # Tighten the broker limit to 2 at runtime, without rebuilding the engine.
    # Built-in policies register under their type name (RateLimitBuilder.NAME).
    engine.configure().rate_limit(
        openpit.pretrade.policies.RateLimitBuilder.NAME,
        broker=openpit.pretrade.policies.RateLimitBrokerBarrier(
            limit=openpit.pretrade.policies.RateLimit(
                max_orders=2,
                window=datetime.timedelta(seconds=60),
            ),
        ),
    )

    # The next order would have passed under the old limit of 5; the new limit
    # of 2 rejects it, proving the live policy reads the retuned value.
    execute_result = engine.execute_pre_trade(order=order)
    assert not execute_result.ok
    assert execute_result.rejects[0].reason == "rate limit exceeded: broker barrier"


@pytest.mark.integration
def test_example_wiki_dynamic_policy_reconfiguration_set_account_pnl() -> None:
    # Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Force-set Accumulated P&L
    # This mirror is intentionally wider than the wiki snippet: it adds the test
    # harness (`order` built via `_aapl_usd_order` and its `account`) so the
    # example runs. Keep the shared user-code flow in sync with the wiki.
    import openpit
    import openpit.pretrade.policies

    # Harness: the AAPL/USD order from Getting Started and its account.
    order = _aapl_usd_order("1", "100")
    account = openpit.param.AccountId.from_int(99224416)

    # Register the kill-switch policy through builtin so the engine keeps a
    # handle to its accumulator; built-in policies are configurable by name.
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            openpit.pretrade.policies.build_pnl_bounds_killswitch().broker_barriers(
                openpit.pretrade.policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl(-100),
                ),
            )
        )
        .build()
    )

    # With no P&L history the order passes against the lower bound of -100.
    execute_result = engine.execute_pre_trade(order=order)
    assert execute_result.ok
    execute_result.reservation.commit()

    # Force-set the account's accumulated P&L to -150 USD, below the bound.
    # Built-in policies register under their type name
    # (PnlBoundsKillswitchBuilder.NAME).
    engine.configure().set_account_pnl(
        openpit.pretrade.policies.PnlBoundsKillswitchBuilder.NAME,
        account=account,
        settlement_asset=openpit.param.Asset("USD"),
        pnl=openpit.param.Pnl(-150),
    )

    # The next order for that account breaches the lower bound and is rejected;
    # the breach also latches an engine-level block on the account.
    execute_result = engine.execute_pre_trade(order=order)
    assert not execute_result.ok
    assert (
        execute_result.rejects[0].reason == "pnl kill switch triggered: broker barrier"
    )


@pytest.mark.integration
def test_example_wiki_dry_run_verdict() -> None:
    # Used in: pit.wiki/Non-Mutating-Dry-Run.md - Read the Dry-Run Verdict
    # This mirror is intentionally wider than the wiki snippet: it adds the test
    # harness (`order` built via `_aapl_usd_order` and the engine setup) so the
    # example runs. Keep the shared user-code flow in sync with the wiki.
    import openpit
    import openpit.pretrade.policies

    order = _aapl_usd_order("100", "185")
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    report = engine.execute_pre_trade_dry_run(order=order)
    if report.is_pass:
        print("order would be admitted")
    else:
        for reject in report.rejects:
            print(
                f"would reject by {reject.policy} "
                f"[{reject.code}]: {reject.reason}: {reject.details}"
            )
    assert report.is_pass


@pytest.mark.integration
def test_example_wiki_dry_run_before_real_call() -> None:
    # Used in: pit.wiki/Non-Mutating-Dry-Run.md - Use the Dry-Run Before a Real Call
    # This mirror is intentionally wider than the wiki snippet: it adds the test
    # harness (`order` built via `_aapl_usd_order` and the engine setup) so the
    # example runs. Keep the shared user-code flow in sync with the wiki.
    import openpit
    import openpit.pretrade.policies

    order = _aapl_usd_order("100", "185")
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    # Probe without spending any budget or creating any reservation.
    probe = engine.execute_pre_trade_dry_run(order=order)
    if not probe.is_pass:
        pass  # would have been rejected - skip the real call
    else:
        # The real call now runs with fresh state; the probe had no effect.
        result = engine.execute_pre_trade(order=order)
        if result:
            result.reservation.commit()
    assert probe.is_pass


@pytest.mark.integration
def test_example_wiki_dry_run_custom_policy_hook() -> None:
    # Used in: pit.wiki/Non-Mutating-Dry-Run.md - Read-Only Custom Start-Stage Hook
    # This mirror is intentionally wider than the wiki snippet: it defines the
    # full MyCountingPolicy class (the snippet shows only the dry-run hooks) and
    # adds assertions proving the dry-run does not increment the counter.
    # Keep the shared user-code flow in sync with the wiki.
    import openpit
    import openpit.pretrade

    class MyCountingPolicy(openpit.pretrade.Policy):
        def __init__(self) -> None:
            self._count = 0

        @property
        def name(self) -> str:
            return "MyCountingPolicy"

        def check_pre_trade_start(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> openpit.pretrade.Rejects:
            self._count += 1  # side effect: spends the real counter budget
            return ()  # pass

        def check_pre_trade_start_dry_run(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> openpit.pretrade.Rejects:
            return ()  # read-only: no increment

    order = _aapl_usd_order("10", "100")
    policy = MyCountingPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    # Dry-run must not increment the counter.
    probe = engine.execute_pre_trade_dry_run(order=order)
    assert probe.is_pass
    assert policy._count == 0  # noqa: SLF001 - intentional harness access

    # Real call increments.
    result = engine.execute_pre_trade(order=order)
    assert result.ok
    result.reservation.commit()
    assert policy._count == 1  # noqa: SLF001
