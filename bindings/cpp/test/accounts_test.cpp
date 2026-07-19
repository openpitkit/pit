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

#include "openpit/accounts.hpp"

#include "openpit/account_id.hpp"
#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/pretrade.hpp"
#include "openpit/reject.hpp"

#include <gtest/gtest.h>

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace {

using openpit::Engine;
using openpit::EngineBuilder;
using openpit::SyncPolicy;
using openpit::accounts::AccountBlockError;
using openpit::accounts::AccountBlockErrorKind;
using openpit::accounts::AccountGroupError;
using openpit::accounts::Accounts;
using openpit::param::AccountGroupId;
using openpit::param::AccountId;
using openpit::param::DefaultAccountGroup;
using openpit::param::Price;
using openpit::param::Quantity;
using openpit::pretrade::RejectCode;

namespace policies = openpit::pretrade::policies;

// An OrderValidation engine: it admits well-formed orders, so a passing /
// blocked pre-trade cleanly reflects the account/group block state under test.
[[nodiscard]] Engine NewAccountsTestEngine() {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  return builder.Build();
}

[[nodiscard]] openpit::model::Order TestOrder(std::uint64_t accountId) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1"));
  op.price = Price::FromString("100");
  order.operation = std::move(op);
  return order;
}

[[nodiscard]] AccountGroupId GroupSeven() {
  return AccountGroupId::FromUint32(7);
}

// Drives one StartPreTrade for account 1 and asserts it is accepted.
void ExpectAccountPasses(const Engine& engine) {
  openpit::pretrade::StartResult result = engine.StartPreTrade(TestOrder(1));
  EXPECT_TRUE(result.rejects.empty());
  EXPECT_TRUE(result.request.has_value());
}

// Drives one StartPreTrade for account 1 and asserts a single AccountBlocked
// reject and no request.
void ExpectAccountBlocked(const Engine& engine) {
  openpit::pretrade::StartResult result = engine.StartPreTrade(TestOrder(1));
  EXPECT_FALSE(result.request.has_value());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::AccountBlocked);
}

// Drives one StartPreTrade for account 1 and asserts an AccountBlocked reject
// whose operator reason equals `wantReason` (the first-reason-wins invariant).
void ExpectAccountBlockedWithReason(const Engine& engine,
                                    const std::string& wantReason) {
  openpit::pretrade::StartResult result = engine.StartPreTrade(TestOrder(1));
  ASSERT_FALSE(result.request.has_value());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::AccountBlocked);
  EXPECT_EQ(result.rejects.front().reason, wantReason);
}

//------------------------------------------------------------------------------
// AccountId round-trips.

TEST(AccountId, Uint64RoundTrips) {
  const AccountId id = AccountId::FromUint64(42);
  EXPECT_EQ(id.Raw(), 42u);
  EXPECT_EQ(id.ToString(), "42");
  EXPECT_EQ(id, AccountId::FromUint64(42));
  EXPECT_NE(id, AccountId::FromUint64(43));
}

TEST(AccountId, FromRawPreservesValue) {
  const AccountId id = AccountId::FromRaw(7);
  EXPECT_EQ(id.Raw(), 7u);
}

TEST(AccountId, StringDerivedIsStable) {
  EXPECT_EQ(AccountId::FromString("desk-A"), AccountId::FromString("desk-A"));
}

TEST(AccountId, EmptyStringThrows) {
  EXPECT_THROW({ (void)AccountId::FromString(""); }, openpit::Error);
}

//------------------------------------------------------------------------------
// Group membership.

TEST(Accounts, GroupOfAbsent) {
  Engine engine = NewAccountsTestEngine();
  EXPECT_FALSE(engine.Accounts().GroupOf(AccountId::FromUint64(1)).has_value());
}

TEST(Accounts, RegisterGroupRejectsDefaultGroup) {
  Engine engine = NewAccountsTestEngine();
  const std::optional<AccountGroupError> error =
      engine.Accounts().RegisterGroup({AccountId::FromUint64(1)},
                                      DefaultAccountGroup);
  EXPECT_TRUE(error.has_value());
}

