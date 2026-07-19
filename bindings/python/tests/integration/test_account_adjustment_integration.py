import openpit
import pytest


class RecordingAdjustmentCheck(openpit.pretrade.Policy):
    def __init__(self, *, reject_on_asset: str | None = None) -> None:
        self.seen_account_ids: list[int] = []
        self.seen_assets: list[str] = []
        self._reject_on_asset = reject_on_asset

    @property
    def name(self) -> str:
        return "RecordingAdjustmentCheck"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyAccountAdjustmentResult:
        self.seen_account_ids.append(account_id.value)
        asset = adjustment.operation.asset
        self.seen_assets.append(asset)
        if self._reject_on_asset == asset:
            return openpit.pretrade.PolicyAccountAdjustmentResult(
                rejects=(
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.OTHER,
                        reason="rejected by test",
                        details=f"asset {asset} is blocked",
                        scope=openpit.pretrade.RejectScope.ORDER,
                    ),
                ),
            )
        return openpit.pretrade.PolicyAccountAdjustmentResult()


# Policy that applies state immediately and returns a mutation per adjustment.
# The engine applies rollback actions in reverse order on batch failure.
# Rollback correctness is verified by running the same batch twice on the same
# engine. If a rejected batch retained committed state, the second run would
# deviate.
class MutatingRecordingPolicy(openpit.pretrade.Policy):
    def __init__(self, *, reject_on_asset: str | None = None) -> None:
        self.seen_assets: list[str] = []
        self._reject_on_asset = reject_on_asset

    @property
    def name(self) -> str:
        return "MutatingRecordingPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyAccountAdjustmentResult:
        asset = adjustment.operation.asset
        self.seen_assets.append(asset)
        if self._reject_on_asset == asset:
            return openpit.pretrade.PolicyAccountAdjustmentResult(
                rejects=(
                    openpit.pretrade.PolicyReject(
                        code=openpit.pretrade.RejectCode.OTHER,
                        reason="rejected by test",
                        details=f"asset {asset} is blocked",
                        scope=openpit.pretrade.RejectScope.ORDER,
                    ),
                ),
            )
        return openpit.pretrade.PolicyAccountAdjustmentResult(
            mutations=(
                openpit.Mutation(
                    commit=lambda: None,
                    rollback=lambda: None,
                ),
            ),
        )


def _make_balance_adjustment(asset_code: str) -> openpit.AccountAdjustment:
    return openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(
            asset=asset_code,
        )
    )


class RecordingPnlCheck(openpit.pretrade.Policy):
    def __init__(self) -> None:
        self.seen_states: list[str] = []

    @property
    def name(self) -> str:
        return "RecordingPnlCheck"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyAccountAdjustmentResult:
        operation = adjustment.operation
        if isinstance(operation, openpit.AccountAdjustmentAccountPnlOperation):
            state = operation.state
        else:
            assert isinstance(operation, openpit.AccountAdjustmentBalanceOperation)
            state = operation.realized_pnl
        self.seen_states.append(str(state))
        return openpit.pretrade.PolicyAccountAdjustmentResult()


@pytest.mark.integration
def test_account_adjustment_integration_pnl_operations_reach_policy() -> None:
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = RecordingPnlCheck()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentAccountPnlOperation(
                    state=openpit.param.Pnl("42.5"),
                )
            ),
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(
                    asset="AAPL",
                    realized_pnl=openpit.pretrade.PnlHaltReason.MISSING_COST_BASIS,
                )
            ),
        ],
    )

    assert result.ok
    assert policy.seen_states == ["42.5", "PnlHaltReason.MISSING_COST_BASIS"]


