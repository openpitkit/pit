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

// See engine.test.ts for the import-resolution scheme. Run `npm run build`
// first.
import { AccountId, Price } from "@openpit/engine/param";
import { OrderOperation } from "@openpit/engine/model";

// Wrapper-valued setters borrow and clone at the wasm boundary. Assigning a
// wrapper must never invalidate the caller's handle.

describe("wrapper setters are non-consuming", () => {
  it("keeps an AccountId usable after direct assignment", () => {
    const account = AccountId.fromInt(99224416n);
    const operation = new OrderOperation();

    operation.accountId = account;

    expect(account.value).toBe(99224416n);
    expect(account.toString()).toBe("99224416");
    const second = new OrderOperation();
    second.accountId = account;
    expect(account.equals(AccountId.fromInt(99224416n))).toBe(true);
  });

  it("keeps a Price usable after direct assignment", () => {
    const price = Price.fromString("185.00");
    const operation = new OrderOperation();

    operation.accountId = AccountId.fromInt(1n);
    operation.price = price;

    expect(price.toString()).toBe("185.00");
    const second = new OrderOperation();
    second.accountId = AccountId.fromInt(2n);
    second.price = price;
    expect(price.toString()).toBe("185.00");
  });
});
