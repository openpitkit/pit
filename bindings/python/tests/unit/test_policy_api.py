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

import gc
import typing

import conftest
import openpit
import pytest


def apply_drop_copy_and_commit(
    engine: openpit.Engine,
    order: openpit.Order,
) -> openpit.pretrade.DropCopyResult:
    """Apply drop copy and finalize the operation the way a caller would.

    The operation's accessors must be read before it is committed.
    """
    result = engine.apply_drop_copy(order=order)
    if result.operation is not None:
        result.operation.commit()
    return result


class BlockAllStartCheck(openpit.pretrade.Policy):
    # @typing.override
    @property
    def name(self) -> str:
        return "BlockAllStartCheck"

    # @typing.override
    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx, order
        return (
            openpit.pretrade.PolicyReject(
                code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
                reason="blocked by policy",
                details="test start check reject",
                scope=openpit.pretrade.RejectScope.ACCOUNT,
            ),
        )

    # @typing.override
    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> openpit.pretrade.PostTradeResult | None:
        _ = ctx, report
        return None


class ReportHookStartCheck(openpit.pretrade.Policy):
    # @typing.override
    @property
    def name(self) -> str:
        return "ReportHookStartCheck"

    # @typing.override
    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx, order
        return ()

    # @typing.override
    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> openpit.pretrade.PostTradeResult:
        _ = ctx, report
        return openpit.pretrade.PostTradeResult(
            account_blocks=[
                openpit.pretrade.AccountBlock(
                    policy=self.name,
                    code=openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED,
                    reason="report block",
                    details="custom policy reported block",
                )
            ]
        )


class FullParityPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "FullParityPolicy"

    @property
    def policy_group_id(self) -> int:
        return 7

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        _ = ctx, order
        return openpit.pretrade.PolicyPreTradeResult.accept(
            account_adjustments=[
                openpit.pretrade.AccountOutcomeEntry(
                    asset="USD",
                    held=openpit.pretrade.OutcomeAmount(
                        delta=openpit.param.PositionSize("2"),
                        absolute=openpit.param.PositionSize("5"),
                    ),
                    realized_pnl=openpit.pretrade.PnlOutcome(
                        pnl=openpit.pretrade.PnlOutcomeAmount(
                            delta=openpit.param.Pnl("1.25"),
                            absolute=openpit.param.Pnl("7.5"),
                        ),
                    ),
                )
            ],
            lock_prices=[openpit.param.Price("11")],
        )

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> openpit.pretrade.PostTradeResult:
        _ = ctx, report
        return openpit.pretrade.PostTradeResult(
            account_blocks=[
                openpit.pretrade.AccountBlock(
                    policy=self.name,
                    code=openpit.pretrade.RejectCode.CUSTOM,
                    reason="custom block",
                    details="post-trade result is passed through",
                    user_data=42,
                )
            ],
            account_pnls=[
                openpit.pretrade.AccountPnlOutcome(
                    policy_group_id=self.policy_group_id,
                    account_id=openpit.param.AccountId.from_int(99224416),
                    pnl=openpit.pretrade.PnlOutcomeAmount(
                        delta=openpit.param.Pnl("1.25"),
                        absolute=openpit.param.Pnl("7.5"),
                    ),
                ),
                openpit.pretrade.AccountPnlOutcome(
                    policy_group_id=self.policy_group_id,
                    account_id=openpit.param.AccountId.from_int(99224416),
                    halt_reason=(
                        openpit.pretrade.PnlHaltReason.MISSING_ACCOUNT_CURRENCY
                    ),
                ),
            ],
            account_adjustments=[
                openpit.pretrade.AccountAdjustmentOutcome(
                    policy_group_id=self.policy_group_id,
                    entry=openpit.pretrade.AccountOutcomeEntry(
                        asset="USD",
                        balance=openpit.pretrade.OutcomeAmount(
                            delta=openpit.param.PositionSize("1"),
                            absolute=openpit.param.PositionSize("6"),
                        ),
                    ),
                )
            ],
        )

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyAccountAdjustmentResult:
        _ = ctx, account_id, adjustment
        return openpit.pretrade.PolicyAccountAdjustmentResult(
            account_adjustments=(
                openpit.pretrade.AccountOutcomeEntry(
                    asset="USD",
                    incoming=openpit.pretrade.OutcomeAmount(
                        delta=openpit.param.PositionSize("3"),
                        absolute=openpit.param.PositionSize("8"),
                    ),
                ),
            ),
        )


class RejectingLockPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RejectingLockPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        _ = ctx, order
        return openpit.pretrade.PolicyPreTradeResult.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="test boundary exceeded",
                    details="drop copy must retain the accepted output",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                )
            ],
            lock_prices=[openpit.param.Price("13")],
        )


class RejectingAdjustmentPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RejectingAdjustmentPolicy"

    @property
    def policy_group_id(self) -> int:
        return 7

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        _ = ctx, order
        return openpit.pretrade.PolicyPreTradeResult.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="adjustment boundary exceeded",
                    details="drop copy must retain the account adjustments",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                )
            ],
            account_adjustments=[
                openpit.pretrade.AccountOutcomeEntry(
                    asset="USD",
                    held=openpit.pretrade.OutcomeAmount(
                        delta=openpit.param.PositionSize("2"),
                        absolute=openpit.param.PositionSize("5"),
                    ),
                )
            ],
        )


class MissingFieldPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "MissingFieldPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        _ = ctx, order
        return openpit.pretrade.PolicyPreTradeResult.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD,
                    reason="missing limit price",
                    details="policy needs a limit price",
                )
            ]
        )


class RaisingDropCopyPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RaisingDropCopyPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        _ = ctx, order
        raise RuntimeError("drop-copy callback failed")


class NestedEnginePolicy(openpit.pretrade.Policy):
    def __init__(self) -> None:
        self.engine = (
            openpit.Engine.builder()
            .no_sync()
            .pre_trade(policy=MissingFieldPolicy())
            .build()
        )

    @property
    def name(self) -> str:
        return "NestedEnginePolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> None:
        del ctx
        self.engine.start_pre_trade_dry_run(order=order)


class DropCopyStartMutationPolicy(openpit.pretrade.Policy):
    def __init__(self, fatal: bool = False) -> None:
        self.fatal = fatal
        self.saw_drop_copy = False
        self.value = 0

    @property
    def name(self) -> str:
        return "DropCopyStartMutationPolicy"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del order
        self.saw_drop_copy = ctx.is_drop_copy
        previous = self.value
        self.value += 1
        ctx.record_drop_copy_start_mutation(
            openpit.Mutation(
                commit=lambda: None,
                rollback=lambda: setattr(self, "value", previous),
            )
        )
        if not self.fatal:
            return ()
        return (
            openpit.pretrade.PolicyReject(
                code=openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD,
                reason="missing field",
                details="forced after start-stage mutation",
            ),
        )


class FailingCommitMutationPolicy(openpit.pretrade.Policy):
    def __init__(self) -> None:
        self.first = False
        self.second = False

    @property
    def name(self) -> str:
        return "FailingCommitMutationPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        del ctx, order

        def fail_second_commit() -> None:
            self.second = True
            raise RuntimeError("mutation commit failed")

        return openpit.pretrade.PolicyPreTradeResult.accept(
            mutations=(
                openpit.Mutation(
                    commit=lambda: setattr(self, "first", True),
                    rollback=lambda: setattr(self, "first", False),
                ),
                openpit.Mutation(
                    commit=fail_second_commit,
                    rollback=lambda: setattr(self, "second", False),
                ),
            )
        )


class FailingRollbackMutationPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "FailingRollbackMutationPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        del ctx, order

        def fail_rollback() -> None:
            raise RuntimeError("mutation rollback failed")

        return openpit.pretrade.PolicyPreTradeResult.reject(
            rejects=(
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD,
                    reason="forced evaluation failure",
                    details="force rollback callback",
                ),
            ),
            mutations=(openpit.Mutation(commit=lambda: None, rollback=fail_rollback),),
        )


class AppliedFailingRollbackMutationPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "AppliedFailingRollbackMutationPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        del ctx, order

        def fail_rollback() -> None:
            raise RuntimeError("explicit mutation rollback failed")

        return openpit.pretrade.PolicyPreTradeResult.accept(
            mutations=(openpit.Mutation(commit=lambda: None, rollback=fail_rollback),)
        )


@pytest.mark.unit
def test_policy_reject_scope_validation() -> None:
    with pytest.raises(TypeError, match="scope must be openpit.pretrade.RejectScope"):
        openpit.pretrade.PolicyReject(
            code=openpit.pretrade.RejectCode.OTHER,
            reason="invalid",
            details="invalid",
            scope="invalid",  # type: ignore[arg-type]
        )


@pytest.mark.unit
def test_policy_decision_and_mutation_factories() -> None:
    committed = []
    rolled_back = []
    mutation = openpit.Mutation(
        commit=lambda: committed.append("USD:10"),
        rollback=lambda: rolled_back.append("USD:0"),
    )
    decision = openpit.pretrade.PolicyDecision.accept(mutations=[mutation])

    assert len(decision.rejects) == 0
    assert len(decision.mutations) == 1
    assert callable(mutation.commit)
    assert callable(mutation.rollback)


@pytest.mark.unit
def test_custom_start_check_reject_is_returned_as_result() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=BlockAllStartCheck())
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].policy == "BlockAllStartCheck"
    assert result.rejects[0].scope == "account"


@pytest.mark.unit
def test_custom_start_check_post_trade_hook_is_supported() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=ReportHookStartCheck())
        .build()
    )

    result = engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("1"))
    )
    assert result.account_blocks
    assert result.account_blocks[0].reason == "report block"


