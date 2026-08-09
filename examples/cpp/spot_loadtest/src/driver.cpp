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

#include "spot_loadtest/driver/driver.hpp"

#include "driver_build.hpp"
#include "driver_oracle.hpp"

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/driver/live.hpp"
#include "spot_loadtest/generator/event.hpp"
#include "spot_loadtest/measurement/observer.hpp"
#include "spot_loadtest/measurement/overhead.hpp"
#include "spot_loadtest/measurement/sink.hpp"
#include "spot_loadtest/measurement/snapshot.hpp"
#include "spot_loadtest/measurement/window.hpp"

#include "openpit/asyncengine/observer.hpp"
#include "openpit/asyncengine/typed.hpp"
#include "openpit/engine.hpp"
#include "openpit/pretrade/policies.hpp"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <deque>
#include <exception>
#include <functional>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <ratio>
#include <stdexcept>
#include <string>
#include <thread>
#include <tuple>
#include <type_traits>
#include <utility>
#include <vector>

namespace spot_loadtest::driver {
namespace {

namespace ae = ::openpit::asyncengine;
using Driver = detail::LockingEngineAdapter;
using Clock = std::chrono::steady_clock;

// Bounds the graceful dispatcher drain and engine stop at the end of a run.
constexpr std::chrono::seconds kStopTimeout{30};
// Empty-submit probes run before the workload to characterise self-overhead.
constexpr int kDefaultOverheadProbes = 200;
// Default collector / finalizer pool sizes.
constexpr int kDefaultCollectors = 16;
constexpr int kDefaultFinalizers = 16;
// Capacity of the FAST channel feeding the finalizer pool.
constexpr std::size_t kFinalizeBuffer = 8192;

template <typename TargetDuration>
[[nodiscard]] constexpr std::optional<std::uintmax_t>
ExactCeilDurationTicks(std::chrono::nanoseconds relativeTime) noexcept {
  if (relativeTime < std::chrono::nanoseconds::zero()) {
    return std::nullopt;
  }

  using Conversion = std::ratio_divide<std::chrono::nanoseconds::period,
                                       typename TargetDuration::period>;
  static_assert(Conversion::num > 0 && Conversion::den > 0);
  constexpr std::uintmax_t numerator =
      static_cast<std::uintmax_t>(Conversion::num);
  constexpr std::uintmax_t denominator =
      static_cast<std::uintmax_t>(Conversion::den);
  constexpr std::uintmax_t uintMax = std::numeric_limits<std::uintmax_t>::max();

  const std::uintmax_t source =
      static_cast<std::uintmax_t>(relativeTime.count());
  const std::uintmax_t whole = source / denominator;
  const std::uintmax_t remainder = source % denominator;
  if (whole > uintMax / numerator) {
    return std::nullopt;
  }
  std::uintmax_t ticks = whole * numerator;

  if (remainder != 0) {
    // The final quotient could fit even when this product does not. Rejecting
    // that rare duration ratio is conservative and never schedules early.
    if (remainder > uintMax / numerator) {
      return std::nullopt;
    }
    const std::uintmax_t scaledRemainder = remainder * numerator;
    std::uintmax_t fractionalTicks = scaledRemainder / denominator;
    if (scaledRemainder % denominator != 0) {
      if (fractionalTicks == uintMax) {
        return std::nullopt;
      }
      ++fractionalTicks;
    }
    if (ticks > uintMax - fractionalTicks) {
      return std::nullopt;
    }
    ticks += fractionalTicks;
  }
  return ticks;
}

constexpr std::uintmax_t kIntegerPrecisionRegression = 9'007'199'254'740'993ULL;
constexpr auto kIntegerPrecisionRegressionTicks =
    ExactCeilDurationTicks<std::chrono::microseconds>(
        std::chrono::nanoseconds{kIntegerPrecisionRegression});
static_assert(kIntegerPrecisionRegressionTicks.has_value() &&
                  *kIntegerPrecisionRegressionTicks == 9'007'199'254'741ULL,
              "deadline conversion must not round beyond integer precision");

[[nodiscard]] Clock::time_point
CheckedDeadline(Clock::time_point start, std::chrono::nanoseconds relativeTime,
                std::uint64_t eventSeq) {
  if (relativeTime < std::chrono::nanoseconds::zero()) {
    throw std::invalid_argument("driver: event seq " +
                                std::to_string(eventSeq) +
                                " has a negative virtualT0");
  }

  using ClockDuration = Clock::duration;
  using ClockRep = ClockDuration::rep;
  static_assert(std::numeric_limits<ClockRep>::is_integer &&
                    std::numeric_limits<ClockRep>::is_bounded &&
                    !std::is_same_v<ClockRep, bool>,
                "spot_loadtest requires a bounded integral clock rep");

  const std::optional<std::uintmax_t> converted =
      ExactCeilDurationTicks<ClockDuration>(relativeTime);
  if (!converted.has_value()) {
    throw std::overflow_error("driver: event seq " + std::to_string(eventSeq) +
                              " virtualT0 exceeds the clock duration range");
  }
  if constexpr (std::numeric_limits<ClockRep>::digits <
                std::numeric_limits<std::uintmax_t>::digits) {
    if (*converted >
        static_cast<std::uintmax_t>(std::numeric_limits<ClockRep>::max())) {
      throw std::overflow_error(
          "driver: event seq " + std::to_string(eventSeq) +
          " virtualT0 exceeds the clock representation range");
    }
  }

  const ClockDuration offset{static_cast<ClockRep>(*converted)};
  if (start > Clock::time_point::max() - offset) {
    throw std::overflow_error("driver: event seq " + std::to_string(eventSeq) +
                              " deadline exceeds the clock time-point range");
  }
  return start + offset;
}

// An unbounded FIFO used as the spill path behind a bounded fast channel so a
// momentarily-full buffer never blocks the producer. Safe for concurrent
// producers and consumers.
template <typename T> class Overflow {
public:
  // Appends one item and returns the new length.
  int Push(T item) {
    std::lock_guard<std::mutex> lock(m_mutex);
    m_items.push_back(std::move(item));
    return static_cast<int>(m_items.size());
  }
  // Removes and returns the oldest item, or false when empty.
  bool Pop(T &out) {
    std::lock_guard<std::mutex> lock(m_mutex);
    if (m_items.empty()) {
      return false;
    }
    out = std::move(m_items.front());
    m_items.pop_front();
    return true;
  }

private:
  std::mutex m_mutex;
  std::deque<T> m_items;
};

// A bounded MPMC channel with non-blocking try-send and a blocking receive,
// plus a close signal for the work handoff.
template <typename T> class Channel {
public:
  explicit Channel(std::size_t capacity) : m_capacity(capacity) {}

  // Non-blocking send; returns false (leaving `item` untouched) when the buffer
  // is full, so the caller can spill the still-intact item to the overflow.
  bool TrySend(T &item) {
    std::lock_guard<std::mutex> lock(m_mutex);
    if (m_buffer.size() >= m_capacity) {
      return false;
    }
    m_buffer.push_back(std::move(item));
    m_cv.notify_one();
    return true;
  }

  // Blocking receive; returns false once the channel is closed and drained.
  bool Receive(T &out) {
    std::unique_lock<std::mutex> lock(m_mutex);
    m_cv.wait(lock, [this] { return !m_buffer.empty() || m_closed; });
    if (m_buffer.empty()) {
      return false;
    }
    out = std::move(m_buffer.front());
    m_buffer.pop_front();
    return true;
  }

  void Close() {
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      m_closed = true;
    }
    m_cv.notify_all();
  }

private:
  std::size_t m_capacity;
  std::mutex m_mutex;
  std::condition_variable m_cv;
  std::deque<T> m_buffer;
  bool m_closed = false;
};

// The async observer adapter records queue-wait and
// engine-compute durations into the measurement ObserverSink and tracks queue
// lifecycle counts.
class MetricsObserver final : public ae::Observer {
public:
  explicit MetricsObserver(measurement::ObserverSink *sink) : m_sink(sink) {}

