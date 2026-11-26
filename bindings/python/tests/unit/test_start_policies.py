import dataclasses
import datetime
import threading

import conftest
import openpit
import pytest


class _RejectPolicy(openpit.pretrade.Policy):
    @property
    def name(self) -> str:
        return "RejectPolicy"

    def check_pre_trade_start(
        self, ctx: openpit.pretrade.Context, order: openpit.Order
    ) -> tuple[openpit.pretrade.PolicyReject, ...]:
        return (
            openpit.pretrade.PolicyReject(
                code=openpit.pretrade.RejectCode.RISK_LIMIT_EXCEEDED,
                reason="limit",
                details="details",
            ),
        )


def _order_size_limit(
    quantity: str = "0",
) -> openpit.pretrade.policies.OrderSizeLimitReadyBuilder:
    policies = openpit.pretrade.policies
    return policies.build_order_size_limit().broker_barrier(
        policies.OrderSizeBrokerBarrier(
            limit=policies.OrderSizeLimit(max_quantity=openpit.param.Quantity(quantity))
        )
    )


@pytest.mark.unit
@pytest.mark.parametrize(
    "mode",
    [
        None,
        openpit.pretrade.RuleMode.REJECT,
        openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
        openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD,
        "ReviewWithHold",
    ],
)
@pytest.mark.parametrize("custom", [False, True])
def test_policy_instance_registration(
    mode: openpit.pretrade.RuleMode | str | None, custom: bool
) -> None:
    builder = openpit.Engine.builder().no_sync()
    ready = (
        builder.pre_trade(_RejectPolicy(), name="named-instance", mode=mode)
        if custom
        else builder.builtin(_order_size_limit(), name="named-instance", mode=mode)
    )
    engine = ready.build()
    result = engine.start_pre_trade(order=conftest.make_order())
    if mode is None or mode == "Reject":
        assert not result.ok
        assert len(result.rejects) == 1
        assert result.rejects[0].mode == "Reject"
    else:
        assert result.ok
        assert not result.rejects
        assert len(result.reviews) == 1
        assert result.reviews[0].mode == mode
        assert result.reviews[0].policy == (
            "RejectPolicy" if custom else "OrderSizeLimitPolicy"
        )


@pytest.mark.unit
def test_policy_instance_defaults_and_validation() -> None:
    policies = openpit.pretrade.policies
    builder = openpit.Engine.builder().no_sync()
    default_engine = (
        openpit.Engine.builder().no_sync().builtin(_order_size_limit()).build()
    )
    assert not default_engine.start_pre_trade(order=conftest.make_order()).ok
    validation_ready = (
        openpit.Engine.builder().no_sync().builtin(policies.build_order_validation())
    )
    for invalid in ["reviewwithhold", "reject", "", "Unknown"]:
        with pytest.raises(ValueError, match="^mode must be") as exc:
            builder.pre_trade(_RejectPolicy(), mode=invalid)
        assert "reject.mode" not in str(exc.value)
        with pytest.raises(ValueError, match="^mode must be"):
            builder.builtin(_order_size_limit(), mode=invalid)
        with pytest.raises(ValueError, match="^mode must be"):
            validation_ready.pre_trade(_RejectPolicy(), mode=invalid)
        with pytest.raises(ValueError, match="^mode must be"):
            validation_ready.builtin(_order_size_limit(), mode=invalid)
    with pytest.raises(ValueError, match="duplicate policy name"):
        openpit.Engine.builder().no_sync().builtin(_order_size_limit()).builtin(
            _order_size_limit()
        ).build()
    with pytest.raises(ValueError, match="duplicate policy name.*OrderSizeLimitPolicy"):
        openpit.Engine.builder().no_sync().builtin(
            _order_size_limit(), name=""
        ).builtin(_order_size_limit()).build()


@pytest.mark.unit
@pytest.mark.parametrize(
    "mode",
    [
        openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
        openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD,
    ],
)
def test_policy_instances_configure_independently(
    mode: openpit.pretrade.RuleMode,
) -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(_order_size_limit(), name="size-reject")
        .builtin(_order_size_limit("100"), name="size-review", mode=mode)
        .build()
    )
    for name, quantity in [("size-reject", "100"), ("size-review", "0")]:
        engine.configure().order_size_limit(
            name,
            broker=policies.OrderSizeBrokerBarrier(
                limit=policies.OrderSizeLimit(
                    max_quantity=openpit.param.Quantity(quantity)
                )
            ),
        )
    result = engine.start_pre_trade(order=conftest.make_order())
    assert result.ok
    assert not result.rejects
    assert len(result.reviews) == 1
    assert result.reviews[0].mode == mode
    assert result.reviews[0].policy == "OrderSizeLimitPolicy"
    assert result.reviews[0].code == openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT
    assert result.reviews[0].scope == openpit.pretrade.RejectScope.ORDER
    engine.configure().order_size_limit(
        "size-reject",
        broker=policies.OrderSizeBrokerBarrier(
            limit=policies.OrderSizeLimit(max_quantity=openpit.param.Quantity("0"))
        ),
    )
    rejected = engine.start_pre_trade(order=conftest.make_order())
    assert not rejected.ok
    assert len(rejected.rejects) == 1
    assert rejected.rejects[0].mode == "Reject"


@pytest.mark.unit
def test_evaluation_failure_in_review_mode_still_rejects() -> None:
    class ReviewPolicy(openpit.pretrade.Policy):
        @property
        def name(self) -> str:
            return "ReviewPolicy"

        def check_pre_trade_start(
            self, ctx: openpit.pretrade.Context, order: openpit.Order
        ) -> tuple[openpit.pretrade.PolicyReject, ...]:
            return (
                openpit.pretrade.PolicyReject(
                    code=openpit.pretrade.RejectCode.REFERENCE_DATA_UNAVAILABLE,
                    reason="reference data unavailable",
                    details="could not evaluate start-stage rule",
                    mode=openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD,
                ),
            )

    engine = openpit.Engine.builder().no_sync().pre_trade(policy=ReviewPolicy()).build()
    order = conftest.make_order()
    start = engine.start_pre_trade(order=order)
    dry_run = engine.start_pre_trade_dry_run(order=order)
    assert not start
    assert not dry_run
    assert start.request is None
    assert start.reviews == dry_run.reviews == []
    assert start.rejects[0].mode == openpit.pretrade.RuleMode.REJECT
    assert dry_run.rejects[0].mode == openpit.pretrade.RuleMode.REJECT


