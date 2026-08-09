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

// Generator property tests and shadow-ledger arithmetic tests.
//

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/decimal.hpp"
#include "spot_loadtest/generator/event.hpp"
#include "spot_loadtest/generator/generator.hpp"
#include "spot_loadtest/generator/ledger.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <cstdint>
#include <limits>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>

namespace {

namespace config = spot_loadtest::config;
namespace gen = spot_loadtest::generator;
using spot_loadtest::Decimal;

[[nodiscard]] Decimal Dec(const std::string &s) {
  return Decimal::FromString(s);
}

// A representative, fully valid in-code configuration.
[[nodiscard]] config::Config TestConfig(std::uint64_t seed,
                                        std::uint64_t totalOps) {
  config::Config c;
  c.run.seed = seed;
  c.run.totalOps = totalOps;
  c.run.window = 1000;
  c.reject = config::Reject{0.05, 0.005};
  c.accounts = config::Accounts{500};
  c.concurrency = config::Concurrency{128, 16, std::chrono::milliseconds(1)};
  c.instruments.symbols = {"AAPL", "SPX",  "MSFT", "AMZN", "GOOG",
                           "META", "TSLA", "NVDA", "JPM",  "BAC"};
  c.instruments.settlement = "USD";
  c.lifecycle = config::Lifecycle{0.40, 0.15, 0.25, 0.20};
  c.funding.trigger = config::FundingTrigger::BalanceBelow;
  c.funding.threshold = Dec("100000");
  c.funding.seed = Dec("1000000");
  c.funding.topUp = Dec("1000000");
  c.cohorts = {
      config::Cohort{"chatty",
                     0.2,
                     0.9,
                     0.7,
                     4,
                     {{1, 1}, {10, 4}, {100, 2}},
                     config::SymbolSkew::Zipf,
                     1.3},
      config::Cohort{"steady",
                     0.5,
                     0.5,
                     0.25,
                     2,
                     {{1, 2}, {10, 3}, {100, 1}},
                     config::SymbolSkew::Uniform,
                     0.0},
      config::Cohort{"dormant",
                     0.3,
                     0.1,
                     0.05,
                     1,
                     {{1, 5}, {10, 1}},
                     config::SymbolSkew::Uniform,
                     0.0},
  };
  return c;
}

[[nodiscard]] double Abs(double x) { return x < 0 ? -x : x; }

//------------------------------------------------------------------------------
// Generator property tests.

TEST(Generator, ZeroOrderCheckBudgetFailsClosed) {
  config::Config cfg = TestConfig(0xC0FFEE, 0);
  try {
    (void)gen::Generate(cfg);
    ADD_FAILURE() << "expected a positive total_ops error";
  } catch (const std::runtime_error &e) {
    EXPECT_STREQ(e.what(), "generator: run.total_ops must be positive");
  }
}

TEST(Generator, NoneReportDelayRejectsIgnoredParameters) {
  config::Config cfg = TestConfig(0xC0FFEE, 1000);
  cfg.reportDelay.mean = std::chrono::milliseconds(2);
  cfg.reportDelay.distribution = config::ReportDelayDistribution::None;
  EXPECT_THROW((void)gen::Generate(cfg), std::runtime_error);

  cfg.reportDelay.mean = std::chrono::nanoseconds(0);
  cfg.reportDelay.sigma = 0.5;
  EXPECT_THROW((void)gen::Generate(cfg), std::runtime_error);
}

TEST(Generator, InvalidProgrammaticReportDelayFailsClosed) {
  config::Config cfg = TestConfig(0xC0FFEE, 1000);
  cfg.reportDelay.mean = std::chrono::nanoseconds(-1);
  EXPECT_THROW((void)gen::Generate(cfg), std::runtime_error);

  cfg.reportDelay.mean = std::chrono::nanoseconds(0);
  cfg.reportDelay.sigma = std::numeric_limits<double>::infinity();
  EXPECT_THROW((void)gen::Generate(cfg), std::runtime_error);

  cfg.reportDelay =
      config::ReportDelay{std::chrono::milliseconds(1),
                          config::ReportDelayDistribution::Fixed, 0.5};
  EXPECT_THROW((void)gen::Generate(cfg), std::runtime_error);

  cfg.reportDelay =
      config::ReportDelay{std::chrono::nanoseconds(0),
                          config::ReportDelayDistribution::Lognormal, 0.5};
  EXPECT_THROW((void)gen::Generate(cfg), std::runtime_error);
}

TEST(Generator, UnknownProgrammaticReportDelayDistributionFailsClosed) {
  config::Config cfg = TestConfig(0xC0FFEE, 1);
  cfg.reportDelay.distribution =
      static_cast<config::ReportDelayDistribution>(99);

  try {
    (void)gen::Generate(cfg);
    ADD_FAILURE() << "expected an unknown report-delay distribution error";
  } catch (const std::runtime_error &error) {
    EXPECT_STREQ(error.what(),
                 "generator: report_delay.distribution has unknown value 99");
  }
}

TEST(Generator, NearMaxReportDelayKeepsVirtualTimelineRepresentable) {
  constexpr std::int64_t kCausalGapNs = 1000;
  const std::chrono::nanoseconds delay(
      std::numeric_limits<std::int64_t>::max() - kCausalGapNs);
  config::Config cfg = TestConfig(0xC0FFEE, 1);
  cfg.arrival.offeredRate = 0;
  cfg.reject.targetRate = 0;
  cfg.reportDelay =
      config::ReportDelay{delay, config::ReportDelayDistribution::Fixed, 0};

  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);
  bool sawSettlement = false;
  for (const gen::Event &event : stream->events) {
    if (event.kind == gen::EventKind::Settlement) {
      EXPECT_EQ(event.virtualT0, delay);
      sawSettlement = true;
    }
  }
  EXPECT_TRUE(sawSettlement);
}