@pytest.mark.unit
def test_custom_policy_preserves_pre_trade_result_group_and_lock_prices() -> None:
    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=FullParityPolicy()).build()
    )

    start = engine.start_pre_trade(order=conftest.make_order())
    assert start.ok
    assert start.request is not None
    executed = start.request.execute()

    assert executed.ok
    assert executed.reservation is not None
    assert executed.reservation.lock.entries() == [(7, openpit.param.Price("11"))]
    adjustments = executed.reservation.account_adjustments
    assert len(adjustments) == 1
    assert adjustments[0].policy_group_id == 7
    assert adjustments[0].entry.asset == "USD"
    assert adjustments[0].entry.held is not None
    assert adjustments[0].entry.held.delta == openpit.param.PositionSize("2")


@pytest.mark.unit
def test_drop_copy_discards_custom_reject_and_preserves_lock() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RejectingLockPolicy())
        .build()
    )

    result = engine.apply_drop_copy(order=conftest.make_order())

    assert result.ok
    assert result.rejects == []
    operation = result.operation
    assert operation is not None
    assert operation.lock.entries() == [(0, openpit.param.Price("13"))]
    block = operation.account_block
    assert block is not None
    assert block.code == openpit.pretrade.RejectCode.ACCOUNT_BLOCKED
    assert block.reason == "test boundary exceeded"
    assert operation.is_account_blocked
    operation.commit()

    blocked = engine.start_pre_trade(order=conftest.make_order())
    assert not blocked.ok
    assert blocked.rejects[0].reason == "test boundary exceeded"


@pytest.mark.unit
def test_drop_copy_runs_on_previously_blocked_account() -> None:
    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=FullParityPolicy()).build()
    )
    account = openpit.param.AccountId.from_int(42)
    engine.accounts().block(account, "existing account block")

    result = engine.apply_drop_copy(order=conftest.make_order(account_id=account))

    assert result.ok
    operation = result.operation
    assert operation is not None
    assert operation.account_block is None
    assert operation.is_account_blocked
    operation.commit()


@pytest.mark.unit
def test_drop_copy_runs_on_previously_blocked_group() -> None:
    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=FullParityPolicy()).build()
    )
    account = openpit.param.AccountId.from_int(42)
    group = openpit.param.AccountGroupId.from_int(7)
    accounts = engine.accounts()
    accounts.register_group([account], group)
    accounts.block_group(group, "existing group block")

    result = engine.apply_drop_copy(order=conftest.make_order(account_id=account))

    assert result.ok
    operation = result.operation
    assert operation is not None
    assert operation.account_block is None
    assert operation.is_account_blocked
    operation.commit()


@pytest.mark.unit
def test_drop_copy_preserves_group_tagged_account_adjustments() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RejectingAdjustmentPolicy())
        .build()
    )

    result = engine.apply_drop_copy(order=conftest.make_order())

    assert result.ok
    operation = result.operation
    assert operation is not None
    adjustments = operation.account_adjustments
    assert len(adjustments) == 1
    assert adjustments[0].policy_group_id == 7
    assert adjustments[0].entry.asset == "USD"
    assert adjustments[0].entry.held is not None
    assert adjustments[0].entry.held.delta == openpit.param.PositionSize("2")
    operation.commit()


@pytest.mark.unit
def test_drop_copy_market_order_runs_price_independent_policy() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RejectingLockPolicy())
        .build()
    )
    market_order = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_int(99224416),
            side=openpit.param.Side.BUY,
            trade_amount=openpit.param.TradeAmount.quantity(1),
        )
    )

    result = apply_drop_copy_and_commit(engine, market_order)

    assert result.ok
    assert result.operation is not None


@pytest.mark.unit
def test_drop_copy_missing_account_rejects_before_policy() -> None:
    calls = 0

    class MustNotRun(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "MustNotRun"

        def check_pre_trade_start(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> tuple[openpit.pretrade.PolicyReject, ...]:
            nonlocal calls
            del ctx, order
            calls += 1
            return ()

    engine = openpit.Engine.builder().no_sync().pre_trade(policy=MustNotRun()).build()
    anonymous_order = openpit.Order()

    result = apply_drop_copy_and_commit(engine, anonymous_order)

    assert not result.ok
    assert result.operation is None
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD
    assert result.rejects[0].reason == "drop-copy requires a readable account ID"
    assert calls == 0


@pytest.mark.unit
def test_drop_copy_returns_missing_field_as_business_reject() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=MissingFieldPolicy())
        .build()
    )

    result = apply_drop_copy_and_commit(engine, conftest.make_order())

    assert not result.ok
    assert result.operation is None
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.MISSING_REQUIRED_FIELD


@pytest.mark.unit
def test_drop_copy_rethrows_custom_policy_exception() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RaisingDropCopyPolicy())
        .build()
    )

    with pytest.raises(RuntimeError, match="drop-copy callback failed"):
        engine.apply_drop_copy(order=conftest.make_order())


