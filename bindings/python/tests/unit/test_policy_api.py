import conftest
import openpit
import pytest


class BlockAllStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    # @typing.override
    @property
    def name(self) -> str:
        return "BlockAllStartPolicy"

    # @typing.override
    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx, order
        return (
            openpit.pretrade.PolicyReject(
                code=openpit.pretrade.RejectCode.COMPLIANCE_RESTRICTION,
                reason="blocked by policy",
                details="test start policy reject",
                scope=openpit.pretrade.RejectScope.ACCOUNT,
            ),
        )

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


class ReportHookStartPolicy(openpit.pretrade.CheckPreTradeStartPolicy):
    # @typing.override
    @property
    def name(self) -> str:
        return "ReportHookStartPolicy"

    # @typing.override
    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx, order
        return ()

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return True


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
def test_custom_start_policy_reject_is_returned_as_result() -> None:
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=BlockAllStartPolicy())
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].policy == "BlockAllStartPolicy"
    assert result.rejects[0].scope == "account"


@pytest.mark.unit
def test_custom_start_policy_post_trade_hook_is_supported() -> None:
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(policy=ReportHookStartPolicy())
        .build()
    )

    result = engine.apply_execution_report(
        report=conftest.make_report(pnl=openpit.param.Pnl("1"))
    )
    assert result.kill_switch_triggered
