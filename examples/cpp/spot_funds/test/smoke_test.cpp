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

// Source: examples/cpp/spot_funds/README.md

#include "spot_funds.hpp"

#include "openpit/engine.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/decision.hpp"

#include <gtest/gtest.h>

// SpotFundsReservationFlow drives the same shared helpers main() uses and
// asserts the four outcomes that make the example a lesson: the first buy is
// accepted (reserving funds), the second identical buy is rejected with
// InsufficientFunds (those funds are held), the fill - carrying the first
// reservation's lock - settles without an account block, and after the switch
// to track-only mode a third identical buy is accepted despite the same
// shortfall.
TEST(SpotFunds, ReservationFlow) {
  const ::openpit::param::AccountId account =
      ::openpit::param::AccountId::FromUint64(spot_funds::kScenarioAccount);

  const ::openpit::Engine engine = spot_funds::BuildEngine();
  ASSERT_TRUE(static_cast<bool>(engine));

  spot_funds::SeedFunds(engine, account, spot_funds::kScenarioSeedFunds);

  // Buy #1 must be accepted and yield a lock to carry to the fill.
  const ::openpit::model::Order buy1 = spot_funds::BuildOrder(account);
  spot_funds::PlaceResult place1 = spot_funds::PlaceOrder(engine, buy1);
  ASSERT_TRUE(place1.Accepted())
      << "buy #1 rejected: " << spot_funds::Describe(place1.rejects);
  ASSERT_TRUE(place1.lock.has_value());
  EXPECT_FALSE(place1.lock->IsEmpty())
      << "buy #1 accepted but produced an empty pre-trade lock";

  // Buy #2 must be rejected with InsufficientFunds: 60000 is held by buy #1,
  // only 40000 is available, and the order needs 60000.
  const ::openpit::model::Order buy2 = spot_funds::BuildOrder(account);
  const spot_funds::PlaceResult place2 = spot_funds::PlaceOrder(engine, buy2);
  EXPECT_FALSE(place2.Accepted())
      << "buy #2 was accepted; expected an InsufficientFunds reject";
  EXPECT_TRUE(spot_funds::ContainsCode(
      place2.rejects, ::openpit::reject::RejectCode::InsufficientFunds))
      << "buy #2 reject codes wrong: " << spot_funds::Describe(place2.rejects);

  // The fill carries buy #1's lock, so SpotFunds settles that reservation; a
  // successful settlement produces no account block.
  const ::openpit::model::ExecutionReport fill =
      spot_funds::BuildFillReport(account);
  const spot_funds::FillResult result =
      spot_funds::ApplyFill(engine, fill, *place1.lock);
  EXPECT_TRUE(result.accountBlocks.empty())
      << "fill produced " << result.accountBlocks.size()
      << " account block(s), want 0";

  // Track-only mode drops the insufficient-funds gate, so buy #3 - identical
  // to the rejected buy #2 and facing the same 40000 available against 60000
  // needed - is accepted and still produces a lock.
  spot_funds::EnableTrackOnly(engine);
  const ::openpit::model::Order buy3 = spot_funds::BuildOrder(account);
  const spot_funds::PlaceResult place3 = spot_funds::PlaceOrder(engine, buy3);
  ASSERT_TRUE(place3.Accepted()) << "buy #3 rejected in track-only mode: "
                                 << spot_funds::Describe(place3.rejects);
  EXPECT_TRUE(place3.rejects.empty()) << "buy #3 accepted but still reported: "
                                      << spot_funds::Describe(place3.rejects);
  EXPECT_FALSE(place3.lock->IsEmpty())
      << "buy #3 accepted but produced an empty pre-trade lock";
}
