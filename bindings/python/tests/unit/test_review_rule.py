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

import conftest
import openpit
import pytest
from openpit.pretrade import RejectCode, RuleMode
from openpit.pretrade.policies import ReviewRule, build_review_rule

ACCOUNT = openpit.param.AccountId.from_int(1001)
OUTSIDE = openpit.param.AccountId.from_int(1002)
GROUP = openpit.param.AccountGroupId.from_int(7)
AAPL = openpit.Instrument("AAPL", "USD")
MSFT = openpit.Instrument("MSFT", "USD")


@pytest.mark.unit
@pytest.mark.parametrize("mode", [RuleMode.REJECT, RuleMode.REVIEW_WITH_HOLD])
def test_account_rule_uses_the_instance_mode(mode: RuleMode) -> None:
    engine = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(
            build_review_rule()
            .with_policy_group_id(3)
            .account_rules(ReviewRule(account_id=ACCOUNT)),
            name="account-review",
            mode=mode,
        )
        .build()
    )
    engine.accounts().register_group([ACCOUNT], GROUP)
    for instrument in (AAPL, MSFT):
        report = engine.execute_pre_trade_dry_run(
            conftest.make_order(account_id=ACCOUNT, instrument=instrument)
        )
        if mode == RuleMode.REJECT:
            assert not report.is_pass
            assert report.rejects
            assert [(r.code, r.mode) for r in report.rejects] == [
                (RejectCode.COMPLIANCE_RESTRICTION, mode)
            ]
            assert not report.reviews
        else:
            assert report.is_pass and not report.rejects
            assert [(r.code, r.mode) for r in report.reviews] == [
                (RejectCode.COMPLIANCE_RESTRICTION, mode)
            ]
    other = engine.execute_pre_trade_dry_run(
        conftest.make_order(account_id=OUTSIDE, instrument=AAPL)
    )
    assert other.is_pass and not other.rejects and not other.reviews


@pytest.mark.unit
@pytest.mark.parametrize(
    ("axis", "rule", "level"),
    [
        ("global_rule", ReviewRule(account_id=ACCOUNT), "global"),
        ("account_group_rules", ReviewRule(account_id=ACCOUNT), "account group"),
        ("account_rules", ReviewRule(account_group_id=GROUP), "account"),
        (
            "account_rules",
            ReviewRule(account_id=ACCOUNT, account_group_id=GROUP),
            "account",
        ),
    ],
)
def test_builder_refuses_ids_from_another_level(
    axis: str, rule: ReviewRule, level: str
) -> None:
    builder = build_review_rule()
    with pytest.raises(ValueError, match=f"incompatible IDs for the {level} level"):
        getattr(builder, axis)(rule)
    engine = openpit.Engine.builder().no_sync().builtin(builder.account_rules()).build()
    assert engine.execute_pre_trade_dry_run(
        conftest.make_order(account_id=ACCOUNT)
    ).is_pass


@pytest.mark.unit
@pytest.mark.parametrize(
    ("axis", "raw_rule"),
    [
        ("global_rule", {"instruments": ["AAPL/USD"]}),
        ("account_group_rules", {"account_group_id": 7, "instruments": ["AAPL/USD"]}),
        ("account_rules", {"account_id": 1001, "instruments": ["AAPL/USD"]}),
    ],
)
def test_dict_rule_leaves_builder_and_configurator_rules_intact(
    axis: str, raw_rule: dict[str, int | list[str]]
) -> None:
    name = "restricted-instruments"
    builder = build_review_rule().global_rule(ReviewRule(instruments=[MSFT]))
    message = r"^expected openpit\.pretrade\.policies\.ReviewRule$"
    with pytest.raises(TypeError, match=message):
        getattr(builder, axis)(raw_rule)

    engine = openpit.Engine.builder().no_sync().builtin(builder, name=name).build()
    engine.accounts().register_group([ACCOUNT], GROUP)
    options = {axis: raw_rule if axis == "global_rule" else [raw_rule]}
    with pytest.raises(TypeError, match=message):
        engine.configure().review_rule(name, **options)

    admitted = engine.execute_pre_trade_dry_run(
        conftest.make_order(account_id=ACCOUNT, instrument=AAPL)
    )
    assert admitted.is_pass and not admitted.rejects and not admitted.reviews
    restricted = engine.execute_pre_trade_dry_run(
        conftest.make_order(account_id=ACCOUNT, instrument=MSFT)
    )
    assert [reject.code for reject in restricted.rejects or []] == [
        RejectCode.COMPLIANCE_RESTRICTION
    ]


@pytest.mark.unit
def test_account_rules_refuses_the_whole_batch() -> None:
    builder = build_review_rule().account_rules()
    with pytest.raises(ValueError, match="incompatible IDs for the account level"):
        builder.account_rules(
            ReviewRule(account_id=ACCOUNT), ReviewRule(account_group_id=GROUP)
        )

    engine = openpit.Engine.builder().no_sync().builtin(builder).build()
    report = engine.execute_pre_trade_dry_run(
        conftest.make_order(account_id=ACCOUNT, instrument=AAPL)
    )
    assert report.is_pass


@pytest.mark.unit
@pytest.mark.parametrize(
    ("axis", "rules", "message"),
    [
        (
            "global_rule",
            [ReviewRule(instruments=[])],
            "an exact instrument list must not be empty",
        ),
        (
            "global_rule",
            [ReviewRule(instruments=[AAPL, AAPL])],
            "duplicate review rule instrument AAPL/USD",
        ),
        (
            "account_group_rules",
            [ReviewRule(account_group_id=GROUP)] * 2,
            "duplicate account group review rule",
        ),
        (
            "account_rules",
            [ReviewRule(account_id=ACCOUNT)] * 2,
            "duplicate account review rule",
        ),
    ],
)
def test_invalid_rules_do_not_register_an_instance(
    axis: str, rules: list[ReviewRule], message: str
) -> None:
    builder = (
        openpit.Engine.builder()
        .no_sync()
        .builtin(openpit.pretrade.policies.build_order_validation())
    )
    with pytest.raises(ValueError, match=message):
        builder.builtin(getattr(build_review_rule(), axis)(*rules), name="rules")
    engine = builder.builtin(build_review_rule().account_rules(), name="rules").build()
    assert engine.execute_pre_trade_dry_run(
        conftest.make_order(account_id=ACCOUNT)
    ).is_pass
