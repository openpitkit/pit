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

#include "openpit/async_engine.hpp"

#include "openpit/error.hpp"
#include "openpit/param/param.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include <openpit.h>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <map>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>
#include <utility>
#include <variant>
#include <vector>

namespace {

namespace ae = openpit::asyncengine;

using openpit::param::AccountId;
using openpit::param::Price;
using std::chrono::milliseconds;
using std::chrono::seconds;

constexpr std::uint64_t kAccountARaw = 1001;
constexpr std::uint64_t kAccountBRaw = 2002;
const AccountId kAccountA = AccountId::FromUint64(kAccountARaw);
const AccountId kAccountB = AccountId::FromUint64(kAccountBRaw);
constexpr std::uint64_t kAccountCRaw = 3003;
const AccountId kAccountC = AccountId::FromUint64(kAccountCRaw);

// A deterministic 5-second cap so a wedged test fails fast rather than hanging
// the suite. Every Await uses it; correct code resolves well within it.
constexpr seconds kAwaitCap{5};

//------------------------------------------------------------------------------
// Mock driver
//
// The async layer is generic over a driver (the engine-call seam), exactly like
// the Go `asyncengine.Driver` interface. Tests inject this stand-in so the
// dispatch/threading/future machinery is exercised without a live engine. Each
// method records the thread it ran on so per-account serialization is testable.

// Tracks the peak number of simultaneously-active driver calls per account, so
// any per-account overlap (a threading-contract violation) is observable.
class ConcurrencyProbe {
 public:
  // RAII span: increments on entry, records the peak, decrements on exit.
  class Span {
   public:
    Span(ConcurrencyProbe& probe, OpenPitParamAccountId account)
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
    OpenPitParamAccountId m_account;
  };

  [[nodiscard]] std::int64_t PeakFor(OpenPitParamAccountId account) {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_peak[account];
  }

 private:
  std::mutex m_mutex;
  std::map<OpenPitParamAccountId, std::int64_t> m_active;
  std::map<OpenPitParamAccountId, std::int64_t> m_peak;
};

// A barrier that blocks the calling worker until released, letting a test pin a
// task "in flight" deterministically (no sleeps).
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

// The driver under test. Stateless apart from the probe/counters it updates.
struct MockDriver {
  ConcurrencyProbe* probe = nullptr;
  std::atomic<std::size_t> calls{0};

  // A single-value engine-style call: echoes `price` back exactly (financial
  // exactness is asserted through the domain type, never a double).
  [[nodiscard]] Price Echo(OpenPitParamAccountId account, Price price) {
    if (probe != nullptr) {
      ConcurrencyProbe::Span span(*probe, account);
      calls.fetch_add(1, std::memory_order_relaxed);
      return price;
    }
    calls.fetch_add(1, std::memory_order_relaxed);
    return price;
  }
};

//------------------------------------------------------------------------------
// Test observer

// Records callback counts so observer wiring is testable deterministically.
class CountingObserver final : public ae::Observer {
 public:
  void OnEnqueue(AccountId, std::size_t) override {
    m_enqueues.fetch_add(1, std::memory_order_relaxed);
  }
  void OnComplete(AccountId, std::chrono::nanoseconds) override {
    m_completes.fetch_add(1, std::memory_order_relaxed);
  }
  void OnQueueCreated(AccountId, std::size_t) override {
    m_created.fetch_add(1, std::memory_order_relaxed);
  }

  [[nodiscard]] std::size_t Enqueues() const { return m_enqueues.load(); }
  [[nodiscard]] std::size_t Completes() const { return m_completes.load(); }
  [[nodiscard]] std::size_t Created() const { return m_created.load(); }

 private:
  std::atomic<std::size_t> m_enqueues{0};
  std::atomic<std::size_t> m_completes{0};
  std::atomic<std::size_t> m_created{0};
};

class ThrowingObserver final : public ae::Observer {
 public:
  void OnEnqueue(AccountId, std::size_t) override {
    throw std::runtime_error("observer failed");
  }

