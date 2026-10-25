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

// Source: https://wiki.openpit.dev/Async-Engine/

#include "openpit/async_engine.hpp"

#include "openpit/accounts/accounts.hpp"
#include "openpit/engine.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/policies.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <optional>
#include <vector>

namespace {

using openpit::Engine;
using openpit::EngineBuilder;
using openpit::SyncPolicy;
using openpit::param::AccountGroupId;
using openpit::param::AccountId;

namespace policies = openpit::pretrade::policies;

// Source: https://wiki.openpit.dev/Async-Engine/ - Exclusive Operations
// Keep the shared user-code flow in sync with the wiki. The engine, the group
// set-up, and the membership check are test harness.
TEST(AsyncEngineWiki, ExclusiveMovesAccountsBetweenGroups) {
  EngineBuilder builder(SyncPolicy::Account);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();
  openpit::asyncengine::OwnedTypedAsyncEngine async =
      openpit::asyncengine::MakeTypedAsyncEngine(engine, 2);
  const AccountGroupId hedgeBook = AccountGroupId::FromUint32(7);
  const AccountGroupId riskBook = AccountGroupId::FromUint32(8);
  const std::vector<AccountId> members = {
      AccountId::FromUint64(10),
      AccountId::FromUint64(11),
  };
  ASSERT_FALSE(async
                   .Exclusive(
                       [&](openpit::asyncengine::EngineAdapter& driver) {
                         ASSERT_FALSE(driver.Accounts().RegisterGroup(
                             members, hedgeBook));
                       },
                       std::chrono::seconds(5))
                   .has_value());

  // Move two accounts from one group to another in one section, so no other
  // call sees them in the default group in between.
  std::optional<openpit::accounts::AccountGroupError> groupErr;
  const std::optional<openpit::asyncengine::Error> err = async.Exclusive(
      [&](openpit::asyncengine::EngineAdapter& driver) {
        const openpit::accounts::Accounts accounts = driver.Accounts();
        groupErr = accounts.UnregisterGroup(members, hedgeBook);
        if (!groupErr) {
          groupErr = accounts.RegisterGroup(members, riskBook);
        }
      },
      std::chrono::seconds(5));
  if (err || groupErr) {
    // handle the section error or the group conflict
    FAIL() << "the move must succeed for registered accounts";
  }

  for (const AccountId& member : members) {
    const std::optional<std::optional<AccountGroupId>> group =
        async.Accounts().GroupOf(member).Await(std::chrono::seconds(5));
    ASSERT_TRUE(group.has_value());
    ASSERT_TRUE(group->has_value());
    EXPECT_EQ(**group, riskBook);
  }
  EXPECT_TRUE(async.StopGraceful(std::chrono::seconds(10)));
}

}  // namespace
