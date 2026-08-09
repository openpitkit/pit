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

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts/accounts.hpp"
#include "openpit/async_engine.hpp"
#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/decision.hpp"
#include "openpit/pretrade/pretrade.hpp"

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
#include <type_traits>
#include <unordered_set>
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
// Idle retirement is timer-driven: the idle window plus up to one sweep
// period, and any lane touch restarts the window. Tests that must observe a
// retirement therefore wait with a wide margin instead of a bound tuned to the
// configured window, which would turn ordinary scheduling jitter into a
// failure.
constexpr seconds kIdleRetireCap{30};
// Short probe for assertions that something must NOT happen. Only a negative
// direction may use it: a positive-direction wait uses `kAwaitCap`.
constexpr std::chrono::milliseconds kNegativeProbe{100};
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

[[nodiscard]] openpit::model::ExecutionReport TestLockedReport(
    std::uint64_t accountId) {
  openpit::model::ExecutionReport report = TestReport(accountId);
  report.fill.emplace();
  report.fill->lock.emplace();
  report.fill->lock->Push(openpit::param::DefaultPolicyGroupId,
                          Price::FromString("100"));
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
 private:
  friend class openpit::detail::NativeAccess;

  [[nodiscard]] openpit::accountadjustment::detail::RawAccountAdjustment
  Native() const noexcept {
    return {};
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
    Span(ConcurrencyProbe& probe, AccountId account)
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
    AccountId m_account;
  };

  [[nodiscard]] std::int64_t PeakFor(AccountId account) {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_peak[account];
  }

 private:
  std::mutex m_mutex;
  std::map<AccountId, std::int64_t> m_active;
  std::map<AccountId, std::int64_t> m_peak;
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

  template <typename Rep, typename Period>
  [[nodiscard]] bool WaitFor(std::chrono::duration<Rep, Period> timeout) {
    std::unique_lock<std::mutex> lock(m_mutex);
    return m_cv.wait_for(lock, timeout, [this] { return m_open; });
  }

 private:
  std::mutex m_mutex;
  std::condition_variable m_cv;
  bool m_open = false;
};

class BlockingCreateObserver final : public ae::Observer {
 public:
  BlockingCreateObserver(Gate* entered, Gate* release)
      : m_entered(entered), m_release(release) {}

  void OnQueueCreated(AccountId, std::size_t) override {
    if (m_first.exchange(false, std::memory_order_relaxed)) {
      m_entered->Open();
      m_release->Wait();
    }
  }

 private:
  Gate* m_entered;
  Gate* m_release;
  std::atomic<bool> m_first{true};
};

class RetiringCreateObserver final : public ae::Observer {
 public:
  RetiringCreateObserver(Gate* firstCreated, Gate* releaseFirstCreate,
                         Gate* removed)
      : m_firstCreated(firstCreated),
        m_releaseFirstCreate(releaseFirstCreate),
        m_removed(removed) {}

  void OnQueueCreated(AccountId, std::size_t) override {
    if (m_creates.fetch_add(1, std::memory_order_relaxed) == 0) {
      m_firstCreated->Open();
      m_releaseFirstCreate->Wait();
    }
  }

  void OnQueueRemoved(AccountId, std::size_t) override { m_removed->Open(); }

  [[nodiscard]] std::size_t Creates() const {
    return m_creates.load(std::memory_order_relaxed);
  }

 private:
  Gate* m_firstCreated;
  Gate* m_releaseFirstCreate;
  Gate* m_removed;
  std::atomic<std::size_t> m_creates{0};
};

class QueueFullGateObserver final : public ae::Observer {
 public:
  explicit QueueFullGateObserver(Gate* blocked, Gate* removed = nullptr)
      : m_blocked(blocked), m_removed(removed) {}

  void OnQueueFullBlocked(AccountId, std::chrono::nanoseconds) override {
    m_blocked->Open();
  }

  void OnQueueRemoved(AccountId accountId, std::size_t) override {
    if (m_removed != nullptr && accountId == AccountId::FromUint64(kAccountA)) {
      m_removed->Open();
    }
  }

 private:
  Gate* m_blocked;
  Gate* m_removed;
};

class QueueFullProducerCountObserver final : public ae::Observer {
 public:
  QueueFullProducerCountObserver(Gate* blocked, std::size_t expected)
      : m_blocked(blocked), m_expected(expected) {}

  void OnQueueFullBlocked(AccountId, std::chrono::nanoseconds) override {
    bool ready = false;
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      m_blockedProducers.insert(std::this_thread::get_id());
      ready = m_blockedProducers.size() >= m_expected;
    }
    if (ready) {
      m_blocked->Open();
    }
  }

 private:
  Gate* m_blocked;
  std::size_t m_expected;
  std::mutex m_mutex;
  std::unordered_set<std::thread::id> m_blockedProducers;
};

// Pushes a fatal evaluation reject, so drop copy aborts and produces rejects
// instead of an operation.
class FatalRejectPolicy {
 public:
  void PerformPreTradeCheck(const openpit::pretrade::Context& /*context*/,
                            openpit::pretrade::PolicyDecision& decision) const {
    decision.Push(openpit::pretrade::Reject(
        "FatalRejectPolicy", openpit::pretrade::RejectScope::Order,
        RejectCode::MissingRequiredField, "fatal evaluation failure",
        "forced failure"));
  }
};

struct TypedAsyncDeskOrder : public openpit::model::Order {};

struct TypedAsyncDeskReport : public openpit::model::ExecutionReport {};

class TypedAsyncPayloadPolicy {
 public:
  TypedAsyncPayloadPolicy(std::atomic<bool>* orderSeen,
                          std::atomic<bool>* reportSeen)
      : m_orderSeen(orderSeen), m_reportSeen(reportSeen) {}

  [[nodiscard]] std::string_view Name() const noexcept {
    return "TypedAsyncPayloadPolicy";
  }

  void PerformPreTradeCheck(const TypedAsyncDeskOrder&,
                            const openpit::pretrade::Context&,
                            openpit::pretrade::PolicyDecision&) const {
    m_orderSeen->store(true, std::memory_order_relaxed);
  }

  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext&,
                       const TypedAsyncDeskReport&,
                       openpit::pretrade::PostTradeAdjustments&,
                       openpit::pretrade::PostTradePnls&) const {
    m_reportSeen->store(true, std::memory_order_relaxed);
    return {};
  }

 private:
  std::atomic<bool>* m_orderSeen;
  std::atomic<bool>* m_reportSeen;
};

using TypedAsyncPayloadAdapter =
    openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
        TypedAsyncPayloadPolicy, TypedAsyncDeskOrder, TypedAsyncDeskReport>;

class BlockingRollbackPolicy {
 public:
  BlockingRollbackPolicy(Gate* started, Gate* release)
      : m_started(started), m_release(release) {}

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& /*context*/,
      openpit::tx::Mutations& mutations, openpit::pretrade::Result& /*result*/,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    Gate* started = m_started;
    Gate* release = m_release;
    mutations.Push([] {},
                   [started, release] {
                     started->Open();
                     release->Wait();
                   });
  }

 private:
  Gate* m_started;
  Gate* m_release;
};

class CountingRollbackPolicy {
 public:
  explicit CountingRollbackPolicy(
      std::atomic<std::size_t>* rollbacks,
      std::atomic<bool>* producerRan = nullptr,
      std::atomic<bool>* cleanupOvertookProducer = nullptr)
      : m_rollbacks(rollbacks),
        m_producerRan(producerRan),
        m_cleanupOvertookProducer(cleanupOvertookProducer) {}

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& /*context*/,
      openpit::tx::Mutations& mutations, openpit::pretrade::Result& /*result*/,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    std::atomic<std::size_t>* rollbacks = m_rollbacks;
    std::atomic<bool>* producerRan = m_producerRan;
    std::atomic<bool>* cleanupOvertookProducer = m_cleanupOvertookProducer;
    mutations.Push(
        [] {},
        [rollbacks, producerRan, cleanupOvertookProducer] {
          if (producerRan != nullptr && cleanupOvertookProducer != nullptr) {
            cleanupOvertookProducer->store(
                !producerRan->load(std::memory_order_acquire),
                std::memory_order_release);
          }
          rollbacks->fetch_add(1, std::memory_order_relaxed);
        });
  }

 private:
  std::atomic<std::size_t>* m_rollbacks;
  std::atomic<bool>* m_producerRan;
  std::atomic<bool>* m_cleanupOvertookProducer;
};

// Accounts stub: records block/unblock so admin routing is observable.
class MockAccounts {
 public:
  MockAccounts(std::atomic<std::size_t>* blocks,
               std::atomic<std::size_t>* globalUnblocks)
      : m_blocks(blocks), m_globalUnblocks(globalUnblocks) {}

  void Block(AccountId, std::string_view) const noexcept {
    m_blocks->fetch_add(1, std::memory_order_relaxed);
  }
  void Unblock(AccountId) const noexcept {}
  void UnblockAll() const noexcept {
    m_globalUnblocks->fetch_add(1, std::memory_order_relaxed);
  }

 private:
  std::atomic<std::size_t>* m_blocks;
  std::atomic<std::size_t>* m_globalUnblocks;
};

struct MockEngineAdapter {
  ConcurrencyProbe* probe = nullptr;
  std::atomic<std::size_t> starts{0};
  std::atomic<std::size_t> blocks{0};
  std::atomic<std::size_t> globalUnblocks{0};
  std::atomic<std::size_t> dropCopies{0};

  [[nodiscard]] openpit::pretrade::StartResult StartPreTrade(
      std::unique_ptr<const openpit::Order> order) {
    const auto* modelOrder =
        dynamic_cast<const openpit::model::Order*>(order.get());
    const AccountId account = modelOrder != nullptr && modelOrder->operation &&
                                      modelOrder->operation->accountId
                                  ? *modelOrder->operation->accountId
                                  : AccountId{};
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
      std::unique_ptr<const openpit::Order>) {
    openpit::pretrade::ExecuteResult result;
    result.reservation.emplace(openpit::pretrade::Reservation());
    return result;
  }

  [[nodiscard]] openpit::pretrade::DropCopyResult ApplyDropCopy(
      std::unique_ptr<const openpit::Order>) {
    dropCopies.fetch_add(1, std::memory_order_relaxed);
    openpit::pretrade::DropCopyResult result;
    result.operation.emplace(openpit::pretrade::DropCopyOperation());
    return result;
  }

  [[nodiscard]] std::size_t DropCopyCalls() const {
    return dropCopies.load(std::memory_order_relaxed);
  }

  [[nodiscard]] openpit::PostTradeResult ApplyExecutionReport(
      std::unique_ptr<const openpit::ExecutionReport>) {
    return openpit::PostTradeResult{};
  }

