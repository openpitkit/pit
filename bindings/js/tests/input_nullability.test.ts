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

import { describe, expect, it } from "vitest";

import {
  AccountAdjustmentBounds,
  FinancialImpact,
  OrderOperation,
} from "@openpit/engine/model";
import { AdjustmentAmount, Trade, TradeAmount } from "@openpit/engine/param";

describe("required and optional domain inputs", () => {
  it("rejects nullish values at required runtime boundaries", () => {
    expect(() => new Trade(null as never, "1")).toThrow(TypeError);
    expect(() => new Trade("100", undefined as never)).toThrow(TypeError);
    expect(() => new FinancialImpact(null as never, "0")).toThrow(TypeError);
    expect(() => new FinancialImpact("0", undefined as never)).toThrow(
      TypeError,
    );
    expect(() => TradeAmount.quantity(null as never)).toThrow(TypeError);
    expect(() => TradeAmount.volume(undefined as never)).toThrow(TypeError);
    expect(() => AdjustmentAmount.delta(null as never)).toThrow(TypeError);
  });

  it("accepts nullish values at optional runtime boundaries", () => {
    const operation = new OrderOperation();
    operation.accountId = null;
    operation.price = undefined;
    expect(operation.accountId).toBeUndefined();
    expect(operation.price).toBeUndefined();

    const bounds = new AccountAdjustmentBounds();
    bounds.balanceUpper = null;
    bounds.incomingLower = undefined;
    expect(bounds.balanceUpper).toBeUndefined();
    expect(bounds.incomingLower).toBeUndefined();
  });
});
