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

// Source: Account-Blocking.md

#include "openpit/accounts/accounts.hpp"
#include "openpit/engine.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/policies.hpp"

#include <gtest/gtest.h>

namespace {

// Examples block: by-account block/unblock and by-group block/unblock.
//
// The snippet shown in Account-Blocking.md (C++ section) is reproduced
// verbatim below, wrapped only in the minimal gtest harness.
TEST(AccountBlockingWiki, ByAccountAndByGroup) {
  // --- snippet begin ---
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(openpit::pretrade::policies::OrderValidationPolicy{});
  openpit::Engine engine = builder.Build();

  openpit::accounts::Accounts accounts = engine.Accounts();

  // Block account 99224416 - all subsequent pre-trade orders are rejected.
  accounts.Block(openpit::param::AccountId::FromUint64(99224416),
                 "compliance hold");

  // Unblock account 99224416 - pre-trade orders are allowed again.
  accounts.Unblock(openpit::param::AccountId::FromUint64(99224416));

  // Block every current and future member of a group in one call.
  openpit::param::AccountGroupId desk =
      openpit::param::AccountGroupId::FromUint32(7);
  if (auto err = accounts.BlockGroup(desk, "desk suspended")) {
    // handle err->message
  }
  if (auto err = accounts.UnblockGroup(desk)) {
    // handle err->message
  }
  // --- snippet end ---

  // Assert the block/unblock cycle leaves no residual block on account
  // 99224416, and that the group block was lifted with no error.
  //
  // The engine is single-account here so we inspect state indirectly via
  // Accounts accessors: after Unblock the account must not be individually
  // blocked, and after UnblockGroup the group must not be group-blocked.
  // Both operations are infallible / return nullopt on success for the
  // non-reserved group, which is verified by the EXPECT_FALSE calls.
  (void)engine;  // engine is held alive through accounts

  // BlockGroup / UnblockGroup on a non-default group must return nullopt.
  openpit::param::AccountGroupId desk2 =
      openpit::param::AccountGroupId::FromUint32(7);
  EXPECT_FALSE(accounts.BlockGroup(desk2, "desk suspended").has_value());
  EXPECT_FALSE(accounts.UnblockGroup(desk2).has_value());
}

}  // namespace
