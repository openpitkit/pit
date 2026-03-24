import conftest
import openpit
import pytest


class AlwaysRejectPolicy(openpit.pretrade.Policy):
    # @typing.override
    @property
    def name(self) -> str:
        return "AlwaysRejectPolicy"

    # @typing.override
    def perform_pre_trade_check(
        self, *, context: openpit.pretrade.PolicyContext
    ) -> openpit.pretrade.PolicyDecision:
        _ = context
        return openpit.pretrade.PolicyDecision.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="main stage rejected",
                    details="synthetic reject",
                    scope=openpit.pretrade.RejectScope.ORDER,
                )
            ]
        )

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


class RejectWithMutationPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RejectWithMutationPolicy"

    def perform_pre_trade_check(
        self, *, context: openpit.pretrade.PolicyContext
    ) -> openpit.pretrade.PolicyDecision:
        _ = context
        return openpit.pretrade.PolicyDecision.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="main stage rejected with mutation",
                    details="synthetic reject with mutation",
                    scope=openpit.pretrade.RejectScope.ORDER,
                )
            ],
            mutations=[
                openpit.pretrade.Mutation(
                    commit=lambda: None,
                    rollback=lambda: None,
                )
            ],
        )

    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


@pytest.mark.unit
def test_start_result_exposes_reject_without_exception() -> None:
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .build()
    )

    start_result = engine.start_pre_trade(
        order=conftest.make_order(trade_amount=openpit.param.Quantity("0"))
    )
    assert not start_result
    assert start_result.request is None
    assert start_result.reject is not None
    assert (
        start_result.reject.policy
        == openpit.pretrade.policies.OrderValidationPolicy.NAME
    )
    assert start_result.reject.scope == "order"
    assert "StartPreTradeResult" in repr(start_result)


@pytest.mark.unit
def test_execute_result_exposes_rejects_without_exception() -> None:
    engine = (
        openpit.Engine.builder()
        .pre_trade_policy(
            policy=AlwaysRejectPolicy(),
        )
        .build()
    )
    request = engine.start_pre_trade(order=conftest.make_order()).request
    execute_result = request.execute()

    assert not execute_result
    assert execute_result.reservation is None
    assert len(execute_result.rejects) == 1
    reject = execute_result.rejects[0]
    assert reject.policy == "AlwaysRejectPolicy"
    assert reject.code == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    assert reject.scope == "order"
    assert "ExecuteResult" in repr(execute_result)


@pytest.mark.unit
def test_execute_result_reject_with_mutations_still_has_no_reservation() -> None:
    engine = (
        openpit.Engine.builder()
        .pre_trade_policy(
            policy=RejectWithMutationPolicy(),
        )
        .build()
    )
    request = engine.start_pre_trade(order=conftest.make_order()).request
    execute_result = request.execute()

    assert not execute_result
    assert execute_result.reservation is None
    assert len(execute_result.rejects) == 1
    reject = execute_result.rejects[0]
    assert reject.policy == "RejectWithMutationPolicy"
    assert reject.code == openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED
    assert reject.scope == "order"


@pytest.mark.unit
def test_reject_error_is_exception_type() -> None:
    assert issubclass(openpit.RejectError, Exception)