  template <typename Adjustment>
  [[nodiscard]] openpit::AdjustmentResult ApplyAccountAdjustment(
      AccountId, const std::vector<Adjustment>&) {
    return openpit::AdjustmentResult{};
  }

  [[nodiscard]] MockAccounts Accounts() {
    return MockAccounts(&blocks, &globalUnblocks);
  }
};

struct MakeTypedAsyncEngineCallable {
  template <typename EngineT>
  auto operator()(EngineT&& engine) const
      -> decltype(ae::MakeTypedAsyncEngine(std::forward<EngineT>(engine), 1));
};

static_assert(
    std::is_move_constructible_v<ae::TypedAsyncEngine<MockEngineAdapter>>);
static_assert(
    !std::is_move_assignable_v<ae::TypedAsyncEngine<MockEngineAdapter>>);
static_assert(std::is_move_constructible_v<ae::OwnedTypedAsyncEngine>);
static_assert(!std::is_move_assignable_v<ae::OwnedTypedAsyncEngine>);
static_assert(std::is_constructible_v<ae::EngineAdapter, const Engine&>);
static_assert(!std::is_constructible_v<ae::EngineAdapter, Engine&&>);
static_assert(!std::is_constructible_v<ae::EngineAdapter, const Engine&&>);
static_assert(std::is_invocable_v<MakeTypedAsyncEngineCallable, const Engine&>);
static_assert(!std::is_invocable_v<MakeTypedAsyncEngineCallable, Engine&&>);
static_assert(
    !std::is_invocable_v<MakeTypedAsyncEngineCallable, const Engine&&>);

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

