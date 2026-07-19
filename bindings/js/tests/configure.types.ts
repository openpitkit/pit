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

import { type Configurator, Engine } from "@openpit/engine";
import { AccountGroupId } from "@openpit/engine/param";
import { PnlHaltReason } from "@openpit/engine/pretrade";
import {
  buildRateLimit,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
  PnlBoundsBrokerBarrier,
  RateLimit,
  RateLimitBrokerBarrier,
  RateLimitBuilder,
  SpotFundsBuilder,
  SpotFundsLimitMode,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsBarrier,
  SpotFundsPnlBoundsKillswitchBuilder,
} from "@openpit/engine/pretrade/policies";

const configurator: Configurator = Engine.builder()

  .builtin(
    buildRateLimit().brokerBarrier(
      new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
    ),
  )
  .build()
  .configure();

configurator.rateLimit(RateLimitBuilder.NAME, {
  broker: new RateLimitBrokerBarrier(new RateLimit(2, 60_000)),
});

const spotFundsName: string = SpotFundsBuilder.NAME;
const spotFundsPnlName: string = SpotFundsPnlBoundsKillswitchBuilder.NAME;
void [spotFundsName, spotFundsPnlName];

configurator.spotFunds(SpotFundsBuilder.NAME, {
  globalLimitMode: SpotFundsLimitMode.TrackOnly,
  accountLimitModes: [
    { accountId: 99_224_416n, mode: SpotFundsLimitMode.TrackOnly },
    { accountId: 99_224_417n, mode: null },
  ],
  accountGroupLimitModes: [{ accountGroupId: 7, mode: null }],
});

configurator.spotFundsPnlBoundsKillswitch(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    globalBarrier: new SpotFundsPnlBoundsBarrier("-500", undefined),
    accountBarriers: [
      new SpotFundsPnlBoundsAccountBarrier(
        99_224_416n,
        new SpotFundsPnlBoundsBarrier("-250", "250"),
      ),
    ],
  },
);

const numericPnlConfiguration = configurator.setSpotFundsAccountPnl(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    account: 99_224_416n,
    state: "-120",
  },
);
void numericPnlConfiguration.accountBlocks;

const haltedPnlConfiguration = configurator.setSpotFundsAccountPnl(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    account: 99_224_416n,
    state: PnlHaltReason.fromMissingFx(),
  },
);
void haltedPnlConfiguration.accountBlocks;

configurator.setSpotFundsAccountPnl(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
  account: 99_224_416n,
  state: "-120",
  // @ts-expect-error - spot-funds PnL assignments require `state`, not `pnl`.
  pnl: "-120",
});

Engine.builder()

  .builtin(
    buildSpotFundsPnlBoundsKillswitch().accountBarriers([
      new SpotFundsPnlBoundsAccountBarrier(
        99_224_416n,
        new SpotFundsPnlBoundsBarrier("-250", "250"),
      ),
    ]),
  )
  .build();

Engine.builder().builtin(buildSpotFunds().withPolicyGroupId(7));

const accounts = Engine.builder().builtin(buildSpotFunds()).build().accounts();
accounts.setCurrency(99_224_416n, "USD");
accounts.clearCurrency(99_224_416n);
accounts.setGroupCurrency(AccountGroupId.DEFAULT(), "USD");
accounts.clearGroupCurrency(AccountGroupId.DEFAULT());

new PnlBoundsBrokerBarrier("USD", "-100", undefined);

// @ts-expect-error - account barriers compose a reusable bounds object.
new SpotFundsPnlBoundsAccountBarrier(99_224_416n, "-250", "250");
