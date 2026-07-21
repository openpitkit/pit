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
    assert executed.reservation.account_block() is None


@pytest.mark.unit
def test_drop_copy_discards_custom_reject_and_preserves_lock() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RejectingLockPolicy())
        .build()
    )

    reservation = engine.execute_pre_trade_drop_copy(order=conftest.make_order())

    assert reservation.lock().entries() == [(0, openpit.param.Price("13"))]
    block = reservation.account_block()
    assert block is not None
    assert block.reason == "test boundary exceeded"
    reservation.commit()
    with pytest.raises(RuntimeError, match="already been finalized"):
        reservation.account_block()
    blocked = engine.start_pre_trade(order=conftest.make_order())
    assert not blocked.ok
    assert blocked.rejects[0].reason == "test boundary exceeded"


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