TEST(TypedAsyncLifecycle, OutstandingWrappersSurviveEngineMove) {
  Engine engine = SingleOrderEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::StartOutcome<ae::EngineAdapter> start =
      async.StartPreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(start.Passed());

  auto moved = std::move(async);
  auto executed = start.request->Execute().Await(kAwaitCap).value();
  ASSERT_TRUE(executed.first);

  auto movedAgain = std::move(moved);
  EXPECT_TRUE(executed.first->CommitAndClose().Await(kAwaitCap).has_value());
  EXPECT_TRUE(movedAgain.StopGraceful(seconds(10)));
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

TEST(TypedAsyncLifecycle, RealEngineDropCopyThenCommitAndClose) {
  Engine engine = SingleOrderEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();

  ASSERT_TRUE(outcome.Passed());
  EXPECT_TRUE(outcome.rejects.empty());
  EXPECT_EQ(outcome.operation->AccountId(),
            ::openpit::param::AccountId::FromUint64(kAccountA));
  EXPECT_TRUE(outcome.operation->Lock().IsEmpty());
  EXPECT_TRUE(outcome.operation->AccountAdjustments().empty());
  EXPECT_FALSE(outcome.operation->AccountBlock().has_value());
  EXPECT_FALSE(outcome.operation->IsAccountBlocked());
  EXPECT_TRUE(outcome.operation->CommitAndClose().Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

// Commit and Close are separate finalizers: the operation stays alive between
// them, exactly like the reservation pair.
TEST(TypedAsyncLifecycle, RealEngineDropCopyCommitThenClose) {
  Engine engine = SingleOrderEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());

  EXPECT_TRUE(outcome.operation->Commit().Await(kAwaitCap).has_value());
  EXPECT_TRUE(outcome.operation->Close().Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncLifecycle, RealEngineDropCopyRollbackThenClose) {
  Engine engine = SingleOrderEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());

  EXPECT_TRUE(outcome.operation->Rollback().Await(kAwaitCap).has_value());
  EXPECT_TRUE(outcome.operation->Close().Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncLifecycle, RealEngineDropCopyRollbackAndClose) {
  Engine engine = SingleOrderEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());

  EXPECT_TRUE(
      outcome.operation->RollbackAndClose().Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

// A Close submitted after a graceful stop cannot enter the closed queue, but
// it must still release the native operation before reporting Stopped.
TEST(TypedAsyncShutdown, DropCopyCloseAfterShardedStopReleasesOperation) {
  Engine engine = SingleOrderEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());
  ASSERT_TRUE(async.StopGraceful(seconds(10)));

  ae::Future<std::monostate> close = outcome.operation->Close();
  try {
    (void)close.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
  EXPECT_THROW({ (void)outcome.operation->Lock(); }, openpit::Error);
}

TEST(TypedAsyncShutdown, DropCopyCloseAfterDynamicStopReleasesOperation) {
  Engine engine = SingleOrderEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Dynamic().Build();

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());
  ASSERT_TRUE(async.StopGraceful(seconds(10)));

  ae::Future<std::monostate> close = outcome.operation->Close();
  try {
    (void)close.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
  EXPECT_THROW({ (void)outcome.operation->Lock(); }, openpit::Error);
}

TEST(TypedAsyncLifecycle, ReservationAndRequestRejectAccessAfterClose) {
  Engine engine = OrderValidationEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::ExecuteOutcome<ae::EngineAdapter> executed =
      async.ExecutePreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(executed.Passed());
  ASSERT_TRUE(executed.reservation->Close().Await(kAwaitCap).has_value());
  try {
    (void)executed.reservation->Commit().Await(kAwaitCap);
    FAIL() << "expected TaskFailed after reservation Close";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::TaskFailed);
  }

  ae::StartOutcome<ae::EngineAdapter> started =
      async.StartPreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(started.Passed());
  ASSERT_TRUE(started.request->Close().Await(kAwaitCap).has_value());
  try {
    (void)started.request->Execute().Await(kAwaitCap);
    FAIL() << "expected TaskFailed after request Close";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::TaskFailed);
  }

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

// A fatal evaluation reject is a VALUE in the outcome, never thrown, and no
// operation is produced.
TEST(TypedAsyncErrorModel, DropCopyFatalRejectIsValueNotThrow) {
  EngineBuilder builder(SyncPolicy::Account);
  openpit::pretrade::CustomPolicy<FatalRejectPolicy> policy(
      "FatalRejectPolicy", FatalRejectPolicy{});
  builder.Add(policy);
  Engine engine = builder.Build();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();

  EXPECT_FALSE(outcome.Passed());
  EXPECT_FALSE(outcome.operation);
  ASSERT_EQ(outcome.rejects.size(), 1u);
  EXPECT_EQ(outcome.rejects.front().code, RejectCode::MissingRequiredField);
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncThreading, DropCopyFinalizationBlocksNextSameAccountCall) {
  Gate rollbackStarted;
  Gate releaseRollback;
  EngineBuilder builder(SyncPolicy::Account);
  openpit::pretrade::CustomPolicy<BlockingRollbackPolicy> policy(
      "BlockingRollbackPolicy",
      BlockingRollbackPolicy(&rollbackStarted, &releaseRollback));
  builder.Add(policy);
  Engine engine = builder.Build();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      async.ApplyDropCopy(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());
  const std::shared_ptr<ae::AsyncDropCopyOperation<ae::EngineAdapter>>
      operation = outcome.operation;

  ae::Future<std::monostate> rollback = operation->Rollback();
  rollbackStarted.Wait();
  ae::Future<ae::StartOutcome<ae::EngineAdapter>> next =
      async.StartPreTrade(TestOrder(kAccountA));
  EXPECT_FALSE(next.Done());

  releaseRollback.Open();
  EXPECT_TRUE(rollback.Await(kAwaitCap).has_value());
  ae::StartOutcome<ae::EngineAdapter> started = next.Await(kAwaitCap).value();
  ASSERT_TRUE(started.Passed());
  EXPECT_TRUE(started.request->Close().Await(kAwaitCap).has_value());
  EXPECT_TRUE(operation->Close().Await(kAwaitCap).has_value());
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
// the future (rethrown by Await on the caller thread). The future resolves
// synchronously on the submitter.
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

// Drop copy needs a readable account id just as much as the other two entry
// points, so the facade refuses the order up front instead of queueing work the
// core would reject with MissingRequiredField anyway.
TEST(TypedAsyncErrorModel, DropCopyMissingAccountFailsWithMissingAccountId) {
  Engine engine = OrderValidationEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  try {
    (void)async.ApplyDropCopy(OrderWithoutAccount()).Await(kAwaitCap);
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

TEST(TypedAsyncPayloadOwnership, LvalueReportRetriesAfterStoppedSubmit) {
  Engine engine = OrderValidationEngine();
  auto stopped = ae::MakeTypedAsyncEngine(engine, 1);
  ASSERT_TRUE(stopped.StopGraceful(seconds(10)));

  openpit::model::ExecutionReport report = TestLockedReport(kAccountA);
  try {
    (void)stopped.ApplyExecutionReport(report).Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }

  ASSERT_TRUE(report.operation.has_value());
  ASSERT_TRUE(report.operation->accountId.has_value());
  EXPECT_EQ(*report.operation->accountId, AccountId::FromUint64(kAccountA));
  ASSERT_TRUE(report.fill.has_value());
  ASSERT_TRUE(report.fill->lock.has_value());
  ASSERT_TRUE(*report.fill->lock);
  EXPECT_EQ(report.fill->lock->Len(), 1u);

  auto retry = ae::MakeTypedAsyncEngine(engine, 1);
  EXPECT_TRUE(retry.ApplyExecutionReport(report).Await(kAwaitCap).has_value());
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership, LvalueReportRetriesAfterQueueLimit) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter limitedDriver(engine);
  auto limited = ae::TypedBuilder<ae::EngineAdapter>(limitedDriver)
                     .Dynamic()
                     .MaxQueues(1)
                     .IdleCleanupAfter(seconds(10))
                     .Build();
  Gate occupied;
  Gate release;
  ae::Future<std::monostate> running =
      limited.Submit(AccountId::FromUint64(kAccountA), [&] {
        occupied.Open();
        release.Wait();
      });
  ASSERT_TRUE(occupied.WaitFor(kAwaitCap));

  openpit::model::ExecutionReport report = TestLockedReport(kAccountA + 1);
  try {
    (void)limited.ApplyExecutionReport(report).Await(kAwaitCap);
    FAIL() << "expected QueueLimit";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::QueueLimit);
  }

  ASSERT_TRUE(report.operation.has_value());
  ASSERT_TRUE(report.operation->accountId.has_value());
  EXPECT_EQ(*report.operation->accountId, AccountId::FromUint64(kAccountA + 1));
  ASSERT_TRUE(report.fill.has_value());
  ASSERT_TRUE(report.fill->lock.has_value());
  ASSERT_TRUE(*report.fill->lock);
  EXPECT_EQ(report.fill->lock->Len(), 1u);

  release.Open();
  ASSERT_TRUE(running.Await(kAwaitCap).has_value());
  ASSERT_TRUE(limited.StopGraceful(seconds(10)));

  ae::EngineAdapter retryDriver(engine);
  auto retry =
      ae::TypedBuilder<ae::EngineAdapter>(retryDriver).Sharded(1).Build();
  EXPECT_TRUE(retry.ApplyExecutionReport(report).Await(kAwaitCap).has_value());
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership,
     LvalueDropCopyRetriesAfterHardStopAbortsQueuedCall) {
  Engine engine = OrderValidationEngine();
  auto stopped = ae::MakeTypedAsyncEngine(engine, 1);
  Gate occupied;
  Gate release;
  const AccountId account = AccountId::FromUint64(kAccountA);
  ae::Future<std::monostate> running = stopped.Submit(account, [&] {
    occupied.Open();
    release.Wait();
  });
  ASSERT_TRUE(occupied.WaitFor(kAwaitCap));

  openpit::model::Order order = TestOrder(kAccountA);
  ae::Future<ae::DropCopyOutcome<ae::EngineAdapter>> aborted =
      stopped.ApplyDropCopy(order);

  EXPECT_FALSE(stopped.StopHard(std::chrono::milliseconds(1)));
  release.Open();
  EXPECT_TRUE(stopped.StopHard(seconds(10)));
  EXPECT_TRUE(running.Await(kAwaitCap).has_value());
  try {
    (void)aborted.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }

  ASSERT_TRUE(order.operation.has_value());
  ASSERT_TRUE(order.operation->accountId.has_value());
  EXPECT_EQ(*order.operation->accountId, account);

  auto retry = ae::MakeTypedAsyncEngine(engine, 1);
  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      retry.ApplyDropCopy(order).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());
  EXPECT_TRUE(
      outcome.operation->RollbackAndClose().Await(kAwaitCap).has_value());
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership, LvalueDropCopyRetriesAfterQueueLimit) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter limitedDriver(engine);
  auto limited = ae::TypedBuilder<ae::EngineAdapter>(limitedDriver)
                     .Dynamic()
                     .MaxQueues(1)
                     .IdleCleanupAfter(seconds(10))
                     .Build();
  Gate occupied;
  Gate release;
  ae::Future<std::monostate> running =
      limited.Submit(AccountId::FromUint64(kAccountA), [&] {
        occupied.Open();
        release.Wait();
      });
  ASSERT_TRUE(occupied.WaitFor(kAwaitCap));

  openpit::model::Order order = TestOrder(kAccountA + 1);
  try {
    (void)limited.ApplyDropCopy(order).Await(kAwaitCap);
    FAIL() << "expected QueueLimit";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::QueueLimit);
  }

  ASSERT_TRUE(order.operation.has_value());
  ASSERT_TRUE(order.operation->accountId.has_value());
  EXPECT_EQ(*order.operation->accountId, AccountId::FromUint64(kAccountA + 1));

  release.Open();
  ASSERT_TRUE(running.Await(kAwaitCap).has_value());
  ASSERT_TRUE(limited.StopGraceful(seconds(10)));

  auto retry = ae::MakeTypedAsyncEngine(engine, 1);
  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      retry.ApplyDropCopy(order).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());
  EXPECT_TRUE(
      outcome.operation->RollbackAndClose().Await(kAwaitCap).has_value());
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership, LvalueDropCopyRetriesAfterSubmitCancelled) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter boundedDriver(engine);
  auto bounded = ae::TypedBuilder<ae::EngineAdapter>(boundedDriver)
                     .WithQueueCapacity(1)
                     .Sharded(1)
                     .Build();
  Gate occupied;
  Gate release;
  const AccountId account = AccountId::FromUint64(kAccountA);
  ae::Future<std::monostate> running = bounded.Submit(account, [&] {
    occupied.Open();
    release.Wait();
  });
  ASSERT_TRUE(occupied.WaitFor(kAwaitCap));
  ae::Future<std::monostate> accepted = bounded.Submit(account, [] {});

  openpit::model::Order order = TestOrder(kAccountA);
  try {
    (void)bounded.ApplyDropCopy(order, std::chrono::milliseconds(50))
        .Await(kAwaitCap);
    FAIL() << "expected SubmitCancelled";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::SubmitCancelled);
  }

  ASSERT_TRUE(order.operation.has_value());
  ASSERT_TRUE(order.operation->accountId.has_value());
  EXPECT_EQ(*order.operation->accountId, account);

  release.Open();
  ASSERT_TRUE(running.Await(kAwaitCap).has_value());
  ASSERT_TRUE(accepted.Await(kAwaitCap).has_value());
  ASSERT_TRUE(bounded.StopGraceful(seconds(10)));

  auto retry = ae::MakeTypedAsyncEngine(engine, 1);
  ae::DropCopyOutcome<ae::EngineAdapter> outcome =
      retry.ApplyDropCopy(order).Await(kAwaitCap).value();
  ASSERT_TRUE(outcome.Passed());
  EXPECT_TRUE(
      outcome.operation->RollbackAndClose().Await(kAwaitCap).has_value());
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership, RvalueReportTransfersFillLock) {
  Engine engine = OrderValidationEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);
  openpit::model::ExecutionReport report = TestLockedReport(kAccountA);

  ae::Future<openpit::PostTradeResult> submitted =
      async.ApplyExecutionReport(std::move(report));

  ASSERT_TRUE(report.fill.has_value());
  ASSERT_TRUE(report.fill->lock.has_value());
  EXPECT_FALSE(*report.fill->lock);
  EXPECT_TRUE(submitted.Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership, NamedConstRvalueReportCopiesFillLock) {
  Engine engine = OrderValidationEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);
  const openpit::model::ExecutionReport report = TestLockedReport(kAccountA);

  ae::Future<openpit::PostTradeResult> submitted = async.ApplyExecutionReport(
      static_cast<const openpit::model::ExecutionReport&&>(report));

  ASSERT_TRUE(report.fill.has_value());
  ASSERT_TRUE(report.fill->lock.has_value());
  ASSERT_TRUE(*report.fill->lock);
  EXPECT_EQ(report.fill->lock->Len(), 1u);
  EXPECT_TRUE(submitted.Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership, DerivedOrderReachesSafeSlowPolicy) {
  std::atomic<bool> orderSeen{false};
  std::atomic<bool> reportSeen{false};
  EngineBuilder builder(SyncPolicy::Full);
  openpit::pretrade::CustomPolicy<TypedAsyncPayloadAdapter> policy(
      "TypedAsyncPayloadPolicy",
      TypedAsyncPayloadAdapter{
          TypedAsyncPayloadPolicy(&orderSeen, &reportSeen)});
  builder.Add(policy);
  Engine engine = builder.Build();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  auto concrete = std::make_unique<TypedAsyncDeskOrder>();
  concrete->operation = TestOrder(kAccountA).operation;
  std::unique_ptr<const openpit::Order> order = std::move(concrete);
  ae::StartOutcome<ae::EngineAdapter> start =
      async.StartPreTrade(std::move(order)).Await(kAwaitCap).value();
  ASSERT_TRUE(start.Passed());

  auto executed = start.request->Execute().Await(kAwaitCap).value();
  ASSERT_TRUE(executed.first);
  EXPECT_TRUE(orderSeen.load(std::memory_order_relaxed));
  EXPECT_TRUE(executed.first->CommitAndClose().Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership, DerivedReportReachesSafeSlowPolicy) {
  std::atomic<bool> orderSeen{false};
  std::atomic<bool> reportSeen{false};
  EngineBuilder builder(SyncPolicy::Full);
  openpit::pretrade::CustomPolicy<TypedAsyncPayloadAdapter> policy(
      "TypedAsyncPayloadPolicy",
      TypedAsyncPayloadAdapter{
          TypedAsyncPayloadPolicy(&orderSeen, &reportSeen)});
  builder.Add(policy);
  Engine engine = builder.Build();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  auto concrete = std::make_unique<TypedAsyncDeskReport>();
  concrete->operation = TestReport(kAccountA).operation;
  std::unique_ptr<const openpit::ExecutionReport> report = std::move(concrete);
  EXPECT_TRUE(async.ApplyExecutionReport(std::move(report))
                  .Await(kAwaitCap)
                  .has_value());
  EXPECT_TRUE(reportSeen.load(std::memory_order_relaxed));
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership,
     IntermediateBaseTypedLvalueReportThrowsBeforeOwnership) {
  Engine engine = OrderValidationEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);
  openpit::model::ExecutionReport payload = TestLockedReport(kAccountA);
  auto concrete = std::make_unique<TypedAsyncDeskReport>();
  concrete->operation = std::move(payload.operation);
  concrete->fill = std::move(payload.fill);
  openpit::model::ExecutionReport& report = *concrete;

  EXPECT_THROW(static_cast<void>(async.ApplyExecutionReport(report)),
               openpit::Error);

  ASSERT_TRUE(concrete->operation.has_value());
  ASSERT_TRUE(concrete->operation->accountId.has_value());
  EXPECT_EQ(*concrete->operation->accountId, AccountId::FromUint64(kAccountA));
  ASSERT_TRUE(concrete->fill.has_value());
  ASSERT_TRUE(concrete->fill->lock.has_value());
  ASSERT_TRUE(*concrete->fill->lock);
  EXPECT_EQ(concrete->fill->lock->Len(), 1u);

  std::unique_ptr<const openpit::ExecutionReport> submitted =
      std::move(concrete);
  EXPECT_TRUE(async.ApplyExecutionReport(std::move(submitted))
                  .Await(kAwaitCap)
                  .has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership,
     TypeErasedReportRetriesFromIndependentPayloadAfterStopped) {
  Engine engine = OrderValidationEngine();
  auto stopped = ae::MakeTypedAsyncEngine(engine, 1);
  ASSERT_TRUE(stopped.StopGraceful(seconds(10)));

  openpit::model::ExecutionReport submittedPayload =
      TestLockedReport(kAccountA);
  auto submitted = std::make_unique<TypedAsyncDeskReport>();
  submitted->operation = std::move(submittedPayload.operation);
  submitted->fill = std::move(submittedPayload.fill);
  std::unique_ptr<const openpit::ExecutionReport> erased = std::move(submitted);

  openpit::model::ExecutionReport retainedPayload = TestLockedReport(kAccountA);
  auto retained = std::make_unique<TypedAsyncDeskReport>();
  retained->operation = std::move(retainedPayload.operation);
  retained->fill = std::move(retainedPayload.fill);

  try {
    (void)stopped.ApplyExecutionReport(std::move(erased)).Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }

  EXPECT_FALSE(erased);
  ASSERT_TRUE(retained->operation.has_value());
  ASSERT_TRUE(retained->operation->accountId.has_value());
  EXPECT_EQ(*retained->operation->accountId, AccountId::FromUint64(kAccountA));
  ASSERT_TRUE(retained->fill.has_value());
  ASSERT_TRUE(retained->fill->lock.has_value());
  ASSERT_TRUE(*retained->fill->lock);
  EXPECT_EQ(retained->fill->lock->Len(), 1u);

  auto retry = ae::MakeTypedAsyncEngine(engine, 1);
  std::unique_ptr<const openpit::ExecutionReport> retryReport =
      std::move(retained);
  EXPECT_TRUE(retry.ApplyExecutionReport(std::move(retryReport))
                  .Await(kAwaitCap)
                  .has_value());
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership,
     IntermediateBaseTypedReferencesToDerivedPayloadsThrow) {
  Engine engine = OrderValidationEngine();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);
  TypedAsyncDeskOrder derivedOrder;
  const openpit::model::Order& order = derivedOrder;
  TypedAsyncDeskReport derivedReport;
  openpit::model::ExecutionReport& report = derivedReport;

  EXPECT_THROW(static_cast<void>(async.StartPreTrade(order)), openpit::Error);
  EXPECT_THROW(static_cast<void>(async.ApplyExecutionReport(std::move(report))),
               openpit::Error);
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership,
     OwnedLvalueAdjustmentBatchRetriesAfterStoppedSubmit) {
  Engine engine = OrderValidationEngine();
  auto stopped = ae::MakeTypedAsyncEngine(engine, 1);
  ASSERT_TRUE(stopped.StopGraceful(seconds(10)));

  openpit::accountadjustment::AccountAdjustment adjustment;
  openpit::accountadjustment::BalanceOperation operation;
  operation.asset = openpit::param::Asset("USD");
  adjustment.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(operation));
  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::Absolute(
      openpit::param::PositionSize::FromString("100"));
  adjustment.amount = amount;
  std::vector<openpit::accountadjustment::AccountAdjustment> adjustments;
  adjustments.push_back(std::move(adjustment));

  try {
    (void)stopped
        .ApplyAccountAdjustment(AccountId::FromUint64(kAccountA), adjustments)
        .Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }

  ASSERT_EQ(adjustments.size(), 1u);
  ASSERT_TRUE(adjustments.front().operation.has_value());

  auto retry = ae::MakeTypedAsyncEngine(engine, 1);
  const ae::AdjustmentOutcome outcome =
      retry
          .ApplyAccountAdjustment(AccountId::FromUint64(kAccountA), adjustments)
          .Await(kAwaitCap)
          .value();
  EXPECT_TRUE(outcome.Passed());
  EXPECT_EQ(adjustments.size(), 1u);
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
}

