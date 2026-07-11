// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

import { Engine } from "@openpit/engine";
import { TradeAmount } from "@openpit/engine/param";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";

const engine = Engine.builder().builtin(buildOrderValidation()).build();
const result = engine.executePreTrade({
  operation: {
    underlyingAsset: "AAPL",
    settlementAsset: "USD",
    accountId: 99224416,
    side: "BUY",
    tradeAmount: TradeAmount.quantity("100"),
    price: "185.00",
  },
});

if (!result.ok) {
  throw new Error(`order rejected: ${result.rejects.map(String).join(", ")}`);
}

if (result.reservation === undefined) {
  throw new Error("accepted order is missing a reservation");
}

result.reservation.rollback();