  void OnComplete(AccountId, std::chrono::nanoseconds) override {
    throw std::runtime_error("observer failed");
  }

  void OnQueueCreated(AccountId, std::size_t) override {
    throw std::runtime_error("observer failed");
  }
};

//------------------------------------------------------------------------------
// Lifecycle: start -> submit -> result -> shutdown (happy path)

TEST(AsyncEngineLifecycle, ShardedCallReturnsExactValueThenStops) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(2).Build();

  const Price input = Price::FromString("185.25");
  ae::Future<Price> future = engine.Call(kAccountA, [input](MockDriver& d) {
    return d.Echo(kAccountARaw, input);
  });

  const std::optional<Price> result = future.Await(kAwaitCap);
  ASSERT_TRUE(result.has_value());
  EXPECT_EQ(result->ToString(), "185.25");
  EXPECT_EQ(driver.calls.load(), 1u);

  EXPECT_TRUE(engine.StopGraceful());
}

TEST(AsyncEngineLifecycle, DynamicCallReturnsExactValueThenStops) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Dynamic().Build();

  const Price input = Price::FromString("0.10");
  ae::Future<Price> future = engine.Call(kAccountB, [input](MockDriver& d) {
    return d.Echo(kAccountBRaw, input);
  });

  const std::optional<Price> result = future.Await(kAwaitCap);
  ASSERT_TRUE(result.has_value());
  EXPECT_EQ(result->ToString(), "0.10");

  EXPECT_TRUE(engine.StopGraceful());
}

TEST(AsyncEngineBuilder, ShardedStageKeepsStopCallbackAcrossBuilds) {
  MockDriver driver;
  std::atomic<std::size_t> stopCalls{0};
  ae::Builder<MockDriver> builder(driver);
  builder.WithStopUnderlying(
      [&stopCalls] { stopCalls.fetch_add(1, std::memory_order_relaxed); });
  auto stage = builder.Sharded(1);

  auto first = stage.Build();
  auto second = stage.Build();

  EXPECT_TRUE(first.StopGraceful());
  EXPECT_TRUE(second.StopGraceful());
  EXPECT_EQ(stopCalls.load(std::memory_order_relaxed), 2u);
}

TEST(AsyncEngineBuilder, DynamicStageKeepsStopCallbackAcrossBuilds) {
  MockDriver driver;
  std::atomic<std::size_t> stopCalls{0};
  ae::Builder<MockDriver> builder(driver);
  builder.WithStopUnderlying(
      [&stopCalls] { stopCalls.fetch_add(1, std::memory_order_relaxed); });
  auto stage = builder.Dynamic();

  auto first = stage.Build();
  auto second = stage.Build();

  EXPECT_TRUE(first.StopGraceful());
  EXPECT_TRUE(second.StopGraceful());
  EXPECT_EQ(stopCalls.load(std::memory_order_relaxed), 2u);
}

TEST(AsyncEngineLifecycle, PairFutureDeliversBothTupleValues) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(1).Build();

  // Mirrors a start-stage tuple shape (value-or-rejects): here (price, count).
  ae::PairFuture<Price, std::int64_t> future =
      engine.Call2<Price, std::int64_t>(kAccountA, [](MockDriver&) {
        return std::make_pair(Price::FromString("42.5"),
                              static_cast<std::int64_t>(7));
      });

  const std::optional<std::pair<Price, std::int64_t>> result =
      future.Await(kAwaitCap);
  ASSERT_TRUE(result.has_value());
  EXPECT_EQ(result->first.ToString(), "42.5");
  EXPECT_EQ(result->second, 7);

  EXPECT_TRUE(engine.StopGraceful());
}

TEST(AsyncEngineLifecycle, RaiiDestructorStopsWithoutExplicitStop) {
  MockDriver driver;
  {
    auto engine = ae::Builder<MockDriver>(driver).Sharded(2).Build();
    const Price input = Price::FromString("1");
    EXPECT_TRUE(engine
                    .Call(kAccountA,
                          [input](MockDriver& d) {
                            return d.Echo(kAccountARaw, input);
                          })
                    .Await(kAwaitCap)
                    .has_value());
    // No explicit Stop: the destructor must hard-stop and join every worker.
  }
  SUCCEED();
}