TEST(TypedAsyncPayloadOwnership,
     OwnedRvalueAdjustmentBatchRetriesFromIndependentCopyAfterStoppedSubmit) {
  Engine engine = OrderValidationEngine();
  auto stopped = ae::MakeTypedAsyncEngine(engine, 1);
  ASSERT_TRUE(stopped.StopGraceful(seconds(10)));

  openpit::accountadjustment::AccountAdjustment adjustment;
  openpit::accountadjustment::BalanceOperation operation;
  operation.asset = openpit::param::Asset("USD");
  adjustment.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(operation));
  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::Absolute(
      openpit::param::PositionSize::FromString("100"));
  adjustment.amount = amount;
  std::vector<openpit::accountadjustment::AccountAdjustment> submittedBatch;
  submittedBatch.push_back(std::move(adjustment));
  const std::vector<openpit::accountadjustment::AccountAdjustment> retryBatch =
      submittedBatch;

  try {
    (void)stopped
        .ApplyAccountAdjustment(AccountId::FromUint64(kAccountA),
                                std::move(submittedBatch))
        .Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }

  auto retry = ae::MakeTypedAsyncEngine(engine, 1);
  const ae::AdjustmentOutcome outcome =
      retry.ApplyAccountAdjustment(AccountId::FromUint64(kAccountA), retryBatch)
          .Await(kAwaitCap)
          .value();
  EXPECT_TRUE(outcome.Passed());
  EXPECT_EQ(retryBatch.size(), 1u);
  EXPECT_TRUE(retry.StopGraceful(seconds(10)));
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
  accounts.reserve(kAccounts);
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
    EXPECT_LE(probe.PeakFor(AccountId::FromUint64(account)), 1)
        << "account " << account << " saw overlapping driver calls";
  }
}

// A drop-copy order with no readable account id is refused before it reaches a
// queue: the core rejects such an order with MissingRequiredField before any
// policy runs, so queueing it could only produce a guaranteed failure - and
// account 0, which is a real account, must never absorb its traffic.
void ExpectDropCopyWithoutAccountIsRefused(
    ae::TypedAsyncEngine<MockEngineAdapter>& async, MockEngineAdapter& driver) {
  Gate accountZeroGate;
  Gate accountZeroStarted;
  ae::Future<std::monostate> accountZero =
      async.Submit(AccountId::FromUint64(0), [&] {
        accountZeroStarted.Open();
        accountZeroGate.Wait();
      });
  accountZeroStarted.Wait();

  ae::Future<ae::DropCopyOutcome<MockEngineAdapter>> dropCopy =
      async.ApplyDropCopy(OrderWithoutAccount());
  try {
    (void)dropCopy.Await(std::chrono::seconds(1));
    ADD_FAILURE() << "expected a MissingAccountId error";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::MissingAccountId);
  }
  accountZeroGate.Open();

  EXPECT_EQ(driver.DropCopyCalls(), 0u);
  EXPECT_TRUE(accountZero.Await(kAwaitCap).has_value());
}

TEST(TypedAsyncThreading, ShardedRefusesDropCopyWithoutAccount) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Sharded(1).Build();

  ExpectDropCopyWithoutAccountIsRefused(async, driver);

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncThreading, DynamicRefusesDropCopyWithoutAccount) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Dynamic().Build();

  ExpectDropCopyWithoutAccountIsRefused(async, driver);

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
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

TEST(TypedAsyncShutdown, DynamicStopWaitsForRegisteredProducer) {
  MockEngineAdapter driver;
  Gate createEntered;
  Gate releaseCreate;
  BlockingCreateObserver observer(&createEntered, &releaseCreate);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .Dynamic()
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();

  std::atomic<bool> producerResolved{false};
  std::thread producer([&] {
    ae::Future<std::monostate> future =
        async.Submit(AccountId::FromUint64(kAccountA), [] {});
    try {
      (void)future.Await(kAwaitCap);
    } catch (const ae::Error&) {
    }
    producerResolved.store(true, std::memory_order_relaxed);
  });
  ASSERT_TRUE(createEntered.WaitFor(kAwaitCap));

  Gate stopReturned;
  std::thread stopper([&] {
    (void)async.StopGraceful(seconds(10));
    stopReturned.Open();
  });
  EXPECT_FALSE(stopReturned.WaitFor(std::chrono::milliseconds(100)));

  releaseCreate.Open();
  producer.join();
  stopper.join();
  EXPECT_TRUE(producerResolved.load(std::memory_order_relaxed));
}

TEST(TypedAsyncCleanup, DynamicRetriesCleanupAfterLaneRetires) {
  MockEngineAdapter driver;
  Gate firstCreated;
  Gate releaseFirstCreate;
  Gate removed;
  Gate cleanupRan;
  RetiringCreateObserver observer(&firstCreated, &releaseFirstCreate, &removed);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .Dynamic()
                   .IdleCleanupAfter(seconds(5))
                   .Build();

  std::thread cleanup([&] {
    (void)async.Generic().ScheduleMandatoryCleanup(
        AccountId::FromUint64(kAccountA), [&] { cleanupRan.Open(); });
  });
  const bool created = firstCreated.WaitFor(kAwaitCap);
  const bool retired = removed.WaitFor(kIdleRetireCap);
  releaseFirstCreate.Open();
  const bool ran = cleanupRan.WaitFor(kAwaitCap);
  cleanup.join();

  EXPECT_TRUE(created);
  EXPECT_TRUE(retired);
  EXPECT_TRUE(ran);
  EXPECT_GE(observer.Creates(), 2u);
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncCleanup, DynamicQueueLimitCleanupHoldsRoutingFence) {
  MockEngineAdapter driver;
  Gate firstCreated;
  Gate releaseFirstCreate;
  Gate removed;
  Gate cleanupStarted;
  Gate releaseCleanup;
  releaseFirstCreate.Open();
  RetiringCreateObserver observer(&firstCreated, &releaseFirstCreate, &removed);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .Dynamic()
                   .MaxQueues(1)
                   .IdleCleanupAfter(seconds(5))
                   .Build();

  ASSERT_TRUE(async.Submit(AccountId::FromUint64(kAccountA), [] {})
                  .Await(kAwaitCap)
                  .has_value());

  std::atomic<ae::ErrorCode> rejectedCode{ae::ErrorCode::TaskFailed};
  std::thread rejected([&] {
    ae::Future<std::monostate> future = async.Generic().Submit(
        AccountId::FromUint64(kAccountA + 1), [] {},
        [&](ae::Promise<std::monostate> promise, ae::Error error,
            bool inAccountLane) mutable {
          EXPECT_FALSE(inAccountLane);
          (void)async.Generic().ScheduleMandatoryCleanup(
              AccountId::FromUint64(kAccountA + 1),
              [promise, error = std::move(error), &cleanupStarted,
               &releaseCleanup]() mutable {
                cleanupStarted.Open();
                releaseCleanup.Wait();
                promise.Fail(std::move(error));
              });
        });
    try {
      (void)future.Await(kAwaitCap);
    } catch (const ae::Error& err) {
      rejectedCode.store(err.Code(), std::memory_order_relaxed);
    }
  });

  const bool startedCleanup = cleanupStarted.WaitFor(kAwaitCap);
  Gate unrelatedRan;
  std::atomic<bool> unrelatedResolved{false};
  std::thread unrelated([&] {
    ae::Future<std::monostate> future = async.Submit(
        AccountId::FromUint64(kAccountA), [&] { unrelatedRan.Open(); });
    try {
      unrelatedResolved.store(future.Await(kAwaitCap).has_value(),
                              std::memory_order_relaxed);
    } catch (const ae::Error&) {
    }
  });
  const bool unrelatedProceeded = unrelatedRan.WaitFor(kAwaitCap);
  const bool removedDuringCleanup = removed.WaitFor(kIdleRetireCap);
  Gate sameAccountRan;
  std::atomic<bool> sameAccountResolved{false};
  std::thread sameAccount([&] {
    ae::Future<std::monostate> future = async.Submit(
        AccountId::FromUint64(kAccountA + 1), [&] { sameAccountRan.Open(); });
    try {
      sameAccountResolved.store(future.Await(kAwaitCap).has_value(),
                                std::memory_order_relaxed);
    } catch (const ae::Error&) {
    }
  });
  const bool sameAccountProceededDuringCleanup =
      sameAccountRan.WaitFor(std::chrono::milliseconds(100));
  releaseCleanup.Open();
  rejected.join();
  unrelated.join();
  sameAccount.join();
  EXPECT_TRUE(startedCleanup);
  EXPECT_TRUE(unrelatedProceeded);
  EXPECT_TRUE(unrelatedResolved.load(std::memory_order_relaxed));
  EXPECT_TRUE(removedDuringCleanup);
  EXPECT_FALSE(sameAccountProceededDuringCleanup);
  EXPECT_TRUE(sameAccountResolved.load(std::memory_order_relaxed));
  EXPECT_EQ(rejectedCode.load(std::memory_order_relaxed),
            ae::ErrorCode::QueueLimit);
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncCleanup, GenericThrowingCleanupDoesNotStopEngine) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .Dynamic()
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();
  ASSERT_TRUE(async.Submit(AccountId::FromUint64(kAccountA), [] {})
                  .Await(kAwaitCap)
                  .has_value());

  Gate cleanupEntered;
  ae::Future<std::monostate> cleanup = async.Generic().ScheduleMandatoryCleanup(
      AccountId::FromUint64(kAccountA), [&] {
        cleanupEntered.Open();
        throw std::runtime_error("generic mandatory cleanup failed");
      });
  ASSERT_TRUE(cleanupEntered.WaitFor(kAwaitCap));
  try {
    (void)cleanup.Await(kAwaitCap);
    FAIL() << "expected cleanup failure";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::TaskFailed);
    EXPECT_EQ(err.Message(), "generic mandatory cleanup failed");
  }

  try {
    EXPECT_TRUE(async.Submit(AccountId::FromUint64(kAccountA), [] {})
                    .Await(kAwaitCap)
                    .has_value());
  } catch (const ae::Error& err) {
    FAIL() << "cleanup stopped the engine: " << err.Message();
  }
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncCleanup, ThrowingMandatoryCleanupDoesNotStopEngine) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .Dynamic()
                   .MaxQueues(1)
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();
  ASSERT_TRUE(async.Submit(AccountId::FromUint64(kAccountA), [] {})
                  .Await(kAwaitCap)
                  .has_value());

  ae::Future<std::monostate> failed;
  try {
    failed = async.Generic().Submit(
        AccountId::FromUint64(kAccountA + 1), [] {},
        [](ae::Promise<std::monostate>, ae::Error, bool) { throw 7; });
  } catch (...) {
    FAIL() << "mandatory cleanup escaped the task boundary";
  }
  try {
    (void)failed.Await(kAwaitCap);
    FAIL() << "expected TaskFailed";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::TaskFailed);
    EXPECT_EQ(err.Message(),
              "mandatory cleanup threw a non-standard exception");
  }

  EXPECT_TRUE(async.Submit(AccountId::FromUint64(kAccountA), [] {})
                  .Await(kAwaitCap)
                  .has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncCleanup, WorkerAbortCleanupFailureResolvesFuture) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Sharded(1).Build();
  Gate running;
  Gate release;
  ae::Future<std::monostate> blocker =
      async.Submit(AccountId::FromUint64(kAccountA), [&] {
        running.Open();
        release.Wait();
      });
  ASSERT_TRUE(running.WaitFor(kAwaitCap));

  ae::Future<std::monostate> failed = async.Generic().Submit(
      AccountId::FromUint64(kAccountA), [] {},
      [](ae::Promise<std::monostate>, ae::Error, bool) {
        throw std::runtime_error("mandatory cleanup failed");
      });
  EXPECT_FALSE(async.StopHard(std::chrono::milliseconds(1)));
  release.Open();
  EXPECT_TRUE(async.StopHard(seconds(10)));
  ASSERT_TRUE(blocker.Await(kAwaitCap).has_value());
  try {
    (void)failed.Await(kAwaitCap);
    FAIL() << "expected TaskFailed";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::TaskFailed);
    EXPECT_EQ(err.Message(), "mandatory cleanup failed");
  }
}