TEST(Generator, MaxReportDelayFailsBeforeCausalDeadlineOverflow) {
  config::Config cfg = TestConfig(0xC0FFEE, 1);
  cfg.arrival.offeredRate = 0;
  cfg.reject.targetRate = 0;
  cfg.reportDelay = config::ReportDelay{
      std::chrono::nanoseconds(std::numeric_limits<std::int64_t>::max()),
      config::ReportDelayDistribution::Fixed, 0};

  EXPECT_THROW(
      {
        try {
          (void)gen::Generate(cfg);
        } catch (const std::runtime_error &error) {
          EXPECT_STREQ(
              error.what(),
              "generator: account causal deadline exceeds nanoseconds range");
          throw;
        }
      },
      std::runtime_error);
}

TEST(Generator, LognormalMaxReportDelaySampleIsRepresentable) {
  config::Config cfg = TestConfig(0xC0FFEE, 1);
  cfg.arrival.offeredRate = 0;
  cfg.reject.targetRate = 0;
  cfg.reportDelay = config::ReportDelay{
      std::chrono::nanoseconds(std::numeric_limits<std::int64_t>::max()),
      config::ReportDelayDistribution::Lognormal, 0};

  EXPECT_THROW(
      {
        try {
          (void)gen::Generate(cfg);
        } catch (const std::runtime_error &error) {
          EXPECT_STREQ(
              error.what(),
              "generator: account causal deadline exceeds nanoseconds range");
          throw;
        }
      },
      std::runtime_error);
}

TEST(Generator, MaxReportDelayFailsBeforeSettlementTimeOverflow) {
  config::Config cfg = TestConfig(0xC0FFEE, 1);
  cfg.arrival.offeredRate = 1;
  cfg.reject.targetRate = 0;
  cfg.reportDelay = config::ReportDelay{
      std::chrono::nanoseconds(std::numeric_limits<std::int64_t>::max()),
      config::ReportDelayDistribution::Fixed, 0};

  EXPECT_THROW(
      {
        try {
          (void)gen::Generate(cfg);
        } catch (const std::runtime_error &error) {
          EXPECT_STREQ(
              error.what(),
              "generator: report settlement time exceeds nanoseconds range");
          throw;
        }
      },
      std::runtime_error);
}

// The same seed and configuration must reproduce the serialized stream.
TEST(Generator, DeterminismByteIdentical) {
  const config::Config cfg = TestConfig(0xC0FFEE, 20000);
  const std::string a = gen::Generate(cfg)->Serialize();
  const std::string b = gen::Generate(cfg)->Serialize();
  EXPECT_EQ(a, b);
  EXPECT_FALSE(a.empty());
}

// Different seeds produce different streams.
TEST(Generator, DeterminismDistinctSeeds) {
  const std::string a = gen::Generate(TestConfig(1, 20000))->Serialize();
  const std::string b = gen::Generate(TestConfig(2, 20000))->Serialize();
  EXPECT_NE(a, b);
}