@pytest.mark.unit
def test_drop_copy_surfaces_market_data_account_group_error_before_apply() -> None:
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    instrument_id = service.register(openpit.Instrument("AAPL", "USD"))

    class RaisingAccountInfo:
        @property
        def account_group(self) -> None:
            raise RuntimeError("account group callback failed")

    class LookupPolicy(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "LookupPolicy"

        def perform_pre_trade_check(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> openpit.pretrade.PolicyPreTradeResult:
            del ctx, order
            service.get_optional(
                instrument_id,
                openpit.param.AccountId.from_int(99224416),
                RaisingAccountInfo(),
                openpit.marketdata.QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT,
            )
            return openpit.pretrade.PolicyPreTradeResult.accept()

    start_policy = DropCopyStartMutationPolicy()
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=start_policy)
        .pre_trade(policy=LookupPolicy())
        .build()
    )

    with pytest.raises(RuntimeError, match="account group callback failed"):
        engine.apply_drop_copy(order=conftest.make_order())
    assert start_policy.value == 0


@pytest.mark.unit
def test_drop_copy_preserves_first_exception_across_later_policy_calls() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RaisingDropCopyPolicy())
        .pre_trade(policy=NestedEnginePolicy())
        .build()
    )

    with pytest.raises(RuntimeError, match="drop-copy callback failed"):
        engine.apply_drop_copy(order=conftest.make_order())


@pytest.mark.unit
def test_drop_copy_preserves_first_exception_across_sibling_failures() -> None:
    calls: list[str] = []

    class RaisingPolicy(openpit.pretrade.Policy):
        def __init__(self, name: str, message: str) -> None:
            self._name = name
            self.message = message

        @property
        def name(self) -> str:
            return self._name

        def perform_pre_trade_check(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> None:
            del ctx, order
            calls.append(self._name)
            raise RuntimeError(self.message)

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RaisingPolicy("first", "first callback failed"))
        .pre_trade(policy=RaisingPolicy("second", "second callback failed"))
        .build()
    )

    with pytest.raises(RuntimeError, match="first callback failed"):
        engine.apply_drop_copy(order=conftest.make_order())
    assert calls == ["first", "second"]


@pytest.mark.unit
def test_drop_copy_start_context_commits_registered_mutation() -> None:
    policy = DropCopyStartMutationPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = apply_drop_copy_and_commit(engine, conftest.make_order())

    assert result.ok
    assert policy.saw_drop_copy
    assert policy.value == 1


@pytest.mark.unit
def test_drop_copy_operation_commit_and_rollback_are_single_use() -> None:
    policy = DropCopyStartMutationPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    committed = engine.apply_drop_copy(order=conftest.make_order()).operation
    assert committed is not None
    assert policy.value == 1
    committed.commit()
    assert policy.value == 1
    with pytest.raises(RuntimeError, match="already been finalized"):
        committed.commit()
    with pytest.raises(RuntimeError, match="already been finalized"):
        committed.rollback()

    rolled_back = engine.apply_drop_copy(order=conftest.make_order()).operation
    assert rolled_back is not None
    assert policy.value == 2
    rolled_back.rollback()
    assert policy.value == 1
    with pytest.raises(RuntimeError, match="already been finalized"):
        rolled_back.rollback()
    with pytest.raises(RuntimeError, match="already been finalized"):
        rolled_back.commit()


@pytest.mark.unit
def test_drop_copy_operation_accessors_require_an_active_operation() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RejectingLockPolicy())
        .build()
    )

    operation = engine.apply_drop_copy(order=conftest.make_order()).operation
    assert operation is not None
    operation.commit()

    for name in ("lock", "account_adjustments", "account_block", "is_account_blocked"):
        with pytest.raises(RuntimeError, match="already been finalized"):
            getattr(operation, name)


@pytest.mark.unit
def test_drop_copy_operation_garbage_collection_rolls_back_implicitly() -> None:
    policy = DropCopyStartMutationPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_drop_copy(order=conftest.make_order())
    assert result.ok
    assert policy.value == 1
    del result
    gc.collect()

    assert policy.value == 0


@pytest.mark.unit
def test_drop_copy_explicit_rollback_rethrows_callback_error() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=AppliedFailingRollbackMutationPolicy())
        .build()
    )
    order = conftest.make_order()
    operation = engine.apply_drop_copy(order=order).operation
    assert operation is not None

    with pytest.raises(RuntimeError, match="explicit mutation rollback failed"):
        operation.rollback()
    with pytest.raises(RuntimeError, match="already been finalized"):
        operation.rollback()

    # The exception reaches this caller, and independently arms the kill
    # switch: a finalizer has no right to fail. The mutation is a custom
    # policy's, so the block covers every account.
    blocked = engine.start_pre_trade(order=order)
    assert not blocked.ok
    assert blocked.rejects[0].code == openpit.pretrade.RejectCode.SYSTEM_UNAVAILABLE


@pytest.mark.unit
def test_drop_copy_start_context_rolls_back_on_fatal_reject() -> None:
    policy = DropCopyStartMutationPolicy(fatal=True)
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = apply_drop_copy_and_commit(engine, conftest.make_order())

    assert not result.ok
    assert policy.saw_drop_copy
    assert policy.value == 0


