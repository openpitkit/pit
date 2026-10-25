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
#include "openpit/pretrade/pretrade.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdlib>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
#include <new>
#include <optional>
#include <thread>
#include <utility>
#include <variant>

namespace allocation_fault {
thread_local std::ptrdiff_t remaining = -1;
thread_local std::size_t allocations = 0;

void Check() {
  ++allocations;
  if (remaining == 0) {
    remaining = -1;
    throw std::bad_alloc();
  }
  if (remaining > 0) {
    --remaining;
  }
}
}  // namespace allocation_fault

void* operator new(std::size_t size) {
  allocation_fault::Check();
  if (void* memory = std::malloc(size == 0 ? 1 : size)) {
    return memory;
  }
  throw std::bad_alloc();
}

void* operator new[](std::size_t size) { return ::operator new(size); }
// Inlining this malloc-backed replacement makes GCC diagnose new/free pairs.
#if defined(__GNUC__) && !defined(__clang__)
[[gnu::noinline]]
#endif
void operator delete(void* memory) noexcept {
  std::free(memory);
}
void operator delete[](void* memory) noexcept { ::operator delete(memory); }
void operator delete(void* memory, std::size_t) noexcept {
  ::operator delete(memory);
}
void operator delete[](void* memory, std::size_t) noexcept {
  ::operator delete(memory);
}

namespace {
namespace ae = openpit::asyncengine;
using Clock = std::chrono::steady_clock;
using namespace std::chrono_literals;
constexpr auto kCap = 5s;
constexpr int kTerminated = 86;
constexpr OpenPitParamAccountId kAccount = 1001;
const auto kNoDeadline = Clock::time_point::max();

class Gate {
 public:
  void Open() {
    std::lock_guard<std::mutex> lock(m_mutex);
    m_open = true;
    m_cv.notify_all();
  }
  void Wait() {
    std::unique_lock<std::mutex> lock(m_mutex);
    if (!m_cv.wait_for(lock, kCap, [this] { return m_open; })) {
      std::abort();
    }
  }

 private:
  std::mutex m_mutex;
  std::condition_variable m_cv;
  bool m_open = false;
};

void Terminated() { std::_Exit(kTerminated); }

class RollbackExitPolicy {
 public:
  void PerformPreTradeCheck(const openpit::pretrade::Context&,
                            openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result&,
                            openpit::pretrade::PolicyDecision&) const {
    mutations.Push([] {}, [] { std::_Exit(87); });
  }
};

void ReleaseWithAllocationFault(bool dynamic, bool dropCopy,
                                std::ptrdiff_t fault) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Account);
  builder.Add(openpit::pretrade::CustomPolicy<RollbackExitPolicy>(
      "RollbackExitPolicy", RollbackExitPolicy{}));
  auto native = builder.Build();
  ae::EngineAdapter driver(native);
  auto engine = dynamic ? ae::Builder(driver).Dynamic().Build()
                        : ae::Builder(driver).Sharded(1).Build();
  const auto account = openpit::param::AccountId::FromUint64(kAccount);
  openpit::model::Order order;
  order.operation.emplace();
  order.operation->accountId = account;
  Gate entered;
  Gate release;
  (void)engine.Submit(account, [&] {
    entered.Open();
    release.Wait();
  });
  entered.Wait();
  if (dropCopy) {
    auto result =
        driver.ApplyDropCopy(std::make_unique<openpit::model::Order>(order));
    auto handle =
        std::make_shared<ae::AsyncDropCopyOperation<ae::EngineAdapter>>(
            std::move(*result.operation), &engine, account);
    allocation_fault::remaining = fault;
    handle.reset();
  } else {
    auto result =
        driver.ExecutePreTrade(std::make_unique<openpit::model::Order>(order));
    auto handle = std::make_shared<ae::AsyncReservation<ae::EngineAdapter>>(
        std::move(*result.reservation), &engine, account);
    allocation_fault::remaining = fault;
    handle.reset();
  }
  std::_Exit(0);
}