  void OnDequeue(::openpit::param::AccountId,
                 std::chrono::nanoseconds waited) override {
    m_sink->RecordDequeue(waited);
  }
  void OnComplete(::openpit::param::AccountId,
                  std::chrono::nanoseconds ran) override {
    m_sink->RecordComplete(ran);
  }
  void OnQueueCreated(::openpit::param::AccountId, std::size_t) override {
    m_sink->RecordQueueCreated();
  }
  void OnQueueRemoved(::openpit::param::AccountId, std::size_t) override {
    m_sink->RecordQueueRemoved();
  }

private:
  measurement::ObserverSink *m_sink;
};

// The op class of one in-flight submission.
enum class OpKind { OrderCheck, Settlement, Funding };

using OrderFuture = ae::Future<ae::ExecuteOutcome<Driver>>;
using SettleFuture = ae::Future<::openpit::PostTradeResult>;
using FundingFuture = ae::Future<ae::AdjustmentOutcome>;

// One submitted operation handed from a submitter to the collector. Exactly one
// future is set, matching event->kind.
struct InFlight {
  const generator::Event *event = nullptr;
  Clock::time_point intendedT0;
  Clock::time_point actualSubmit;
  OpKind kind = OpKind::OrderCheck;
  std::optional<OrderFuture> orderFut;
  std::optional<SettleFuture> settleFut;
  std::optional<FundingFuture> fundingFut;
};

using ReservationPtr = std::shared_ptr<ae::AsyncReservation<Driver>>;

using EventChain = std::vector<const generator::Event *>;
using EventChains = std::vector<EventChain>;
using SubmitterShard = std::vector<const EventChain *>;

// Splits the stream into one ordered slice per account, preserving each
// account's relative (emission) order, excluding seeds (applied synchronously).
[[nodiscard]] EventChains
PartitionChains(const std::vector<generator::Event> &events) {
  std::vector<std::string> order;
  std::map<std::string, EventChain> byAccount;
  for (const generator::Event &ev : events) {
    if (ev.kind == generator::EventKind::Funding && ev.fundingIsSeed) {
      continue;
    }
    if (byAccount.find(ev.account) == byAccount.end()) {
      order.push_back(ev.account);
    }
    byAccount[ev.account].push_back(&ev);
  }
  EventChains chains;
  chains.reserve(order.size());
  for (const std::string &acc : order) {
    chains.push_back(byAccount[acc]);
  }
  return chains;
}

[[nodiscard]] std::vector<SubmitterShard>
PartitionSubmitterShards(const EventChains &chains,
                         std::size_t submitterWorkers) {
  if (!chains.empty() && submitterWorkers == 0) {
    throw std::invalid_argument(
        "driver: submitterWorkers must be positive for account chains");
  }
  const std::size_t threadCount = std::min(chains.size(), submitterWorkers);
  std::vector<SubmitterShard> shards(threadCount);
  for (std::size_t chainIndex = 0; chainIndex < chains.size(); ++chainIndex) {
    shards[chainIndex % threadCount].push_back(&chains[chainIndex]);
  }
  return shards;
}

// Reports whether the carried error is a dispatch-capacity backpressure signal
// (QueueLimit), the only async error that is a measured outcome rather than a
// transport failure.
[[nodiscard]] bool IsQueueLimit(const ae::Error &err) {
  return err.Code() == ae::ErrorCode::QueueLimit;
}

// The internal run state for one driver invocation.
class RunState {
public:
  RunState(const generator::Stream &stream, const Config &cfg)
      : m_stream(stream), m_cfg(cfg) {}