@pytest.mark.unit
def test_rate_limit_rejects_second_order_in_window() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .build()
    )

    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok
    second = engine.start_pre_trade(order=conftest.make_order())
    assert not second.ok
    assert len(second.rejects) == 1
    assert second.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_rate_limit_reject_and_review_share_target() -> None:
    policies = openpit.pretrade.policies
    mode = openpit.pretrade.RuleMode
    window = datetime.timedelta(hours=1)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit()
            .broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(2, window), mode=mode.REJECT
                )
            )
            .broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(1, window), mode=mode.REVIEW_WITHOUT_HOLD
                )
            )
        )
        .build()
    )
    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok
    assert first.reviews == []
    second = engine.start_pre_trade(order=conftest.make_order())
    assert second.ok
    assert len(second.reviews) == 1
    assert second.reviews[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED
    assert second.reviews[0].mode == mode.REVIEW_WITHOUT_HOLD
    third = engine.start_pre_trade(order=conftest.make_order())
    assert not third.ok
    assert len(third.rejects) == 1
    assert third.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED
    assert third.rejects[0].mode == mode.REJECT


@pytest.mark.unit
@pytest.mark.parametrize(
    "mode",
    [
        openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
        openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD,
    ],
)
def test_rate_limit_confirmed_spends_both_thresholds_and_drops_review_marks(
    mode: openpit.pretrade.RuleMode,
) -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(4, datetime.timedelta(hours=1)),
                    mode=openpit.pretrade.RuleMode.REJECT,
                ),
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(2, datetime.timedelta(hours=1)), mode=mode
                ),
            )
        )
        .build()
    )
    order = conftest.make_order()
    first = engine.start_pre_trade(order=order)
    assert first.ok
    assert not first.reviews
    confirmed = openpit.Order(
        operation=order.operation, review=openpit.OrderReview(confirmed=True)
    )
    second = engine.start_pre_trade(order=confirmed)
    assert second.ok
    assert not second.reviews
    third = engine.start_pre_trade(order=order)
    assert third.ok
    assert len(third.reviews) == 1
    assert third.reviews[0].mode == mode
    assert third.reviews[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED
    fourth = engine.start_pre_trade(order=confirmed)
    assert fourth.ok
    assert not fourth.reviews
    fifth = engine.start_pre_trade(order=order)
    assert not fifth.ok
    assert len(fifth.rejects) == 1
    assert fifth.rejects[0].mode == openpit.pretrade.RuleMode.REJECT
    assert fifth.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
@pytest.mark.parametrize("axis", ["broker", "asset", "account", "account_asset"])
def test_rate_limit_duplicate_keys_and_distinct_modes(
    axis: str, rate_limit_barriers: dict
) -> None:
    policies = openpit.pretrade.policies
    barrier = rate_limit_barriers[axis]
    builder = policies.build_rate_limit()
    add = getattr(builder, f"{axis}_barriers")
    with pytest.raises(ValueError, match="Reject"):
        openpit.Engine.builder().no_sync().builtin(add(barrier, barrier)).build()

    review = dataclasses.replace(
        barrier, mode=openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD
    )
    ready = getattr(policies.build_rate_limit(), f"{axis}_barriers")(barrier, review)
    if axis != "broker":
        ready.broker_barriers(rate_limit_barriers["broker"])
    engine = openpit.Engine.builder().no_sync().builtin(ready).build()
    assert engine.start_pre_trade(order=conftest.make_order()).ok
    duplicate = dataclasses.replace(
        barrier, limit=policies.RateLimit(10, datetime.timedelta(hours=1))
    )
    update = {f"{axis}_barriers": [duplicate, duplicate]}
    if axis != "broker":
        update["broker_barriers"] = [
            policies.RateLimitBrokerBarrier(
                policies.RateLimit(10, datetime.timedelta(hours=1))
            )
        ]
    with pytest.raises(openpit.PolicyConfigureError, match="Reject") as caught:
        engine.configure().rate_limit(policies.RateLimitBuilder.NAME, **update)
    assert caught.value.kind == openpit.ConfigureErrorKind.VALIDATION
    outside_target = conftest.make_order(
        account_id=openpit.param.AccountId.from_int(99224417),
        instrument=openpit.Instrument("MSFT", "EUR"),
    )
    assert not engine.start_pre_trade(order=outside_target).ok


@pytest.fixture
def rate_limit_barriers() -> dict:
    policies = openpit.pretrade.policies
    limit = policies.RateLimit(1, datetime.timedelta(hours=1))
    account = conftest.make_order().operation.account_id
    asset = openpit.param.Asset("USD")
    return {
        "broker": policies.RateLimitBrokerBarrier(limit),
        "asset": policies.RateLimitAssetBarrier(limit, asset),
        "account": policies.RateLimitAccountBarrier(limit, account),
        "account_asset": policies.RateLimitAccountAssetBarrier(limit, account, asset),
    }


@pytest.mark.unit
@pytest.mark.parametrize("axis", ["broker", "asset", "account", "account_asset"])
@pytest.mark.parametrize("phase", ["build", "configure"])
def test_rate_limit_none_mode_is_reject(
    axis: str, phase: str, rate_limit_barriers: dict
) -> None:
    policies = openpit.pretrade.policies
    barrier = dataclasses.replace(rate_limit_barriers[axis], mode=None)
    explicit = dataclasses.replace(barrier, mode=openpit.pretrade.RuleMode.REJECT)
    if phase == "build":
        with pytest.raises(ValueError, match="duplicate.*Reject"):
            (
                openpit.Engine.builder()
                .no_sync()
                .builtin(
                    getattr(policies.build_rate_limit(), f"{axis}_barriers")(
                        barrier, explicit
                    )
                )
                .build()
            )
        return
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(policies.build_rate_limit().broker_barriers())
        .build()
    )
    with pytest.raises(openpit.PolicyConfigureError, match="duplicate.*Reject"):
        engine.configure().rate_limit(
            policies.RateLimitBuilder.NAME,
            **{f"{axis}_barriers": [barrier, explicit]},
        )
    assert engine.start_pre_trade(order=conftest.make_order()).ok


