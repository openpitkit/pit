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

import { expect } from "vitest";

import { type Engine } from "@openpit/engine";
import { AdjustmentAmount } from "@openpit/engine/param";

export function holdings(engine: Engine, accountId: bigint, asset: string) {
  const result = engine.applyAccountAdjustment(accountId, [
    {
      operation: { asset },
      amount: {
        balance: AdjustmentAmount.delta("0"),
        held: AdjustmentAmount.delta("0"),
        incoming: AdjustmentAmount.delta("0"),
      },
    },
  ]);
  expect(result.ok).toBe(true);
  const entry = result.outcomes[0]!.entry;
  return {
    balance: entry.balance!.absolute.toString(),
    held: entry.held!.absolute.toString(),
    incoming: entry.incoming!.absolute.toString(),
  };
}