void ExpectPartialStopCleanupReturnsBeforeWorkerExit(
    ae::TypedAsyncEngine<ae::EngineAdapter>& async,
    std::atomic<std::size_t>& rollbacks, Gate& producerBlocked,
    std::atomic<bool>& producerRan,
    std::atomic<bool>& cleanupOvertookProducer) {
  ae::ExecuteOutcome<ae::EngineAdapter> executed =
      async.ExecutePreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(executed.Passed());

  Gate workerEntered;
  Gate releaseWorker;
  ae::Future<std::monostate> running =
      async.Submit(AccountId::FromUint64(kAccountA), [&] {
        workerEntered.Open();
        releaseWorker.Wait();
      });
  const bool workerStarted = workerEntered.WaitFor(kAwaitCap);
  ae::Future<std::monostate> accepted =
      async.Submit(AccountId::FromUint64(kAccountA), [] {});

  ae::Future<std::monostate> blocked;
  std::thread producer([&] {
    blocked = async.Submit(AccountId::FromUint64(kAccountA), [&] {
      producerRan.store(true, std::memory_order_release);
    });
  });
  const bool producerWasBlocked = producerBlocked.WaitFor(kAwaitCap);
  const bool partialStop = !async.StopGraceful(std::chrono::milliseconds(1));

  Gate closeReturned;
  ae::Future<std::monostate> close;
  std::thread closer([&] {
    close = executed.reservation->Close();
    closeReturned.Open();
  });
  const bool returnedPromptly =
      closeReturned.WaitFor(std::chrono::milliseconds(500));

  releaseWorker.Open();
  if (!returnedPromptly) {
    // Recovery for the pre-fix implementation: otherwise its synchronous
    // worker-exit wait would leave this regression test wedged forever.
    (void)async.StopGraceful(seconds(10));
  }
  closer.join();
  producer.join();

  EXPECT_TRUE(workerStarted);
  EXPECT_TRUE(producerWasBlocked);
  EXPECT_TRUE(partialStop);
  EXPECT_TRUE(returnedPromptly);
  EXPECT_TRUE(running.Await(kAwaitCap).has_value());
  EXPECT_TRUE(accepted.Await(kAwaitCap).has_value());
  EXPECT_TRUE(blocked.Await(kAwaitCap).has_value());
  try {
    (void)close.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
  EXPECT_EQ(rollbacks.load(std::memory_order_relaxed), 1u);
  EXPECT_TRUE(producerRan.load(std::memory_order_acquire));
  EXPECT_FALSE(cleanupOvertookProducer.load(std::memory_order_acquire));

  if (!returnedPromptly) {
    return;
  }

  Gate cleanupEntered;
  Gate releaseCleanup;
  ae::Future<std::monostate> cleanup = async.Generic().ScheduleMandatoryCleanup(
      AccountId::FromUint64(kAccountA), [&] {
        cleanupEntered.Open();
        releaseCleanup.Wait();
      });
  const bool cleanupStarted = cleanupEntered.WaitFor(kAwaitCap);
  Gate stopReturned;
  std::atomic<bool> stopSucceeded{false};
  std::thread stopper([&] {
    stopSucceeded.store(async.StopGraceful(seconds(10)),
                        std::memory_order_relaxed);
    stopReturned.Open();
  });
  const bool stopWaitedForCleanup =
      !stopReturned.WaitFor(std::chrono::milliseconds(100));
  releaseCleanup.Open();
  stopper.join();

  EXPECT_TRUE(cleanupStarted);
  EXPECT_TRUE(stopWaitedForCleanup);
  EXPECT_TRUE(stopSucceeded.load(std::memory_order_relaxed));
  EXPECT_TRUE(cleanup.Await(kAwaitCap).has_value());
}

TEST(TypedAsyncCleanup, ShardedPartialStopCleanupReturnsBeforeWorkerExit) {
  std::atomic<std::size_t> rollbacks{0};
  std::atomic<bool> producerRan{false};
  std::atomic<bool> cleanupOvertookProducer{false};
  EngineBuilder builder(SyncPolicy::Account);
  openpit::pretrade::CustomPolicy<CountingRollbackPolicy> policy(
      "CountingRollbackPolicy",
      CountingRollbackPolicy(&rollbacks, &producerRan,
                             &cleanupOvertookProducer));
  builder.Add(policy);
  Engine engine = builder.Build();
  ae::EngineAdapter driver(engine);
  Gate producerBlocked;
  QueueFullGateObserver observer(&producerBlocked);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver)
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Sharded(1)
                   .Build();

  ExpectPartialStopCleanupReturnsBeforeWorkerExit(
      async, rollbacks, producerBlocked, producerRan, cleanupOvertookProducer);
}

TEST(TypedAsyncCleanup, DynamicPartialStopCleanupReturnsBeforeWorkerExit) {
  std::atomic<std::size_t> rollbacks{0};
  std::atomic<bool> producerRan{false};
  std::atomic<bool> cleanupOvertookProducer{false};
  EngineBuilder builder(SyncPolicy::Account);
  openpit::pretrade::CustomPolicy<CountingRollbackPolicy> policy(
      "CountingRollbackPolicy",
      CountingRollbackPolicy(&rollbacks, &producerRan,
                             &cleanupOvertookProducer));
  builder.Add(policy);
  Engine engine = builder.Build();
  ae::EngineAdapter driver(engine);
  Gate producerBlocked;
  QueueFullGateObserver observer(&producerBlocked);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver)
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Dynamic()
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();

  ExpectPartialStopCleanupReturnsBeforeWorkerExit(
      async, rollbacks, producerBlocked, producerRan, cleanupOvertookProducer);
}

void ExpectStoppedLaneCleanupDrainsMultipleBlockedProducers(
    ae::TypedAsyncEngine<MockEngineAdapter>& async, Gate& producersBlocked) {
  const AccountId account = AccountId::FromUint64(kAccountA);
  Gate workerEntered;
  Gate releaseWorker;
  ae::Future<std::monostate> running = async.Submit(account, [&] {
    workerEntered.Open();
    releaseWorker.Wait();
  });
  ASSERT_TRUE(workerEntered.WaitFor(kAwaitCap));
  ae::Future<std::monostate> accepted = async.Submit(account, [] {});

  std::atomic<std::size_t> producersRan{0};
  ae::Future<std::monostate> first;
  ae::Future<std::monostate> second;
  std::thread firstProducer([&] {
    first = async.Submit(
        account, [&] { producersRan.fetch_add(1, std::memory_order_release); },
        seconds(2));
  });
  std::thread secondProducer([&] {
    second = async.Submit(
        account, [&] { producersRan.fetch_add(1, std::memory_order_release); },
        seconds(2));
  });
  const bool bothProducersBlocked = producersBlocked.WaitFor(kAwaitCap);
  const bool partialStop = !async.StopGraceful(std::chrono::milliseconds(1));

  std::atomic<bool> cleanupOvertookProducer{false};
  Gate cleanupRan;
  ae::Future<std::monostate> cleanup =
      async.Generic().ScheduleMandatoryCleanup(account, [&] {
        cleanupOvertookProducer.store(
            producersRan.load(std::memory_order_acquire) != 2,
            std::memory_order_release);
        cleanupRan.Open();
      });

  Gate retryReturned;
  std::atomic<bool> retrySucceeded{false};
  std::thread retryStop([&] {
    retrySucceeded.store(async.StopGraceful(seconds(4)),
                         std::memory_order_relaxed);
    retryReturned.Open();
  });
  releaseWorker.Open();
  const bool retryReturnedWhileProducersCouldStillSubmit =
      retryReturned.WaitFor(seconds(1));

  firstProducer.join();
  secondProducer.join();
  retryStop.join();

  EXPECT_TRUE(bothProducersBlocked);
  EXPECT_TRUE(partialStop);
  EXPECT_TRUE(retryReturnedWhileProducersCouldStillSubmit);
  EXPECT_TRUE(retrySucceeded.load(std::memory_order_relaxed));
  EXPECT_TRUE(running.Await(kAwaitCap).has_value());
  EXPECT_TRUE(accepted.Await(kAwaitCap).has_value());
  EXPECT_TRUE(first.Await(kAwaitCap).has_value());
  EXPECT_TRUE(second.Await(kAwaitCap).has_value());
  EXPECT_TRUE(cleanupRan.WaitFor(kAwaitCap));
  EXPECT_TRUE(cleanup.Await(kAwaitCap).has_value());
  EXPECT_FALSE(cleanupOvertookProducer.load(std::memory_order_acquire));
}