@pytest.mark.unit
@pytest.mark.parametrize("axis", ["broker", "asset", "account", "account_asset"])
@pytest.mark.parametrize("mode", ["reject", "Unknown", 7])
@pytest.mark.parametrize("phase", ["build", "configure"])
def test_rate_limit_invalid_modes_use_construction_and_runtime_errors(
    axis: str, mode: object, phase: str, rate_limit_barriers: dict
) -> None:
    policies = openpit.pretrade.policies
    barrier = dataclasses.replace(rate_limit_barriers[axis], mode=mode)
    suffix = f", got {mode!r}" if isinstance(mode, str) else ""
    message = (
        "barrier.mode must be 'Reject', 'ReviewWithHold', or 'ReviewWithoutHold'"
        f"{suffix}"
    )
    if phase == "build":
        with pytest.raises(ValueError, match="mode must be") as caught:
            (
                openpit.Engine.builder()
                .no_sync()
                .builtin(
                    getattr(policies.build_rate_limit(), f"{axis}_barriers")(barrier)
                )
                .build()
            )
        assert str(caught.value) == message
        return
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().broker_barriers(rate_limit_barriers["broker"])
        )
        .build()
    )
    with pytest.raises(openpit.PolicyConfigureError, match="mode must be") as caught:
        engine.configure().rate_limit(
            policies.RateLimitBuilder.NAME, **{f"{axis}_barriers": [barrier]}
        )
    assert caught.value.kind == openpit.ConfigureErrorKind.VALIDATION
    assert str(caught.value).endswith(message)
    assert "ValueError:" not in str(caught.value)
    assert engine.start_pre_trade(order=conftest.make_order()).ok
    assert not engine.start_pre_trade(order=conftest.make_order()).ok


@pytest.mark.unit
def test_pnl_broker_barrier_defaults_to_reject() -> None:
    barrier = openpit.pretrade.policies.PnlBoundsBrokerBarrier
    usd = openpit.param.Asset("USD")
    lower_bound = openpit.param.Pnl("-100")
    assert barrier(usd, lower_bound).mode is openpit.pretrade.RuleMode.REJECT
    assert barrier(usd, lower_bound, mode=None).mode is openpit.pretrade.RuleMode.REJECT


@pytest.mark.unit
@pytest.mark.parametrize(
    ("mode", "spelling"),
    [
        (openpit.pretrade.RuleMode.REJECT, "Reject"),
        (openpit.pretrade.RuleMode.REVIEW_WITH_HOLD, "ReviewWithHold"),
        (openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD, "ReviewWithoutHold"),
    ],
)
def test_pnl_broker_barrier_accepts_enum_and_exact_string(
    mode: openpit.pretrade.RuleMode, spelling: str
) -> None:
    barrier = openpit.pretrade.policies.PnlBoundsBrokerBarrier
    usd = openpit.param.Asset("USD")
    lower_bound = openpit.param.Pnl("-100")
    assert barrier(usd, lower_bound, mode=mode).mode is mode
    assert barrier(usd, lower_bound, mode=spelling).mode is mode


@pytest.mark.unit
@pytest.mark.parametrize(
    ("mode", "suffix"),
    [
        ("reject", ", got 'reject'"),
        ("Review", ", got 'Review'"),
        ("Reject\n", r", got 'Reject\n'"),
        (1, ""),
    ],
)
def test_pnl_broker_barrier_rejects_invalid_mode(mode: object, suffix: str) -> None:
    with pytest.raises(ValueError) as exc_info:
        openpit.pretrade.policies.PnlBoundsBrokerBarrier(
            openpit.param.Asset("USD"), openpit.param.Pnl("-100"), mode=mode
        )
    expected = (
        "PnlBoundsBrokerBarrier.mode must be 'Reject', 'ReviewWithHold', "
        f"or 'ReviewWithoutHold'{suffix}"
    )
    assert str(exc_info.value) == expected


@pytest.mark.unit
@pytest.mark.parametrize("native", [False, True], ids=["dataclass", "native"])
def test_pnl_broker_barrier_ignores_mode_repr_override(native: bool) -> None:
    policies = openpit.pretrade.policies
    asset = openpit.param.Asset("USD")
    lower_bound = openpit.param.Pnl("-100")
    mode = conftest.ModeWithRaisingRepr("Review")

    with pytest.raises(ValueError) as exc_info:
        if native:
            barrier = policies.PnlBoundsBrokerBarrier(asset, lower_bound)
            object.__setattr__(barrier, "mode", mode)
            policy = policies.build_pnl_bounds_killswitch().broker_barriers(barrier)
            openpit.Engine.builder().no_sync().builtin(policy)
        else:
            policies.PnlBoundsBrokerBarrier(
                asset,
                lower_bound,
                mode=mode,  # type: ignore[arg-type]
            )
    assert str(exc_info.value) == (
        "PnlBoundsBrokerBarrier.mode must be 'Reject', 'ReviewWithHold', "
        "or 'ReviewWithoutHold', got 'Review'"
    )