TEST(Accounts, RegisterGroupConflictReportsCurrentGroup) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);

  ASSERT_FALSE(accounts.RegisterGroup({account}, GroupSeven()).has_value());

  const std::optional<AccountGroupError> error =
      accounts.RegisterGroup({account}, AccountGroupId::FromUint32(8));
  ASSERT_TRUE(error.has_value());
  EXPECT_EQ(error->account, account);
  ASSERT_TRUE(error->currentGroup.has_value());
  EXPECT_EQ(*error->currentGroup, GroupSeven());
}

TEST(Accounts, RegisterGroupSucceedsAndGroupOfReflectsIt) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);

  ASSERT_FALSE(accounts.RegisterGroup({account}, GroupSeven()).has_value());

  const std::optional<AccountGroupId> group = accounts.GroupOf(account);
  ASSERT_TRUE(group.has_value());
  EXPECT_EQ(*group, GroupSeven());
}

TEST(Accounts, UnregisterGroupRejectsAbsentMember) {
  Engine engine = NewAccountsTestEngine();
  const std::optional<AccountGroupError> error =
      engine.Accounts().UnregisterGroup({AccountId::FromUint64(1)},
                                        GroupSeven());
  EXPECT_TRUE(error.has_value());
}

//------------------------------------------------------------------------------
// Account blocking by id.

TEST(Accounts, BlockGatesPreTradeAndUnblockRestores) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);

  ExpectAccountPasses(engine);

  accounts.Block(account, "manual kill-switch");
  ExpectAccountBlocked(engine);

  accounts.Unblock(account);
  ExpectAccountPasses(engine);
}

TEST(Accounts, UnblockAbsentIsNoOp) {
  Engine engine = NewAccountsTestEngine();
  engine.Accounts().Unblock(AccountId::FromUint64(1));
  ExpectAccountPasses(engine);
}

TEST(Accounts, BlockIdempotentKeepsFirstReason) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);

  accounts.Block(account, "first");
  // Blocking again must not replace the recorded reason.
  accounts.Block(account, "second");

  ExpectAccountBlockedWithReason(engine, "first");
}

TEST(Accounts, ReplaceBlockReasonUpdatesBlockedAccount) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);

  accounts.Block(account, "first");
  EXPECT_FALSE(accounts.ReplaceBlockReason(account, "second").has_value());
  ExpectAccountBlocked(engine);
}

TEST(Accounts, ReplaceBlockReasonRejectsUnblockedAccount) {
  Engine engine = NewAccountsTestEngine();
  const AccountId account = AccountId::FromUint64(1);

  const std::optional<AccountBlockError> error =
      engine.Accounts().ReplaceBlockReason(account, "reason");
  ASSERT_TRUE(error.has_value());
  EXPECT_EQ(error->kind, AccountBlockErrorKind::AccountNotBlocked);
  ASSERT_TRUE(error->account.has_value());
  EXPECT_EQ(*error->account, account);
  EXPECT_FALSE(error->group.has_value());
}

//------------------------------------------------------------------------------
// Account blocking by account-group predicate.

TEST(Accounts, BlockGroupGatesMember) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);
  const AccountGroupId group = GroupSeven();

  ASSERT_FALSE(accounts.RegisterGroup({account}, group).has_value());
  ExpectAccountPasses(engine);

  EXPECT_FALSE(accounts.BlockGroup(group, "group kill-switch").has_value());
  ExpectAccountBlocked(engine);
}

TEST(Accounts, BlockGroupGatesLaterMemberAndUngroupReleases) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);
  const AccountGroupId group = GroupSeven();

  ASSERT_FALSE(accounts.BlockGroup(group, "group kill-switch").has_value());

  // An account registered into an already-blocked group is gated.
  ASSERT_FALSE(accounts.RegisterGroup({account}, group).has_value());
  ExpectAccountBlocked(engine);

  // Removing it from the blocked group releases the gate.
  ASSERT_FALSE(accounts.UnregisterGroup({account}, group).has_value());
  ExpectAccountPasses(engine);
}

TEST(Accounts, UnblockGroupRestoresMembers) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);
  const AccountGroupId group = GroupSeven();

  ASSERT_FALSE(accounts.RegisterGroup({account}, group).has_value());
  ASSERT_FALSE(accounts.BlockGroup(group, "group kill-switch").has_value());
  ExpectAccountBlocked(engine);

  EXPECT_FALSE(accounts.UnblockGroup(group).has_value());
  ExpectAccountPasses(engine);
}