@pytest.mark.integration
def test_account_pnl_operation_reaches_builtin_spot_funds_batch_path() -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds_pnl_bounds_killswitch().global_barrier(
                policies.SpotFundsPnlBoundsBarrier(
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        )
        .build()
    )

    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentAccountPnlOperation(
                    state=openpit.param.Pnl("-150"),
                )
            )
        ],
    )

    assert result.ok
    assert result.failed_index is None
    assert not result.rejects
    assert len(result.account_blocks) == 1
    assert (
        result.account_blocks[0].code
        == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )


def _spot_funds_killswitch_engine() -> openpit.Engine:
    policies = openpit.pretrade.policies
    return (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds_pnl_bounds_killswitch().global_barrier(
                policies.SpotFundsPnlBoundsBarrier(
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        )
        .build()
    )


def _assert_account_pnl(
    engine: openpit.Engine,
    account_id: openpit.param.AccountId,
    state: openpit.param.Pnl | openpit.pretrade.PnlHaltReason,
) -> openpit.pretrade.AccountAdjustmentBatchResult:
    return engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentAccountPnlOperation(state=state)
            )
        ],
    )


@pytest.mark.integration
def test_account_pnl_halt_through_builtin_spot_funds_batch_path_blocks() -> None:
    # A halted account PnL is uncomputable, so a configured barrier can no
    # longer be evaluated: the batch commits and the account is blocked.
    account_id = openpit.param.AccountId.from_int(99224417)
    engine = _spot_funds_killswitch_engine()

    result = _assert_account_pnl(
        engine, account_id, openpit.pretrade.PnlHaltReason.MISSING_FX
    )

    assert result.ok
    assert result.failed_index is None
    assert not result.rejects
    assert len(result.account_blocks) == 1
    assert (
        result.account_blocks[0].code
        == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )


@pytest.mark.integration
def test_account_pnl_numeric_state_re_arms_builtin_spot_funds_batch_path() -> None:
    # Asserting a numeric state re-arms the accumulator: a value inside the
    # barrier produces no further block. The block latched by the preceding
    # breach is lifted separately, by the operator.
    account_id = openpit.param.AccountId.from_int(99224418)
    engine = _spot_funds_killswitch_engine()

    breached = _assert_account_pnl(engine, account_id, openpit.param.Pnl("-150"))
    assert breached.ok
    assert len(breached.account_blocks) == 1

    re_armed = _assert_account_pnl(engine, account_id, openpit.param.Pnl("-50"))

    assert re_armed.ok
    assert re_armed.failed_index is None
    assert not re_armed.rejects
    assert not re_armed.account_blocks


@pytest.mark.integration
def test_account_pnl_re_arm_after_halt_through_builtin_spot_funds_batch_path() -> None:
    # Re-arming out of a halt is the same numeric assertion: the state becomes
    # computable again and is checked against the barrier as usual.
    account_id = openpit.param.AccountId.from_int(99224419)
    engine = _spot_funds_killswitch_engine()

    halted = _assert_account_pnl(
        engine, account_id, openpit.pretrade.PnlHaltReason.MISSING_ACCOUNT_CURRENCY
    )
    assert halted.ok
    assert len(halted.account_blocks) == 1

    re_armed = _assert_account_pnl(engine, account_id, openpit.param.Pnl("-25"))
    assert re_armed.ok
    assert not re_armed.account_blocks

    # The re-armed accumulator is live: a fresh breach blocks again.
    breached = _assert_account_pnl(engine, account_id, openpit.param.Pnl("-150"))

    assert breached.ok
    assert not breached.rejects
    assert len(breached.account_blocks) == 1
    assert (
        breached.account_blocks[0].code
        == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )


@pytest.mark.integration
def test_account_adjustment_integration_successful_batch() -> None:
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = RecordingAdjustmentCheck()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            _make_balance_adjustment("USD"),
            _make_balance_adjustment("EUR"),
            _make_balance_adjustment("GBP"),
        ],
    )

    assert result.ok
    assert policy.seen_account_ids == [99224416, 99224416, 99224416]
    assert policy.seen_assets == ["USD", "EUR", "GBP"]