@pytest.mark.unit
@pytest.mark.parametrize("axis", ["broker", "account"])
@pytest.mark.parametrize(
    "mode",
    [
        openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
        openpit.pretrade.RuleMode.REVIEW_WITHOUT_HOLD,
    ],
)
def test_pnl_kill_switch_breach_marks_order_for_review(
    mode: openpit.pretrade.RuleMode, axis: str
) -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    barrier = policies.PnlBoundsBrokerBarrier(
        openpit.param.Asset("USD"), openpit.param.Pnl("1"), mode=mode
    )
    builder = policies.build_pnl_bounds_killswitch()
    if axis == "broker":
        policy = builder.broker_barriers(barrier)
    else:
        policy = builder.account_barriers(
            policies.PnlBoundsAccountAssetBarrier(
                barrier, account_id, openpit.param.Pnl("0")
            )
        )
    engine = openpit.Engine.builder().no_sync().builtin(policy).build()

    order = conftest.make_order(account_id=account_id)
    first = engine.start_pre_trade(order=order)
    assert first.rejects == []
    assert [review.mode for review in first.reviews] == [mode]
    assert (
        first.reviews[0].code == openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    )
    assert first.request is not None
    executed = first.request.execute()
    assert executed.rejects == []
    assert [review.mode for review in executed.reviews] == [mode]
    assert executed.reservation is not None
    executed.reservation.rollback()

    next_order = openpit.Order(
        operation=order.operation, review=openpit.OrderReview(confirmed=True)
    )
    following = engine.start_pre_trade(order=next_order)
    assert following.ok
    assert following.reviews == []


@pytest.mark.unit
def test_pnl_kill_switch_reject_breach_blocks_account_in_pre_trade() -> None:
    policies = openpit.pretrade.policies
    usd = openpit.param.Asset("USD")
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(usd, openpit.param.Pnl("1"))
            )
        )
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert not result.ok
    assert result.reviews == []
    assert [reject.code for reject in result.rejects] == [
        openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
    ]
    assert result.rejects[0].mode == openpit.pretrade.RuleMode.REJECT

    engine.configure().pnl_bounds_killswitch(
        policies.PnlBoundsKillswitchBuilder.NAME,
        broker_barriers=[
            policies.PnlBoundsBrokerBarrier(usd, openpit.param.Pnl("-100"))
        ],
    )
    following = engine.start_pre_trade(order=conftest.make_order())
    assert not following.ok
    assert [reject.code for reject in following.rejects] == [
        openpit.pretrade.RejectCode.ACCOUNT_BLOCKED
    ]


@pytest.mark.unit
@pytest.mark.parametrize("mode", list(openpit.pretrade.RuleMode))
def test_pnl_kill_switch_execution_report_breach_blocks_only_in_reject_mode(
    mode: openpit.pretrade.RuleMode,
) -> None:
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    usd = openpit.param.Asset("USD")
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=usd,
                    lower_bound=openpit.param.Pnl("-100"),
                    mode=mode,
                )
            )
        )
        .build()
    )

    post_trade = engine.apply_execution_report(
        report=conftest.make_report(
            account_id=account_id, pnl=openpit.param.Pnl("-120")
        )
    )
    order = conftest.make_order(account_id=account_id)
    if mode is openpit.pretrade.RuleMode.REJECT:
        assert post_trade.account_blocks
        engine.configure().set_account_pnl(
            policies.PnlBoundsKillswitchBuilder.NAME,
            account=account_id,
            settlement_asset=usd,
            pnl=openpit.param.Pnl("0"),
        )
        following = engine.start_pre_trade(order=order)
        assert not following.ok
        assert [reject.code for reject in following.rejects] == [
            openpit.pretrade.RejectCode.PNL_KILL_SWITCH_TRIGGERED
        ]
        assert following.rejects[0].scope == "account"
    else:
        assert not post_trade.account_blocks
        following = engine.start_pre_trade(order=order)
        assert following.ok
        assert [review.mode for review in following.reviews] == [mode]


@pytest.mark.unit
def test_pnl_kill_switch_broker_barriers_are_unique_by_asset_and_mode() -> None:
    policies = openpit.pretrade.policies
    usd = openpit.param.Asset("USD")
    reject = policies.PnlBoundsBrokerBarrier(usd, openpit.param.Pnl("-100"))
    review = policies.PnlBoundsBrokerBarrier(
        usd, openpit.param.Pnl("-50"), mode=openpit.pretrade.RuleMode.REVIEW_WITH_HOLD
    )

    with pytest.raises(ValueError, match="duplicate broker barrier.*USD.*Reject"):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(reject, reject)
        ).build()

    openpit.Engine.builder().no_sync().builtin(
        policies.build_pnl_bounds_killswitch().broker_barriers(reject, review)
    ).build()


@pytest.mark.unit
def test_pnl_kill_switch_account_barriers_are_unique_by_account_asset_and_mode() -> (
    None
):
    policies = openpit.pretrade.policies
    barrier = policies.PnlBoundsAccountAssetBarrier(
        policies.PnlBoundsBrokerBarrier(
            openpit.param.Asset("USD"), openpit.param.Pnl("-100")
        ),
        openpit.param.AccountId.from_int(99224416),
        openpit.param.Pnl("0"),
    )

    with pytest.raises(ValueError, match="duplicate account barrier.*USD.*Reject"):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_pnl_bounds_killswitch().account_barriers(barrier, barrier)
        ).build()


@pytest.mark.unit
@pytest.mark.parametrize("review_initial_pnl", ["0", "1"])
def test_pnl_kill_switch_account_barriers_share_initial_pnl(
    review_initial_pnl: str,
) -> None:
    policies = openpit.pretrade.policies
    usd = openpit.param.Asset("USD")
    account_id = openpit.param.AccountId.from_int(99224416)
    reject = policies.PnlBoundsAccountAssetBarrier(
        policies.PnlBoundsBrokerBarrier(usd, openpit.param.Pnl("-100")),
        account_id,
        openpit.param.Pnl("0"),
    )
    review = policies.PnlBoundsAccountAssetBarrier(
        policies.PnlBoundsBrokerBarrier(
            usd,
            openpit.param.Pnl("-50"),
            mode=openpit.pretrade.RuleMode.REVIEW_WITH_HOLD,
        ),
        account_id,
        openpit.param.Pnl(review_initial_pnl),
    )
    builder = openpit.Engine.builder().no_sync()
    policy = policies.build_pnl_bounds_killswitch().account_barriers(reject, review)
    if review_initial_pnl == "0":
        builder.builtin(policy).build()
    else:
        with pytest.raises(ValueError, match="conflicting initial pnl.*USD"):
            builder.builtin(policy).build()