//------------------------------------------------------------------------------
// Threading contract: per-account serialization

// Many concurrent submitters hammer a small set of accounts. The probe must
// never observe two driver calls for one account overlapping. Several
// submitters per account are required: with one submitter the driver itself
// never overlaps a single account, so the queue would not be the thing under
// test.
TEST(AsyncEngineThreading, PerAccountSerializationHoldsUnderConcurrency) {
  constexpr int kAccounts = 4;
  constexpr int kSubmittersPerAccount = 4;
  constexpr int kPerSubmitter = 50;

  ConcurrencyProbe probe;
  MockDriver driver;
  driver.probe = &probe;

  // Sharded with several workers so distinct accounts genuinely run in
  // parallel while same-account calls stay serialized by the queue.
  auto engine = ae::Builder<MockDriver>(driver).Sharded(4).Build();

  std::vector<OpenPitParamAccountId> accounts;
  for (int i = 0; i < kAccounts; ++i) {
    accounts.push_back(static_cast<OpenPitParamAccountId>(100 + i));
  }

  const Price input = Price::FromString("10");
  std::vector<std::thread> submitters;
  std::atomic<int> failures{0};
  for (const OpenPitParamAccountId account : accounts) {
    for (int s = 0; s < kSubmittersPerAccount; ++s) {
      submitters.emplace_back([&, account] {
        for (int j = 0; j < kPerSubmitter; ++j) {
          ae::Future<Price> f = engine.Call(
              AccountId::FromUint64(account),
              [&, account](MockDriver& d) { return d.Echo(account, input); });
          const std::optional<Price> r = f.Await(kAwaitCap);
          if (!r.has_value() || r->ToString() != "10") {
            failures.fetch_add(1, std::memory_order_relaxed);
          }
        }
      });
    }
  }
  for (std::thread& t : submitters) {
    t.join();
  }

  EXPECT_TRUE(engine.StopGraceful(seconds(10)));
  EXPECT_EQ(failures.load(), 0);

  for (const OpenPitParamAccountId account : accounts) {
    EXPECT_LE(probe.PeakFor(account), 1)
        << "account " << account << " saw overlapping driver calls";
  }
}

TEST(AsyncEngineThreading, TaskRunsOnWorkerNotSubmitterThread) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(1).Build();

  const std::thread::id submitter = std::this_thread::get_id();
  std::atomic<bool> sameThread{true};
  ae::Future<int> future = engine.Call(kAccountA, [&](MockDriver&) {
    sameThread.store(std::this_thread::get_id() == submitter);
    return 0;
  });
  ASSERT_TRUE(future.Await(kAwaitCap).has_value());
  EXPECT_FALSE(sameThread.load());

  EXPECT_TRUE(engine.StopGraceful());
}

//------------------------------------------------------------------------------
// Clean drain with in-flight work

// A graceful stop must wait for an already-queued task to run to completion.
// The task is pinned in flight by a gate; stop runs on a helper thread and must
// not complete until the gate is opened.
TEST(AsyncEngineShutdown, GracefulDrainsInFlightWork) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(1).Build();

  Gate gate;
  std::atomic<bool> taskFinished{false};
  ae::Future<int> future = engine.Call(kAccountA, [&](MockDriver&) {
    gate.Wait();
    taskFinished.store(true);
    return 1;
  });

  std::atomic<bool> stopReturned{false};
  std::thread stopper([&] {
    EXPECT_TRUE(engine.StopGraceful(seconds(10)));
    stopReturned.store(true);
  });

  // The in-flight task is blocked on the gate, so graceful stop cannot return.
  // (We cannot assert on timing without flakiness, but we assert the ordering:
  // the task must be finished by the time stop returns.)
  gate.Open();
  stopper.join();

  EXPECT_TRUE(stopReturned.load());
  EXPECT_TRUE(taskFinished.load());
  ASSERT_TRUE(future.Await(kAwaitCap).has_value());
}

