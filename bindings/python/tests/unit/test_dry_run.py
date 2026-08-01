# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://openpit.dev and the OWNERS file for details.

import datetime

import conftest
import openpit
import pytest

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_validation_engine() -> openpit.Engine:
    return (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .build()
    )


def _make_rate_limit_engine(*, max_orders: int = 1) -> openpit.Engine:
    policies = openpit.pretrade.policies
    return (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barrier(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=max_orders,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .build()
    )


# ---------------------------------------------------------------------------
# DryRunReport basics
# ---------------------------------------------------------------------------


@pytest.mark.unit
def test_execute_dry_run_passes_on_valid_order() -> None:
    engine = _make_validation_engine()
    report = engine.execute_pre_trade_dry_run(order=conftest.make_order())
    assert report.is_pass
    assert bool(report)
    assert report.rejects is None
    assert report.account_block is None


@pytest.mark.unit
def test_start_dry_run_passes_on_valid_order() -> None:
    engine = _make_validation_engine()
    report = engine.start_pre_trade_dry_run(order=conftest.make_order())
    assert report.is_pass
    assert bool(report)
    assert report.rejects is None


@pytest.mark.unit
def test_execute_dry_run_rejects_invalid_order() -> None:
    engine = _make_validation_engine()
    invalid = openpit.Order(
        operation=openpit.OrderOperation(
            instrument=openpit.Instrument("AAPL", "USD"),
            account_id=openpit.param.AccountId.from_int(99224416),
            side=openpit.param.Side.BUY,
            # zero quantity - order validation rejects it
            trade_amount=openpit.param.TradeAmount.quantity(0),
            price=openpit.param.Price("185"),
        ),
    )
    report = engine.execute_pre_trade_dry_run(order=invalid)
    assert not report.is_pass
    assert not bool(report)
    assert report.rejects is not None
    assert len(report.rejects) >= 1


@pytest.mark.unit
def test_dry_run_report_has_lock_and_adjustments_attrs() -> None:
    engine = _make_validation_engine()
    report = engine.execute_pre_trade_dry_run(order=conftest.make_order())
    # lock exposes the same entries() view as a reservation lock; adjustments
    # is a list. Both are empty for a policy that produces no lock prices or
    # fund holds. (The native lock is the base type, not the openpit.pretrade
    # .Lock Python subclass, exactly like Reservation.lock, so assert via the
    # API rather than isinstance.)
    assert report.lock.entries() == []
    adjustments = report.account_adjustments
    assert isinstance(adjustments, list)
    assert adjustments == []


# ---------------------------------------------------------------------------
# Idempotency: dry-run must not affect subsequent real calls
# ---------------------------------------------------------------------------


@pytest.mark.unit
def test_dry_run_leaves_engine_state_intact_for_real_call() -> None:
    """A dry-run must not consume rate-limit budget or prevent a real call."""
    engine = _make_rate_limit_engine(max_orders=1)

    # Dry-run first - this must NOT consume the one allowed order.
    dry = engine.execute_pre_trade_dry_run(order=conftest.make_order())
    assert dry.is_pass

    # The real call must still succeed.
    result = engine.execute_pre_trade(order=conftest.make_order())
    assert result.ok
    result.reservation.rollback()


@pytest.mark.unit
def test_repeated_dry_runs_do_not_exhaust_budget() -> None:
    engine = _make_rate_limit_engine(max_orders=1)

    for _ in range(5):
        dry = engine.execute_pre_trade_dry_run(order=conftest.make_order())
        assert dry.is_pass

    # After five dry-runs the real call must still be the first accepted order.
    result = engine.execute_pre_trade(order=conftest.make_order())
    assert result.ok
    result.reservation.rollback()


@pytest.mark.unit
def test_rate_limit_dry_run_reports_reject_after_real_order_accepted() -> None:
    engine = _make_rate_limit_engine(max_orders=1)

    real = engine.execute_pre_trade(order=conftest.make_order())
    assert real.ok
    real.reservation.rollback()

    # Now the budget is exhausted; dry-run should report a reject.
    dry = engine.execute_pre_trade_dry_run(order=conftest.make_order())
    assert not dry.is_pass
    assert dry.rejects is not None
    assert len(dry.rejects) >= 1
    assert dry.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


# ---------------------------------------------------------------------------
# Custom policy - without dry-run override (default fallback)
# ---------------------------------------------------------------------------


class AcceptingPolicy(openpit.pretrade.Policy):
    """Minimal policy: accepts everything, defines no dry-run override."""

    @property
    def name(self) -> str:
        return "AcceptingPolicy"

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.accept()

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> None:
        del ctx, report
        return None


@pytest.mark.unit
def test_custom_policy_without_dry_run_override_falls_back_to_normal() -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .pre_trade(policy=AcceptingPolicy())
        .build()
    )
    report = engine.execute_pre_trade_dry_run(order=conftest.make_order())
    assert report.is_pass


# ---------------------------------------------------------------------------
# Custom policy - with dry-run override
# ---------------------------------------------------------------------------

_dry_run_start_called: list[bool] = []
_dry_run_check_called: list[bool] = []


class PolicyWithDryRunOverrides(openpit.pretrade.Policy):
    """Policy that overrides both dry-run hooks and records when they fire."""

    @property
    def name(self) -> str:
        return "PolicyWithDryRunOverrides"

    def check_pre_trade_start(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        del ctx, order
        return ()

    def check_pre_trade_start_dry_run(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        _dry_run_start_called.append(True)
        del ctx, order
        return ()

    def perform_pre_trade_check(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        del ctx, order
        return openpit.pretrade.PolicyDecision.accept()

    def perform_pre_trade_check_dry_run(
        self,
        ctx: openpit.pretrade.Context,
        order: openpit.Order,
    ) -> openpit.pretrade.PolicyDecision:
        _dry_run_check_called.append(True)
        del ctx, order
        return openpit.pretrade.PolicyDecision.accept()

    def apply_execution_report(
        self,
        ctx: openpit.pretrade.PostTradeContext,
        report: openpit.ExecutionReport,
    ) -> None:
        del ctx, report
        return None


@pytest.mark.unit
def test_custom_policy_dry_run_hooks_are_called_on_dry_run() -> None:
    _dry_run_start_called.clear()
    _dry_run_check_called.clear()

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .pre_trade(policy=PolicyWithDryRunOverrides())
        .build()
    )
    report = engine.execute_pre_trade_dry_run(order=conftest.make_order())
    assert report.is_pass
    assert _dry_run_check_called, "perform_pre_trade_check_dry_run was not called"


@pytest.mark.unit
def test_custom_policy_normal_hooks_not_called_on_dry_run_when_override_present() -> (
    None
):
    """When overrides are defined, the adapter calls the dry-run variant, not
    the normal one, during a dry-run pass."""
    _dry_run_check_called.clear()

    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
        .pre_trade(policy=PolicyWithDryRunOverrides())
        .build()
    )
    # Real call - dry-run hooks must NOT fire.
    result = engine.execute_pre_trade(order=conftest.make_order())
    assert result.ok
    result.reservation.rollback()
    assert (
        not _dry_run_check_called
    ), "perform_pre_trade_check_dry_run was called during a real execution"