@pytest.mark.unit
def test_pnl_account_barrier_requires_initial_pnl() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(TypeError, match="initial_pnl"):
        policies.PnlBoundsAccountAssetBarrier(
            barrier=policies.PnlBoundsBrokerBarrier(
                settlement_asset=openpit.param.Asset("USD"),
                lower_bound=openpit.param.Pnl("-100"),
            ),
            account_id=openpit.param.AccountId.from_int(99224416),
        )


@pytest.mark.unit
def test_pnl_account_barrier_update_is_not_a_construction_barrier() -> None:
    policies = openpit.pretrade.policies
    update = policies.PnlBoundsAccountAssetBarrierUpdate(
        barrier=policies.PnlBoundsBrokerBarrier(
            settlement_asset=openpit.param.Asset("USD"),
            lower_bound=openpit.param.Pnl("-100"),
        ),
        account_id=openpit.param.AccountId.from_int(99224416),
    )

    with pytest.raises(TypeError, match="PnlBoundsAccountAssetBarrier"):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(policies.build_pnl_bounds_killswitch().account_barriers(update))
            .build()
        )


def _huge_broker_barrier() -> openpit.pretrade.policies.OrderSizeBrokerBarrier:
    policies = openpit.pretrade.policies
    return policies.OrderSizeBrokerBarrier(
        limit=policies.OrderSizeLimit(
            max_quantity=openpit.param.Quantity("1000000"),
            max_notional=openpit.param.Volume("1000000000"),
        )
    )


@pytest.mark.unit
@pytest.mark.parametrize(
    ("limit_asset", "quantity", "volume", "price", "expected_code"),
    [
        (
            "EUR",
            openpit.param.Quantity("1"),
            None,
            openpit.param.Price("100"),
            None,
        ),
        (
            "AAPL",
            openpit.param.Quantity("11"),
            None,
            openpit.param.Price("90"),
            openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT,
        ),
        (
            "AAPL",
            openpit.param.Quantity("10"),
            None,
            openpit.param.Price("100"),
            None,
        ),
        (
            "USD",
            openpit.param.Quantity("10"),
            None,
            openpit.param.Price("101"),
            openpit.pretrade.RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT,
        ),
        (
            "USD",
            openpit.param.Quantity("10"),
            None,
            openpit.param.Price("100"),
            None,
        ),
        (
            "USD",
            None,
            openpit.param.Volume("100"),
            openpit.param.Price("100"),
            None,
        ),
        (
            "USD",
            openpit.param.Quantity("10"),
            None,
            None,
            openpit.pretrade.RejectCode.ORDER_VALUE_CALCULATION_FAILED,
        ),
    ],
)
def test_order_size_limit_paths(
    limit_asset: str,
    quantity: openpit.param.Quantity | None,
    volume: openpit.param.Volume | None,
    price: openpit.param.Price | None,
    expected_code: str | None,
) -> None:
    policies = openpit.pretrade.policies
    if limit_asset == "AAPL":
        asset_limit = policies.OrderSizeLimit(max_quantity=openpit.param.Quantity("10"))
    else:
        asset_limit = policies.OrderSizeLimit(max_notional=openpit.param.Volume("1000"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=asset_limit,
                    asset=limit_asset,
                )
            )
        )
        .build()
    )
    trade_amount: openpit.param.TradeAmount | None
    if quantity is not None:
        trade_amount = openpit.param.TradeAmount.quantity(quantity)
    elif volume is not None:
        trade_amount = openpit.param.TradeAmount.volume(volume)
    else:
        trade_amount = None
    if price is None:
        order = openpit.Order(
            operation=openpit.OrderOperation(
                instrument=openpit.Instrument(
                    "AAPL",
                    "USD",
                ),
                side=openpit.param.Side.BUY,
                account_id=openpit.param.AccountId.from_int(99224416),
                trade_amount=trade_amount,
            ),
        )
    else:
        order = conftest.make_order(trade_amount=trade_amount, price=price)
    start_result = engine.start_pre_trade(order=order)

    if expected_code is None:
        assert start_result.ok
        start_result.request.execute().reservation.rollback()
    else:
        assert not start_result.ok
        assert len(start_result.rejects) == 1
        assert start_result.rejects[0].code == expected_code


@pytest.mark.unit
@pytest.mark.parametrize(
    ("limit", "expected_code"),
    [
        (
            openpit.pretrade.policies.OrderSizeLimit(
                max_quantity=openpit.param.Quantity("0")
            ),
            openpit.pretrade.RejectCode.ORDER_QTY_EXCEEDS_LIMIT,
        ),
        (
            openpit.pretrade.policies.OrderSizeLimit(
                max_notional=openpit.param.Volume("0")
            ),
            openpit.pretrade.RejectCode.ORDER_NOTIONAL_EXCEEDS_LIMIT,
        ),
    ],
)
def test_order_size_limit_explicit_zero_caps_reject(
    limit: openpit.pretrade.policies.OrderSizeLimit,
    expected_code: str,
) -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit().broker_barrier(
                policies.OrderSizeBrokerBarrier(limit=limit)
            )
        )
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == expected_code


