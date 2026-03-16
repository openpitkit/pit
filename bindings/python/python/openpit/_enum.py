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

import enum
import typing

if hasattr(enum, "StrEnum"):
    StrEnum = enum.StrEnum
else:

    class StrEnum(str, enum.Enum):  # noqa: UP042
        # @typing.override
        def __str__(self) -> str:
            return self.value


class classproperty:
    def __init__(self, getter: typing.Callable[[type[typing.Any]], typing.Any]) -> None:
        self._getter = getter

    def __get__(self, _instance: object, owner: type[typing.Any]) -> typing.Any:
        return self._getter(owner)