// Replays the stream through an independent ledger and asserts available >= 0
// and held >= 0 everywhere, and that the predicted balances match the replay.
TEST(Generator, FundInvariants) {
  const config::Config cfg = TestConfig(0xBEEF, 40000);
  const std::unique_ptr<gen::Stream> s = gen::Generate(cfg);
  gen::Ledger replay;
  for (const gen::Event &e : s->events) {
    switch (e.kind) {
    case gen::EventKind::Funding:
      (void)replay.ApplyFunding(e.account, e.fundingAsset, e.fundingKind,
                                e.fundingAmount);
      break;
    case gen::EventKind::OrderCheck:
      if (e.accept) {
        (void)replay.PreTrade(e.account, e.side, e.underlying, e.settlement,
                              e.quantity, e.price);
      }
      break;
    case gen::EventKind::Settlement: {
      const gen::SettlementResult r = replay.SettleFullFill(
          e.account, e.side, e.underlying, e.settlement, e.quantity, e.price);
      ASSERT_FALSE(r.error) << "settlement replay error at seq " << e.seq;
      break;
    }
    }
    for (const gen::Balance &b : e.post) {
      auto [h, ok] = replay.Get(e.account, b.asset);
      (void)ok;
      ASSERT_FALSE(h.available.IsNegative())
          << "available negative seq " << e.seq;
      ASSERT_FALSE(h.held.IsNegative()) << "held negative seq " << e.seq;
      EXPECT_EQ(h.available, b.available) << "available mismatch seq " << e.seq;
      EXPECT_EQ(h.held, b.held) << "held mismatch seq " << e.seq;
    }
  }
}

// No Sell ever exceeds the held position, and the position never goes negative.
TEST(Generator, LifecycleValidity) {
  const config::Config cfg = TestConfig(0xABCD, 40000);
  const std::unique_ptr<gen::Stream> s = gen::Generate(cfg);
  std::map<std::pair<std::string, std::string>, std::uint64_t> pos;
  for (const gen::Event &e : s->events) {
    if (e.kind != gen::EventKind::Settlement) {
      continue;
    }
    const auto lots = static_cast<std::uint64_t>(e.quantity.ToWholeInt());
    const auto key = std::make_pair(e.account, e.underlying);
    if (e.side == gen::Side::Buy) {
      pos[key] += lots;
    } else {
      ASSERT_LE(lots, pos[key]) << "sell exceeds position at seq " << e.seq;
      pos[key] -= lots;
    }
  }
}

// The predicted reject rate over a large stream lands within tolerance of the
// configured target.
TEST(Generator, RejectControllerConvergence) {
  for (double target : {0.01, 0.05, 0.10, 0.20}) {
    config::Config cfg = TestConfig(0x5EED, 200000);
    cfg.reject.targetRate = target;
    cfg.reject.tolerance = 0.005;
    const std::unique_ptr<gen::Stream> s = gen::Generate(cfg);
    const double got = s->stats.PredictedRejectRate();
    EXPECT_LE(Abs(got - target), cfg.reject.tolerance)
        << "target " << target << " got " << got;
  }
}

// Every forced reject is the InsufficientFunds reason, with both accepts and
// rejects present.
TEST(Generator, ForcedRejectsReason) {
  config::Config cfg = TestConfig(0x1234, 50000);
  cfg.reject.targetRate = 0.10;
  const std::unique_ptr<gen::Stream> s = gen::Generate(cfg);
  EXPECT_GT(s->stats.accepts, 0u);
  EXPECT_GT(s->stats.rejects, 0u);
  for (const gen::Event &e : s->events) {
    if (e.kind == gen::EventKind::OrderCheck && !e.accept) {
      EXPECT_EQ(e.reason, gen::RejectReason::InsufficientFunds)
          << "seq " << e.seq;
    }
  }
}

// The generator emits exactly TotalOps order-checks.
TEST(Generator, OrderCheckBudgetRespected) {
  constexpr std::uint64_t kOps = 12345;
  const std::unique_ptr<gen::Stream> s = gen::Generate(TestConfig(7, kOps));
  EXPECT_EQ(s->stats.orderChecks, kOps);
}