@pytest.mark.unit
@pytest.mark.parametrize(
    "limit",
    [
        openpit.pretrade.policies.OrderSizeLimit(
            max_quantity=openpit.param.Quantity("10")
        ),
        openpit.pretrade.policies.OrderSizeLimit(
            max_notional=openpit.param.Volume("1000")
        ),
    ],
)
def test_order_size_limit_absence_is_not_zero(
    limit: openpit.pretrade.policies.OrderSizeLimit,
) -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit().broker_barrier(
                policies.OrderSizeBrokerBarrier(limit=limit)
            )
        )
        .build()
    )

    result = engine.start_pre_trade(
        order=conftest.make_order(
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("100"),
        )
    )
    assert result.ok
    result.request.execute().reservation.rollback()


@pytest.mark.unit
def test_order_size_limit_rejects_capless_limit() -> None:
    policies = openpit.pretrade.policies
    limit = policies.OrderSizeLimit()
    ready = policies.build_order_size_limit().broker_barrier(
        policies.OrderSizeBrokerBarrier(limit=limit)
    )
    with pytest.raises(
        ValueError,
        match=("^at least one of max_quantity or max_notional must be configured$"),
    ):
        (openpit.Engine.builder().no_sync().builtin(ready).build())


@pytest.mark.unit
def test_order_size_limit_rejects_duplicate_asset_key() -> None:
    policies = openpit.pretrade.policies
    limit = policies.OrderSizeLimit(max_quantity=openpit.param.Quantity("10"))
    ready = policies.build_order_size_limit().asset_barriers(
        policies.OrderSizeAssetBarrier(limit=limit, asset="AAPL"),
        policies.OrderSizeAssetBarrier(limit=limit, asset="AAPL"),
    )
    with pytest.raises(
        ValueError,
        match="^duplicate asset barrier for asset AAPL$",
    ):
        (openpit.Engine.builder().no_sync().builtin(ready).build())


@pytest.mark.unit
def test_order_size_limit_policy_asset_barrier_requires_asset_string() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises((TypeError, ValueError)):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity(10),
                        max_notional=openpit.param.Volume(1000),
                    ),
                    asset=123,  # type: ignore[arg-type]
                )
            )
        ).build()


@pytest.mark.unit
def test_pnl_kill_switch_requires_asset_string() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(TypeError, match="asset must be a str"):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=123,  # type: ignore[arg-type]
                    lower_bound=openpit.param.Pnl("-100"),
                )
            )
        ).build()


@pytest.mark.unit
def test_pnl_kill_switch_requires_at_least_one_bound() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_pnl_bounds_killswitch().broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                )
            )
        ).build()


@pytest.mark.unit
def test_rate_limit_asset_barrier_rejects_when_limit_is_exceeded() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().asset_barriers(
                policies.RateLimitAssetBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    ),
                    settlement_asset=openpit.param.Asset("USD"),
                )
            )
        )
        .build()
    )

    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok
    second = engine.start_pre_trade(order=conftest.make_order())
    assert not second.ok
    assert len(second.rejects) == 1
    assert second.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_rate_limit_account_barrier_independent_of_asset() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().account_barriers(
                policies.RateLimitAccountBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                )
            )
        )
        .build()
    )

    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok
    second = engine.start_pre_trade(order=conftest.make_order())
    assert not second.ok
    assert len(second.rejects) == 1
    assert second.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_rate_limit_account_asset_barrier_specific_to_pair() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit().account_asset_barriers(
                policies.RateLimitAccountAssetBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=60),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                    settlement_asset=openpit.param.Asset("USD"),
                )
            )
        )
        .build()
    )

    first = engine.start_pre_trade(order=conftest.make_order())
    assert first.ok

    second = engine.start_pre_trade(order=conftest.make_order())
    assert not second.ok
    assert second.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED

    third = engine.start_pre_trade(
        order=conftest.make_order(
            account_id=openpit.param.AccountId.from_int(11111111),
        )
    )
    assert third.ok


@pytest.mark.unit
def test_order_size_limit_account_asset_overrides_asset_baseline() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("5"),
                        max_notional=openpit.param.Volume("10000"),
                    ),
                    asset=openpit.param.Asset("AAPL"),
                )
            )
            .account_asset_barriers(
                policies.OrderSizeAccountAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("100"),
                        max_notional=openpit.param.Volume("10000"),
                    ),
                    account_id=openpit.param.AccountId.from_int(99224416),
                    asset=openpit.param.Asset("AAPL"),
                )
            )
        )
        .build()
    )

    result = engine.start_pre_trade(
        order=conftest.make_order(
            trade_amount=openpit.param.TradeAmount.quantity(
                openpit.param.Quantity("10")
            ),
            price=openpit.param.Price("100"),
        )
    )
    assert result.ok
    result.request.execute().reservation.rollback()


@pytest.mark.unit
def test_order_size_limit_unknown_asset_passes() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_order_size_limit()
            .broker_barrier(_huge_broker_barrier())
            .asset_barriers(
                policies.OrderSizeAssetBarrier(
                    limit=policies.OrderSizeLimit(
                        max_quantity=openpit.param.Quantity("1"),
                        max_notional=openpit.param.Volume("100"),
                    ),
                    asset=openpit.param.Asset("EUR"),
                )
            )
        )
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert result.ok
    result.request.execute().reservation.rollback()


@pytest.mark.unit
def test_with_full_sync_uses_full_locking_for_builtin_policies() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .full_sync()
        .builtin(
            policies.build_rate_limit().broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=100,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .build()
    )

    result = engine.start_pre_trade(order=conftest.make_order())
    assert result.ok


def _run_concurrent_start_pre_trade(
    engine: openpit.Engine,
    orders: list[openpit.Order],
    per_thread: int,
) -> None:
    errors: list[BaseException] = []

    def worker(order: openpit.Order) -> None:
        try:
            for _ in range(per_thread):
                result = engine.start_pre_trade(order=order)
                if not result.ok:
                    raise AssertionError(f"unexpected rejects: {result.rejects}")
                del result
        except BaseException as error:
            errors.append(error)

    threads = [threading.Thread(target=worker, args=(order,)) for order in orders]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    assert not errors


