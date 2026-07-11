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

// Compile-only regression checks for required and optional boundary inputs.

import type { Accounts } from "@openpit/engine/accounts";
import {
  AccountAdjustmentBounds,
  FinancialImpact,
  OrderOperation,
} from "@openpit/engine/model";
import { AdjustmentAmount, Trade, TradeAmount } from "@openpit/engine/param";
import {
  Lock,
  type LockEntries,
  type PolicyAccountOutcomeEntry,
  type PolicyOutcomeAmount,
  type PolicyPnlOutcomeAmount,
} from "@openpit/engine/pretrade";
import {
  PnlBoundsBrokerBarrier,
  SpotFundsOverride,
} from "@openpit/engine/pretrade/policies";

// Optional model and policy inputs accept explicit null/undefined to clear or
// omit their values.
const operation = new OrderOperation();
operation.accountId = null;
operation.price = undefined;

const bounds = new AccountAdjustmentBounds();
bounds.balanceUpper = null;
bounds.incomingLower = undefined;

new PnlBoundsBrokerBarrier("USD", null, undefined);
new SpotFundsOverride(1, null, undefined, null);

// Public outcome interfaces and the full primitive-friendly lock entry shape
// are exported from the pretrade barrel.
const amount: PolicyOutcomeAmount = { delta: "1", absolute: 2n };
const pnlAmount: PolicyPnlOutcomeAmount = { delta: -1, absolute: "0" };
const outcome: PolicyAccountOutcomeEntry = {
  asset: "USD",
  balance: amount,
  realizedPnl: pnlAmount,
};
const lockEntries: LockEntries = [
  [0, "100.25"],
  [1, 101],
  [2, 102n],
];
new Lock(lockEntries);
void outcome;

declare const accounts: Accounts;

// Required domain inputs reject nullish values in the declaration surface.
// @ts-expect-error - a trade price is required.
new Trade(null, "1");
// @ts-expect-error - a trade quantity is required.
new Trade("100", undefined);
// @ts-expect-error - a financial-impact P&L is required.
new FinancialImpact(null, "0");
// @ts-expect-error - a financial-impact fee is required.
new FinancialImpact("0", undefined);
// @ts-expect-error - a quantity-denominated amount requires a quantity.
TradeAmount.quantity(null);
// @ts-expect-error - a volume-denominated amount requires a volume.
TradeAmount.volume(undefined);
// @ts-expect-error - an adjustment delta requires a position size.
AdjustmentAmount.delta(null);
// @ts-expect-error - an adjustment absolute value requires a position size.
AdjustmentAmount.absolute(undefined);
// @ts-expect-error - account controls require an account id.
accounts.block(null, "manual block");
// @ts-expect-error - group controls require an account-group id.
accounts.blockGroup(undefined, "manual block");