  RunResult Execute(std::string &invalidReason);

private:
  // Builds the AccountSync engine with the built-in spot funds policy in
  // limit-only mode, wrapped in a TypedAsyncEngine with the configured
  // strategy.
  void BuildEngine();

  void ApplySeeds();
  void StartCollectors();
  void StartFinalizers();
  void SubmitShard(Clock::time_point start, const SubmitterShard &chains);
  void SubmitEvent(Clock::time_point start, const generator::Event &event);

  void HandOffWork(InFlight item);
  void HandOffFinalize(ReservationPtr reservation);

  void Collect();
  void CollectOne(InFlight &item);
  void CollectOrder(InFlight &item);
  void CollectSettlement(InFlight &item);
  void CollectFunding(InFlight &item);
  void FinalizeLoop();
  void FinalizeOne(const ReservationPtr &reservation);

  [[nodiscard]] std::chrono::nanoseconds OverheadProbe();

  [[nodiscard]] bool SleepUntil(Clock::time_point deadline);
  void RecordThreadFailure(const char *context,
                           std::exception_ptr failure) noexcept;
  void RethrowThreadFailure();

  const generator::Stream &m_stream;
  Config m_cfg;

  std::unique_ptr<::openpit::Engine> m_engine;
  std::unique_ptr<Driver> m_driverImpl;
  std::unique_ptr<ae::TypedAsyncEngine<Driver>> m_async;
  std::unique_ptr<MetricsObserver> m_observer;
  std::unique_ptr<measurement::ObserverSink> m_obsSink;

  std::unique_ptr<measurement::Windows> m_windows;
  std::unique_ptr<measurement::Sink> m_sink;
  detail::Oracle m_oracle;

  std::unique_ptr<Channel<InFlight>> m_work;
  Overflow<InFlight> m_workOverflow;
  std::unique_ptr<Channel<ReservationPtr>> m_finalize;
  Overflow<ReservationPtr> m_finalizeOverflow;

  int m_collectors = 0;
  int m_finalizers = 0;
  std::atomic<int> m_sampleCount{0};

  std::atomic<bool> m_stop{false};
  std::mutex m_stopMutex;
  std::condition_variable m_stopCv;

