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
import {
  PnlHaltReason,
  type ReviewRuleConfigureOptions,
} from "@openpit/engine/pretrade";
import { RuleMode } from "@openpit/engine/reject";
import {
  buildRateLimit,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
  PnlBoundsBrokerBarrier,
  RateLimit,
  RateLimitAssetBarrier,
  RateLimitAccountBarrier,
  RateLimitAccountAssetBarrier,
  RateLimitBrokerBarrier,
  RateLimitBuilder,
  ReviewRule,
  SpotFundsBuilder,
  SpotFundsLimitMode,
  type SpotFundsPositionLimitEntry,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsBarrier,
  SpotFundsPnlBoundsKillswitchBuilder,
} from "@openpit/engine/pretrade/policies";

const configurator: Configurator = Engine.builder()

  .builtin(
    buildRateLimit().brokerBarriers([
      new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
    ]),
  )
  .build()
  .configure();

configurator.rateLimit(RateLimitBuilder.NAME, {
  brokerBarriers: [new RateLimitBrokerBarrier(new RateLimit(2, 60_000))],
});

const reviewRuleOptions: ReviewRuleConfigureOptions = {
  globalRule: new ReviewRule({
    instruments: [{ underlyingAsset: "MSFT", settlementAsset: "USD" }],
  }),
  accountGroupRules: new Set([new ReviewRule({ accountGroupId: 7 })]),
  accountRules: null,
};
configurator.reviewRule("restricted-instruments", reviewRuleOptions);
configurator.reviewRule("restricted-instruments", {
  clearGlobalRule: true,
  accountRules: [],
});
configurator.reviewRule("restricted-instruments", {
  // @ts-expect-error Runtime updates require ReviewRule wrappers.
  accountRules: [{ accountId: 1001 }],
});

const rateLimit = new RateLimit(2, 60_000);
const reviewBroker = new RateLimitBrokerBarrier(
  rateLimit,
  RuleMode.ReviewWithoutHold,
);
const reviewAsset = new RateLimitAssetBarrier(
  rateLimit,
  "USD",
  RuleMode.ReviewWithHold,
);
const reviewAccount = new RateLimitAccountBarrier(
  rateLimit,
  7,
  RuleMode.ReviewWithoutHold,
);
const reviewAccountAsset = new RateLimitAccountAssetBarrier(
  rateLimit,
  7,
  "USD",
  RuleMode.Reject,
);
const barrierMode: RuleMode = reviewBroker.mode;
void [barrierMode, reviewAsset, reviewAccount, reviewAccountAsset];
Engine.builder().builtin(
  buildRateLimit().brokerBarriers(new Set([reviewBroker])),
);
configurator.rateLimit(RateLimitBuilder.NAME, {
  brokerBarriers: new Set([reviewBroker]),
});
configurator.rateLimit(RateLimitBuilder.NAME, { brokerBarriers: null });
configurator.rateLimit(RateLimitBuilder.NAME, { brokerBarriers: [] });
// @ts-expect-error Rate-limit brokerBarrier was replaced by brokerBarriers.
void buildRateLimit().brokerBarrier;
configurator.rateLimit(RateLimitBuilder.NAME, {
  // @ts-expect-error Rate-limit broker was replaced by brokerBarriers.
  broker: reviewBroker,
});
configurator.rateLimit(RateLimitBuilder.NAME, {
  // @ts-expect-error An empty brokerBarriers iterable clears the axis.
  clearBroker: true,
});
// @ts-expect-error RuleMode strings use exact PascalCase spelling.
new RateLimitBrokerBarrier(rateLimit, "reject");

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
  positionLimits: [
    { accountId: 99_224_416n, asset: "AAPL", limit: "2" },
    { accountId: 99_224_417n, asset: "AAPL", limit: null },
    { accountId: 99_224_418n, asset: "AAPL", limit: 2n },
  ],
});

for (const mode of ["ReviewWithHold", "ReviewWithoutHold"] as const) {
  configurator.spotFunds(SpotFundsBuilder.NAME, {
    globalLimitMode: mode,
    accountLimitModes: [{ accountId: 42n, mode }],
    accountGroupLimitModes: [{ accountGroupId: 7, mode }],
  });
}

const positionLimitEntry: SpotFundsPositionLimitEntry = {
  accountId: 99_224_416n,
  asset: "AAPL",
  limit: "2",
};
void positionLimitEntry;

const positionLimitModes: readonly (RuleMode | null)[] = [
  "Reject",
  "ReviewWithHold",
  "ReviewWithoutHold",
  null,
];
for (const mode of positionLimitModes) {
  const entry: SpotFundsPositionLimitEntry = { ...positionLimitEntry, mode };
  void entry;
}
const numericPositionLimitMode: SpotFundsPositionLimitEntry = {
  ...positionLimitEntry,
  // @ts-expect-error - a numeric value is not a rule mode.
  mode: 1,
};
void numericPositionLimitMode;

const sweptPnlConfiguration = configurator.spotFundsPnlBoundsKillswitch(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    globalBarriers: [new SpotFundsPnlBoundsBarrier("USD", "-500", undefined)],
    accountBarriers: [
      new SpotFundsPnlBoundsAccountBarrier(
        99_224_416n,
        new SpotFundsPnlBoundsBarrier("USD", "-250", "250"),
      ),
    ],
  },
);
void sweptPnlConfiguration.accountBlocks[0]?.accountId.value;
void sweptPnlConfiguration.accountBlocks[0]?.block.code;
configurator.spotFundsPnlBoundsKillswitch(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    globalBarriers: new Set([
      new SpotFundsPnlBoundsBarrier("USD", "-10", undefined, "ReviewWithHold"),
    ]),
  },
);
configurator.spotFundsPnlBoundsKillswitch(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    globalBarriers: null,
  },
);
configurator.spotFundsPnlBoundsKillswitch(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    // @ts-expect-error - the singular global barrier option was removed.
    globalBarrier: new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
  },
);
configurator.spotFundsPnlBoundsKillswitch(
  SpotFundsPnlBoundsKillswitchBuilder.NAME,
  {
    // @ts-expect-error - global barriers require an iterable.
    globalBarriers: new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
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
        new SpotFundsPnlBoundsBarrier("USD", "-250", "250"),
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