@pytest.mark.unit
def test_drop_copy_commit_failure_keeps_every_mutation_applied() -> None:
    policy = FailingCommitMutationPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()
    order = conftest.make_order()

    operation = engine.apply_drop_copy(order=order).operation
    assert operation is not None
    with pytest.raises(RuntimeError, match="mutation commit failed"):
        operation.commit()

    # A failing commit callback never stops the batch and nothing rolls back.
    assert policy.first
    assert policy.second
    # It still arms the kill switch: a finalizer has no right to fail.
    blocked = engine.start_pre_trade(order=order)
    assert not blocked.ok
    assert blocked.rejects[0].code == openpit.pretrade.RejectCode.SYSTEM_UNAVAILABLE


def _arm_kill_switch(engine: openpit.Engine) -> None:
    """Arm the kill switch the only way a Python caller can.

    A custom policy registers a mutation whose commit callable raises.
    """
    operation = engine.apply_drop_copy(order=conftest.make_order()).operation
    assert operation is not None
    with pytest.raises(RuntimeError, match="mutation commit failed"):
        operation.commit()


def _engine_with_failing_commit() -> openpit.Engine:
    return (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=FailingCommitMutationPolicy())
        .build()
    )


@pytest.mark.unit
def test_finalizer_failure_blocks_an_untouched_account() -> None:
    engine = _engine_with_failing_commit()
    other = openpit.param.AccountId.from_int(99224417)
    _arm_kill_switch(engine)

    # The mutation belongs to a custom policy, so its reach is unknown and the
    # block is engine-wide, not scoped to the order's own account.
    result = engine.start_pre_trade(order=conftest.make_order(account_id=other))

    assert not result.ok
    assert result.rejects[0].code == openpit.pretrade.RejectCode.SYSTEM_UNAVAILABLE


@pytest.mark.unit
def test_unblock_all_clears_the_engine_wide_block() -> None:
    engine = _engine_with_failing_commit()
    other = openpit.param.AccountId.from_int(99224417)
    _arm_kill_switch(engine)
    assert not engine.start_pre_trade(order=conftest.make_order(account_id=other)).ok

    engine.accounts().unblock_all()

    result = engine.start_pre_trade(order=conftest.make_order(account_id=other))
    assert result.ok
    result.request.execute().reservation.rollback()


@pytest.mark.unit
def test_unblock_all_keeps_an_individually_blocked_account_blocked() -> None:
    engine = _engine_with_failing_commit()
    other = openpit.param.AccountId.from_int(99224417)
    third = openpit.param.AccountId.from_int(99224418)
    engine.accounts().block(other, "compliance hold")
    _arm_kill_switch(engine)

    engine.accounts().unblock_all()

    blocked = engine.start_pre_trade(order=conftest.make_order(account_id=other))
    assert not blocked.ok
    assert blocked.rejects[0].code == openpit.pretrade.RejectCode.ACCOUNT_BLOCKED
    assert blocked.rejects[0].reason == "compliance hold"
    assert engine.start_pre_trade(order=conftest.make_order(account_id=third)).ok


@pytest.mark.unit
def test_drop_copy_rollback_failure_rethrows_original_exception() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=FailingRollbackMutationPolicy())
        .build()
    )

    order = conftest.make_order()
    with pytest.raises(RuntimeError, match="mutation rollback failed"):
        engine.apply_drop_copy(order=order)

    assert not engine.start_pre_trade(order=order).ok


@pytest.mark.unit
def test_custom_policy_returns_full_post_trade_result() -> None:
    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=FullParityPolicy()).build()
    )

    result = engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("1"))
    )

    assert len(result.account_blocks) == 1
    assert result.account_blocks[0].code == openpit.pretrade.RejectCode.CUSTOM
    assert result.account_blocks[0].reason == "custom block"
    assert result.account_blocks[0].user_data == 42
    assert len(result.account_pnls) == 2
    assert result.account_pnls[0].account_id == openpit.param.AccountId.from_int(
        99224416
    )
    assert result.account_pnls[0].pnl is not None
    assert result.account_pnls[0].pnl.delta == openpit.param.Pnl("1.25")
    assert result.account_pnls[0].policy_group_id == 7
    assert (
        result.account_pnls[1].halt_reason
        == openpit.pretrade.PnlHaltReason.MISSING_ACCOUNT_CURRENCY
    )
    assert result.account_pnls[1].policy_group_id == 7
    assert result.account_pnls[1].pnl is None
    assert len(result.account_adjustments) == 1
    assert result.account_adjustments[0].policy_group_id == 7
    assert result.account_adjustments[0].entry.balance is not None
    assert result.account_adjustments[
        0
    ].entry.balance.absolute == openpit.param.PositionSize("6")