// Self-funding fires beyond the seeds, every funding accepts, and no
// starvation-driven rejects appear when target_rate = 0.
TEST(Generator, SelfFundingPreventsStarvation) {
  config::Config cfg = TestConfig(0x9, 60000);
  cfg.accounts.count = 3;
  cfg.funding.seed = Dec("2000");
  cfg.funding.threshold = Dec("1000");
  cfg.funding.topUp = Dec("2000");
  cfg.reject.targetRate = 0.0;
  cfg.concurrency.activeAccounts = 3;
  const std::unique_ptr<gen::Stream> s = gen::Generate(cfg);
  EXPECT_GT(s->stats.fundings, cfg.accounts.count);
  EXPECT_EQ(s->stats.rejects, 0u);
  for (const gen::Event &e : s->events) {
    if (e.kind == gen::EventKind::Funding) {
      EXPECT_TRUE(e.accept) << "funding rejected at seq " << e.seq;
    }
  }
}

// The virtual causal timeline invariants the open-loop driver depends on.
TEST(Generator, VirtualTimelineCausal) {
  config::Config cfg = TestConfig(0xC0FFEE, 40000);
  cfg.arrival.offeredRate = 50000;
  cfg.reportDelay =
      config::ReportDelay{std::chrono::milliseconds(2),
                          config::ReportDelayDistribution::Lognormal, 0.5};
  const std::unique_ptr<gen::Stream> s = gen::Generate(cfg);

  std::map<std::string, std::chrono::nanoseconds> lastByAccount;
  std::map<std::uint64_t, std::chrono::nanoseconds> ocVirtualByCorr;
  std::map<std::string, std::chrono::nanoseconds> lastSettleByAccount;
  std::chrono::nanoseconds maxVirtual{0};

  for (const gen::Event &e : s->events) {
    if (e.virtualT0 > maxVirtual) {
      maxVirtual = e.virtualT0;
    }
    if (e.kind == gen::EventKind::Funding && e.fundingIsSeed) {
      EXPECT_EQ(e.virtualT0.count(), 0)
          << "seed virtualT0 nonzero seq " << e.seq;
      continue;
    }
    auto it = lastByAccount.find(e.account);
    if (it != lastByAccount.end()) {
      EXPECT_GE(e.virtualT0, it->second) << "non-monotone seq " << e.seq;
    }
    switch (e.kind) {
    case gen::EventKind::OrderCheck: {
      ocVirtualByCorr[e.correlationId] = e.virtualT0;
      auto st = lastSettleByAccount.find(e.account);
      if (st != lastSettleByAccount.end()) {
        EXPECT_GE(e.virtualT0, st->second)
            << "order precedes prior settlement seq " << e.seq;
      }
      break;
    }
    case gen::EventKind::Settlement: {
      auto oc = ocVirtualByCorr.find(e.correlationId);
      ASSERT_NE(oc, ocVirtualByCorr.end())
          << "settlement without order seq " << e.seq;
      EXPECT_GE(e.virtualT0, oc->second)
          << "negative report delay seq " << e.seq;
      lastSettleByAccount[e.account] = e.virtualT0;
      break;
    }
    default:
      break;
    }
    lastByAccount[e.account] = e.virtualT0;
  }
  EXPECT_GT(maxVirtual.count(), 0) << "timeline never advanced";
}

// The virtual times are a pure function of (seed, config).
TEST(Generator, VirtualTimelineDeterministic) {
  config::Config cfg = TestConfig(0x5EED, 20000);
  cfg.arrival.offeredRate = 50000;
  cfg.reportDelay =
      config::ReportDelay{std::chrono::milliseconds(2),
                          config::ReportDelayDistribution::Lognormal, 0.5};
  const std::unique_ptr<gen::Stream> a = gen::Generate(cfg);
  const std::unique_ptr<gen::Stream> b = gen::Generate(cfg);
  ASSERT_EQ(a->events.size(), b->events.size());
  for (std::size_t i = 0; i < a->events.size(); ++i) {
    EXPECT_EQ(a->events[i].virtualT0, b->events[i].virtualT0)
        << "virtualT0 differs at " << i;
  }
}

//------------------------------------------------------------------------------
// Shadow-ledger arithmetic tests.

void AssertBal(gen::Ledger &l, const std::string &asset,
               const std::string &wantAvail, const std::string &wantHeld) {
  auto [h, ok] = l.Get("a", asset);
  (void)ok;
  EXPECT_EQ(h.available, Dec(wantAvail)) << "available " << asset;
  EXPECT_EQ(h.held, Dec(wantHeld)) << "held " << asset;
}

