import gc

import conftest
import openpit
import pytest


@pytest.mark.unit
def test_request_execute_is_single_use() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )
    request = engine.start_pre_trade(order=conftest.make_order()).request
    first = request.execute()
    assert first.ok
    first.reservation.rollback()

    with pytest.raises(RuntimeError, match="already been executed"):
        request.execute()


@pytest.mark.unit
def test_reservation_finalize_is_single_use() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )
    start_result = engine.start_pre_trade(order=conftest.make_order())
    reservation = start_result.request.execute().reservation
    reservation.commit()

    with pytest.raises(RuntimeError, match="already been finalized"):
        reservation.rollback()


@pytest.mark.unit
def test_implicit_reservation_rollback_exception_does_not_leak() -> None:
    class FailingRollbackPolicy(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "FailingRollbackPolicy"

        def perform_pre_trade_check(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> openpit.pretrade.PolicyPreTradeResult:
            del ctx, order

            def fail_rollback() -> None:
                raise RuntimeError("implicit rollback failed")

            return openpit.pretrade.PolicyPreTradeResult.accept(
                mutations=(
                    openpit.Mutation(commit=lambda: None, rollback=fail_rollback),
                )
            )

    source_engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=FailingRollbackPolicy())
        .build()
    )
    source_request = source_engine.start_pre_trade(order=conftest.make_order()).request
    reservations = [source_request.execute().reservation]

    class DropReservationPolicy(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "DropReservationPolicy"

        def perform_pre_trade_check(
            self,
            ctx: openpit.pretrade.Context,
            order: openpit.Order,
        ) -> None:
            del ctx, order
            reservations.clear()
            gc.collect()

    unrelated_engine = (
        openpit.Engine.builder()
        .no_sync()
        .pre_trade(policy=DropReservationPolicy())
        .build()
    )

    result = unrelated_engine.execute_pre_trade(order=conftest.make_order())

    assert result.ok
    assert result.reservation is not None
    result.reservation.rollback()
    assert reservations == []
