import conftest
import openpit
import pytest


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
    ) -> list[openpit.pretrade.AccountOutcomeEntry]:
        _ = ctx, account_id, adjustment
        return [
            openpit.pretrade.AccountOutcomeEntry(
                asset="USD",
                incoming=openpit.pretrade.OutcomeAmount(
                    delta=openpit.param.PositionSize("3"),
                    absolute=openpit.param.PositionSize("8"),
                ),
            )
        ]


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
    assert executed.reservation.lock().entries() == [(7, openpit.param.Price("11"))]
    adjustments = executed.reservation.account_adjustments()
    assert len(adjustments) == 1
    assert adjustments[0].policy_group_id == 7
    assert adjustments[0].entry.asset == "USD"
    assert adjustments[0].entry.held is not None
    assert adjustments[0].entry.held.delta == openpit.param.PositionSize("2")


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
    assert len(result.account_adjustments) == 1
    assert result.account_adjustments[0].policy_group_id == 7
    assert result.account_adjustments[0].entry.balance is not None
    assert result.account_adjustments[
        0
    ].entry.balance.absolute == openpit.param.PositionSize("6")


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
def test_custom_policy_blocks_account_via_account_control() -> None:
    class BlockViaControlPolicy(openpit.pretrade.Policy):
        def __init__(self) -> None:
            self.blocked: list[str] = []

        @property
        def name(self) -> str:
            return "BlockViaControlPolicy"

        def apply_account_adjustment(
            self,
            ctx: openpit.AccountAdjustmentContext,
            account_id: openpit.param.AccountId,
            adjustment: openpit.AccountAdjustment,
        ) -> None:
            _ = account_id, adjustment
            control: openpit.AccountControl = ctx.account_control
            control.block(
                openpit.pretrade.AccountBlock(
                    policy=self.name,
                    code=openpit.pretrade.RejectCode.ACCOUNT_BLOCKED,
                    reason="blocked via account_control",
                    details="custom policy blocked the account from a callback",
                )
            )
            self.blocked.append("USD")
            return None

    policy = BlockViaControlPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_int(99224416),
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD")
            )
        ],
    )

    assert policy.blocked == ["USD"]

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
    g = openpit.param.AccountGroupId.from_int(42)
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