TEST(TypedAsyncCleanup,
     ShardedStoppedLaneCleanupDrainsMultipleBlockedProducers) {
  MockEngineAdapter driver;
  Gate producersBlocked;
  QueueFullProducerCountObserver observer(&producersBlocked, 2);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Sharded(1)
                   .Build();

  ExpectStoppedLaneCleanupDrainsMultipleBlockedProducers(async,
                                                         producersBlocked);
}

TEST(TypedAsyncCleanup,
     DynamicStoppedLaneCleanupDrainsMultipleBlockedProducers) {
  MockEngineAdapter driver;
  Gate producersBlocked;
  QueueFullProducerCountObserver observer(&producersBlocked, 2);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Dynamic()
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();

  ExpectStoppedLaneCleanupDrainsMultipleBlockedProducers(async,
                                                         producersBlocked);
}

void ExpectDeferredCleanupCannotLoseProducerWake(bool dynamic, bool hardStop,
                                                 bool publicationRace = false) {
  Gate workerAtWaitGap;
  Gate allowProducerFinish;
  Gate producerReturned;
  Gate deferredPublished;
  Gate allowDeferredRegistration;
  Gate cleanupScheduled;
  std::atomic<bool> armWaitGap{false};
  std::atomic<bool> armPublicationGap{false};
  std::atomic<bool> producerReturnedBeforeWait{false};
  std::atomic<bool> cleanupScheduledBeforeWait{false};

  ae::detail::BaseConfig config;
  config.queueCapacity = 1;
  // Without the publication race the producer is expected NOT to return while
  // the worker holds the lane, so a short probe is the whole point; with it the
  // producer is expected to return, so that direction gets the full cap.
  const std::chrono::milliseconds producerProbe =
      publicationRace ? std::chrono::milliseconds(kAwaitCap) : kNegativeProbe;

  ae::detail::WorkerSeams seams;
  seams.beforeWorkerWait = [&] {
    if (!armWaitGap.exchange(false, std::memory_order_acq_rel)) {
      return;
    }
    workerAtWaitGap.Open();
    allowProducerFinish.Open();
    producerReturnedBeforeWait.store(producerReturned.WaitFor(producerProbe),
                                     std::memory_order_release);
    if (publicationRace) {
      allowDeferredRegistration.Open();
      cleanupScheduledBeforeWait.store(cleanupScheduled.WaitFor(kNegativeProbe),
                                       std::memory_order_release);
    }
  };
  seams.afterDeferredPublication = [&] {
    if (!armPublicationGap.exchange(false, std::memory_order_acq_rel)) {
      return;
    }
    deferredPublished.Open();
    allowDeferredRegistration.Wait();
  };

  std::unique_ptr<ae::detail::Strategy> strategy;
  if (dynamic) {
    strategy = std::make_unique<ae::detail::DynamicStrategy>(
        config, 0, std::chrono::nanoseconds(0), false, std::move(seams));
  } else {
    strategy = std::make_unique<ae::detail::ShardedStrategy>(config, 1,
                                                             std::move(seams));
  }

  const OpenPitParamAccountId account = kAccountA;
  const auto noDeadline = std::chrono::steady_clock::time_point::max();
  Gate workerEntered;
  Gate releaseWorker;
  std::atomic<bool> setupFailed{false};
  strategy->Submit(account,
                   ae::detail::MakeTask(
                       [&] {
                         workerEntered.Open();
                         releaseWorker.Wait();
                       },
                       [](ae::Error) {}),
                   noDeadline, [&](ae::Error) {
                     setupFailed.store(true, std::memory_order_relaxed);
                   });
  ASSERT_TRUE(workerEntered.WaitFor(kAwaitCap));
  strategy->Submit(
      account, ae::detail::MakeTask([] {}, [](ae::Error) {}), noDeadline,
      [&](ae::Error) { setupFailed.store(true, std::memory_order_relaxed); });

  Gate failureHandlerEntered;
  ae::ErrorCode producerError = ae::ErrorCode::Stopped;
  std::thread producer([&] {
    strategy->Submit(
        account, ae::detail::MakeTask([] {}, [](ae::Error) {}),
        std::chrono::steady_clock::now() + std::chrono::milliseconds(50),
        [&](ae::Error error) {
          producerError = error.Code();
          failureHandlerEntered.Open();
          allowProducerFinish.Wait();
        });
    producerReturned.Open();
  });
  ASSERT_TRUE(failureHandlerEntered.WaitFor(kAwaitCap));

  const auto initialDeadline =
      std::chrono::steady_clock::now() + std::chrono::milliseconds(1);
  const bool partialStop = hardStop ? !strategy->StopHard(initialDeadline)
                                    : !strategy->StopGraceful(initialDeadline);

  Gate cleanupRan;
  std::optional<ae::Future<std::monostate>> cleanup;
  std::thread cleanupScheduler;
  if (publicationRace) {
    armPublicationGap.store(true, std::memory_order_release);
    cleanupScheduler = std::thread([&] {
      cleanup.emplace(strategy->ScheduleMandatoryCleanup(
          account, [&] { cleanupRan.Open(); }));
      cleanupScheduled.Open();
    });
    ASSERT_TRUE(deferredPublished.WaitFor(kAwaitCap));
  } else {
    cleanup.emplace(strategy->ScheduleMandatoryCleanup(
        account, [&] { cleanupRan.Open(); }));
  }
  armWaitGap.store(true, std::memory_order_release);

  std::atomic<bool> retrySucceeded{false};
  std::thread retryStop([&] {
    const auto deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(1);
    retrySucceeded.store(hardStop ? strategy->StopHard(deadline)
                                  : strategy->StopGraceful(deadline),
                         std::memory_order_relaxed);
  });
  releaseWorker.Open();
  retryStop.join();
  producer.join();
  if (cleanupScheduler.joinable()) {
    cleanupScheduler.join();
  }

  bool recoverySucceeded = true;
  std::optional<ae::Future<std::monostate>> recovery;
  if (!retrySucceeded.load(std::memory_order_relaxed)) {
    recovery.emplace(strategy->ScheduleMandatoryCleanup(account, [] {}));
    const auto deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(5);
    recoverySucceeded = hardStop ? strategy->StopHard(deadline)
                                 : strategy->StopGraceful(deadline);
  }

  EXPECT_FALSE(setupFailed.load(std::memory_order_relaxed));
  EXPECT_EQ(producerError, ae::ErrorCode::SubmitCancelled);
  EXPECT_TRUE(partialStop);
  EXPECT_TRUE(workerAtWaitGap.WaitFor(kAwaitCap));
  if (publicationRace) {
    EXPECT_TRUE(producerReturnedBeforeWait.load(std::memory_order_acquire));
    EXPECT_FALSE(cleanupScheduledBeforeWait.load(std::memory_order_acquire));
  } else {
    EXPECT_FALSE(producerReturnedBeforeWait.load(std::memory_order_acquire));
  }
  EXPECT_TRUE(retrySucceeded.load(std::memory_order_relaxed));
  EXPECT_TRUE(recoverySucceeded);
  EXPECT_TRUE(cleanupRan.WaitFor(kAwaitCap));
  ASSERT_TRUE(cleanup.has_value());
  EXPECT_TRUE(cleanup->Await(kAwaitCap).has_value());
  if (recovery.has_value()) {
    EXPECT_TRUE(recovery->Await(kAwaitCap).has_value());
  }
}

TEST(TypedAsyncCleanup, ShardedGracefulRetryDoesNotLoseProducerWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(false, false);
}

TEST(TypedAsyncCleanup, ShardedHardRetryDoesNotLoseProducerWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(false, true);
}

TEST(TypedAsyncCleanup, DynamicGracefulRetryDoesNotLoseProducerWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(true, false);
}

TEST(TypedAsyncCleanup, DynamicHardRetryDoesNotLoseProducerWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(true, true);
}

TEST(TypedAsyncCleanup, ShardedGracefulDeferredPublicationDoesNotLoseWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(false, false, true);
}

TEST(TypedAsyncCleanup, ShardedHardDeferredPublicationDoesNotLoseWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(false, true, true);
}

TEST(TypedAsyncCleanup, DynamicGracefulDeferredPublicationDoesNotLoseWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(true, false, true);
}

TEST(TypedAsyncCleanup, DynamicHardDeferredPublicationDoesNotLoseWake) {
  ExpectDeferredCleanupCannotLoseProducerWake(true, true, true);
}

