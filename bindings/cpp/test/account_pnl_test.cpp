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

#include "openpit/openpit.hpp"

#include <gtest/gtest.h>

#include <array>
#include <utility>

namespace {

namespace aa = openpit::accountadjustment;
namespace param = openpit::param;

// The public umbrella header exposes the PnL outcome types used by both
// post-trade account PnL and per-asset account-adjustment outcomes.
TEST(AccountPnlOutcome, AvailablePreservesZeroDelta) {
  OpenPitAccountPnlOutcome raw{};
  raw.account_id = 42;
  raw.policy_group_id = 7;
  raw.halt_reason = OPENPIT_PNL_HALT_REASON_NONE;
  raw.amount.is_set = true;
  raw.amount.value.delta = param::Pnl::FromString("0").Raw();
  raw.amount.value.absolute = param::Pnl::FromString("25.5").Raw();

  const aa::AccountPnlOutcome outcome = aa::AccountPnlOutcome::FromRaw(raw);

  EXPECT_EQ(outcome.accountId, param::AccountId::FromUint64(42));
  EXPECT_EQ(outcome.policyGroupId, param::GroupId(7));
  const auto& result = outcome.Get();
  ASSERT_TRUE(std::holds_alternative<aa::PnlOutcomeAmount>(result));
  const auto& pnl = std::get<aa::PnlOutcomeAmount>(result);
  EXPECT_EQ(pnl.delta.ToString(), "0");
  EXPECT_EQ(pnl.absolute.ToString(), "25.5");
}

TEST(AccountPnlOutcome, MissingAccountCurrencyDoesNotExposeThePnlAmount) {
  OpenPitAccountPnlOutcome raw{};
  raw.account_id = 42;
  raw.halt_reason = OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY;

  const aa::AccountPnlOutcome outcome = aa::AccountPnlOutcome::FromRaw(raw);

  const auto& result = outcome.Get();
  ASSERT_TRUE(std::holds_alternative<aa::PnlHaltReason>(result));
  EXPECT_EQ(std::get<aa::PnlHaltReason>(result),
            aa::PnlHaltReason::MissingAccountCurrency);
}

TEST(AccountOutcomeEntry, FirstPositionPnlHaltExposesTheReason) {
  OpenPitAccountOutcomeEntry raw{};
  raw.realized_pnl.is_set = true;
  raw.realized_pnl.value.halt_reason = OPENPIT_PNL_HALT_REASON_MISSING_FX;

  const aa::AccountOutcomeEntry outcome = aa::AccountOutcomeEntry::FromRaw(raw);

  ASSERT_TRUE(outcome.realizedPnl.has_value());
  const auto& result = outcome.realizedPnl->Get();
  ASSERT_TRUE(std::holds_alternative<aa::PnlHaltReason>(result));
  EXPECT_EQ(std::get<aa::PnlHaltReason>(result), aa::PnlHaltReason::MissingFx);
}

TEST(PnlOutcome, MapsEveryHaltReason) {
  const std::array<std::pair<OpenPitPnlHaltReason, aa::PnlHaltReason>, 5> cases{
      {
          {OPENPIT_PNL_HALT_REASON_MISSING_FX, aa::PnlHaltReason::MissingFx},
          {OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY,
           aa::PnlHaltReason::MissingAccountCurrency},
          {OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL,
           aa::PnlHaltReason::MissingInitialPnl},
          {OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS,
           aa::PnlHaltReason::MissingCostBasis},
          {OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW,
           aa::PnlHaltReason::ArithmeticOverflow},
      }};

  for (const auto& [rawReason, expected] : cases) {
    OpenPitPnlOutcome raw{};
    raw.halt_reason = rawReason;
    const aa::PnlOutcome outcome = aa::PnlOutcome::FromRaw(raw);
    ASSERT_TRUE(std::holds_alternative<aa::PnlHaltReason>(outcome.result));
    EXPECT_EQ(std::get<aa::PnlHaltReason>(outcome.result), expected);
    EXPECT_EQ(outcome.Raw().halt_reason, rawReason);
  }
}

TEST(PnlOutcome, RejectsInvalidWireStates) {
  OpenPitPnlOutcome unknown{};
  unknown.halt_reason = 255;
  EXPECT_THROW(static_cast<void>(aa::PnlOutcome::FromRaw(unknown)),
               openpit::Error);

  OpenPitPnlOutcome missingAmount{};
  missingAmount.halt_reason = OPENPIT_PNL_HALT_REASON_NONE;
  EXPECT_THROW(static_cast<void>(aa::PnlOutcome::FromRaw(missingAmount)),
               openpit::Error);

  OpenPitPnlOutcome haltedWithAmount{};
  haltedWithAmount.halt_reason = OPENPIT_PNL_HALT_REASON_MISSING_FX;
  haltedWithAmount.amount.is_set = true;
  EXPECT_THROW(static_cast<void>(aa::PnlOutcome::FromRaw(haltedWithAmount)),
               openpit::Error);

  const aa::PnlOutcome invalidCpp{static_cast<aa::PnlHaltReason>(255)};
  static_assert(noexcept(invalidCpp.Raw()));
  EXPECT_DEATH_IF_SUPPORTED(static_cast<void>(invalidCpp.Raw()), ".*");
}

TEST(AccountPnlOperation, ValueAndHaltRoundTrip) {
  const aa::AccountPnlOperation value(param::Pnl::FromString("12.5"));
  const aa::AccountPnlOperation restoredValue =
      aa::AccountPnlOperation::FromRaw(value.Raw());
  ASSERT_TRUE(std::holds_alternative<param::Pnl>(restoredValue.Get()));
  EXPECT_EQ(std::get<param::Pnl>(restoredValue.Get()).ToString(), "12.5");

  const aa::AccountPnlOperation halted(aa::PnlHaltReason::MissingFx);
  const aa::AccountPnlOperation restoredHalt =
      aa::AccountPnlOperation::FromRaw(halted.Raw());
  ASSERT_TRUE(std::holds_alternative<aa::PnlHaltReason>(restoredHalt.Get()));
  EXPECT_EQ(std::get<aa::PnlHaltReason>(restoredHalt.Get()),
            aa::PnlHaltReason::MissingFx);
}

TEST(AccountPnlOperation, RejectsInvalidWireStates) {
  OpenPitAccountAdjustmentAccountPnlOperation unknown{};
  unknown.state.kind = 255;
  EXPECT_THROW(static_cast<void>(aa::AccountPnlOperation::FromRaw(unknown)),
               openpit::Error);

  OpenPitAccountAdjustmentAccountPnlOperation valueWithHalt{};
  valueWithHalt.state.kind = OPENPIT_PNL_STATE_VALUE;
  valueWithHalt.state.halt_reason = OPENPIT_PNL_HALT_REASON_MISSING_FX;
  EXPECT_THROW(
      static_cast<void>(aa::AccountPnlOperation::FromRaw(valueWithHalt)),
      openpit::Error);

  OpenPitAccountAdjustmentAccountPnlOperation haltedWithoutReason{};
  haltedWithoutReason.state.kind = OPENPIT_PNL_STATE_HALTED;
  EXPECT_THROW(
      static_cast<void>(aa::AccountPnlOperation::FromRaw(haltedWithoutReason)),
      openpit::Error);

  const aa::AccountPnlOperation invalidCpp(static_cast<aa::PnlHaltReason>(255));
  static_assert(noexcept(invalidCpp.Raw()));
  EXPECT_DEATH_IF_SUPPORTED(static_cast<void>(invalidCpp.Raw()), ".*");
}

}  // namespace