TEST(Ledger, BuyChargeAndSettlement) {
  gen::Ledger l;
  ASSERT_FALSE(
      l.ApplyFunding("a", "USD", gen::FundingKind::Absolute, Dec("10000"))
          .rejected);
  const gen::PreTradeResult res =
      l.PreTrade("a", gen::Side::Buy, "AAPL", "USD", Dec("10"), Dec("200"));
  ASSERT_TRUE(res.Accepted());
  EXPECT_EQ(res.chargeAsset, "USD");
  EXPECT_EQ(res.chargeAmount, Dec("2000"));
  AssertBal(l, "USD", "8000", "2000");

  const gen::SettlementResult sr = l.SettleFullFill(
      "a", gen::Side::Buy, "AAPL", "USD", Dec("10"), Dec("200"));
  ASSERT_FALSE(sr.error);
  EXPECT_EQ(sr.creditAmount, Dec("10"));
  AssertBal(l, "USD", "8000", "0");
  AssertBal(l, "AAPL", "10", "0");
}

TEST(Ledger, SellChargeAndSettlement) {
  gen::Ledger l;
  (void)l.ApplyFunding("a", "AAPL", gen::FundingKind::Absolute, Dec("10"));
  const gen::PreTradeResult res =
      l.PreTrade("a", gen::Side::Sell, "AAPL", "USD", Dec("4"), Dec("150"));
  ASSERT_TRUE(res.Accepted());
  EXPECT_EQ(res.chargeAsset, "AAPL");
  EXPECT_EQ(res.chargeAmount, Dec("4"));
  AssertBal(l, "AAPL", "6", "4");

  const gen::SettlementResult sr = l.SettleFullFill(
      "a", gen::Side::Sell, "AAPL", "USD", Dec("4"), Dec("150"));
  ASSERT_FALSE(sr.error);
  EXPECT_EQ(sr.creditAmount, Dec("600"));
  AssertBal(l, "AAPL", "6", "0");
  AssertBal(l, "USD", "600", "0");
}

TEST(Ledger, InsufficientFunds) {
  gen::Ledger l;
  (void)l.ApplyFunding("a", "USD", gen::FundingKind::Absolute, Dec("100"));
  const gen::PreTradeResult res =
      l.PreTrade("a", gen::Side::Buy, "AAPL", "USD", Dec("1"), Dec("150"));
  EXPECT_FALSE(res.Accepted());
  EXPECT_EQ(res.reason, gen::RejectReason::InsufficientFunds);
  AssertBal(l, "USD", "100", "0");
}

TEST(Ledger, MissingRecordRejectsAsInsufficientFunds) {
  gen::Ledger l;
  const gen::PreTradeResult res =
      l.PreTrade("a", gen::Side::Buy, "AAPL", "USD", Dec("1"), Dec("1"));
  EXPECT_FALSE(res.Accepted());
  EXPECT_EQ(res.reason, gen::RejectReason::InsufficientFunds);
}

TEST(Ledger, PruneWhenZero) {
  gen::Ledger l;
  (void)l.ApplyFunding("a", "USD", gen::FundingKind::Absolute, Dec("200"));
  (void)l.PreTrade("a", gen::Side::Buy, "AAPL", "USD", Dec("1"), Dec("200"));
  AssertBal(l, "USD", "0", "200");
  ASSERT_FALSE(
      l.SettleFullFill("a", gen::Side::Buy, "AAPL", "USD", Dec("1"), Dec("200"))
          .error);
  auto [h, ok] = l.Get("a", "USD");
  (void)h;
  EXPECT_FALSE(ok) << "zero USD slot should have been pruned";
}

TEST(Ledger, FundingDeltaSemantics) {
  gen::Ledger l;
  EXPECT_FALSE(
      l.ApplyFunding("a", "USD", gen::FundingKind::Delta, Dec("100")).rejected);
  AssertBal(l, "USD", "100", "0");
  EXPECT_FALSE(
      l.ApplyFunding("a", "USD", gen::FundingKind::Delta, Dec("50")).rejected);
  AssertBal(l, "USD", "150", "0");
  EXPECT_FALSE(l.ApplyFunding("a", "USD", gen::FundingKind::Delta, Dec("-1000"))
                   .rejected);
  AssertBal(l, "USD", "-850", "0");
}

TEST(Ledger, SettlementUnderflowIsError) {
  gen::Ledger l;
  EXPECT_TRUE(
      l.SettleFullFill("a", gen::Side::Buy, "AAPL", "USD", Dec("1"), Dec("1"))
          .error);
}

} // namespace
