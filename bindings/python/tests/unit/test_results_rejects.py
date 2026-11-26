import types

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
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
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
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = ctx, report
        return False


class RejectWithMutationPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RejectWithMutationPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
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
                openpit.Mutation(
                    commit=lambda: None,
                    rollback=lambda: None,
                )
            ],
        )

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = ctx, report
        return False


class RejectWithUserDataTokenPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RejectWithUserDataTokenPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="main stage rejected",
                    details="token roundtrip",
                    user_data=0xCAFEBABE,
                )
            ]
        )

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = ctx, report
        return False


@pytest.mark.unit
def test_start_result_exposes_reject_without_exception() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )

    start_result = engine.start_pre_trade(
        order=conftest.make_order(trade_amount=openpit.param.TradeAmount.quantity(0))
    )
    assert not start_result
    assert start_result.request is None
    assert len(start_result.rejects) == 1
    assert start_result.rejects[0].policy == "OrderValidationPolicy"
    assert start_result.rejects[0].scope == "order"
    assert start_result.rejects[0].mode == openpit.pretrade.RuleMode.REJECT
    assert start_result.reviews == []
    assert repr(start_result) == "StartResult(ok=False, rejects=1)"


@pytest.mark.unit
def test_execute_result_exposes_rejects_without_exception() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(
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
    assert reject.mode == "Reject"
    assert type(reject.mode) is str
    assert ', mode="Reject",' in repr(reject)
    assert execute_result.reviews == []
    assert repr(execute_result) == "ExecuteResult(ok=False, rejects=1)"


@pytest.mark.unit
def test_execute_result_reject_with_mutations_still_has_no_reservation() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(
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
def test_execute_result_reject_roundtrips_integer_user_data_token() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RejectWithUserDataTokenPolicy())
        .build()
    )
    request = engine.start_pre_trade(order=conftest.make_order()).request
    execute_result = request.execute()

    assert not execute_result
    assert len(execute_result.rejects) == 1
    reject = execute_result.rejects[0]
    assert reject.user_data == 0xCAFEBABE


@pytest.mark.unit
def test_execute_result_reject_user_data_defaults_to_zero() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=AlwaysRejectPolicy())
        .build()
    )
    request = engine.start_pre_trade(order=conftest.make_order()).request
    execute_result = request.execute()

    assert not execute_result
    assert len(execute_result.rejects) == 1
    assert execute_result.rejects[0].user_data == 0


class RejectWithInvalidUserDataPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RejectWithInvalidUserDataPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.reject(
            rejects=[
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="main stage rejected",
                    details="invalid token type",
                    user_data="not-an-int",
                )
            ]
        )

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = ctx, report
        return False


@pytest.mark.unit
def test_policy_reject_without_mode_attribute_defaults_to_reject() -> None:
    class DefaultModePolicy(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "DefaultModePolicy"

        def check_pre_trade_start(
            self, ctx: openpit.pretrade.Context, order: openpit.Order
        ) -> tuple[types.SimpleNamespace, ...]:
            return (
                types.SimpleNamespace(
                    code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                    reason="risk limit reached",
                    details="custom reject without a mode",
                    scope=openpit.pretrade.RejectScope.ORDER,
                ),
            )

    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=DefaultModePolicy()).build()
    )
    start = engine.start_pre_trade(order=conftest.make_order())
    assert not start
    assert start.rejects[0].mode == openpit.pretrade.RuleMode.REJECT
    assert start.reviews == []


@pytest.mark.unit
def test_execute_result_reject_user_data_invalid_type_raises_value_error() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=RejectWithInvalidUserDataPolicy())
        .build()
    )
    request = engine.start_pre_trade(order=conftest.make_order()).request
    with pytest.raises(
        ValueError,
        match="reject.user_data must be an integer token \\(default 0\\)",
    ):
        request.execute()


@pytest.mark.unit
@pytest.mark.parametrize("mode", ["reject", "Review", " Reject", None, 1])
def test_invalid_policy_reject_mode_raises_at_construction(
    mode: str | int | None,
) -> None:
    with pytest.raises(
        ValueError, match="'Reject', 'ReviewWithHold', or 'ReviewWithoutHold'"
    ):
        openpit.pretrade.PolicyReject(
            code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
            reason="risk limit reached",
            details="invalid mode",
            mode=mode,  # type: ignore[arg-type]
        )


@pytest.mark.unit
@pytest.mark.parametrize("mode", ["Reject", "ReviewWithHold", "ReviewWithoutHold"])
def test_policy_reject_preserves_mode_string(mode: str) -> None:
    reject = openpit.pretrade.PolicyReject(
        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
        reason="risk limit reached",
        details="explicit mode",
        mode=mode,  # type: ignore[arg-type]
    )
    assert reject.mode is mode


@pytest.mark.unit
@pytest.mark.parametrize("mode", ["reject", "Review", None, 1])
def test_engine_rejects_invalid_mode_on_custom_object(mode: str | int | None) -> None:
    class InvalidModePolicy(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "InvalidModePolicy"

        def perform_pre_trade_check(
            self, ctx: openpit.pretrade.Context, order: openpit.Order
        ) -> openpit.pretrade.PolicyPreTradeResult:
            return openpit.pretrade.PolicyPreTradeResult.reject(
                rejects=[
                    types.SimpleNamespace(  # type: ignore[list-item]
                        code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                        reason="risk limit reached",
                        details="invalid mode",
                        scope=openpit.pretrade.RejectScope.ORDER,
                        mode=mode,
                    )
                ]
            )

    engine = (
        openpit.Engine.builder().no_sync().pre_trade(policy=InvalidModePolicy()).build()
    )
    order = conftest.make_order()
    request = engine.start_pre_trade(order=order).request
    assert request is not None
    for execute in (
        request.execute,
        lambda: engine.execute_pre_trade(order=order),
        lambda: engine.execute_pre_trade_dry_run(order=order),
    ):
        with pytest.raises(
            ValueError, match="'Reject', 'ReviewWithHold', or 'ReviewWithoutHold'"
        ):
            execute()


@pytest.mark.unit
def test_reject_error_is_exception_type() -> None:
    assert issubclass(openpit.RejectError, Exception)
