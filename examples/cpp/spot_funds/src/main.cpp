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

// Example spot_funds is the smallest end-to-end integration of OpenPit's
// built-in SpotFunds pre-trade policy: it shows how a buy order reserves
// settlement cash, how a second order is rejected because that cash is still
// held, and how a fill settles the held reservation.
//
// What is illustrated:
//
//   - building a limit-only engine with SpotFunds + OrderValidation
//   - seeding an account's available cash via ApplyAccountAdjustment
//   - the reservation mechanic: a committed BUY holds settlement funds, so a
//     follow-up BUY that needs the same cash is rejected with InsufficientFunds
//   - tying a fill back to its reservation by carrying the pre-trade lock on
//     the execution report, so SpotFunds settles the right held amount
//
// Audience: an integrator who wants to lift the SpotFunds call pattern into
// their own order/fill pipeline.
//
// What you typically change to adapt this example to your own application:
//
//  1. Engine policies - see BuildEngine() in spot_funds.hpp.
//  2. The seed balance and the orders - here they are hard-coded constants
//     chosen so the reservation mechanic is the lesson; your system feeds
//     real account state and strategy orders.
//  3. The print statements - replace them with your order-router and
//     fill-handler side effects.
//
// The example is deliberately flat: RunExample() reads top-to-bottom as a
// story, and every engine call is factored into a small named helper (in
// spot_funds.hpp) that the smoke test reuses. This is the C++ mirror of the Go
// example at examples/go/spot_funds/main.go.

#include "spot_funds.hpp"

#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/decision.hpp"

#include <cstdio>
#include <exception>

namespace {

// RunExample is the linear integration story. It is split out from main() so
// the engine's RAII destruction runs before the process exits on error.
void RunExample() {
  const ::openpit::param::AccountId account =
      ::openpit::param::AccountId::FromUint64(spot_funds::kScenarioAccount);

  // Step 1 - build the engine. Limit-only SpotFunds plus OrderValidation; do
  // this once at platform start-up.
  const ::openpit::Engine engine = spot_funds::BuildEngine();

  // Step 2 - seed the account's available settlement cash. SpotFunds has no
  // initial-balance builder option; the balance is established through the
  // account-adjustment pipeline, exactly as a deposit would be.
  spot_funds::SeedFunds(engine, account, spot_funds::kScenarioSeedFunds);
  std::printf("seeded account with %s %s available\n",
              spot_funds::kScenarioSeedFunds, spot_funds::kScenarioAssetSettle);

  // Step 3 - Buy #1: BUY 30 AAPL @ 2000 (60000 USD notional). It fits inside
  // the 100000 balance, so the pre-trade check accepts it. Committing the
  // reservation moves 60000 from available to held. We capture the
  // reservation's pre-trade lock - the fill in Step 5 must carry it back so
  // SpotFunds settles this exact reservation.
  const ::openpit::model::Order buy1 = spot_funds::BuildOrder(account);
  spot_funds::PlaceResult place1 = spot_funds::PlaceOrder(engine, buy1);
  if (!place1.Accepted()) {
    throw ::openpit::Error("buy #1 unexpectedly rejected: " +
                           spot_funds::Describe(place1.rejects));
  }
  std::printf("buy #1 accepted: held %d %s, %d %s now available\n",
              spot_funds::kOrderNotional, spot_funds::kScenarioAssetSettle,
              spot_funds::kAvailableAfterBuy1,
              spot_funds::kScenarioAssetSettle);

  // Step 4 - Buy #2: an identical BUY 30 AAPL @ 2000. This is the teaching
  // point. Only 40000 USD is available now (60000 is held by Buy #1), but the
  // order needs 60000, so SpotFunds rejects it with InsufficientFunds. A
  // rejected order produces no reservation - there is nothing to commit.
  const ::openpit::model::Order buy2 = spot_funds::BuildOrder(account);
  const spot_funds::PlaceResult place2 = spot_funds::PlaceOrder(engine, buy2);
  if (place2.Accepted()) {
    throw ::openpit::Error("buy #2 unexpectedly accepted");
  }
  if (!spot_funds::ContainsCode(
          place2.rejects, ::openpit::reject::RejectCode::InsufficientFunds)) {
    throw ::openpit::Error("buy #2 rejected for the wrong reason: " +
                           spot_funds::Describe(place2.rejects));
  }
  std::printf("buy #2 rejected: %s (held funds reduce what is available)\n",
              spot_funds::Describe(place2.rejects).c_str());

  // Step 5 - fill Buy #1 in full. The execution report carries the lock we
  // captured at commit time, so SpotFunds matches the fill to Buy #1's
  // reservation and settles the 60000 it was holding. No account block means
  // the settlement succeeded.
  const ::openpit::model::ExecutionReport fill =
      spot_funds::BuildFillReport(account);
  const spot_funds::FillResult result =
      spot_funds::ApplyFill(engine, fill, *place1.lock);
  if (!result.accountBlocks.empty()) {
    throw ::openpit::Error("fill produced an unexpected account block");
  }
  std::printf("buy #1 filled: %d %s reservation settled, no account block\n",
              spot_funds::kOrderNotional, spot_funds::kScenarioAssetSettle);
}

} // namespace

int main() {
  try {
    RunExample();
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
  return 0;
}
