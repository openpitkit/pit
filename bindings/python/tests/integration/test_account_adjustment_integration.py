import openpit
import pytest


class RecordingAdjustmentPolicy(openpit.AccountAdjustmentPolicy):
    def __init__(self, *, reject_on_asset: str | None = None) -> None:
        self.seen_account_ids: list[int] = []
        self.seen_assets: list[str] = []
        self._reject_on_asset = reject_on_asset

    # @typing.override
    @property
    def name(self) -> str:
        return "RecordingAdjustmentPolicy"

    # @typing.override
    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> list[openpit.pretrade.PolicyReject] | tuple[openpit.Mutation, ...] | None:
        self.seen_account_ids.append(account_id.value)
        asset = adjustment.operation.asset
        self.seen_assets.append(asset)
        if self._reject_on_asset == asset:
            return [
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.OTHER,
                    reason="rejected by test",
                    details=f"asset {asset} is blocked",
                    scope=openpit.pretrade.RejectScope.ORDER,
                )
            ]
        return None


# Policy that applies state immediately and returns a mutation per adjustment.
# The engine applies rollback actions in reverse order on batch failure.
# Rollback correctness is verified by running the same batch a second time on the same
# engine and confirming identical behaviour — if the engine had retained committed state
# from the first run, the second run would deviate.
class MutatingRecordingPolicy(openpit.AccountAdjustmentPolicy):
    def __init__(self, *, reject_on_asset: str | None = None) -> None:
        self.seen_assets: list[str] = []
        self._reject_on_asset = reject_on_asset

    # @typing.override
    @property
    def name(self) -> str:
        return "MutatingRecordingPolicy"

    # @typing.override
    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> list[openpit.pretrade.PolicyReject] | tuple[openpit.Mutation, ...] | None:
        asset = adjustment.operation.asset
        self.seen_assets.append(asset)
        if self._reject_on_asset == asset:
            return [
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.OTHER,
                    reason="rejected by test",
                    details=f"asset {asset} is blocked",
                    scope=openpit.pretrade.RejectScope.ORDER,
                )
            ]
        return (
            openpit.Mutation(
                commit=lambda: None,
                rollback=lambda: None,
            ),
        )


def _make_balance_adjustment(asset_code: str) -> openpit.AccountAdjustment:
    return openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(
            asset=asset_code,
        )
    )


@pytest.mark.integration
def test_account_adjustment_integration_successful_batch() -> None:
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = RecordingAdjustmentPolicy()
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = RecordingAdjustmentPolicy(reject_on_asset="USD")
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = RecordingAdjustmentPolicy(reject_on_asset="GBP")
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = RecordingAdjustmentPolicy(reject_on_asset="EUR")
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = MutatingRecordingPolicy()
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = MutatingRecordingPolicy(reject_on_asset="USD")
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = MutatingRecordingPolicy(reject_on_asset="GBP")
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
    account_id = openpit.param.AccountId.from_u64(99224416)
    policy = MutatingRecordingPolicy(reject_on_asset="EUR")
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

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