TEST(Accounts, BlockGroupIdempotentKeepsFirstReason) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);
  const AccountGroupId group = GroupSeven();

  ASSERT_FALSE(accounts.RegisterGroup({account}, group).has_value());
  ASSERT_FALSE(accounts.BlockGroup(group, "first").has_value());
  // Re-blocking the same group keeps the first reason and does not error.
  ASSERT_FALSE(accounts.BlockGroup(group, "second").has_value());

  ExpectAccountBlockedWithReason(engine, "first");
}

TEST(Accounts, UnblockGroupAbsentIsNoOp) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);
  const AccountGroupId group = GroupSeven();

  ASSERT_FALSE(accounts.RegisterGroup({account}, group).has_value());
  // Unblocking a never-blocked group must not error.
  EXPECT_FALSE(accounts.UnblockGroup(group).has_value());
  ExpectAccountPasses(engine);
}

TEST(Accounts, ReplaceGroupBlockReasonUpdatesBlockedGroup) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);
  const AccountGroupId group = GroupSeven();

  ASSERT_FALSE(accounts.RegisterGroup({account}, group).has_value());
  ASSERT_FALSE(accounts.BlockGroup(group, "original reason").has_value());

  EXPECT_FALSE(
      accounts.ReplaceGroupBlockReason(group, "new reason").has_value());
  ExpectAccountBlocked(engine);
}

//------------------------------------------------------------------------------
// Reserved-default-group and not-blocked failure outcomes.

TEST(Accounts, BlockGroupRejectsDefaultGroup) {
  Engine engine = NewAccountsTestEngine();
  const std::optional<AccountBlockError> error =
      engine.Accounts().BlockGroup(DefaultAccountGroup, "reason");
  ASSERT_TRUE(error.has_value());
  EXPECT_EQ(error->kind, AccountBlockErrorKind::ReservedGroup);
}

TEST(Accounts, UnblockGroupRejectsDefaultGroup) {
  Engine engine = NewAccountsTestEngine();
  const std::optional<AccountBlockError> error =
      engine.Accounts().UnblockGroup(DefaultAccountGroup);
  ASSERT_TRUE(error.has_value());
  EXPECT_EQ(error->kind, AccountBlockErrorKind::ReservedGroup);
}

TEST(Accounts, ReplaceGroupBlockReasonRejectsDefaultGroup) {
  Engine engine = NewAccountsTestEngine();
  const std::optional<AccountBlockError> error =
      engine.Accounts().ReplaceGroupBlockReason(DefaultAccountGroup, "reason");
  ASSERT_TRUE(error.has_value());
  EXPECT_EQ(error->kind, AccountBlockErrorKind::ReservedGroup);
}

TEST(Accounts, ReplaceGroupBlockReasonRejectsUnblockedGroup) {
  Engine engine = NewAccountsTestEngine();
  const AccountGroupId group = GroupSeven();

  const std::optional<AccountBlockError> error =
      engine.Accounts().ReplaceGroupBlockReason(group, "reason");
  ASSERT_TRUE(error.has_value());
  EXPECT_EQ(error->kind, AccountBlockErrorKind::GroupNotBlocked);
  ASSERT_TRUE(error->group.has_value());
  EXPECT_EQ(*error->group, group);
  EXPECT_FALSE(error->account.has_value());
}

//------------------------------------------------------------------------------
// Unified list / race-guard observable behavior: an individual block and a
// group block coexist on one list, and lifting one does not lift the other.

TEST(Accounts, IndividualAndGroupBlocksAreIndependentOnUnifiedList) {
  Engine engine = NewAccountsTestEngine();
  Accounts accounts = engine.Accounts();
  const AccountId account = AccountId::FromUint64(1);
  const AccountGroupId group = GroupSeven();

  ASSERT_FALSE(accounts.RegisterGroup({account}, group).has_value());

  // Both an individual block and a group block target the same account.
  accounts.Block(account, "individual");
  ASSERT_FALSE(accounts.BlockGroup(group, "group").has_value());
  ExpectAccountBlocked(engine);

  // Lifting only the group block leaves the individual block in force.
  ASSERT_FALSE(accounts.UnblockGroup(group).has_value());
  ExpectAccountBlocked(engine);

  // Lifting the individual block too finally releases the account.
  accounts.Unblock(account);
  ExpectAccountPasses(engine);
}

}  // namespace