TEST(TypedAsyncCleanup, DynamicRetryStopWaitsForNoLaneCleanup) {
  MockEngineAdapter driver;
  Gate producerBlocked;
  Gate accountRemoved;
  QueueFullGateObserver observer(&producerBlocked, &accountRemoved);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Dynamic()
                   .IdleCleanupAfter(seconds(5))
                   .Build();

  ASSERT_TRUE(async.Submit(AccountId::FromUint64(kAccountA), [] {})
                  .Await(kAwaitCap)
                  .has_value());
  ASSERT_TRUE(accountRemoved.WaitFor(kIdleRetireCap));

  Gate workerEntered;
  Gate releaseWorker;
  ae::Future<std::monostate> running =
      async.Submit(AccountId::FromUint64(kAccountA + 1), [&] {
        workerEntered.Open();
        releaseWorker.Wait();
      });
  ASSERT_TRUE(workerEntered.WaitFor(kAwaitCap));
  ae::Future<std::monostate> accepted =
      async.Submit(AccountId::FromUint64(kAccountA + 1), [] {});
  ae::Future<std::monostate> blocked;
  std::thread producer([&] {
    blocked = async.Submit(AccountId::FromUint64(kAccountA + 1), [] {});
  });
  ASSERT_TRUE(producerBlocked.WaitFor(kAwaitCap));
  EXPECT_FALSE(async.StopGraceful(std::chrono::milliseconds(1)));

  Gate cleanupEntered;
  Gate releaseCleanup;
  Gate cleanupScheduled;
  ae::Future<std::monostate> cleanup;
  std::thread cleanupCaller([&] {
    cleanup = async.Generic().ScheduleMandatoryCleanup(
        AccountId::FromUint64(kAccountA), [&] {
          cleanupEntered.Open();
          releaseCleanup.Wait();
        });
    cleanupScheduled.Open();
  });
  ASSERT_TRUE(cleanupEntered.WaitFor(kAwaitCap));

  releaseWorker.Open();
  producer.join();
  EXPECT_TRUE(running.Await(kAwaitCap).has_value());
  EXPECT_TRUE(accepted.Await(kAwaitCap).has_value());
  EXPECT_TRUE(blocked.Await(kAwaitCap).has_value());

  Gate retryReturned;
  std::atomic<bool> retrySucceeded{false};
  std::thread retryStop([&] {
    retrySucceeded.store(async.StopGraceful(seconds(10)),
                         std::memory_order_relaxed);
    retryReturned.Open();
  });
  const bool retryWaitedForCleanup =
      !retryReturned.WaitFor(std::chrono::milliseconds(100));
  releaseCleanup.Open();
  cleanupCaller.join();
  retryStop.join();

  EXPECT_TRUE(cleanupScheduled.WaitFor(kAwaitCap));
  EXPECT_TRUE(retryWaitedForCleanup);
  EXPECT_TRUE(retrySucceeded.load(std::memory_order_relaxed));
  EXPECT_TRUE(cleanup.Await(kAwaitCap).has_value());
}

void ExpectStopWaitsForSynchronousAbortHandoff(bool hardStop) {
  MockEngineAdapter driver;
  Gate producerBlocked;
  QueueFullGateObserver observer(&producerBlocked);
  Gate underlyingReleased;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithStopUnderlying([&] { underlyingReleased.Open(); })
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Sharded(1)
                   .Build();

  Gate workerEntered;
  Gate releaseWorker;
  ae::Future<std::monostate> running =
      async.Submit(AccountId::FromUint64(kAccountA), [&] {
        workerEntered.Open();
        releaseWorker.Wait();
      });
  ASSERT_TRUE(workerEntered.WaitFor(kAwaitCap));
  ae::Future<std::monostate> accepted =
      async.Submit(AccountId::FromUint64(kAccountA), [] {});

  Gate abortHandlerEntered;
  Gate allowCleanupHandoff;
  Gate cleanupEntered;
  Gate releaseCleanup;
  Gate submitReturned;
  // A hard stop releases the producer blocked on the full lane, so that variant
  // needs no deadline of its own and fails with Stopped; a graceful stop leaves
  // the producer alone, so there its own deadline is what fails the submit.
  const std::chrono::nanoseconds submitTimeout =
      hardStop ? std::chrono::nanoseconds(seconds(30))
               : std::chrono::nanoseconds(std::chrono::milliseconds(50));
  const ae::ErrorCode expectedFailure =
      hardStop ? ae::ErrorCode::Stopped : ae::ErrorCode::SubmitCancelled;
  ae::Future<std::monostate> failed;
  std::thread submitter([&] {
    failed = async.Generic().Submit(
        AccountId::FromUint64(kAccountA), [] {},
        [&](ae::Promise<std::monostate> promise, ae::Error error,
            bool inAccountLane) mutable {
          EXPECT_FALSE(inAccountLane);
          abortHandlerEntered.Open();
          allowCleanupHandoff.Wait();
          (void)async.Generic().ScheduleMandatoryCleanup(
              AccountId::FromUint64(kAccountA),
              [promise, error = std::move(error), &cleanupEntered,
               &releaseCleanup]() mutable {
                cleanupEntered.Open();
                releaseCleanup.Wait();
                promise.Fail(std::move(error));
              });
        },
        submitTimeout);
    submitReturned.Open();
  });
  ASSERT_TRUE(producerBlocked.WaitFor(kAwaitCap));

  Gate stopReturned;
  std::atomic<bool> stopSucceeded{false};
  std::thread stopper([&] {
    const bool stopped = hardStop ? async.StopHard(seconds(10))
                                  : async.StopGraceful(seconds(10));
    stopSucceeded.store(stopped, std::memory_order_relaxed);
    stopReturned.Open();
  });
  const bool handlerStarted = abortHandlerEntered.WaitFor(kAwaitCap);
  releaseWorker.Open();
  const bool stopPassedHandoff =
      stopReturned.WaitFor(std::chrono::milliseconds(500));
  const bool underlyingPassedHandoff =
      underlyingReleased.WaitFor(std::chrono::milliseconds(1));

  allowCleanupHandoff.Open();
  const bool cleanupStarted = cleanupEntered.WaitFor(kAwaitCap);
  const bool stopPassedCleanup =
      stopReturned.WaitFor(std::chrono::milliseconds(100));
  releaseCleanup.Open();
  submitter.join();
  stopper.join();

  EXPECT_TRUE(handlerStarted);
  EXPECT_FALSE(stopPassedHandoff);
  EXPECT_FALSE(underlyingPassedHandoff);
  EXPECT_TRUE(cleanupStarted);
  EXPECT_FALSE(stopPassedCleanup);
  EXPECT_TRUE(stopSucceeded.load(std::memory_order_relaxed));
  EXPECT_TRUE(stopReturned.WaitFor(kAwaitCap));
  EXPECT_TRUE(underlyingReleased.WaitFor(kAwaitCap));
  EXPECT_TRUE(submitReturned.WaitFor(kAwaitCap));
  EXPECT_TRUE(running.Await(kAwaitCap).has_value());
  if (hardStop) {
    try {
      (void)accepted.Await(kAwaitCap);
      FAIL() << "expected Stopped";
    } catch (const ae::Error& err) {
      EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
    }
  } else {
    EXPECT_TRUE(accepted.Await(kAwaitCap).has_value());
  }
  try {
    (void)failed.Await(kAwaitCap);
    FAIL() << "expected a failed submit";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), expectedFailure);
  }
}

TEST(TypedAsyncCleanup, GracefulStopWaitsForSynchronousAbortHandoff) {
  ExpectStopWaitsForSynchronousAbortHandoff(false);
}

TEST(TypedAsyncCleanup, HardStopWaitsForSynchronousAbortHandoff) {
  ExpectStopWaitsForSynchronousAbortHandoff(true);
}

