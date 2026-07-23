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

// Source: Account-Groups.md

#include "openpit/accounts/accounts.hpp"
#include "openpit/engine.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/policies.hpp"

#include <gtest/gtest.h>

#include <optional>
#include <vector>

namespace {

using openpit::Engine;
using openpit::EngineBuilder;
using openpit::SyncPolicy;
using openpit::accounts::Accounts;
using openpit::param::AccountGroupId;
using openpit::param::AccountId;

namespace policies = openpit::pretrade::policies;

// Account-Groups.md § Examples
// Register two accounts into one group, read membership back by id,
// and unregister the group.
TEST(AccountGroupsWiki, RegisterReadUnregister) {
  // Build an engine with the order-validation policy (mirrors the
  // Go/Python/Rust siblings that use FullSync / no_sync with the built-in
  // order-validation policy; FullSync is chosen here to match the Go sibling).
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();

  // Group two accounts under one compact identifier.
  Accounts accounts = engine.Accounts();
  const AccountGroupId hedgeBook = AccountGroupId::FromUint32(7);
  const std::vector<AccountId> members = {
      AccountId::FromUint64(10),
      AccountId::FromUint64(11),
  };
  if (auto err = accounts.RegisterGroup(members, hedgeBook)) {
    // handle err->message on conflict
    FAIL() << "RegisterGroup must not fail for fresh accounts";
  }

  // Membership is readable by id, without enumerating the accounts.
  const std::optional<AccountGroupId> group10 =
      accounts.GroupOf(AccountId::FromUint64(10));
  ASSERT_TRUE(group10.has_value());
  EXPECT_EQ(*group10, hedgeBook);

  const std::optional<AccountGroupId> group99 =
      accounts.GroupOf(AccountId::FromUint64(99));
  EXPECT_FALSE(group99.has_value());

  // Removing the group is atomic too: every listed account must be a member.
  if (auto err = accounts.UnregisterGroup(members, hedgeBook)) {
    // handle err->message on conflict
    FAIL() << "UnregisterGroup must not fail for registered members";
  }
  EXPECT_FALSE(accounts.GroupOf(AccountId::FromUint64(10)).has_value());
}

}  // namespace
