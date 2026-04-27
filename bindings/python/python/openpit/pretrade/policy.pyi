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

from .. import ExecutionReport, Order
from ..core import Mutation
from ._enum import RejectScope

class PreTradeContext: ...

@dataclasses.dataclass(frozen=True)
class PolicyReject:
    code: str
    reason: str
    details: str
    scope: RejectScope = RejectScope.ORDER

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

class CheckPreTradeStartPolicy(abc.ABC):
    @property
    @abc.abstractmethod
    def name(self) -> str: ...
    @abc.abstractmethod
    def check_pre_trade_start(
        self,
        ctx: PreTradeContext,
        order: Order,
    ) -> collections.abc.Iterable[PolicyReject]: ...
    @abc.abstractmethod
    def apply_execution_report(self, report: ExecutionReport) -> bool: ...

class PreTradePolicy(abc.ABC):
    @property
    @abc.abstractmethod
    def name(self) -> str: ...
    @abc.abstractmethod
    def perform_pre_trade_check(
        self,
        ctx: PreTradeContext,
        order: Order,
    ) -> PolicyDecision: ...
    @abc.abstractmethod
    def apply_execution_report(self, report: ExecutionReport) -> bool: ...