TEST(TypedAsyncCleanup, HardStopAbortReleasesReservationInAccountLane) {
  std::atomic<std::size_t> rollbacks{0};
  EngineBuilder builder(SyncPolicy::Account);
  openpit::pretrade::CustomPolicy<CountingRollbackPolicy> policy(
      "CountingRollbackPolicy", CountingRollbackPolicy(&rollbacks));
  builder.Add(policy);
  Engine engine = builder.Build();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::ExecuteOutcome<ae::EngineAdapter> executed =
      async.ExecutePreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(executed.Passed());
  Gate running;
  Gate release;
  ae::Future<std::monostate> blocker =
      async.Submit(AccountId::FromUint64(kAccountA), [&] {
        running.Open();
        release.Wait();
      });
  ASSERT_TRUE(running.WaitFor(kAwaitCap));
  ae::Future<std::monostate> close = executed.reservation->Close();

  EXPECT_FALSE(async.StopHard(std::chrono::milliseconds(1)));
  release.Open();
  EXPECT_TRUE(async.StopHard(seconds(10)));
  ASSERT_TRUE(blocker.Await(kAwaitCap).has_value());
  try {
    (void)close.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
  EXPECT_EQ(rollbacks.load(std::memory_order_relaxed), 1u);
}

TEST(TypedAsyncCleanup, SubmitFailureReleasesReservationAfterStop) {
  std::atomic<std::size_t> rollbacks{0};
  EngineBuilder builder(SyncPolicy::Account);
  openpit::pretrade::CustomPolicy<CountingRollbackPolicy> policy(
      "CountingRollbackPolicy", CountingRollbackPolicy(&rollbacks));
  builder.Add(policy);
  Engine engine = builder.Build();
  auto async = ae::MakeTypedAsyncEngine(engine, 1);

  ae::ExecuteOutcome<ae::EngineAdapter> executed =
      async.ExecutePreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  ASSERT_TRUE(executed.Passed());
  ASSERT_TRUE(async.StopGraceful(seconds(10)));

  ae::Future<std::monostate> close = executed.reservation->Close();
  try {
    (void)close.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
  EXPECT_EQ(rollbacks.load(std::memory_order_relaxed), 1u);
}

TEST(TypedAsyncCleanup, HardStopAbortReleasesRequestInAccountLane) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Sharded(1).Build();

  openpit::model::Order order = TestOrder(kAccountA);
  openpit::pretrade::StartResult started = engine.StartPreTrade(order);
  ASSERT_TRUE(started.Passed());
  auto request = std::make_shared<ae::AsyncRequest<ae::EngineAdapter>>(
      std::move(*started.request), &async.Generic(),
      AccountId::FromUint64(kAccountA));

  Gate running;
  Gate release;
  ae::Future<std::monostate> blocker =
      async.Submit(AccountId::FromUint64(kAccountA), [&] {
        running.Open();
        release.Wait();
      });
  ASSERT_TRUE(running.WaitFor(kAwaitCap));
  ae::Future<std::monostate> close = request->Close();

  EXPECT_FALSE(async.StopHard(std::chrono::milliseconds(1)));
  release.Open();
  EXPECT_TRUE(async.StopHard(seconds(10)));
  ASSERT_TRUE(blocker.Await(kAwaitCap).has_value());
  try {
    (void)close.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
}

TEST(TypedAsyncCleanup, SubmitFailureReleasesRequestAfterStop) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver)
                   .Dynamic()
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();

  openpit::model::Order order = TestOrder(kAccountA);
  openpit::pretrade::StartResult started = engine.StartPreTrade(order);
  ASSERT_TRUE(started.Passed());
  auto request = std::make_shared<ae::AsyncRequest<ae::EngineAdapter>>(
      std::move(*started.request), &async.Generic(),
      AccountId::FromUint64(kAccountA));
  ASSERT_TRUE(async.StopGraceful(seconds(10)));

  ae::Future<std::monostate> close = request->Close();
  try {
    (void)close.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
}

//------------------------------------------------------------------------------
// Bounded waits: neither a producer blocked on a full lane nor the cleanup that
// repairs a failed submit may wait on something a stop cannot deliver.

// A producer blocked on a full lane waits with no deadline of its own, so only
// the engine can release it. A hard stop no longer promises the lane will make
// room, so it must release that producer promptly and must not wait for it.
void ExpectHardStopReleasesBlockedProducer(
    ae::TypedAsyncEngine<MockEngineAdapter>& async, Gate& producerBlocked) {
  const AccountId account = AccountId::FromUint64(kAccountA);
  Gate workerEntered;
  Gate releaseWorker;
  ae::Future<std::monostate> running = async.Submit(account, [&] {
    workerEntered.Open();
    releaseWorker.Wait();
  });
  ASSERT_TRUE(workerEntered.WaitFor(kAwaitCap));
  ae::Future<std::monostate> accepted = async.Submit(account, [] {});

  Gate producerReturned;
  std::atomic<bool> blockedTaskRan{false};
  ae::Future<std::monostate> blocked;
  std::thread producer([&] {
    blocked = async.Submit(account, [&] {
      blockedTaskRan.store(true, std::memory_order_release);
    });
    producerReturned.Open();
  });
  ASSERT_TRUE(producerBlocked.WaitFor(kAwaitCap));

  Gate stopReturned;
  std::atomic<bool> stopSucceeded{false};
  std::thread stopper([&] {
    stopSucceeded.store(async.StopHard(seconds(10)), std::memory_order_relaxed);
    stopReturned.Open();
  });
  const bool producerReleased = producerReturned.WaitFor(kAwaitCap);
  // Released unconditionally: a regression must fail this test rather than
  // wedge the suite behind a producer that can no longer make progress.
  releaseWorker.Open();
  producer.join();
  stopper.join();

  EXPECT_TRUE(producerReleased);
  EXPECT_TRUE(stopSucceeded.load(std::memory_order_relaxed));
  EXPECT_FALSE(blockedTaskRan.load(std::memory_order_acquire));
  EXPECT_TRUE(running.Await(kAwaitCap).has_value());
  try {
    (void)blocked.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
  try {
    (void)accepted.Await(kAwaitCap);
    FAIL() << "expected Stopped";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
}

TEST(TypedAsyncShutdown, ShardedHardStopReleasesProducerBlockedOnFullLane) {
  MockEngineAdapter driver;
  Gate producerBlocked;
  QueueFullGateObserver observer(&producerBlocked);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Sharded(1)
                   .Build();

  ExpectHardStopReleasesBlockedProducer(async, producerBlocked);
}

TEST(TypedAsyncShutdown, DynamicHardStopReleasesProducerBlockedOnFullLane) {
  MockEngineAdapter driver;
  Gate producerBlocked;
  QueueFullGateObserver observer(&producerBlocked);
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithObserver(observer)
                   .WithQueueCapacity(1)
                   .WithSlowSubmitThreshold(std::chrono::milliseconds(1))
                   .Dynamic()
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();

  ExpectHardStopReleasesBlockedProducer(async, producerBlocked);
}

// Cleanup repairs a submit that may already have spent its deadline, so its
// enqueue must never wait for lane capacity - while still running behind the
// work the lane has already accepted, and exactly once.
void ExpectCleanupEnqueueNeverBlocksOnFullLane(
    ae::TypedAsyncEngine<MockEngineAdapter>& async) {
  const AccountId account = AccountId::FromUint64(kAccountA);
  Gate workerEntered;
  Gate releaseWorker;
  ae::Future<std::monostate> running = async.Submit(account, [&] {
    workerEntered.Open();
    releaseWorker.Wait();
  });
  ASSERT_TRUE(workerEntered.WaitFor(kAwaitCap));
  std::atomic<bool> acceptedRan{false};
  ae::Future<std::monostate> accepted = async.Submit(
      account, [&] { acceptedRan.store(true, std::memory_order_release); });

  std::atomic<std::size_t> cleanupRuns{0};
  std::atomic<bool> cleanupOvertookAcceptedWork{false};
  Gate cleanupScheduled;
  Gate cleanupRan;
  ae::Future<std::monostate> cleanup;
  std::thread scheduler([&] {
    cleanup = async.Generic().ScheduleMandatoryCleanup(account, [&] {
      cleanupOvertookAcceptedWork.store(
          !acceptedRan.load(std::memory_order_acquire),
          std::memory_order_release);
      cleanupRuns.fetch_add(1, std::memory_order_relaxed);
      cleanupRan.Open();
    });
    cleanupScheduled.Open();
  });

  const bool scheduledWhileLaneFull = cleanupScheduled.WaitFor(kAwaitCap);
  const bool orderedBehindAcceptedWork = !cleanupRan.WaitFor(kNegativeProbe);
  releaseWorker.Open();
  scheduler.join();

  EXPECT_TRUE(scheduledWhileLaneFull);
  EXPECT_TRUE(orderedBehindAcceptedWork);
  EXPECT_TRUE(cleanupRan.WaitFor(kAwaitCap));
  EXPECT_EQ(cleanupRuns.load(std::memory_order_relaxed), 1u);
  EXPECT_FALSE(cleanupOvertookAcceptedWork.load(std::memory_order_acquire));
  EXPECT_TRUE(running.Await(kAwaitCap).has_value());
  EXPECT_TRUE(accepted.Await(kAwaitCap).has_value());
  EXPECT_TRUE(cleanup.Await(kAwaitCap).has_value());
  EXPECT_TRUE(async.StopGraceful(seconds(10)));
}

TEST(TypedAsyncCleanup, ShardedCleanupEnqueueNeverBlocksOnFullLane) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithQueueCapacity(1)
                   .Sharded(1)
                   .Build();

  ExpectCleanupEnqueueNeverBlocksOnFullLane(async);
}

TEST(TypedAsyncCleanup, DynamicCleanupEnqueueNeverBlocksOnFullLane) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver)
                   .WithQueueCapacity(1)
                   .Dynamic()
                   .IdleCleanupAfter(std::chrono::nanoseconds(0))
                   .Build();

  ExpectCleanupEnqueueNeverBlocksOnFullLane(async);
}

//------------------------------------------------------------------------------
// Task-boundary exception containment, exercised on a raw closure task the
// engine layer would normally wrap.

TEST(TypedAsyncErrorModel, ThrowingRunClosureKeepsTheLaneUsable) {
  const ae::detail::BaseConfig config;
  ae::detail::ShardedStrategy strategy(config, 1);
  const OpenPitParamAccountId account = kAccountA;
  const auto noDeadline = std::chrono::steady_clock::time_point::max();
  std::atomic<bool> submitFailed{false};
  const ae::detail::SubmitFailureHandler onFailure = [&](ae::Error) {
    submitFailed.store(true, std::memory_order_relaxed);
  };

  strategy.Submit(account,
                  ae::detail::MakeTask(
                      [] { throw std::runtime_error("run closure escaped"); },
                      [](ae::Error) {}),
                  noDeadline, onFailure);
  Gate laneStillRuns;
  strategy.Submit(
      account,
      ae::detail::MakeTask([&] { laneStillRuns.Open(); }, [](ae::Error) {}),
      noDeadline, onFailure);

  EXPECT_TRUE(laneStillRuns.WaitFor(kAwaitCap));
  EXPECT_FALSE(submitFailed.load(std::memory_order_relaxed));
  EXPECT_TRUE(
      strategy.StopGraceful(std::chrono::steady_clock::now() + seconds(10)));
}

TEST(TypedAsyncErrorModel, ThrowingAbortClosureStillDrainsTheLane) {
  const ae::detail::BaseConfig config;
  ae::detail::ShardedStrategy strategy(config, 1);
  const OpenPitParamAccountId account = kAccountA;
  const auto noDeadline = std::chrono::steady_clock::time_point::max();
  std::atomic<bool> submitFailed{false};
  const ae::detail::SubmitFailureHandler onFailure = [&](ae::Error) {
    submitFailed.store(true, std::memory_order_relaxed);
  };

  Gate workerEntered;
  Gate releaseWorker;
  strategy.Submit(account,
                  ae::detail::MakeTask(
                      [&] {
                        workerEntered.Open();
                        releaseWorker.Wait();
                      },
                      [](ae::Error) {}),
                  noDeadline, onFailure);
  ASSERT_TRUE(workerEntered.WaitFor(kAwaitCap));
  strategy.Submit(
      account,
      ae::detail::MakeTask(
          [] {},
          [](ae::Error) { throw std::runtime_error("abort closure escaped"); }),
      noDeadline, onFailure);
  Gate lastTaskAborted;
  strategy.Submit(
      account,
      ae::detail::MakeTask([] {}, [&](ae::Error) { lastTaskAborted.Open(); }),
      noDeadline, onFailure);

  EXPECT_FALSE(strategy.StopHard(std::chrono::steady_clock::now() +
                                 std::chrono::milliseconds(1)));
  releaseWorker.Open();

  EXPECT_TRUE(lastTaskAborted.WaitFor(kAwaitCap));
  EXPECT_FALSE(submitFailed.load(std::memory_order_relaxed));
  EXPECT_TRUE(
      strategy.StopHard(std::chrono::steady_clock::now() + seconds(10)));
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

// UnblockAll names neither an account nor a group, so it pins to the
// engine-wide queue and still reaches the driver.
TEST(TypedAsyncAccounts, UnblockAllRoutesThroughEngineWideQueue) {
  MockEngineAdapter driver;
  auto async = ae::TypedBuilder<MockEngineAdapter>(driver).Dynamic().Build();

  ae::AsyncAccounts<MockEngineAdapter> accounts = async.Accounts();
  ASSERT_TRUE(accounts.UnblockAll().Await(kAwaitCap).has_value());

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
  EXPECT_EQ(driver.globalUnblocks.load(), 1u);
}

// Clearing the engine-wide block leaves an individually blocked account
// blocked, exactly as on the synchronous admin surface.
TEST(TypedAsyncAccounts, UnblockAllLeavesIndividuallyBlockedAccountBlocked) {
  Engine engine = OrderValidationEngine();
  ae::EngineAdapter driver(engine);
  auto async = ae::TypedBuilder<ae::EngineAdapter>(driver).Sharded(1).Build();

  ae::AsyncAccounts<ae::EngineAdapter> accounts = async.Accounts();
  ASSERT_TRUE(accounts.Block(AccountId::FromUint64(kAccountA), "by operator")
                  .Await(kAwaitCap)
                  .has_value());
  ASSERT_TRUE(accounts.UnblockAll().Await(kAwaitCap).has_value());

  const ae::StartOutcome<ae::EngineAdapter> start =
      async.StartPreTrade(TestOrder(kAccountA)).Await(kAwaitCap).value();
  EXPECT_FALSE(start.Passed());
  ASSERT_EQ(start.rejects.size(), 1u);
  EXPECT_EQ(start.rejects.front().code, RejectCode::AccountBlocked);
  EXPECT_EQ(start.rejects.front().reason, "by operator");

  EXPECT_TRUE(async.StopGraceful(seconds(10)));
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
