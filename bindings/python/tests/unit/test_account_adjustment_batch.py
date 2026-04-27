import openpit
import pytest


def _make_adjustment(*, pending: str | None = None) -> openpit.AccountAdjustment:
    amount = None
    if pending is not None:
        amount = openpit.AccountAdjustmentAmount(
            pending=openpit.param.AdjustmentAmount.delta(
                openpit.param.PositionSize(pending)
            )
        )
    return openpit.AccountAdjustment(
        operation=openpit.AccountAdjustmentBalanceOperation(
            asset="USD",
        ),
        amount=amount,
    )


class PassAdjustmentPolicy(openpit.AccountAdjustmentPolicy):
    @property
    def name(self) -> str:
        return "PassAdjustmentPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> list[openpit.pretrade.PolicyReject] | None:
        _ = account_id, adjustment
        return None


class RejectOnPendingPolicy(openpit.AccountAdjustmentPolicy):
    def __init__(self) -> None:
        self.seen_pending: list[str | None] = []

    @property
    def name(self) -> str:
        return "RejectOnPendingPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> list[openpit.pretrade.PolicyReject] | None:
        _ = account_id
        pending = (
            None
            if adjustment.amount is None or adjustment.amount.pending is None
            else (
                str(adjustment.amount.pending.as_delta)
                if adjustment.amount.pending.is_delta
                and adjustment.amount.pending.as_delta is not None
                else None
            )
        )
        self.seen_pending.append(pending)
        if pending == "2":
            return [
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="blocked",
                    details="pending value 2 is forbidden",
                    scope=openpit.pretrade.RejectScope.ACCOUNT,
                )
            ]
        return None


class MutatingAdjustmentPolicy(openpit.AccountAdjustmentPolicy):
    """Policy that registers a kill-switch mutation on every adjustment."""

    @property
    def name(self) -> str:
        return "MutatingAdjustmentPolicy"

    def apply_account_adjustment(
        self,
        ctx: openpit.AccountAdjustmentContext,
        account_id: openpit.param.AccountId,
        adjustment: openpit.AccountAdjustment,
    ) -> openpit.pretrade.PolicyReject | tuple[openpit.Mutation, ...] | None:
        _ = account_id, adjustment
        return (
            openpit.Mutation(
                commit=lambda: None,
                rollback=lambda: None,
            ),
        )


@pytest.mark.unit
def test_account_adjustment_policy_passes_when_none_returned() -> None:
    engine = (
        openpit.Engine.builder()
        .account_adjustment_policy(policy=PassAdjustmentPolicy())
        .build()
    )

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_u64(99224416),
        adjustments=[_make_adjustment()],
    )

    assert result.ok
    assert result.failed_index is None
    assert result.rejects == []
    assert result


@pytest.mark.unit
def test_account_adjustment_policy_rejects_batch() -> None:
    engine = (
        openpit.Engine.builder()
        .account_adjustment_policy(policy=RejectOnPendingPolicy())
        .build()
    )

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_u64(99224416),
        adjustments=[_make_adjustment(pending="2")],
    )

    assert not result.ok
    assert result.failed_index == 0
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    assert result.rejects[0].reason == "blocked"
    assert not result


@pytest.mark.unit
def test_account_adjustment_batch_stops_on_first_reject() -> None:
    policy = RejectOnPendingPolicy()
    engine = openpit.Engine.builder().account_adjustment_policy(policy=policy).build()

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_u64(99224416),
        adjustments=[
            _make_adjustment(pending="1"),
            _make_adjustment(pending="2"),
            _make_adjustment(pending="3"),
        ],
    )

    assert not result.ok
    assert result.failed_index == 1
    assert policy.seen_pending == ["1", "2"]


@pytest.mark.unit
def test_account_adjustment_policy_passes_with_mutations() -> None:
    engine = (
        openpit.Engine.builder()
        .account_adjustment_policy(policy=MutatingAdjustmentPolicy())
        .build()
    )

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_u64(99224416),
        adjustments=[_make_adjustment()],
    )

    assert result.ok


@pytest.mark.unit
def test_account_adjustment_policy_none_and_mutations_interleaved() -> None:
    """First policy returns None, second returns mutations. Batch passes."""
    engine = (
        openpit.Engine.builder()
        .account_adjustment_policy(policy=PassAdjustmentPolicy())
        .account_adjustment_policy(policy=MutatingAdjustmentPolicy())
        .build()
    )

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_u64(99224416),
        adjustments=[_make_adjustment()],
    )

    assert result.ok


@pytest.mark.unit
def test_account_adjustment_mutations_do_not_prevent_reject() -> None:
    """First policy returns mutations, second rejects. Batch rejected."""
    engine = (
        openpit.Engine.builder()
        .account_adjustment_policy(policy=MutatingAdjustmentPolicy())
        .account_adjustment_policy(policy=RejectOnPendingPolicy())
        .build()
    )

    result = engine.apply_account_adjustment(
        account_id=openpit.param.AccountId.from_u64(99224416),
        adjustments=[_make_adjustment(pending="2")],
    )

    assert not result.ok
    assert result.failed_index == 0
    assert len(result.rejects) == 1


@pytest.mark.unit
def test_account_adjustment_rejects_raw_account_id_int() -> None:
    engine = openpit.Engine.builder().build()

    with pytest.raises(TypeError, match="account_id must be openpit.param.AccountId"):
        engine.apply_account_adjustment(
            account_id=99224416,  # type: ignore[arg-type]
            adjustments=[_make_adjustment()],
        )


@pytest.mark.unit
def test_account_adjustment_rejects_raw_account_id_str() -> None:
    engine = openpit.Engine.builder().build()

    with pytest.raises(TypeError, match="account_id must be openpit.param.AccountId"):
        engine.apply_account_adjustment(
            account_id="my-account",  # type: ignore[arg-type]
            adjustments=[_make_adjustment()],
        )
