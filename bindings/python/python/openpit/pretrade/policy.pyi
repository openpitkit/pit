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
# Please see https://github.com/openpitkit and the OWNERS file for details.

from __future__ import annotations

import abc
import collections.abc
import dataclasses

from .. import (
    AccountAdjustment,
    AccountAdjustmentContext,
    ExecutionReport,
    Order,
)
from .._openpit import (
    AccountBlock,
    AccountOutcomeEntry,
    Context,
    PostTradeContext,
    PostTradeResult,
)
from ..core import Mutation
from ..param import AccountId, Price
from ._enum import RejectScope

@dataclasses.dataclass(frozen=True)
class PolicyReject:
    code: str
    reason: str
    details: str
    scope: RejectScope = RejectScope.ORDER
    user_data: int = 0

@dataclasses.dataclass(frozen=True)
class PolicyDecision:
    rejects: tuple[PolicyReject, ...] = ()
    mutations: tuple[Mutation, ...] = ()

    @classmethod
    def accept(
        cls,
        mutations: collections.abc.Iterable[Mutation] = (),
    ) -> PolicyDecision: ...
    @classmethod
    def reject(
        cls,
        rejects: collections.abc.Iterable[PolicyReject],
        mutations: collections.abc.Iterable[Mutation] = (),
    ) -> PolicyDecision: ...

@dataclasses.dataclass(frozen=True)
class PolicyPreTradeResult:
    rejects: tuple[PolicyReject, ...] = ()
    mutations: tuple[Mutation, ...] = ()
    account_adjustments: tuple[AccountOutcomeEntry, ...] = ()
    lock_prices: tuple[Price, ...] = ()

    @classmethod
    def accept(
        cls,
        mutations: collections.abc.Iterable[Mutation] = (),
        account_adjustments: collections.abc.Iterable[AccountOutcomeEntry] = (),
        lock_prices: collections.abc.Iterable[Price] = (),
    ) -> PolicyPreTradeResult: ...
    @classmethod
    def reject(
        cls,
        rejects: collections.abc.Iterable[PolicyReject],
        mutations: collections.abc.Iterable[Mutation] = (),
        account_adjustments: collections.abc.Iterable[AccountOutcomeEntry] = (),
        lock_prices: collections.abc.Iterable[Price] = (),
    ) -> PolicyPreTradeResult: ...

@dataclasses.dataclass(frozen=True)
class PolicyAccountAdjustmentResult:
    rejects: tuple[PolicyReject, ...] = ()
    mutations: tuple[Mutation, ...] = ()
    account_adjustments: tuple[AccountOutcomeEntry, ...] = ()
    account_blocks: tuple[AccountBlock, ...] = ()

class Policy(abc.ABC):
    @property
    @abc.abstractmethod
    def name(self) -> str: ...
    @property
    def policy_group_id(self) -> int: ...
    def check_pre_trade_start(
        self,
        ctx: Context,
        order: Order,
    ) -> collections.abc.Iterable[PolicyReject]: ...
    def perform_pre_trade_check(
        self,
        ctx: Context,
        order: Order,
    ) -> PolicyPreTradeResult: ...
    def apply_execution_report(
        self,
        ctx: PostTradeContext,
        report: ExecutionReport,
    ) -> PostTradeResult | None: ...
    def apply_account_adjustment(
        self,
        ctx: AccountAdjustmentContext,
        account_id: AccountId,
        adjustment: AccountAdjustment,
    ) -> PolicyAccountAdjustmentResult: ...