struct CallbackFailure : std::exception {
  const char* what() const noexcept override {
    return "callback failure whose diagnostic requires a string allocation";
  }
};

void FailReporting(bool standard) {
  allocation_fault::remaining = 0;
  if (standard) {
    throw CallbackFailure();
  }
  throw 7;
}

std::unique_ptr<ae::detail::Strategy> MakeStrategy(
    bool dynamic, ae::detail::WorkerSeams seams = {}) {
  ae::detail::BaseConfig config;
  config.queueCapacity = 8;
  if (dynamic) {
    return std::make_unique<ae::detail::DynamicStrategy>(config, 1, 0ns, true,
                                                         std::move(seams));
  }
  return std::make_unique<ae::detail::ShardedStrategy>(config, 1,
                                                       std::move(seams));
}

void Submit(ae::detail::Strategy& strategy, std::function<void()> run) {
  strategy.Submit(kAccount,
                  ae::detail::MakeTask(std::move(run), [](ae::Error) {}),
                  kNoDeadline, [](ae::Error) { std::abort(); });
}

// Death-test children skip teardown so a stranded lifecycle fails the test
// instead of hanging its parent.
class AsyncAllocDeathTest : public testing::TestWithParam<bool> {};

TEST_P(AsyncAllocDeathTest, ImplicitReleaseHandoffFailureKeepsBoxAlive) {
  for (bool dropCopy : {false, true}) {
    for (std::ptrdiff_t fault = 0; fault != 3; ++fault) {
      EXPECT_EXIT(
          {
            std::set_terminate(Terminated);
            ReleaseWithAllocationFault(GetParam(), dropCopy, fault);
          },
          testing::ExitedWithCode(kTerminated), "");
    }
  }
}

TEST_P(AsyncAllocDeathTest, DrainReportingFailureTerminates) {
  for (bool standard : {false, true}) {
    EXPECT_EXIT(
        {
          std::set_terminate(Terminated);
          auto* strategy = MakeStrategy(true).release();
          Gate ran;
          Submit(*strategy, [&] { ran.Open(); });
          ran.Wait();
          try {
            (void)strategy->Exclusive(Clock::now() + kCap, [&] {
              (void)strategy->ScheduleMandatoryCleanup(
                  kAccount + 1, [=] { FailReporting(standard); });
            });
          } catch (const std::bad_alloc&) {
          }
          std::_Exit(0);
        },
        testing::ExitedWithCode(kTerminated), "");
  }
}

TEST_P(AsyncAllocDeathTest, WorkerReportingFailureTerminates) {
  for (bool standard : {false, true}) {
    EXPECT_EXIT(
        {
          std::set_terminate(Terminated);
          auto* strategy = MakeStrategy(GetParam()).release();
          Gate entered;
          Gate release;
          Submit(*strategy, [&] {
            entered.Open();
            release.Wait();
          });
          entered.Wait();
          (void)strategy->ScheduleMandatoryCleanup(
              kAccount, [=] { FailReporting(standard); });
          (void)strategy->ScheduleMandatoryCleanup(kAccount, [] {});
          release.Open();
          (void)strategy->StopGraceful(Clock::now() + 100ms);
          std::_Exit(0);
        },
        testing::ExitedWithCode(kTerminated), "");
  }
}

TEST_P(AsyncAllocDeathTest, LaneFreeReportingFailureTerminates) {
  for (bool standard : {false, true}) {
    EXPECT_EXIT(
        {
          std::set_terminate(Terminated);
          auto* strategy = MakeStrategy(true).release();
          if (GetParam()) {
            (void)strategy->StopGraceful(Clock::now() + kCap);
          } else {
            Gate ran;
            Submit(*strategy, [&] { ran.Open(); });
            ran.Wait();
          }
          try {
            (void)strategy->ScheduleMandatoryCleanup(
                kAccount + 1, [=] { FailReporting(standard); });
          } catch (const std::bad_alloc&) {
          }
          std::_Exit(0);
        },
        testing::ExitedWithCode(kTerminated), "");
  }
}

