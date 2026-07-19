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

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <deque>
#include <functional>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>
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
// plus a close signal. Mirrors a Go buffered channel used by the work handoff.
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

// The async observer adapter (mirror of observer.go). Records queue-wait and
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
  // The submit acknowledgement for a settlement: the lock-bearing report is
  // applied via a Submit closure, so a failed submit (stop / queue limit)
  // surfaces here while the result future would otherwise block forever.
  std::shared_ptr<ae::Future<std::monostate>> settleSubmitAck;
};

using ReservationPtr = std::shared_ptr<ae::AsyncReservation<Driver>>;

// Splits the stream into one ordered slice per account, preserving each
// account's relative (emission) order, excluding seeds (applied synchronously).
[[nodiscard]] std::vector<std::vector<const generator::Event *>>
PartitionChains(const std::vector<generator::Event> &events) {
  std::vector<std::string> order;
  std::map<std::string, std::vector<const generator::Event *>> byAccount;
  for (const generator::Event &ev : events) {
    if (ev.kind == generator::EventKind::Funding && ev.fundingIsSeed) {
      continue;
    }
    if (byAccount.find(ev.account) == byAccount.end()) {
      order.push_back(ev.account);
    }
    byAccount[ev.account].push_back(&ev);
  }
  std::vector<std::vector<const generator::Event *>> chains;
  chains.reserve(order.size());
  for (const std::string &acc : order) {
    chains.push_back(byAccount[acc]);
  }
  return chains;
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
  void SubmitChain(Clock::time_point start,
                   const std::vector<const generator::Event *> &events);

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

  static void SleepUntil(Clock::time_point deadline);

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

void RunState::SleepUntil(Clock::time_point deadline) {
  const auto now = Clock::now();
  if (deadline <= now) {
    return; // virtual arrival already past: submit as fast as we can issue.
  }
  std::this_thread::sleep_until(deadline);
}

void RunState::SubmitChain(
    Clock::time_point start,
    const std::vector<const generator::Event *> &events) {
  for (const generator::Event *ev : events) {
    const Clock::time_point deadline = start + ev->virtualT0;
    SleepUntil(deadline);
    switch (ev->kind) {
    case generator::EventKind::OrderCheck: {
      ::openpit::param::AccountId account;
      ::openpit::model::Order order;
      try {
        order = detail::BuildOrder(*ev, account);
      } catch (const std::exception &e) {
        m_oracle.FailExternal(std::string("driver: build order: ") + e.what());
        return;
      }
      m_sink->RecordSubmit();
      const Clock::time_point actualSubmit = Clock::now();
      OrderFuture fut = m_async->ExecutePreTrade(std::move(order));
      InFlight item;
      item.event = ev;
      item.intendedT0 = deadline;
      item.actualSubmit = actualSubmit;
      item.kind = OpKind::OrderCheck;
      item.orderFut = std::move(fut);
      HandOffWork(std::move(item));
      break;
    }
    case generator::EventKind::Settlement: {
      ::openpit::param::AccountId account;
      detail::ReportWithLock report = detail::BuildReport(*ev, account);
      m_sink->RecordSubmit();
      const Clock::time_point actualSubmit = Clock::now();
      // Route the lock-bearing report through the generic per-account queue so
      // the AccountSync invariant holds; the typed ApplyExecutionReport takes
      // model::ExecutionReport (no lock), so we submit a closure that applies
      // our lock-bearing report on the worker thread and resolves a future.
      ae::Promise<::openpit::PostTradeResult> promise;
      SettleFuture fut = promise.GetFuture();
      const Driver *driver = m_driverImpl.get();
      auto report_ptr =
          std::make_shared<detail::ReportWithLock>(std::move(report));
      ae::Future<std::monostate> submitted =
          m_async->Submit(account, [promise, driver, report_ptr]() {
            try {
              promise.Resolve(driver->ApplyExecutionReport(*report_ptr));
            } catch (const std::exception &ex) {
              promise.Fail(ae::Error(ae::ErrorCode::TaskFailed, ex.what()));
            }
          });
      // If the submit itself failed (stop / queue limit), forward that error
      // so the collector records backpressure rather than hanging on the
      // result future.
      InFlight item;
      item.event = ev;
      item.intendedT0 = deadline;
      item.actualSubmit = actualSubmit;
      item.kind = OpKind::Settlement;
      // Bridge the submit error onto the result future: a background-free
      // check at collection time. We piggy-back by storing the result future;
      // submit errors surface through `submitted` which we await first.
      item.settleFut = std::move(fut);
      // Stash the submit ack so the collector can detect a failed submit.
      // We resolve it by awaiting submitted in the collector via a wrapper:
      // simplest faithful behaviour is to await the result future, which the
      // closure resolves on success; on a submit failure the closure never
      // runs, so we must observe `submitted`. Await it here non-blockingly is
      // wrong (open-loop), so we fold the submit ack into the work item.
      item.settleSubmitAck =
          std::make_shared<ae::Future<std::monostate>>(std::move(submitted));
      HandOffWork(std::move(item));
      break;
    }
    case generator::EventKind::Funding: {
      if (ev->fundingIsSeed) {
        continue; // seeds applied synchronously; defensive guard.
      }
      ::openpit::param::AccountId account;
      ::openpit::accountadjustment::AccountAdjustment adj;
      try {
        adj = detail::BuildAdjustment(*ev, account);
      } catch (const std::exception &e) {
        m_oracle.FailExternal(std::string("driver: build adjustment: ") +
                              e.what());
        return;
      }
      m_sink->RecordSubmit();
      const Clock::time_point actualSubmit = Clock::now();
      FundingFuture fut =
          m_async->ApplyAccountAdjustment(account, std::vector{adj});
      InFlight item;
      item.event = ev;
      item.intendedT0 = deadline;
      item.actualSubmit = actualSubmit;
      item.kind = OpKind::Funding;
      item.fundingFut = std::move(fut);
      HandOffWork(std::move(item));
      break;
    }
    }
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
  // First observe the submit ack: a failed submit (stop / queue limit) means
  // the closure never ran, so the result future would block forever.
  try {
    (void)item.settleSubmitAck->Await();
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
        std::string("driver: ApplyExecutionReport submit error (account ") +
        item.event->account + "): " + err.what());
    return;
  }

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

RunResult RunState::Execute(std::string &invalidReason) {
  invalidReason.clear();

  BuildEngine();

  m_collectors = m_cfg.collectors > 0 ? m_cfg.collectors : kDefaultCollectors;
  m_finalizers = m_cfg.finalizers > 0 ? m_cfg.finalizers : kDefaultFinalizers;
  std::int64_t windowSize = m_cfg.windowSize > 0 ? m_cfg.windowSize : 10'000;

  measurement::WindowUnit unit = m_cfg.windowUnit;
  m_windows = std::make_unique<measurement::Windows>(unit, windowSize,
                                                     m_cfg.wallWindow);
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

  const auto chains = PartitionChains(m_stream.events);

  std::vector<std::thread> collectorThreads;
  collectorThreads.reserve(static_cast<std::size_t>(m_collectors));
  for (int i = 0; i < m_collectors; ++i) {
    collectorThreads.emplace_back([this] { Collect(); });
  }
  std::vector<std::thread> finalizerThreads;
  finalizerThreads.reserve(static_cast<std::size_t>(m_finalizers));
  for (int i = 0; i < m_finalizers; ++i) {
    finalizerThreads.emplace_back([this] { FinalizeLoop(); });
  }

  const Clock::time_point start = Clock::now();
  std::vector<std::thread> submitterThreads;
  submitterThreads.reserve(chains.size());
  for (const auto &chain : chains) {
    submitterThreads.emplace_back(
        [this, start, &chain] { SubmitChain(start, chain); });
  }

  for (std::thread &t : submitterThreads) {
    t.join();
  }
  m_work->Close();
  for (std::thread &t : collectorThreads) {
    t.join();
  }
  m_finalize->Close();
  for (std::thread &t : finalizerThreads) {
    t.join();
  }

  // Shutdown: drain the dispatcher gracefully, then stop the engine.
  (void)m_async->StopGraceful(kStopTimeout);
  m_engine.reset(); // releases the engine after the dispatcher has drained.

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
  stats.backpressure = msStats.backpressure;
  stats.handoffStalls = msStats.handoffStalls;
  stats.maxWorkOverflow = msStats.maxWorkOverflow;
  stats.checksum = msStats.checksum;
  stats.maxInFlight = msStats.maxInFlight;
  stats.sampleCount = m_sampleCount.load(std::memory_order_relaxed);

  result.stats = stats;

  // Methodology invariants: publish ONLY when it is a valid latency
  // measurement.
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
  out.dispatchStrategy =
      cfg.asyncEngine.strategy == config::AsyncEngineStrategy::Sharded
          ? DispatchStrategy::Sharded
          : DispatchStrategy::Dynamic;
  out.maxQueues = cfg.asyncEngine.maxQueues;
  out.idleCleanup = cfg.asyncEngine.idleCleanup;
  out.shardedWorkers = cfg.asyncEngine.shardedWorkers;
  out.queueCapacity = cfg.asyncEngine.queueCapacity;
  out.slowSubmitThreshold = cfg.asyncEngine.slowSubmitThreshold;
  std::int64_t windowSize = static_cast<std::int64_t>(cfg.run.window);
  if (windowSize <= 0) {
    windowSize = 10'000;
  }
  out.windowSize = windowSize;
  out.windowUnit = cfg.run.windowUnit == config::WindowUnit::Wall
                       ? measurement::WindowUnit::Wall
                       : measurement::WindowUnit::Ops;
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
  if (invalidReason == "backpressure") {
    throw BackpressureInvalidRun();
  }
  if (invalidReason == "zero-checksum") {
    throw ZeroChecksumInvalidRun();
  }
  return result;
}

} // namespace spot_loadtest::driver