@pytest.mark.unit
def test_pnl_outcome_requires_exactly_one_value() -> None:
    amount = openpit.pretrade.PnlOutcomeAmount(
        delta=openpit.param.Pnl("1"),
        absolute=openpit.param.Pnl("2"),
    )

    with pytest.raises(ValueError, match="exactly one"):
        openpit.pretrade.PnlOutcome()
    with pytest.raises(ValueError, match="exactly one"):
        openpit.pretrade.PnlOutcome(
            pnl=amount,
            halt_reason=openpit.pretrade.PnlHaltReason.MISSING_FX,
        )
    with pytest.raises(ValueError, match="exactly one"):
        openpit.pretrade.AccountPnlOutcome(
            policy_group_id=7,
            account_id=openpit.param.AccountId.from_int(99224416),
        )
    with pytest.raises(ValueError, match="exactly one"):
        openpit.pretrade.AccountPnlOutcome(
            policy_group_id=7,
            account_id=openpit.param.AccountId.from_int(99224416),
            pnl=amount,
            halt_reason=openpit.pretrade.PnlHaltReason.MISSING_FX,
        )


@pytest.mark.unit
@pytest.mark.parametrize(
    ("reason", "public_name"),
    [
        (openpit.pretrade.PnlHaltReason.MISSING_FX, "MISSING_FX"),
        (
            openpit.pretrade.PnlHaltReason.MISSING_ACCOUNT_CURRENCY,
            "MISSING_ACCOUNT_CURRENCY",
        ),
        (
            openpit.pretrade.PnlHaltReason.MISSING_INITIAL_PNL,
            "MISSING_INITIAL_PNL",
        ),
        (
            openpit.pretrade.PnlHaltReason.MISSING_COST_BASIS,
            "MISSING_COST_BASIS",
        ),
        (
            openpit.pretrade.PnlHaltReason.ARITHMETIC_OVERFLOW,
            "ARITHMETIC_OVERFLOW",
        ),
    ],
)
def test_pnl_outcome_repr_uses_public_reason_name(
    reason: openpit.pretrade.PnlHaltReason,
    public_name: str,
) -> None:
    outcome = openpit.pretrade.PnlOutcome(halt_reason=reason)
    account_outcome = openpit.pretrade.AccountPnlOutcome(
        policy_group_id=7,
        account_id=openpit.param.AccountId.from_int(99224416),
        halt_reason=reason,
    )

    assert f"PnlHaltReason.{public_name}" in repr(outcome)
    assert f"PnlHaltReason.{public_name}" in repr(account_outcome)


@pytest.mark.unit
def test_account_pnl_outcome_repr_contains_state() -> None:
    computed = openpit.pretrade.AccountPnlOutcome(
        policy_group_id=7,
        account_id=openpit.param.AccountId.from_int(99224416),
        pnl=openpit.pretrade.PnlOutcomeAmount(
            delta=openpit.param.Pnl("1"),
            absolute=openpit.param.Pnl("2"),
        ),
    )
    halted = openpit.pretrade.AccountPnlOutcome(
        policy_group_id=7,
        account_id=openpit.param.AccountId.from_int(99224416),
        halt_reason=openpit.pretrade.PnlHaltReason.MISSING_ACCOUNT_CURRENCY,
    )

    assert "pnl=PnlOutcomeAmount" in repr(computed)
    assert "halt_reason=PnlHaltReason.MISSING_ACCOUNT_CURRENCY" in repr(halted)


@pytest.mark.unit
def test_custom_policy_bool_true_report_hook_is_not_synthetic_block() -> None:
    class LegacyBoolTrueReportHook(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "LegacyBoolTrueReportHook"

        def apply_execution_report(
            self,
            ctx: openpit.pretrade.PostTradeContext,
            report: openpit.ExecutionReport,
        ) -> bool:
            _ = ctx, report
            return True

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=LegacyBoolTrueReportHook())
        .build()
    )

    with pytest.raises(TypeError, match="PostTradeResult or None"):
        engine.apply_execution_report(
            report=conftest.make_report(pnl=openpit.param.Pnl("1"))
        )


@pytest.mark.unit
def test_custom_policy_blocks_account_from_account_adjustment_result() -> None:
    class BlockingAdjustmentPolicy(openpit.pretrade.Policy):
        def __init__(self) -> None:
            self.blocked: list[str] = []

        @property
        def name(self) -> str:
            return "BlockingAdjustmentPolicy"

        def apply_account_adjustment(
            self,
            ctx: openpit.AccountAdjustmentContext,
            account_id: openpit.param.AccountId,
            adjustment: openpit.AccountAdjustment,
        ) -> openpit.pretrade.PolicyAccountAdjustmentResult:
            _ = ctx, account_id, adjustment
            self.blocked.append("USD")
            return openpit.pretrade.PolicyAccountAdjustmentResult(
                account_blocks=(
                    openpit.pretrade.AccountBlock(
                        policy=self.name,
                        code=openpit.pretrade.RejectCode.ACCOUNT_BLOCKED,
                        reason="blocked from accepted adjustment",
                        details=(
                            "custom policy accepted the adjustment "
                            "and blocked the account"
                        ),
                    ),
                ),
            )

    policy = BlockingAdjustmentPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_int(99224416),
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD")
            )
        ],
    )

    assert policy.blocked == ["USD"]
    assert result.ok
    assert len(result.account_blocks) == 1
    assert result.account_blocks[0].code == openpit.pretrade.RejectCode.ACCOUNT_BLOCKED

    # The block takes effect in the engine: a later order on the same account
    # is rejected without any policy start-check involvement.
    blocked = engine.start_pre_trade(
        order=conftest.make_order(
            account_id=openpit.param.AccountId.from_int(99224416),
        )
    )
    assert not blocked.ok
    assert blocked.rejects[0].code == openpit.pretrade.RejectCode.ACCOUNT_BLOCKED