//------------------------------------------------------------------------------
// Error / abort paths

TEST(AsyncEngineErrors, SubmitAfterStopFailsWithStopped) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(1).Build();
  EXPECT_TRUE(engine.StopGraceful());

  ae::Future<int> future =
      engine.Call(kAccountA, [](MockDriver&) { return 5; });
  try {
    (void)future.Await();
    FAIL() << "expected a Stopped error";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
  }
}

// Hard stop aborts a not-yet-started task with Stopped. The first task is
// pinned in flight by a gate; a second task is queued behind it and must be
// aborted (never run) when the hard stop fires.
TEST(AsyncEngineErrors, HardStopAbortsQueuedTaskWithStopped) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(1).Build();

  Gate gate;
  Gate started;
  ae::Future<int> running = engine.Call(kAccountA, [&](MockDriver&) {
    started.Open();
    gate.Wait();
    return 1;
  });

  std::atomic<bool> secondRan{false};
  ae::Future<int> queued = engine.Call(kAccountA, [&](MockDriver&) {
    secondRan.store(true);
    return 2;
  });

  std::thread stopper([&] {
    // Confirm the first task is in-flight before releasing the gate and
    // hard-stopping; otherwise the worker may not have picked up the task yet
    // and StopHard would abort it before it ever runs.
    started.Wait();
    gate.Open();
    EXPECT_TRUE(engine.StopHard(seconds(10)));
  });
  stopper.join();

  // The first task completed normally.
  ASSERT_TRUE(running.Await(kAwaitCap).has_value());

  // The queued task's outcome is either a clean run (if the worker reached it
  // before the hard stop) or a Stopped abort. The deterministic guarantee we
  // assert: it never silently vanishes - the future is always resolved.
  try {
    const std::optional<int> r = queued.Await(kAwaitCap);
    // Ran before the abort: value present and the closure executed.
    EXPECT_TRUE(r.has_value());
    EXPECT_TRUE(secondRan.load());
  } catch (const ae::Error& err) {
    // Aborted: Stopped, and the closure never ran.
    EXPECT_EQ(err.Code(), ae::ErrorCode::Stopped);
    EXPECT_FALSE(secondRan.load());
  }
}

TEST(AsyncEngineErrors, SubmitClosureExceptionBecomesTaskFailed) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(1).Build();

  ae::Future<std::monostate> future =
      engine.Submit(kAccountA, [] { throw std::runtime_error("boom"); });
  try {
    (void)future.Await();
    FAIL() << "expected a TaskFailed error";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::TaskFailed);
    EXPECT_THAT(err.Message(), testing::HasSubstr("boom"));
  }

  EXPECT_TRUE(engine.StopGraceful());
}

TEST(AsyncEngineErrors, DynamicQueueLimitRejectsNewAccount) {
  MockDriver driver;
  // A cap of n means n usable account queues. A gate keeps the two permitted
  // accounts occupied, forcing a third to hit the cap.
  auto engine = ae::Builder<MockDriver>(driver)
                    .Dynamic()
                    .MaxQueues(2)
                    .IdleCleanupAfter(std::chrono::nanoseconds(0))
                    .Build();

  Gate gate;
  ae::Future<int> occupiedA = engine.Call(kAccountA, [&](MockDriver&) {
    gate.Wait();
    return 1;
  });
  ae::Future<int> occupiedB = engine.Call(kAccountB, [&](MockDriver&) {
    gate.Wait();
    return 2;
  });

  // A third, distinct account cannot get a queue: the cap is reached.
  ae::Future<int> rejected =
      engine.Call(kAccountC, [](MockDriver&) { return 3; });
  try {
    (void)rejected.Await(kAwaitCap);
    FAIL() << "expected a QueueLimit error";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::QueueLimit);
  }

  gate.Open();
  ASSERT_TRUE(occupiedA.Await(kAwaitCap).has_value());
  ASSERT_TRUE(occupiedB.Await(kAwaitCap).has_value());
  EXPECT_TRUE(engine.StopGraceful(seconds(10)));
}