@pytest.mark.unit
def test_engine_full_sync_concurrent_start_pre_trade_is_safe() -> None:
    policies = openpit.pretrade.policies
    total_threads = 4
    per_thread = 500
    account_ids = [
        openpit.param.AccountId.from_int(index) for index in range(total_threads)
    ]

    engine = (
        openpit.Engine.builder()
        .full_sync()
        .builtin(
            policies.build_rate_limit().account_barriers(
                *[
                    policies.RateLimitAccountBarrier(
                        limit=policies.RateLimit(
                            max_orders=per_thread,
                            window=datetime.timedelta(seconds=60),
                        ),
                        account_id=account_id,
                    )
                    for account_id in account_ids
                ]
            )
        )
        .build()
    )

    orders = [conftest.make_order(account_id=account_id) for account_id in account_ids]
    _run_concurrent_start_pre_trade(engine, orders, per_thread)

    result = engine.start_pre_trade(
        order=conftest.make_order(account_id=account_ids[0])
    )
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_engine_full_sync_concurrent_broker_rate_limit_is_safe() -> None:
    policies = openpit.pretrade.policies
    total_threads = 4
    per_thread = 500
    total_calls = total_threads * per_thread

    engine = (
        openpit.Engine.builder()
        .full_sync()
        .builtin(
            policies.build_rate_limit().broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=total_calls,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .build()
    )

    orders = [conftest.make_order() for _ in range(total_threads)]
    _run_concurrent_start_pre_trade(engine, orders, per_thread)

    result = engine.start_pre_trade(order=conftest.make_order())
    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.RATE_LIMIT_EXCEEDED


@pytest.mark.unit
def test_spot_funds_builder_limit_only_mode() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder().no_sync().builtin(policies.build_spot_funds()).build()
    )
    assert engine is not None


@pytest.mark.unit
def test_builtin_policy_builders_reject_duplicate_non_default_group_id() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError, match="duplicate non-default policy_group_id: 7"):
        (
            openpit.Engine.builder()
            .no_sync()
            .builtin(policies.build_order_validation().with_policy_group_id(7))
            .builtin(policies.build_spot_funds().with_policy_group_id(7))
            .build()
        )


@pytest.mark.unit
def test_builtin_policy_builders_accept_non_default_group_id() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_rate_limit()
            .with_policy_group_id(1)
            .broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=10,
                        window=datetime.timedelta(seconds=60),
                    )
                )
            )
        )
        .builtin(
            policies.build_order_size_limit()
            .with_policy_group_id(2)
            .broker_barrier(_huge_broker_barrier())
        )
        .builtin(
            policies.build_pnl_bounds_killswitch()
            .with_policy_group_id(3)
            .broker_barriers(
                policies.PnlBoundsBrokerBarrier(
                    settlement_asset=openpit.param.Asset("USD"),
                    lower_bound=openpit.param.Pnl("-1000"),
                )
            )
        )
        .builtin(policies.build_order_validation().with_policy_group_id(4))
        .builtin(policies.build_spot_funds().with_policy_group_id(5))
        .build()
    )

    assert engine is not None


@pytest.mark.unit
def test_spot_funds_policy_group_id_tags_outcomes_and_lock_prices() -> None:
    policies = openpit.pretrade.policies
    policy_group_id = 7
    account_id = openpit.param.AccountId.from_int(99224416)
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(policies.build_spot_funds().with_policy_group_id(policy_group_id))
        .build()
    )

    seed_result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
                amount=openpit.AccountAdjustmentAmount(
                    balance=openpit.param.AdjustmentAmount.absolute(
                        openpit.param.PositionSize("10000")
                    )
                ),
            )
        ],
    )

    assert seed_result.ok
    assert seed_result.outcomes
    assert {outcome.policy_group_id for outcome in seed_result.outcomes} == {
        policy_group_id
    }

    result = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account_id,
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("200"),
        )
    )

    assert result.ok
    assert result.reservation is not None
    assert result.reservation.lock.entries() == [
        (policy_group_id, openpit.param.Price("200"))
    ]
    assert result.reservation.account_adjustments
    assert {
        outcome.policy_group_id for outcome in result.reservation.account_adjustments
    } == {policy_group_id}


@pytest.mark.unit
def test_spot_funds_insufficient_funds_reject_does_not_leak_account_id() -> None:
    # An insufficient-funds reject surfaced through the Python binding must not
    # carry the account id in its reason or details. The digit run 424242 is the
    # sentinel account id; the order operands never contain it.
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(424242)
    engine = (
        openpit.Engine.builder().no_sync().builtin(policies.build_spot_funds()).build()
    )

    # Buy 10 AAPL @ 200 = 2000 notional against an unfunded account: rejects with
    # insufficient funds.
    result = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account_id,
            trade_amount=openpit.param.TradeAmount.quantity("10"),
            price=openpit.param.Price("200"),
        )
    )

    assert not result.ok
    assert len(result.rejects) == 1
    assert result.rejects[0].code == openpit.pretrade.RejectCode.INSUFFICIENT_FUNDS
    assert "424242" not in result.rejects[0].reason
    assert "424242" not in result.rejects[0].details