@pytest.mark.unit
def test_custom_policy_account_adjustment_returns_outcome_entries() -> None:
    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=FullParityPolicy()).build()
    )

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_int(99224416),
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD")
            )
        ],
    )

    assert result.ok
    assert len(result.outcomes) == 1
    assert result.outcomes[0].policy_group_id == 7
    assert result.outcomes[0].entry.incoming is not None
    assert result.outcomes[0].entry.incoming.delta == openpit.param.PositionSize("3")


@pytest.mark.unit
def test_post_trade_context_group_is_none_when_account_not_registered() -> None:
    captured: list[openpit.param.AccountGroupId | None] = []

    class CaptureGroup(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "CaptureGroup"

        def apply_execution_report(
            self,
            ctx: openpit.pretrade.PostTradeContext,
            report: openpit.ExecutionReport,
        ) -> openpit.pretrade.PostTradeResult | None:
            captured.append(ctx.account_group)
            return None

    engine = openpit.Engine.builder().no_sync().pre_trade(policy=CaptureGroup()).build()
    engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("1"))
    )
    assert captured[-1] is None


@pytest.mark.unit
def test_post_trade_context_group_returns_registered_group() -> None:
    captured: list[openpit.param.AccountGroupId | None] = []

    class CaptureGroup(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "CaptureGroup"

        def apply_execution_report(
            self,
            ctx: openpit.pretrade.PostTradeContext,
            report: openpit.ExecutionReport,
        ) -> openpit.pretrade.PostTradeResult | None:
            captured.append(ctx.account_group)
            return None

    engine = openpit.Engine.builder().no_sync().pre_trade(policy=CaptureGroup()).build()
    account = openpit.param.AccountId.from_int(99224416)  # same as conftest default
    g = openpit.param.AccountGroupId.from_int(99224416)
    engine.accounts().register_group([account], g)
    engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("1"))
    )
    assert captured[-1] == g