TEST_P(AsyncAllocDeathTest, HardStopAbortErrorAllocationTerminates) {
  EXPECT_EXIT(
      {
        std::set_terminate(Terminated);
        auto* strategy = MakeStrategy(GetParam()).release();
        Gate entered;
        Gate release;
        Submit(*strategy, [&] {
          entered.Open();
          release.Wait();
          allocation_fault::remaining = 0;
        });
        entered.Wait();
        (void)strategy->ScheduleMandatoryCleanup(kAccount, [] {});
        (void)strategy->ScheduleMandatoryCleanup(kAccount, [] {});
        (void)strategy->StopHard(Clock::now());
        release.Open();
        (void)strategy->StopHard(Clock::now() + 100ms);
        std::_Exit(0);
      },
      testing::ExitedWithCode(kTerminated), "");
}

TEST_P(AsyncAllocDeathTest, EscapingWorkerExceptionTerminates) {
  EXPECT_EXIT(
      {
        std::set_terminate(Terminated);
        Gate entered;
        ae::detail::WorkerSeams seams;
        seams.beforeWorkerWait = [&] {
          entered.Open();
          throw 7;
        };
        auto* strategy = MakeStrategy(GetParam(), std::move(seams)).release();
        if (GetParam()) {
          Submit(*strategy, [] {});
        }
        entered.Wait();
        (void)strategy->StopGraceful(Clock::now() + 100ms);
        std::_Exit(0);
      },
      testing::ExitedWithCode(kTerminated), "");
}

struct Driver {
  void Stop() {}
};

void RunEngineFailure(int operation, bool standard) {
  Driver driver;
  auto engine = ae::Builder(driver).Sharded(1).Build();
  const auto account = openpit::param::AccountId::FromUint64(kAccount);
  auto fail = [=] { FailReporting(standard); };
  switch (operation) {
    case 0:
      (void)engine.Submit(account, fail);
      break;
    case 1:
      (void)engine.Submit(account, fail,
                          [](auto promise, ae::Error error, bool) {
                            promise.Fail(std::move(error));
                          });
      break;
    case 2:
      (void)engine.Call(account, [=](Driver&) {
        fail();
        return 0;
      });
      break;
    case 3:
      (void)engine.Call2<int, int>(account, [=](Driver&) {
        fail();
        return std::pair<int, int>{};
      });
      break;
    case 4:
      (void)engine.Call2<int, int>(
          account,
          [=](Driver&) {
            fail();
            return std::pair<int, int>{};
          },
          [](auto promise, ae::Error error, bool) {
            promise.Fail(std::move(error));
          },
          0ns);
      break;
  }
  (void)engine.StopGraceful(kCap);
}

TEST_P(AsyncAllocDeathTest, EngineClosureReportingFailureTerminates) {
  for (int operation = 0; operation < 5; ++operation) {
    for (bool standard : {false, true}) {
      EXPECT_EXIT(
          {
            std::set_terminate(Terminated);
            RunEngineFailure(operation, standard);
            std::_Exit(0);
          },
          testing::ExitedWithCode(kTerminated), "");
    }
  }
}

TEST_P(AsyncAllocDeathTest, AbortHandlerReportingFailureTerminates) {
  for (bool standard : {false, true}) {
    EXPECT_EXIT(
        {
          std::set_terminate(Terminated);
          Driver driver;
          auto engine = ae::Builder(driver).Sharded(1).Build();
          const auto account = openpit::param::AccountId::FromUint64(kAccount);
          (void)engine.StopGraceful(kCap);
          try {
            (void)engine.Submit(
                account, [] {},
                [=](auto, ae::Error, bool) { FailReporting(standard); });
          } catch (const std::bad_alloc&) {
          }
          std::_Exit(0);
        },
        testing::ExitedWithCode(kTerminated), "");
  }
}

