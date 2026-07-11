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

import {
  FillType,
  ParamKind,
  RoundingStrategies,
  Trade,
  TradeAmount,
  type RoundingStrategy,
  type TradeInit,
} from "@openpit/engine/param";

const canonical: RoundingStrategy[] = [
  RoundingStrategies.MidpointNearestEven,
  RoundingStrategies.MidpointAwayFromZero,
  RoundingStrategies.Up,
  RoundingStrategies.Down,
];
const aliases: RoundingStrategy[] = [
  RoundingStrategies.Default,
  RoundingStrategies.Banker,
  RoundingStrategies.ConservativeProfit,
  RoundingStrategies.ConservativeLoss,
];
void canonical;
void aliases;

const fill: FillType = FillType.AutoDeleverage;
const param: ParamKind = ParamKind.PositionSize;
void fill;
void param;

const tradeInit: TradeInit = { price: "100", quantity: "2" };
const tradeConstructor: typeof Trade = Trade;
const tradeAmountConstructor: typeof TradeAmount = TradeAmount;
void tradeInit;
void tradeConstructor;
void tradeAmountConstructor;

// @ts-expect-error - rounding strategies are a closed eight-value vocabulary.
const invalidRounding: RoundingStrategy = "ceil";
// @ts-expect-error - fill types use the core uppercase wire values.
const invalidFill: FillType = "MANUAL";
// @ts-expect-error - parameter kinds mirror the nine core variants.
const invalidParam: ParamKind = "Margin";
void invalidRounding;
void invalidFill;
void invalidParam;
