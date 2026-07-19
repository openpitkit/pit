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

// Concrete typed async surface tests: the named pre-trade pipeline operations
// (StartPreTrade / ExecutePreTrade / ApplyExecutionReport /
// ApplyAccountAdjustment / Accounts) driven both by a real `openpit::Engine`
// (via `EngineAdapter`) and by a faithful mock driver. The mock exercises
// per-account ordering, abort, and drain deterministically (no engine
// concurrency assumptions); the real engine validates the end-to-end pipeline
// and the reject-vs-throw error model.

#include "openpit/account_id.hpp"
#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts.hpp"
#include "openpit/async_engine.hpp"
#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/pretrade.hpp"
#include "openpit/reject.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include <openpit.h>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <thread>
#include <utility>
#include <variant>
#include <vector>

namespace {

namespace ae = openpit::asyncengine;

using openpit::Engine;
using openpit::EngineBuilder;
using openpit::SyncPolicy;
using openpit::param::AccountId;
using openpit::param::Price;
using openpit::param::Quantity;
using openpit::pretrade::RejectCode;
using std::chrono::seconds;

namespace policies = openpit::pretrade::policies;

// Deterministic 5-second await cap so a wedged test fails fast.
constexpr seconds kAwaitCap{5};
constexpr std::uint64_t kAccountA = 1001;

//------------------------------------------------------------------------------
// Fixtures: real engine + canonical order/report

// Builds the canonical buy-1-AAPL/USD order at price 100 for `accountId`.
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

// Same order but with no operation view, so it carries no account id.
[[nodiscard]] openpit::model::Order OrderWithoutAccount() {
  openpit::model::Order order;
  return order;
}

[[nodiscard]] openpit::model::ExecutionReport TestReport(
    std::uint64_t accountId) {
  openpit::model::ExecutionReport report;
  openpit::model::ExecutionReportOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  report.operation = std::move(op);
  return report;
}

// An AccountSync rate-limit engine that admits a single order per account on
// the broker axis: the second pre-trade for an account rejects with
// RateLimitExceeded.
[[nodiscard]] Engine SingleOrderEngine() {
  EngineBuilder builder(SyncPolicy::Account);
  policies::RateLimitPolicy config;
  config.BrokerBarrier(policies::RateLimitBrokerBarrier(policies::RateLimit(
      /*maxOrders=*/1, /*windowNanoseconds=*/60'000'000'000)));
  config.AddTo(builder);
  return builder.Build();
}

[[nodiscard]] Engine OrderValidationEngine() {
  EngineBuilder builder(SyncPolicy::Account);
  builder.Add(policies::OrderValidationPolicy{});
  return builder.Build();
}

struct StubAdjustment {
  [[nodiscard]] OpenPitAccountAdjustment Raw() const noexcept {
    return OpenPitAccountAdjustment{};
  }
};

//------------------------------------------------------------------------------
// Mock driver: faithful stand-in for the engine-call seam.
//
// Mirrors the five `EngineAdapter` members so
// `TypedAsyncEngine<MockEngineAdapter>` compiles and exercises
// dispatch/threading deterministically. Each pre-trade returns a passing result
// with a default (null-handle) Request/Reservation, which is sufficient: the
// typed layer only inspects `Passed()` and wraps the handle, and the void
// finalizers are no-ops on a null handle.

class ConcurrencyProbe {
 public:
  class Span {
   public:
    Span(ConcurrencyProbe& probe, std::uint64_t account)
        : m_probe(&probe), m_account(account) {
      std::lock_guard<std::mutex> lock(m_probe->m_mutex);
      const std::int64_t now = ++m_probe->m_active[account];
      std::int64_t& peak = m_probe->m_peak[account];
      if (now > peak) {
        peak = now;
      }
    }
    ~Span() {
      std::lock_guard<std::mutex> lock(m_probe->m_mutex);
      --m_probe->m_active[m_account];
    }
    Span(const Span&) = delete;
    Span& operator=(const Span&) = delete;

   private:
    ConcurrencyProbe* m_probe;
    std::uint64_t m_account;
  };

  [[nodiscard]] std::int64_t PeakFor(std::uint64_t account) {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_peak[account];
  }

 private:
  std::mutex m_mutex;
  std::map<std::uint64_t, std::int64_t> m_active;
  std::map<std::uint64_t, std::int64_t> m_peak;
};

class Gate {
 public:
  void Wait() {
    std::unique_lock<std::mutex> lock(m_mutex);
    m_cv.wait(lock, [this] { return m_open; });
  }
  void Open() {
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      m_open = true;
    }
    m_cv.notify_all();
  }

 private:
  std::mutex m_mutex;
  std::condition_variable m_cv;
  bool m_open = false;
};

// Accounts stub: records block/unblock so admin routing is observable.
class MockAccounts {
 public:
  explicit MockAccounts(std::atomic<std::size_t>* blocks) : m_blocks(blocks) {}

  void Block(AccountId, std::string_view) const noexcept {
    m_blocks->fetch_add(1, std::memory_order_relaxed);
  }
  void Unblock(AccountId) const noexcept {}

 private:
  std::atomic<std::size_t>* m_blocks;
};

struct MockEngineAdapter {
  ConcurrencyProbe* probe = nullptr;
  std::atomic<std::size_t> starts{0};
  std::atomic<std::size_t> blocks{0};

  [[nodiscard]] openpit::pretrade::StartResult StartPreTrade(
      const openpit::model::Order& order) {
    const std::uint64_t account = order.operation && order.operation->accountId
                                      ? order.operation->accountId->Raw()
                                      : 0;
    std::optional<ConcurrencyProbe::Span> span;
    if (probe != nullptr) {
      span.emplace(*probe, account);
    }
    starts.fetch_add(1, std::memory_order_relaxed);
    openpit::pretrade::StartResult result;
    result.request.emplace(openpit::pretrade::Request());  // null-handle pass.
    return result;
  }

  [[nodiscard]] openpit::pretrade::ExecuteResult ExecutePreTrade(
      const openpit::model::Order&) {
    openpit::pretrade::ExecuteResult result;
    result.reservation.emplace(openpit::pretrade::Reservation());
    return result;
  }

  [[nodiscard]] openpit::PostTradeResult ApplyExecutionReport(
      const openpit::model::ExecutionReport&) {
    return openpit::PostTradeResult{};
  }

  template <typename Adjustment>
  [[nodiscard]] openpit::AdjustmentResult ApplyAccountAdjustment(
      AccountId, const std::vector<Adjustment>&) {
    return openpit::AdjustmentResult{};
  }

  [[nodiscard]] MockAccounts Accounts() { return MockAccounts(&blocks); }
};

//------------------------------------------------------------------------------
// Lifecycle (real engine): start -> execute -> commit, then clean stop.

TEST(TypedAsyncLifecycle, RealEngineStartExecuteCommit) {
  Engine engine = SingleOrderEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 2);

  ae::StartOutcome<ae::EngineAdapter> start =
      async.StartPreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(start.Passed());
  ASSERT_TRUE(start.rejects.empty());

  auto executed = start.request->Execute().Await(kAwaitCap).value();
  ASSERT_TRUE(executed.first);  // non-null reservation.
  EXPECT_TRUE(executed.second.empty());

  // Commit-and-close finalizes the reservation through the same account queue.
  EXPECT_TRUE(executed.first->CommitAndClose().Await(kAwaitCap).has_value());

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncLifecycle, RealEngineExecutePreTradeThenCommit) {
  Engine engine = SingleOrderEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Dynamic().Build();

  ae::ExecuteOutcome<ae::EngineAdapter> exec =
      async.ExecutePreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(exec.Passed());
  EXPECT_TRUE(exec.rejects.empty());
  EXPECT_TRUE(exec.reservation->Commit().Await(kAwaitCap).has_value());
  EXPECT_TRUE(exec.reservation->Close().Await(kAwaitCap).has_value());

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

//------------------------------------------------------------------------------
// Reject vs throw (real engine).

// A policy reject is a VALUE in the outcome tuple, never thrown.
TEST(TypedAsyncErrorModel, RateLimitRejectIsValueNotThrow) {
  Engine engine = SingleOrderEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Sharded(1).Build();

  // First order commits the single-order budget.
  {
    auto exec =
        async.ExecutePreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
    ASSERT_TRUE(exec.Passed());
    EXPECT_TRUE(
        exec.reservation->CommitAndClose().Await(kAwaitCap).has_value());
  }

  // Second order is rejected: the future resolves (does not throw) with a
  // non-passing outcome carrying the rate-limit reject.
  ae::ExecuteOutcome<ae::EngineAdapter> rejected =
      async.ExecutePreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  EXPECT_FALSE(rejected.Passed());
  EXPECT_FALSE(rejected.reservation);
  ASSERT_EQ(rejected.rejects.size(), 1u);
  EXPECT_EQ(rejected.rejects.front().code, RejectCode::RateLimitExceeded);

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

// A missing account id is delivered as the MissingAccountId VALUE error through
// the future (rethrown by Await on the caller thread), mirroring Go's
// ErrMissingAccountID. The future resolves synchronously on the submitter.
TEST(TypedAsyncErrorModel, MissingAccountIdResolvesWithError) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Sharded(1).Build();

  ae::Future<ae::StartOutcome<ae::EngineAdapter>> future =
      async.StartPreTrade(OrderWithoutAccount());
  try {
    (void)future.Await();
    FAIL() << "expected MissingAccountId";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::MissingAccountId);
  }

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

// An ABI/boundary failure inside a driver call (here: a null-handle engine)
// surfaces as a TaskFailed error VALUE on the future; the engine's thrown
// openpit::Error never crosses the worker thread.
TEST(TypedAsyncErrorModel, AbiFailureBecomesTaskFailed) {
  const Engine nullEngine;  // default-constructed: null C handle.
  ae::EngineAdapter driver(nullEngine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Sharded(1).Build();

  ae::Future<openpit::PostTradeResult> future =
      async.ApplyExecutionReport(TestReport(kAccountA));
  try {
    (void)future.Await(kAwaitCap);
    FAIL() << "expected TaskFailed";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::TaskFailed);
  }

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

// Empty-batch adjustment applies cleanly: not rejected, no outcomes.
TEST(TypedAsyncErrorModel, EmptyAdjustmentBatchApplies) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Sharded(1).Build();

  ae::AdjustmentOutcome out = async
                                  .ApplyAccountAdjustment<StubAdjustment>(
                                      AccountId::FromUint64(kAccountA),
                                      /*adjustments=*/{})
                                  .Await(kAwaitCap)
                                  .value();
  EXPECT_TRUE(out.Passed());
  EXPECT_FALSE(out.batchError);
  EXPECT_TRUE(out.outcomes.empty());

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

//------------------------------------------------------------------------------
// Account-pinned ordering (mock driver): no two same-account driver calls
// overlap, even under heavy concurrent submission.

TEST(TypedAsyncThreading, PerAccountSerializationHolds) {
  constexpr int kAccounts = 4;
  constexpr int kSubmittersPerAccount = 4;
  constexpr int kPerSubmitter = 40;

  ConcurrencyProbe probe;
  MockEngineAdapter driver;
  driver.probe = &probe;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Sharded(4).Build();

  std::vector<std::uint64_t> accounts;
  for (int i = 0; i < kAccounts; ++i) {
    accounts.push_back(static_cast<std::uint64_t>(100 + i));
  }

  std::vector<std::thread> submitters;
  std::atomic<int> failures{0};
  for (const std::uint64_t account : accounts) {
    for (int s = 0; s < kSubmittersPerAccount; ++s) {
      submitters.emplace_back([&, account] {
        for (int j = 0; j < kPerSubmitter; ++j) {
          ae::StartOutcome<MockEngineAdapter> r =
              async.StartPreTrade(TestOrder(account)).Await(kAwaitCap).value();
          if (!r.Passed()) {
            failures.fetch_add(1, std::memory_order_relaxed);
          }
        }
      });
    }
  }
  for (std::thread& t : submitters) {
    t.join();
  }

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
  EXPECT_EQ(failures.load(), 0);
  for (const std::uint64_t account : accounts) {
    EXPECT_LE(probe.PeakFor(account), 1)
        << "account " << account << " saw overlapping driver calls";
  }
}

//------------------------------------------------------------------------------
// Clean drain with in-flight work (mock driver).

TEST(TypedAsyncShutdown, GracefulDrainsInFlightWork) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Sharded(1).Build();

  Gate gate;
  std::atomic<bool> taskFinished{false};
  // Pin a task in flight via Submit so graceful stop must wait for it.
  ae::Future<std::monostate> inFlight =
      async.Submit(AccountId::FromUint64(kAccountA), [&] {
        gate.Wait();
        taskFinished.store(true);
      });

  std::atomic<bool> stopReturned{false};
  std::thread stopper([&] {
    EXPECT_TRUE(async.StopGraceful(seconds(10)));
    stopReturned.store(true);
  });

  gate.Open();
  stopper.join();

  EXPECT_TRUE(stopReturned.load());
  EXPECT_TRUE(taskFinished.load());
  ASSERT_TRUE(inFlight.Await(kAwaitCap).has_value());
}

//------------------------------------------------------------------------------
// Submit after stop / hard-stop abort (mock driver).

TEST(TypedAsyncShutdown, SubmitAfterStopFailsWithStopped) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Sharded(1).Build();
  EXPECT_TRUE(async.StopGraceful());

  ae::Future<ae::StartOutcome<MockEngineAdapter>> future =
      async.StartPreTrade(TestOrder(kAccountA));
  try {
    (void)future.Await();
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
}

// Hard stop aborts a not-yet-started typed call with Stopped; the abort path
// resolves the typed future (it never silently vanishes).
TEST(TypedAsyncShutdown, HardStopAbortsQueuedCall) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Sharded(1).Build();

  Gate gate;
  Gate started;
  ae::Future<std::monostate> running =
      async.Submit(AccountId::FromUint64(kAccountA), [&] {
        started.Open();
        gate.Wait();
      });

  // Queue a typed call behind the gated task on the same account.
  ae::Future<ae::StartOutcome<MockEngineAdapter>> queued =
      async.StartPreTrade(TestOrder(kAccountA));

  std::thread stopper([&] {
    started.Wait();
    gate.Open();
    EXPECT_TRUE(async.StopHard(seconds(10)));
  });
  stopper.join();

  ASSERT_TRUE(running.Await(kAwaitCap).has_value());

  // The queued call either ran (passed) or was aborted with Stopped; either way
  // the future is resolved.
  try {
    ae::StartOutcome<MockEngineAdapter> r = queued.Await(kAwaitCap).value();
    EXPECT_TRUE(r.Passed());
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
}

//------------------------------------------------------------------------------
// Account-admin routing (mock driver): Block routes to the account queue.

TEST(TypedAsyncAccounts, BlockRoutesThroughAccountQueue) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Dynamic().Build();

  ae::AsyncAccounts<MockEngineAdapter> accounts = async.Accounts();
  ASSERT_TRUE(accounts.Block(AccountId::FromUint64(kAccountA), "kill")
                  .Await(kAwaitCap)
                  .has_value());
  ASSERT_TRUE(accounts.Unblock(AccountId::FromUint64(kAccountA))
                  .Await(kAwaitCap)
                  .has_value());

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
  EXPECT_EQ(driver.blocks.load(), 1u);
}

//------------------------------------------------------------------------------
// RegisterGroup with empty accounts -> MissingAccountId value error (real
// engine, so the driver's Accounts().RegisterGroup signature is satisfied).

TEST(TypedAsyncAccounts, RegisterGroupEmptyAccountsMissingId) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Sharded(1).Build();

  ae::AsyncAccounts<ae::EngineAdapter> accounts = async.Accounts();
  ae::Future<std::optional<openpit::accounts::AccountGroupError>> future =
      accounts.RegisterGroup(/*accounts=*/{},
                             openpit::param::AccountGroupId::FromUint32(7));
  try {
    (void)future.Await(kAwaitCap);
    FAIL() << "expected MissingAccountId";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::MissingAccountId);
  }

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

}  // namespace
