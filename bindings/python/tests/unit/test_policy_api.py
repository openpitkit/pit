import openpit
import pytest
from conftest import make_order, make_report


class BlockAllStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    @property
    def name(self) -> str:
        return "BlockAllStartPolicy"

    def check_pre_trade_start(
        self, *, order: openpit.Order
    ) -> openpit.pretrade.PolicyReject | None:
        _ = order
        return openpit.pretrade.PolicyReject(
            code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
            reason="blocked by policy",
            details="test start policy reject",
            scope="account",
        )

    def apply_execution_report(
        self,
        *,
        report: openpit.pretrade.ExecutionReport,
    ) -> bool:
        _ = report
        return False


class ReportHookStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    @property
    def name(self) -> str:
        return "ReportHookStartPolicy"

    def check_pre_trade_start(
        self,
        *,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyReject | None:
        _ = order
        return None

    def apply_execution_report(
        self,
        *,
        report: openpit.pretrade.ExecutionReport,
    ) -> bool:
        _ = report
        return True


@pytest.mark.unit
def test_policy_reject_scope_validation() -> None:
    with pytest.raises(ValueError, match="scope must be either"):
        openpit.pretrade.PolicyReject(
            code=openpit.pretrade.RejectCode.OTHER,
            reason="invalid",
            details="invalid",
            scope="invalid",
        )


@pytest.mark.unit
def test_policy_decision_and_mutation_factories() -> None:
    mutation = openpit.pretrade.Mutation.reserve_notional(
        settlement_asset="USD",
        commit_amount="10",
        rollback_amount="0",
    )
    decision = openpit.pretrade.PolicyDecision.accept(mutations=[mutation])

    assert len(decision.rejects) == 0
    assert len(decision.mutations) == 1
    assert decision.mutations[0].commit.kind == "reserve_notional"
    assert decision.mutations[0].rollback.kind == "reserve_notional"


@pytest.mark.unit
def test_custom_start_policy_reject_is_returned_as_result() -> None:
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=BlockAllStartPolicy())
        .build()
    )

    result = engine.start_pre_trade(order=make_order())
    assert not result.ok
    assert result.reject.policy == "BlockAllStartPolicy"
    assert result.reject.scope == "account"


@pytest.mark.unit
def test_custom_start_policy_post_trade_hook_is_supported() -> None:
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=ReportHookStartPolicy())
        .build()
    )

    result = engine.apply_execution_report(report=make_report(pnl=1.0))
    assert result.kill_switch_triggered