@pytest.mark.unit
def test_pre_trade_context_group_returns_registered_group() -> None:
    captured: list[openpit.param.AccountGroupId | None] = []

    class CaptureGroup(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "CaptureGroup"

        def check_pre_trade_start(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> tuple[openpit.pretrade.PolicyReject, ...]:
            captured.append(ctx.account_group)
            return ()

    engine = openpit.Engine.builder().no_sync().pre_trade(policy=CaptureGroup()).build()
    account = openpit.param.AccountId.from_int(77)
    g = openpit.param.AccountGroupId.from_int(5)
    engine.accounts().register_group([account], g)
    engine.start_pre_trade(order=conftest.make_order(account_id=account))
    assert captured[-1] == g


def _raise_nested() -> None:
    raise RuntimeError("nested policy failed")


class HookProbePolicy(openpit.pretrade.Policy):
    """Runs ``action`` from one named hook and stays inert in every other."""

    def __init__(
        self,
        *,
        name: str,
        hook: str,
        action: typing.Callable[[], None],
    ) -> None:
        self._name = name
        self._hook = hook
        self._action = action

    @property
    def name(self) -> str:
        return self._name

    def _fire(self, hook: str) -> None:
        if hook == self._hook:
            self._action()

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx, order
        self._fire("check_pre_trade_start")
        return ()

    def check_pre_trade_start_dry_run(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx, order
        self._fire("check_pre_trade_start_dry_run")
        return ()

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        del ctx, order
        self._fire("perform_pre_trade_check")
        return openpit.pretrade.PolicyPreTradeResult.accept()

    def perform_pre_trade_check_dry_run(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyPreTradeResult:
        del ctx, order
        self._fire("perform_pre_trade_check_dry_run")
        return openpit.pretrade.PolicyPreTradeResult.accept()

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> openpit.pretrade.PostTradeResult | None:
        del ctx, report
        self._fire("apply_execution_report")
        return None

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyAccountAdjustmentResult:
        del ctx, account_id, adjustment
        self._fire("apply_account_adjustment")
        return openpit.pretrade.PolicyAccountAdjustmentResult()


# Hooks that keep running the rest of the pipeline after one policy raises,
# paired with an engine entry point that drives them. ``apply_account_adjustment``
# is absent on purpose: a raising policy aborts the batch, so no later Python
# callback of that operation can run - it is covered on its own below.
_CALLBACK_HOOKS: tuple[tuple[str, typing.Callable[[openpit.Engine], object]], ...] = (
    (
        "check_pre_trade_start",
        lambda engine: engine.apply_drop_copy(order=conftest.make_order()),
    ),
    (
        "perform_pre_trade_check",
        lambda engine: engine.apply_drop_copy(order=conftest.make_order()),
    ),
    (
        "check_pre_trade_start_dry_run",
        lambda engine: engine.start_pre_trade_dry_run(order=conftest.make_order()),
    ),
    (
        "perform_pre_trade_check_dry_run",
        lambda engine: engine.execute_pre_trade_dry_run(order=conftest.make_order()),
    ),
    (
        "apply_execution_report",
        lambda engine: engine.apply_execution_report(
            report=conftest.make_report(pnl=openpit.param.Pnl("1"))
        ),
    ),
)


@pytest.mark.unit
@pytest.mark.parametrize(
    ("hook", "run"), _CALLBACK_HOOKS, ids=[h for h, _ in _CALLBACK_HOOKS]
)
def test_nested_engine_call_leaves_the_enclosing_exception_alone(
    hook: str,
    run: typing.Callable[[openpit.Engine], object],
) -> None:
    """A nested call from a callback must not consume the owner's exception."""
    nested = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(
            policy=HookProbePolicy(name="nested-inert", hook="", action=lambda: None)
        )
        .build()
    )
    observed: list[object] = []

    def raise_own() -> None:
        raise RuntimeError(f"{hook} failed")

    def call_nested_engine() -> None:
        # A plausible caller guard around a nested lookup: it must not receive
        # - and therefore must not be able to swallow - the exception that the
        # enclosing operation already owns.
        try:
            nested.apply_drop_copy(order=conftest.make_order())
            observed.append("clean")
        except BaseException as error:  # noqa: BLE001 - the point of the test
            observed.append(error)

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=HookProbePolicy(name="raising", hook=hook, action=raise_own))
        .pre_trade(
            policy=HookProbePolicy(name="nesting", hook=hook, action=call_nested_engine)
        )
        .build()
    )

    with pytest.raises(RuntimeError, match=f"{hook} failed"):
        run(engine)

    assert observed == ["clean"]


@pytest.mark.unit
def test_nested_engine_call_from_account_adjustment_hook_owns_its_error() -> None:
    """The adjustment hook's own exception survives a nested engine call."""
    nested = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(
            policy=HookProbePolicy(
                name="nested-raising",
                hook="perform_pre_trade_check",
                action=_raise_nested,
            )
        )
        .build()
    )
    observed: list[BaseException] = []

    def call_nested_engine_then_raise() -> None:
        try:
            nested.apply_drop_copy(order=conftest.make_order())
        except RuntimeError as error:
            observed.append(error)
        raise RuntimeError("apply_account_adjustment failed")

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(
            policy=HookProbePolicy(
                name="nesting-adjustment",
                hook="apply_account_adjustment",
                action=call_nested_engine_then_raise,
            )
        )
        .build()
    )

    with pytest.raises(RuntimeError, match="apply_account_adjustment failed"):
        engine.apply_account_adjustment(
            account_id=openpit.param.AccountId.from_int(99224416),
            adjustments=[
                openpit.AccountAdjustment(
                    operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
                )
            ],
        )

    # The nested call raised its own failure and left the hook free to raise.
    assert len(observed) == 1
    assert "nested policy failed" in str(observed[0])


@pytest.mark.unit
def test_nested_market_data_lookup_keeps_the_enclosing_exception() -> None:
    """A caught nested failure must not hide the enclosing operation's own."""
    service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    instrument_id = service.register(openpit.Instrument("AAPL", "USD"))

    class RaisingAccountInfo:
        @property
        def account_group(self) -> None:
            raise RuntimeError("account group callback failed")

    caught: list[BaseException] = []

    class SwallowingLookupPolicy(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "SwallowingLookupPolicy"

        def perform_pre_trade_check(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> openpit.pretrade.PolicyPreTradeResult:
            del ctx, order
            try:
                service.get_optional(
                    instrument_id,
                    openpit.param.AccountId.from_int(99224416),
                    RaisingAccountInfo(),
                    openpit.marketdata.QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT,
                )
            except RuntimeError as error:
                caught.append(error)
            return openpit.pretrade.PolicyPreTradeResult.accept()

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RaisingDropCopyPolicy())
        .pre_trade(policy=SwallowingLookupPolicy())
        .build()
    )

    with pytest.raises(RuntimeError, match="drop-copy callback failed"):
        engine.apply_drop_copy(order=conftest.make_order())

    # The lookup raised its own group-resolution failure, not the drop copy's.
    assert len(caught) == 1
    assert "account group callback failed" in str(caught[0])