  std::mutex m_failureMutex;
  std::exception_ptr m_threadFailure;
  const char *m_failureContext = nullptr;
};

void RunState::BuildEngine() {
  ::openpit::EngineBuilder builder(::openpit::SyncPolicy::Account);
  // Limit-only spot funds: market orders are rejected (v1 drives limit only).
  builder.Add(::openpit::pretrade::policies::SpotFundsPolicy{});
  m_engine = std::make_unique<::openpit::Engine>(builder.Build());
  m_driverImpl = std::make_unique<Driver>(*m_engine);

  ae::TypedBuilder<Driver> typedBuilder(*m_driverImpl);
  if (m_cfg.observer) {
    m_obsSink = std::make_unique<measurement::ObserverSink>();
    m_observer = std::make_unique<MetricsObserver>(m_obsSink.get());
    typedBuilder.WithObserver(*m_observer);
  }
  typedBuilder.WithQueueCapacity(static_cast<std::size_t>(
      m_cfg.queueCapacity > 0 ? m_cfg.queueCapacity : 0));
  typedBuilder.WithSlowSubmitThreshold(m_cfg.slowSubmitThreshold);

  if (m_cfg.dispatchStrategy == DispatchStrategy::Sharded) {
    m_async = std::make_unique<ae::TypedAsyncEngine<Driver>>(
        typedBuilder.Sharded(static_cast<std::size_t>(m_cfg.shardedWorkers))
            .Build());
  } else {
    auto dynamic = typedBuilder.Dynamic();
    dynamic.MaxQueues(static_cast<std::size_t>(m_cfg.maxQueues));
    dynamic.IdleCleanupAfter(m_cfg.idleCleanup);
    m_async = std::make_unique<ae::TypedAsyncEngine<Driver>>(dynamic.Build());
  }
}

void RunState::ApplySeeds() {
  // Seeding is SETUP, not measured load: apply the initial per-account balance
  // seeds synchronously on the underlying engine BEFORE the async run. The
  // shadow oracle still verifies each seed's predicted post-balance.
  for (const generator::Event &ev : m_stream.events) {
    if (ev.kind != generator::EventKind::Funding || !ev.fundingIsSeed) {
      continue;
    }
    ::openpit::param::AccountId account;
    const ::openpit::accountadjustment::AccountAdjustment adj =
        detail::BuildAdjustment(ev, account);
    const ::openpit::AdjustmentResult result =
        m_engine->ApplyAccountAdjustment(account, std::vector{adj});
    detail::FundingObservation obs;
    obs.rejected = !result.Passed();
    obs.outcomes = result.accountAdjustmentOutcomes;
    m_oracle.CheckFunding(ev, obs);
    if (!result.Passed()) {
      throw std::runtime_error("driver: seed rejected for account " +
                               ev.account + " (setup must succeed)");
    }
  }
}

std::chrono::nanoseconds RunState::OverheadProbe() {
  static const ::openpit::param::AccountId probeAccount =
      ::openpit::param::AccountId::FromString("__overhead_probe__");
  const ::openpit::accountadjustment::AccountAdjustment adj =
      detail::BuildProbeAdjustment();
  const Clock::time_point t0 = Clock::now();
  ae::Future<ae::AdjustmentOutcome> fut =
      m_async->ApplyAccountAdjustment(probeAccount, std::vector{adj});
  try {
    (void)fut.Await();
  } catch (const ae::Error &) {
    return std::chrono::nanoseconds(-1); // signal an error to MeasureOverhead.
  }
  return Clock::now() - t0;
}

bool RunState::SleepUntil(Clock::time_point deadline) {
  if (m_stop.load(std::memory_order_acquire)) {
    return false;
  }
  const auto now = Clock::now();
  if (deadline <= now) {
    return true; // virtual arrival already past: submit as fast as possible.
  }
  std::unique_lock<std::mutex> lock(m_stopMutex);
  return !m_stopCv.wait_until(lock, deadline, [this] {
    return m_stop.load(std::memory_order_acquire);
  });
}

void RunState::SubmitEvent(Clock::time_point start,
                           const generator::Event &event) {
  const Clock::time_point deadline =
      CheckedDeadline(start, event.virtualT0, event.seq);
  if (!SleepUntil(deadline)) {
    return;
  }
  switch (event.kind) {
  case generator::EventKind::OrderCheck: {
    ::openpit::param::AccountId account;
    ::openpit::model::Order order = detail::BuildOrder(event, account);
    m_sink->RecordSubmit();
    const Clock::time_point actualSubmit = Clock::now();
    OrderFuture fut = m_async->ExecutePreTrade(std::move(order));
    InFlight item;
    item.event = &event;
    item.intendedT0 = deadline;
    item.actualSubmit = actualSubmit;
    item.kind = OpKind::OrderCheck;
    item.orderFut = std::move(fut);
    HandOffWork(std::move(item));
    m_sink->RecordSubmitLag(actualSubmit - deadline, m_cfg.maxSubmitLag);
    break;
  }
  case generator::EventKind::Settlement: {
    ::openpit::param::AccountId account;
    ::openpit::model::ExecutionReport report =
        detail::BuildReport(event, account);
    m_sink->RecordSubmit();
    const Clock::time_point actualSubmit = Clock::now();
    SettleFuture fut = m_async->ApplyExecutionReport(std::move(report));
    InFlight item;
    item.event = &event;
    item.intendedT0 = deadline;
    item.actualSubmit = actualSubmit;
    item.kind = OpKind::Settlement;
    item.settleFut = std::move(fut);
    HandOffWork(std::move(item));
    m_sink->RecordSubmitLag(actualSubmit - deadline, m_cfg.maxSubmitLag);
    break;
  }
  case generator::EventKind::Funding: {
    if (event.fundingIsSeed) {
      throw std::logic_error("driver: submitter received a seed funding");
    }
    ::openpit::param::AccountId account;
    ::openpit::accountadjustment::AccountAdjustment adj =
        detail::BuildAdjustment(event, account);
    m_sink->RecordSubmit();
    const Clock::time_point actualSubmit = Clock::now();
    FundingFuture fut =
        m_async->ApplyAccountAdjustment(account, std::vector{adj});
    InFlight item;
    item.event = &event;
    item.intendedT0 = deadline;
    item.actualSubmit = actualSubmit;
    item.kind = OpKind::Funding;
    item.fundingFut = std::move(fut);
    HandOffWork(std::move(item));
    m_sink->RecordSubmitLag(actualSubmit - deadline, m_cfg.maxSubmitLag);
    break;
  }
  }
}

void RunState::SubmitShard(Clock::time_point start,
                           const SubmitterShard &chains) {
  std::vector<std::size_t> next(chains.size(), 0);
  std::optional<std::chrono::nanoseconds> previousVirtualT0;
  while (!m_stop.load(std::memory_order_acquire)) {
    std::optional<std::size_t> selected;
    for (std::size_t chainIndex = 0; chainIndex < chains.size(); ++chainIndex) {
      const EventChain &chain = *chains[chainIndex];
      if (next[chainIndex] >= chain.size()) {
        continue;
      }
      if (!selected) {
        selected = chainIndex;
        continue;
      }
      const generator::Event &candidate = *chain[next[chainIndex]];
      const EventChain &selectedChain = *chains[*selected];
      const generator::Event &current = *selectedChain[next[*selected]];
      if (std::tie(candidate.virtualT0, candidate.seq, chainIndex) <
          std::tie(current.virtualT0, current.seq, *selected)) {
        selected = chainIndex;
      }
    }
    if (!selected) {
      return;
    }
    const generator::Event &event = *chains[*selected]->at(next[*selected]);
    // A deadline regression makes open-loop latency depend on avoidable harness
    // serialization, so reject the run instead of publishing invalid numbers.
    if (previousVirtualT0 && event.virtualT0 < *previousVirtualT0) {
      throw std::logic_error(
          "driver: submitter shard selected a regressing virtual deadline");
    }
    previousVirtualT0 = event.virtualT0;
    ++next[*selected];
    SubmitEvent(start, event);
  }
}

void RunState::HandOffWork(InFlight item) {
  if (m_work->TrySend(item)) {
    return;
  }
  // Fast buffer full (almost always: collectors legitimately blocked in Await
  // because the engine is slow — real latency, NOT a harness stall). Spill to
  // the unbounded overflow and record the peak depth as a diagnostic only.
  const int depth = m_workOverflow.Push(std::move(item));
  m_sink->RecordWorkOverflowDepth(depth);
}

void RunState::HandOffFinalize(ReservationPtr reservation) {
  if (m_finalize->TrySend(reservation)) {
    return;
  }
  // Fast path full: the finalizer pool fell behind. Count the HARNESS stall
  // diagnostic (off the measured path; never invalidates the run) and spill.
  m_sink->RecordHandoffStall();
  m_finalizeOverflow.Push(std::move(reservation));
}

void RunState::CollectOrder(InFlight &item) {
  OrderFuture &fut = *item.orderFut;
  try {
    ae::ExecuteOutcome<Driver> outcome = fut.Await();
    const Clock::time_point resolve = Clock::now();
    const auto latency = resolve - item.intendedT0;
    const bool accepted = outcome.Passed();
    m_sink->RecordOrderCheck(latency, accepted);
    m_sink->RecordServiceTime(resolve - item.actualSubmit);
    m_sampleCount.fetch_add(1, std::memory_order_relaxed);

    detail::OrderObservation obs;
    obs.accepted = accepted;
    obs.rejects = outcome.rejects;
    m_oracle.CheckOrder(*item.event, obs);

    if (!accepted) {
      return; // a rejected order reserved nothing.
    }
    HandOffFinalize(outcome.reservation);
  } catch (const ae::Error &err) {
    const Clock::time_point resolve = Clock::now();
    const auto latency = resolve - item.intendedT0;
    if (IsQueueLimit(err)) {
      m_sink->RecordBackpressure(latency);
      return;
    }
    m_sink->RecordOrderCheck(latency, false);
    m_sampleCount.fetch_add(1, std::memory_order_relaxed);
    m_oracle.FailExternal(
        std::string("driver: ExecutePreTrade transport error (account ") +
        item.event->account + "): " + err.what());
  }
}

void RunState::CollectSettlement(InFlight &item) {
  SettleFuture &fut = *item.settleFut;
  try {
    ::openpit::PostTradeResult result = fut.Await();
    const Clock::time_point resolve = Clock::now();
    const auto latency = resolve - item.intendedT0;
    const bool blocked = !result.accountBlocks.empty();
    m_sink->RecordSettlement(latency, !blocked);
    m_sampleCount.fetch_add(1, std::memory_order_relaxed);
    detail::SettleObservation obs;
    obs.blocked = blocked;
    obs.outcomes = result.accountAdjustments;
    m_oracle.CheckSettlement(*item.event, obs);
  } catch (const ae::Error &err) {
    const Clock::time_point resolve = Clock::now();
    const auto latency = resolve - item.intendedT0;
    if (IsQueueLimit(err)) {
      m_sink->RecordBackpressure(latency);
      return;
    }
    m_sink->RecordSettlement(latency, false);
    m_sampleCount.fetch_add(1, std::memory_order_relaxed);
    m_oracle.FailExternal(
        std::string("driver: ApplyExecutionReport transport error (account ") +
        item.event->account + "): " + err.what());
  }
}

void RunState::CollectFunding(InFlight &item) {
  FundingFuture &fut = *item.fundingFut;
  try {
    ae::AdjustmentOutcome outcome = fut.Await();
    const bool rejected = !outcome.Passed();
    m_sink->RecordFunding(!rejected);
    m_sampleCount.fetch_add(1, std::memory_order_relaxed);
    detail::FundingObservation obs;
    obs.rejected = rejected;
    obs.outcomes = outcome.outcomes;
    m_oracle.CheckFunding(*item.event, obs);
  } catch (const ae::Error &err) {
    const Clock::time_point resolve = Clock::now();
    if (IsQueueLimit(err)) {
      m_sink->RecordBackpressure(resolve - item.intendedT0);
      return;
    }
    m_oracle.FailExternal(
        std::string(
            "driver: ApplyAccountAdjustment transport error (account ") +
        item.event->account + "): " + err.what());
  }
}

void RunState::CollectOne(InFlight &item) {
  switch (item.kind) {
  case OpKind::OrderCheck:
    CollectOrder(item);
    break;
  case OpKind::Settlement:
    CollectSettlement(item);
    break;
  case OpKind::Funding:
    CollectFunding(item);
    break;
  }
}

void RunState::Collect() {
  InFlight item;
  while (true) {
    if (m_workOverflow.Pop(item)) {
      CollectOne(item);
      continue;
    }
    if (!m_work->Receive(item)) {
      // Closed and drained: drain any late overflow, then exit.
      while (m_workOverflow.Pop(item)) {
        CollectOne(item);
      }
      return;
    }
    CollectOne(item);
  }
}

void RunState::FinalizeOne(const ReservationPtr &reservation) {
  try {
    (void)reservation->CommitAndClose().Await();
  } catch (const ae::Error &err) {
    m_oracle.FailExternal(std::string("driver: CommitAndClose: ") + err.what());
  }
}

void RunState::FinalizeLoop() {
  ReservationPtr reservation;
  while (true) {
    if (m_finalizeOverflow.Pop(reservation)) {
      FinalizeOne(reservation);
      continue;
    }
    if (!m_finalize->Receive(reservation)) {
      while (m_finalizeOverflow.Pop(reservation)) {
        FinalizeOne(reservation);
      }
      return;
    }
    FinalizeOne(reservation);
  }
}

void RunState::RecordThreadFailure(const char *context,
                                   std::exception_ptr failure) noexcept {
  {
    std::lock_guard<std::mutex> lock(m_failureMutex);
    if (!m_threadFailure) {
      m_threadFailure = failure;
      m_failureContext = context;
    }
  }
  {
    std::lock_guard<std::mutex> lock(m_stopMutex);
    m_stop.store(true, std::memory_order_release);
  }
  m_stopCv.notify_all();
}

void RunState::RethrowThreadFailure() {
  std::exception_ptr failure;
  const char *context = nullptr;
  {
    std::lock_guard<std::mutex> lock(m_failureMutex);
    failure = m_threadFailure;
    context = m_failureContext;
  }
  if (!failure) {
    return;
  }
  try {
    std::rethrow_exception(failure);
  } catch (const std::exception &error) {
    throw std::runtime_error(std::string("driver: ") + context + ": " +
                             error.what());
  } catch (...) {
    throw std::runtime_error(std::string("driver: ") + context +
                             ": unknown exception");
  }
}

RunResult RunState::Execute(std::string &invalidReason) {
  invalidReason.clear();

  if (!m_stream.events.empty() && m_cfg.submitterWorkers == 0) {
    throw std::invalid_argument(
        "driver: submitterWorkers must be positive for a non-empty stream");
  }
  if (m_cfg.windowSize <= 0) {
    throw std::invalid_argument("driver: windowSize must be positive");
  }
  if (m_cfg.maxSubmitLag.count() <= 0) {
    throw std::invalid_argument("driver: maxSubmitLag must be positive");
  }

  BuildEngine();

  m_collectors = m_cfg.collectors > 0 ? m_cfg.collectors : kDefaultCollectors;
  m_finalizers = m_cfg.finalizers > 0 ? m_cfg.finalizers : kDefaultFinalizers;
  const std::int64_t windowSize = m_cfg.windowSize;

  m_windows = std::make_unique<measurement::Windows>(windowSize);
  m_sink = std::make_unique<measurement::Sink>(m_windows.get());

  // Publish the live-counter accessor before any thread starts.
  if (m_cfg.live != nullptr) {
    measurement::Sink *sink = m_sink.get();
    m_cfg.live->Store([sink] { return sink->Live(); });
  }

  m_work = std::make_unique<Channel<InFlight>>(
      static_cast<std::size_t>(m_collectors * 4));
  m_finalize = std::make_unique<Channel<ReservationPtr>>(kFinalizeBuffer);

  ApplySeeds();

  measurement::OverheadSummary overhead;
  if (m_cfg.overheadProbes > 0) {
    overhead = measurement::MeasureOverhead(m_cfg.overheadProbes,
                                            [this] { return OverheadProbe(); });
  }

  const EventChains chains = PartitionChains(m_stream.events);
  const std::vector<SubmitterShard> submitterShards =
      PartitionSubmitterShards(chains, m_cfg.submitterWorkers);

  std::vector<std::thread> collectorThreads;
  collectorThreads.reserve(static_cast<std::size_t>(m_collectors));
  std::vector<std::thread> finalizerThreads;
  finalizerThreads.reserve(static_cast<std::size_t>(m_finalizers));
  std::vector<std::thread> submitterThreads;
  submitterThreads.reserve(submitterShards.size());

  try {
    for (int i = 0; i < m_collectors; ++i) {
      collectorThreads.emplace_back([this] {
        try {
          Collect();
        } catch (...) {
          RecordThreadFailure("collector worker", std::current_exception());
        }
      });
    }
    for (int i = 0; i < m_finalizers; ++i) {
      finalizerThreads.emplace_back([this] {
        try {
          FinalizeLoop();
        } catch (...) {
          RecordThreadFailure("finalizer worker", std::current_exception());
        }
      });
    }

    const Clock::time_point start = Clock::now();
    for (const SubmitterShard &shard : submitterShards) {
      const SubmitterShard *shardPtr = &shard;
      submitterThreads.emplace_back([this, start, shardPtr] {
        try {
          SubmitShard(start, *shardPtr);
        } catch (...) {
          RecordThreadFailure("submitter worker", std::current_exception());
        }
      });
    }
  } catch (...) {
    RecordThreadFailure("thread setup", std::current_exception());
  }

  const auto joinAll = [](std::vector<std::thread> &threads) {
    for (std::thread &thread : threads) {
      if (thread.joinable()) {
        thread.join();
      }
    }
  };

  joinAll(submitterThreads);
  m_work->Close();
  joinAll(collectorThreads);
  m_finalize->Close();
  joinAll(finalizerThreads);

  // Stop the dispatcher while its borrowed driver and engine are alive.
  bool gracefulStopped = false;
  std::exception_ptr gracefulFailure;
  try {
    gracefulStopped = m_async->StopGraceful(kStopTimeout);
    if (!gracefulStopped) {
      gracefulFailure = std::make_exception_ptr(std::runtime_error(
          "graceful dispatcher shutdown timed out after 30 seconds"));
    }
  } catch (...) {
    gracefulFailure = std::current_exception();
  }
  if (!gracefulStopped) {
    try {
      (void)m_async->StopHard();
    } catch (...) {
      // Preserve the original graceful failure. Destroying the owning facade
      // below is the guaranteed hard-stop-and-join backstop.
    }
  }
  m_async.reset();
  m_driverImpl.reset();
  m_engine.reset();

  // A workload/thread failure remains the primary run failure after shutdown.
  RethrowThreadFailure();
  if (gracefulFailure) {
    try {
      std::rethrow_exception(gracefulFailure);
    } catch (const std::exception &ex) {
      throw std::runtime_error(std::string("driver: dispatcher shutdown: ") +
                               ex.what());
    } catch (...) {
      throw std::runtime_error(
          "driver: dispatcher shutdown: non-standard exception");
    }
  }

  if (auto err = m_oracle.Err()) {
    throw std::runtime_error(*err);
  }
  if (auto err = m_oracle.CheckInvariants(m_stream.events)) {
    throw std::runtime_error(*err);
  }

  RunResult result;
  result.snapshot =
      measurement::Build(*m_windows, *m_sink, m_obsSink.get(), overhead);
  const measurement::SinkStats msStats = m_sink->Stats();

  Stats stats;
  stats.orderChecks = msStats.orderChecks + msStats.fundings;
  stats.settlements = msStats.settlements;
  stats.accepts = msStats.orderCheckAccepts;
  stats.rejects = msStats.orderCheckRejects;
  stats.fundings = msStats.fundings;
  stats.fundingAccepts = msStats.fundingAccepts;
  stats.fundingRejects = msStats.fundingRejects;
  stats.submitLagBreaches = msStats.submitLagBreaches;
  stats.backpressure = msStats.backpressure;
  stats.handoffStalls = msStats.handoffStalls;
  stats.maxWorkOverflow = msStats.maxWorkOverflow;
  stats.checksum = msStats.checksum;
  stats.maxInFlight = msStats.maxInFlight;
  stats.submitterThreads = submitterShards.size();
  stats.sampleCount = m_sampleCount.load(std::memory_order_relaxed);

  result.stats = stats;

  // Methodology invariants: publish ONLY when it is a valid latency
  // measurement.
  if (stats.submitLagBreaches > 0) {
    invalidReason = "submit-lag";
    return result;
  }
  if (stats.backpressure > 0) {
    invalidReason = "backpressure";
    return result;
  }
  const std::uint64_t resolved =
      msStats.orderChecks + msStats.settlements + msStats.fundings;
  if (resolved > 0 && stats.checksum == 0) {
    invalidReason = "zero-checksum";
    return result;
  }
  return result;
}

} // namespace