// A cap of one must leave one usable account queue. It previously left none:
// the first account submit hit the limit and every later one did too.
TEST(AsyncEngineErrors, DynamicMaxQueuesOneAllowsOneAccountQueue) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver)
                    .Dynamic()
                    .MaxQueues(1)
                    .IdleCleanupAfter(std::chrono::nanoseconds(0))
                    .Build();

  Gate gate;
  ae::Future<int> occupied = engine.Call(kAccountA, [&](MockDriver&) {
    gate.Wait();
    return 1;
  });

  // A second, distinct account cannot get a queue: the single slot is taken.
  ae::Future<int> rejected =
      engine.Call(kAccountB, [](MockDriver&) { return 2; });
  try {
    (void)rejected.Await(kAwaitCap);
    FAIL() << "expected a QueueLimit error";
  } catch (const ae::Error& err) {
    EXPECT_EQ(err.Code(), ae::ErrorCode::QueueLimit);
  }

  gate.Open();
  ASSERT_EQ(occupied.Await(kAwaitCap).value(), 1);
  EXPECT_TRUE(engine.StopGraceful(seconds(10)));
}

//------------------------------------------------------------------------------
// Builder validation

TEST(AsyncEngineBuilder, ShardedZeroWorkersThrows) {
  MockDriver driver;
  EXPECT_THROW(
      { auto e = ae::Builder<MockDriver>(driver).Sharded(0).Build(); },
      openpit::Error);
}

//------------------------------------------------------------------------------
// Observer

TEST(AsyncEngineObserver, DynamicFiresCreateEnqueueComplete) {
  MockDriver driver;
  CountingObserver observer;
  auto engine = ae::Builder<MockDriver>(driver)
                    .WithObserver(observer)
                    .Dynamic()
                    .IdleCleanupAfter(std::chrono::nanoseconds(0))
                    .Build();

  const Price input = Price::FromString("3.125");
  for (int i = 0; i < 3; ++i) {
    ASSERT_TRUE(engine
                    .Call(kAccountA,
                          [input](MockDriver& d) {
                            return d.Echo(kAccountARaw, input);
                          })
                    .Await(kAwaitCap)
                    .has_value());
  }

  EXPECT_TRUE(engine.StopGraceful(seconds(10)));

  // One account -> exactly one queue created; three submits -> three enqueues
  // and three completions.
  EXPECT_EQ(observer.Created(), 1u);
  EXPECT_EQ(observer.Enqueues(), 3u);
  EXPECT_EQ(observer.Completes(), 3u);
}

TEST(AsyncEngineObserver, ExceptionsDoNotAffectDispatch) {
  MockDriver driver;
  ThrowingObserver observer;
  auto engine =
      ae::Builder<MockDriver>(driver).WithObserver(observer).Dynamic().Build();

  const auto result =
      engine
          .Call(kAccountA,
                [](MockDriver& d) {
                  return d.Echo(kAccountARaw, Price::FromString("7.5"));
                })
          .Await(kAwaitCap);
  ASSERT_TRUE(result.has_value());
  EXPECT_EQ(result->ToString(), "7.5");
  EXPECT_TRUE(engine.StopGraceful());
}

//------------------------------------------------------------------------------
// Caller-owned Submit

TEST(AsyncEngineSubmit, RunsClosureAndResolvesVoidFuture) {
  MockDriver driver;
  auto engine = ae::Builder<MockDriver>(driver).Sharded(1).Build();

  std::atomic<int> sideEffect{0};
  ae::Future<std::monostate> future =
      engine.Submit(kAccountA, [&] { sideEffect.store(99); });
  ASSERT_TRUE(future.Await(kAwaitCap).has_value());
  EXPECT_EQ(sideEffect.load(), 99);

  EXPECT_TRUE(engine.StopGraceful());
}

}  // namespace
