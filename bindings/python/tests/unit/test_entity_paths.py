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

import openpit
from openpit.pretrade import policies


def test_pretrade_entities_have_rust_like_module_paths() -> None:
    assert hasattr(openpit, "Instrument")
    assert hasattr(openpit, "Order")
    assert hasattr(openpit, "ExecutionReport")
    assert hasattr(openpit, "PostTradeResult")
    assert hasattr(openpit, "Mutation")
    assert not hasattr(openpit, "Request")
    assert not hasattr(openpit, "Reservation")
    assert not hasattr(openpit, "Reject")
    assert not hasattr(openpit, "RejectCode")

    assert not hasattr(openpit.pretrade, "Order")
    assert openpit.Instrument.__module__ == "openpit.core"
    assert openpit.Order.__module__ == "openpit.core"
    assert openpit.ExecutionReport.__module__ == "openpit.core"
    assert openpit.Mutation.__module__ == "openpit.core"
    assert openpit.pretrade.Request.__module__ == "openpit.pretrade"
    assert openpit.pretrade.Reservation.__module__ == "openpit.pretrade"
    assert not hasattr(openpit.pretrade, "PreTradeRequest")
    assert not hasattr(openpit.pretrade, "PreTradeReservation")
    assert not hasattr(openpit.pretrade, "ExecutionReport")
    assert not hasattr(openpit.pretrade, "Mutation")
    assert openpit.pretrade.Reject.__module__ == "openpit.pretrade"
    assert openpit.pretrade.RejectCode.__module__ == "openpit.pretrade"
    assert openpit.PostTradeResult.__module__ == "openpit.pretrade"
    assert openpit.pretrade.PostTradeResult is openpit.PostTradeResult
    assert openpit.pretrade.AccountBlock.__module__ == "openpit.pretrade"
    assert openpit.pretrade.OutcomeAmount.__module__ == "openpit.pretrade"
    assert openpit.pretrade.PnlHaltReason.__module__ == "openpit.pretrade"
    assert openpit.pretrade.PnlOutcome.__module__ == "openpit.pretrade"
    assert openpit.pretrade.PnlOutcomeAmount.__module__ == "openpit.pretrade"
    assert openpit.pretrade.AccountPnlOutcome.__module__ == "openpit.pretrade"
    assert openpit.pretrade.AccountOutcomeEntry.__module__ == "openpit.pretrade"
    assert openpit.pretrade.AccountAdjustmentOutcome.__module__ == "openpit.pretrade"

    assert openpit.account_adjustment.Adjustment.__module__ == (
        "openpit.account_adjustment"
    )
    assert openpit.account_adjustment.Amount.__module__ == "openpit.account_adjustment"
    assert openpit.account_adjustment.AccountPnlOperation.__module__ == (
        "openpit.account_adjustment"
    )
    assert not hasattr(openpit.account_adjustment, "AccountAdjustment")
    assert not hasattr(openpit.account_adjustment, "AccountAdjustmentAmount")


def test_builtins_policies_have_rust_like_module_paths() -> None:
    assert not hasattr(openpit, "OrderSizeLimit")
    assert not hasattr(openpit, "PnlBoundsBrokerBarrier")
    assert not hasattr(openpit, "PnlBoundsAccountAssetBarrier")
    assert not hasattr(openpit, "PnlBoundsAccountAssetBarrierUpdate")

    assert policies.OrderSizeLimit.__module__ == "openpit.pretrade.policies"
    assert policies.PnlBoundsBrokerBarrier.__module__ == "openpit.pretrade.policies"
    assert (
        policies.PnlBoundsAccountAssetBarrier.__module__ == "openpit.pretrade.policies"
    )
    assert (
        policies.PnlBoundsAccountAssetBarrierUpdate.__module__
        == "openpit.pretrade.policies"
    )
    assert (
        policies.SpotFundsOverrideTargetInstrument.__module__
        == "openpit.pretrade.policies"
    )
    assert (
        policies.SpotFundsOverrideTargetInstrumentAccount.__module__
        == "openpit.pretrade.policies"
    )
    assert (
        policies.SpotFundsOverrideTargetInstrumentAccountGroup.__module__
        == "openpit.pretrade.policies"
    )


def test_configurator_entities_have_rust_like_module_paths() -> None:
    # Configurator is accessor-only: reached via Engine.configure(), never
    # constructed or imported at the package top level.
    assert not hasattr(openpit, "Configurator")
    assert callable(openpit.Engine.configure)
    assert openpit.PolicyConfigureError.__module__ == "openpit"
    assert openpit.ConfigureErrorKind.__module__ == "openpit"
    assert (
        openpit.ConfigureErrorKind.NESTED_CONFIGURATION
        != openpit.ConfigureErrorKind.VALIDATION
    )
