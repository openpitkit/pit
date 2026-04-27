import conftest
import openpit
import pytest


class AcceptPolicy(openpit.pretrade.PreTradePolicy):
    # @typing.override
    @property
    def name(self) -> str:
        return "AcceptPolicy"

    # @typing.override
    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.accept()

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        _ = report
        return False


class NamedRejectPolicy(openpit.pretrade.PreTradePolicy):
    # @typing.override
    def __init__(self, *, policy_name: str) -> None:
        self._policy_name = policy_name

    # @typing.override
    @property
    def name(self) -> str:
        return self._policy_name

    # @typing.override
    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.PreTradeContext,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.accept()

    # @typing.override
    def apply_execution_report(
        self,
        *,
        report: openpit.ExecutionReport,
    ) -> bool:
        del report
        return False


class TaggedOrder(openpit.Order):
    # @typing.override
    def __init__(self, *, strategy_tag: str) -> None:
        super().__init__(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    "AAPL",
                    "USD",
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.TradeAmount.quantity(1),
                price=openpit.param.Price(10),
            ),
        )
        self.strategy_tag = strategy_tag


class MissingPriceOrder(openpit.Order):
    # @typing.override
    def __init__(self) -> None:
        super().__init__(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    "AAPL",
                    "USD",
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_u64(99224416),
                trade_amount=openpit.param.TradeAmount.quantity(1),
            ),
        )


@pytest.mark.unit
def test_engine_builder_supports_chaining_and_main_stage_policy() -> None:
    engine = (
        openpit.Engine.builder()
        .check_pre_trade_start_policy(
            policy=openpit.pretrade.policies.OrderValidationPolicy(),
        )
        .pre_trade_policy(policy=AcceptPolicy())
        .build()
    )

    start_result = engine.start_pre_trade(order=conftest.make_order())
    assert start_result.ok
    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.rollback()


@pytest.mark.unit
def test_builder_rejects_duplicate_policy_names() -> None:
    with pytest.raises(ValueError, match="duplicate policy name"):
        (
            openpit.Engine.builder()
            .pre_trade_policy(policy=NamedRejectPolicy(policy_name="dup"))
            .pre_trade_policy(policy=NamedRejectPolicy(policy_name="dup"))
            .build()
        )


@pytest.mark.unit
def test_engine_start_pre_trade_accepts_order_subclass() -> None:
    engine = openpit.Engine.builder().build()

    order = TaggedOrder(strategy_tag="alpha-1")
    start_result = engine.start_pre_trade(order=order)

    assert start_result.ok
    assert start_result.request is not None
    execute_result = start_result.request.execute()
    assert execute_result.ok
    execute_result.reservation.rollback()


@pytest.mark.unit
def test_engine_start_pre_trade_rejects_plain_python_object() -> None:
    engine = openpit.Engine.builder().build()

    with pytest.raises(TypeError, match="order must inherit from openpit.Order"):
        engine.start_pre_trade(order=object())


@pytest.mark.unit
def test_engine_start_pre_trade_accepts_order_subclass_without_price() -> None:
    engine = openpit.Engine.builder().build()

    start_result = engine.start_pre_trade(order=MissingPriceOrder())
    assert start_result.ok
    start_result.request.execute().reservation.rollback()