TEST_P(AsyncAllocDeathTest, MoveOnlyResultDeliveryFailureTerminates) {
  for (int operation = 0; operation != 3; ++operation) {
    EXPECT_EXIT(
        ([&] {
          std::set_terminate(Terminated);
          Driver driver;
          auto engine = GetParam() ? ae::Builder(driver).Dynamic().Build()
                                   : ae::Builder(driver).Sharded(1).Build();
          const auto account = openpit::param::AccountId::FromUint64(kAccount);
          auto value = [](Driver&) {
            auto result = std::make_unique<int>(42);
            allocation_fault::remaining = 0;
            return result;
          };
          auto pair = [&](Driver& borrowed) {
            return std::make_pair(value(borrowed), 7);
          };
          try {
            if (operation == 0) {
              (void)engine.Call(account, value).Await(kCap);
            } else if (operation == 1) {
              (void)engine.Call2<std::unique_ptr<int>, int>(account, pair)
                  .Await(kCap);
            } else {
              (void)engine
                  .Call2<std::unique_ptr<int>, int>(
                      account, pair,
                      [](auto promise, ae::Error error, bool) {
                        promise.Fail(std::move(error));
                      },
                      0ns)
                  .Await(kCap);
            }
          } catch (const std::future_error&) {
            std::_Exit(87);
          }
          std::_Exit(0);
        }()),
        testing::ExitedWithCode(kTerminated), "");
  }
}

INSTANTIATE_TEST_SUITE_P(Strategies, AsyncAllocDeathTest, testing::Bool());

TEST(AsyncAlloc, DeferredQueueRegistersOnceAfterStop) {
  for (bool dynamic : {false, true}) {
    auto strategy = MakeStrategy(dynamic);
    Gate entered;
    Submit(*strategy, [&] { entered.Open(); });
    entered.Wait();
    ASSERT_TRUE(strategy->StopGraceful(Clock::now() + kCap));
    (void)strategy->ScheduleMandatoryCleanup(kAccount, [] {}).Await(kCap);
    std::optional<std::size_t> expected;
    for (int iteration = 0; iteration != 16; ++iteration) {
      allocation_fault::allocations = 0;
      auto future = strategy->ScheduleMandatoryCleanup(kAccount, [] {});
      const auto count = allocation_fault::allocations;
      if (!expected) {
        expected = count;
      }
      EXPECT_EQ(count, *expected) << "iteration " << iteration;
      EXPECT_TRUE(future.Await(kCap).has_value());
    }
  }
}

TEST(AsyncAlloc, DeferredRegistrationFailurePublishesNothing) {
  for (bool dynamic : {false, true}) {
    std::size_t allocationCount = 0;
    for (std::ptrdiff_t fault = -1;
         fault < static_cast<std::ptrdiff_t>(allocationCount); ++fault) {
      auto strategy = MakeStrategy(dynamic);
      Gate entered;
      Gate release;
      Submit(*strategy, [&] {
        entered.Open();
        release.Wait();
      });
      entered.Wait();
      ASSERT_FALSE(strategy->StopGraceful(Clock::now()));
      bool ran = false;
      bool failed = false;
      std::optional<ae::Future<std::monostate>> future;
      const std::function<void()> cleanup = [&] { ran = true; };
      allocation_fault::allocations = 0;
      allocation_fault::remaining = fault;
      try {
        future.emplace(strategy->ScheduleMandatoryCleanup(kAccount, cleanup));
      } catch (const std::bad_alloc&) {
        failed = true;
      }
      allocation_fault::remaining = -1;
      if (fault == -1) {
        allocationCount = allocation_fault::allocations;
        EXPECT_GT(allocationCount, 0U);
      }
      EXPECT_EQ(future.has_value(), !failed) << "allocation index " << fault;
      release.Open();
      ASSERT_TRUE(strategy->StopGraceful(Clock::now() + kCap));
      EXPECT_EQ(ran, !failed) << "allocation index " << fault;
      if (future) {
        EXPECT_TRUE(future->Await(kCap).has_value());
      }
    }
  }
}
}  // namespace