@pytest.mark.integration
def test_account_adjustment_integration_reject_on_first() -> None:
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = RecordingAdjustmentCheck(reject_on_asset="USD")
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            _make_balance_adjustment("USD"),
            _make_balance_adjustment("EUR"),
            _make_balance_adjustment("GBP"),
        ],
    )

    assert not result.ok
    assert result.failed_index == 0
    assert policy.seen_assets == ["USD"]


@pytest.mark.integration
def test_account_adjustment_integration_reject_on_last() -> None:
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = RecordingAdjustmentCheck(reject_on_asset="GBP")
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            _make_balance_adjustment("USD"),
            _make_balance_adjustment("EUR"),
            _make_balance_adjustment("GBP"),
        ],
    )

    assert not result.ok
    assert result.failed_index == 2
    assert policy.seen_assets == ["USD", "EUR", "GBP"]


@pytest.mark.integration
def test_account_adjustment_integration_reject_on_middle() -> None:
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = RecordingAdjustmentCheck(reject_on_asset="EUR")
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            _make_balance_adjustment("USD"),
            _make_balance_adjustment("EUR"),
            _make_balance_adjustment("GBP"),
        ],
    )

    assert not result.ok
    assert result.failed_index == 1
    assert policy.seen_assets == ["USD", "EUR"]
    # engine stops on first reject; GBP must not be seen


@pytest.mark.integration
def test_account_adjustment_integration_rollback_commits_on_success() -> None:
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = MutatingRecordingPolicy()
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            _make_balance_adjustment("USD"),
            _make_balance_adjustment("EUR"),
            _make_balance_adjustment("GBP"),
        ],
    )

    assert result.ok
    assert policy.seen_assets == ["USD", "EUR", "GBP"]


@pytest.mark.integration
def test_account_adjustment_integration_rollback_consistent_after_reject_first() -> (
    None
):
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = MutatingRecordingPolicy(reject_on_asset="USD")
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    adjustments = [
        _make_balance_adjustment("USD"),
        _make_balance_adjustment("EUR"),
        _make_balance_adjustment("GBP"),
    ]

    result1 = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert not result1.ok
    assert result1.failed_index == 0

    # Second run on same engine: engine state must be clean after rollback.
    result2 = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert not result2.ok
    assert result2.failed_index == 0
    # Both runs saw only USD before rejection; accumulated across two calls.
    assert policy.seen_assets == ["USD", "USD"]


@pytest.mark.integration
def test_account_adjustment_integration_rollback_consistent_after_reject_last() -> None:
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = MutatingRecordingPolicy(reject_on_asset="GBP")
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    adjustments = [
        _make_balance_adjustment("USD"),
        _make_balance_adjustment("EUR"),
        _make_balance_adjustment("GBP"),
    ]

    result1 = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert not result1.ok
    assert result1.failed_index == 2

    result2 = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert not result2.ok
    assert result2.failed_index == 2
    assert policy.seen_assets == ["USD", "EUR", "GBP", "USD", "EUR", "GBP"]


@pytest.mark.integration
def test_account_adjustment_integration_rollback_consistent_after_reject_middle() -> (
    None
):
    account_id = openpit.param.AccountId.from_int(99224416)
    policy = MutatingRecordingPolicy(reject_on_asset="EUR")
    engine = openpit.Engine.builder().no_sync().pre_trade(policy=policy).build()

    adjustments = [
        _make_balance_adjustment("USD"),
        _make_balance_adjustment("EUR"),
        _make_balance_adjustment("GBP"),
    ]

    result1 = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert not result1.ok
    assert result1.failed_index == 1

    # engine stops on first reject; GBP must not be seen in either run
    result2 = engine.apply_account_adjustment(
        account_id=account_id, adjustments=adjustments
    )
    assert not result2.ok
    assert result2.failed_index == 1
    assert policy.seen_assets == ["USD", "EUR", "USD", "EUR"]