Config FromAppConfig(const config::Config &cfg) {
  Config out;
  out.observer = cfg.run.observer;
  out.activeAccounts = cfg.concurrency.activeAccounts;
  if (cfg.concurrency.submitterWorkers >
      static_cast<std::uint64_t>(std::numeric_limits<std::size_t>::max())) {
    throw std::invalid_argument(
        "driver: concurrency.submitter_workers exceeds platform size_t");
  }
  out.submitterWorkers =
      static_cast<std::size_t>(cfg.concurrency.submitterWorkers);
  if (cfg.concurrency.maxSubmitLag.count() <= 0) {
    throw std::invalid_argument(
        "driver: concurrency.max_submit_lag must be positive");
  }
  out.maxSubmitLag = cfg.concurrency.maxSubmitLag;
  out.dispatchStrategy =
      cfg.asyncEngine.strategy == config::AsyncEngineStrategy::Sharded
          ? DispatchStrategy::Sharded
          : DispatchStrategy::Dynamic;
  out.maxQueues = cfg.asyncEngine.maxQueues;
  out.idleCleanup = cfg.asyncEngine.idleCleanup;
  out.shardedWorkers = cfg.asyncEngine.shardedWorkers;
  out.queueCapacity = cfg.asyncEngine.queueCapacity;
  out.slowSubmitThreshold = cfg.asyncEngine.slowSubmitThreshold;
  if (cfg.run.window == 0 ||
      cfg.run.window > static_cast<std::uint64_t>(
                           std::numeric_limits<std::int64_t>::max())) {
    throw std::invalid_argument(
        "driver: run.window must be between 1 and INT64_MAX");
  }
  out.windowSize = static_cast<std::int64_t>(cfg.run.window);
  out.overheadProbes = kDefaultOverheadProbes;
  return out;
}

RunResult RunCollecting(const generator::Stream &stream, const Config &cfg,
                        std::string &invalidReason) {
  RunState run(stream, cfg);
  return run.Execute(invalidReason);
}

RunResult Run(const generator::Stream &stream, const Config &cfg) {
  std::string invalidReason;
  RunResult result = RunCollecting(stream, cfg, invalidReason);
  if (invalidReason == "submit-lag") {
    throw SubmitLagInvalidRun();
  }
  if (invalidReason == "backpressure") {
    throw BackpressureInvalidRun();
  }
  if (invalidReason == "zero-checksum") {
    throw ZeroChecksumInvalidRun();
  }
  return result;
}

} // namespace spot_loadtest::driver