def _mock_market_data(*quotes: tuple[str, str]) -> openpit.marketdata.MarketDataService:
    service = (
        openpit.Engine.builder()
        .full_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    for underlying, mark in quotes:
        instrument_id = service.register(openpit.Instrument(underlying, "USD"))
        service.push(
            instrument_id,
            openpit.marketdata.Quote(mark=mark),
            datetime.timedelta(),
        )
    return service


@pytest.mark.unit
def test_spot_funds_builder_market_data_with_quotes() -> None:
    policies = openpit.pretrade.policies
    service = _mock_market_data(("AAPL", "200"), ("BTC", "50000"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(service, global_slippage_bps=2000)
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_builder_market_data_zero_slippage() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                _mock_market_data(), global_slippage_bps=0
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_builder_market_data_max_slippage_accepted() -> None:
    policies = openpit.pretrade.policies
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                _mock_market_data(), global_slippage_bps=10_000
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_market_data_slippage_out_of_range_rejected() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_spot_funds().market_data(
                _mock_market_data(), global_slippage_bps=10_001
            )
        ).build()


@pytest.mark.unit
def test_spot_funds_full_engine_with_local_md_service_is_rejected() -> None:
    policies = openpit.pretrade.policies
    # No-sync MD service: derived from a no_sync() engine builder (no
    # full_sync upgrade).
    local_service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    with pytest.raises((ValueError, RuntimeError), match="multi-threaded|full_sync"):
        openpit.Engine.builder().full_sync().builtin(
            policies.build_spot_funds().market_data(
                local_service, global_slippage_bps=100
            )
        ).build()


@pytest.mark.unit
def test_spot_funds_local_engine_with_full_md_service_is_accepted() -> None:
    policies = openpit.pretrade.policies
    # Full MD service: derived from a no_sync() builder then upgraded via full_sync().
    full_service = (
        openpit.Engine.builder()
        .no_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .full_sync()
        .build()
    )
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                full_service, global_slippage_bps=100
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_rate_limit_zero_window_rejected_by_builder() -> None:
    policies = openpit.pretrade.policies
    with pytest.raises(ValueError, match="rate limit window must be positive"):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_rate_limit().broker_barriers(
                policies.RateLimitBrokerBarrier(
                    limit=policies.RateLimit(
                        max_orders=1,
                        window=datetime.timedelta(seconds=0),
                    )
                )
            )
        ).build()


def _mock_market_data_named(
    *quotes: tuple[str, str],
) -> tuple[
    openpit.marketdata.MarketDataService,
    dict[str, openpit.marketdata.InstrumentId],
]:
    service = (
        openpit.Engine.builder()
        .full_sync()
        .market_data(openpit.marketdata.QuoteTtl.infinite())
        .build()
    )
    ids: dict[str, openpit.marketdata.InstrumentId] = {}
    for underlying, mark in quotes:
        instrument_id = service.register(openpit.Instrument(underlying, "USD"))
        service.push(
            instrument_id,
            openpit.marketdata.Quote(mark=mark),
            datetime.timedelta(),
        )
        ids[underlying] = instrument_id
    return service, ids


def _seed_balance(engine: openpit.Engine, account_id: openpit.param.AccountId) -> None:
    result = engine.apply_account_adjustment(
        account_id=account_id,
        adjustments=[
            openpit.AccountAdjustment(
                operation=openpit.AccountAdjustmentBalanceOperation(asset="USD"),
                amount=openpit.AccountAdjustmentAmount(
                    balance=openpit.param.AdjustmentAmount.absolute(
                        openpit.param.PositionSize("10000")
                    )
                ),
            )
        ],
    )
    assert result.ok


@pytest.mark.unit
def test_spot_funds_override_instrument_only_accepted() -> None:
    """Instrument-only override is accepted and the engine builds."""
    policies = openpit.pretrade.policies
    service, ids = _mock_market_data_named(("AAPL", "200"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                global_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverrideEntry(
                        target=(
                            policies.SpotFundsOverrideTargetInstrument(
                                instrument=ids["AAPL"],
                            )
                        ),
                        override=policies.SpotFundsOverride(slippage_bps=0),
                    )
                ],
            )
        )
        .build()
    )
    assert engine is not None


@pytest.mark.unit
def test_spot_funds_override_account_scoped_accepted() -> None:
    """Account-scoped override is accepted and influences a pre-trade check."""
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    service, ids = _mock_market_data_named(("AAPL", "200"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                global_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverrideEntry(
                        target=(
                            policies.SpotFundsOverrideTargetInstrumentAccount(
                                instrument=ids["AAPL"],
                                account_id=account_id,
                            )
                        ),
                        override=policies.SpotFundsOverride(slippage_bps=0),
                    )
                ],
            )
        )
        .build()
    )
    _seed_balance(engine, account_id)

    result = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account_id,
            trade_amount=openpit.param.TradeAmount.quantity("1"),
            price=None,
        )
    )
    assert result.ok
    result.reservation.rollback()


@pytest.mark.unit
def test_spot_funds_override_group_scoped_accepted() -> None:
    """Group-scoped override is accepted and influences a pre-trade check."""
    policies = openpit.pretrade.policies
    account_id = openpit.param.AccountId.from_int(99224416)
    group_id = openpit.param.AccountGroupId.from_int(7)
    service, ids = _mock_market_data_named(("AAPL", "200"))
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            policies.build_spot_funds().market_data(
                service,
                global_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverrideEntry(
                        target=(
                            policies.SpotFundsOverrideTargetInstrumentAccountGroup(
                                instrument=ids["AAPL"],
                                account_group_id=group_id,
                            )
                        ),
                        override=policies.SpotFundsOverride(slippage_bps=0),
                    )
                ],
            )
        )
        .build()
    )
    engine.accounts().register_group([account_id], group_id)
    _seed_balance(engine, account_id)

    result = engine.execute_pre_trade(
        order=conftest.make_order(
            account_id=account_id,
            trade_amount=openpit.param.TradeAmount.quantity("1"),
            price=None,
        )
    )
    assert result.ok
    result.reservation.rollback()


@pytest.mark.unit
def test_spot_funds_override_rejects_unknown_target_entity() -> None:
    """Construction rejects objects outside the explicit target variants."""
    policies = openpit.pretrade.policies
    service, _ = _mock_market_data_named(("AAPL", "200"))
    with pytest.raises(
        TypeError,
        match=r"SpotFundsOverrideEntry\.target must be one of",
    ):
        openpit.Engine.builder().no_sync().builtin(
            policies.build_spot_funds().market_data(
                service,
                global_slippage_bps=500,
                overrides=[
                    policies.SpotFundsOverrideEntry(
                        target=object(),  # type: ignore[arg-type]
                        override=policies.SpotFundsOverride(slippage_bps=0),
                    )
                ],
            )
        ).build()
