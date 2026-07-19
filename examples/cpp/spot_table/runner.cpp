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

#include "runner.hpp"

#include "openpit/account_id.hpp"
#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts.hpp"
#include "openpit/async_engine.hpp"
#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/policies.hpp"
#include "openpit/reject.hpp"

#include <openpit.h>

#include <algorithm>
#include <cctype>
#include <functional>
#include <memory>
#include <mutex>
#include <sstream>
#include <thread>
#include <unordered_map>
#include <utility>
#include <vector>

#include "builder.hpp"
#include "marketdata.hpp"

namespace spot_table {

namespace param = openpit::param;
namespace model = openpit::model;
namespace md = openpit::marketdata;
namespace adj = openpit::accountadjustment;
namespace policies = openpit::pretrade::policies;
namespace ae = openpit::asyncengine;
using openpit::pretrade::Reject;
using openpit::pretrade::RejectCode;

namespace {

// Bounds graceful shutdown and reservation draining of the async engine.
constexpr std::chrono::seconds kAsyncStopTimeout{30};

// True once the run's deadline has passed; the mirror of `ctx.Err() != nil`.
[[nodiscard]] bool Expired(Deadline deadline) {
  return std::chrono::steady_clock::now() >= deadline;
}

//------------------------------------------------------------------------------

// The case-insensitive map from the table's `reject` column to the reject Code.
const std::unordered_map<std::string, RejectCode> &CodeNames() {
  static const std::unordered_map<std::string, RejectCode> table = {
      {"missingrequiredfield", RejectCode::MissingRequiredField},
      {"invalidfieldformat", RejectCode::InvalidFieldFormat},
      {"invalidfieldvalue", RejectCode::InvalidFieldValue},
      {"unsupportedordertype", RejectCode::UnsupportedOrderType},
      {"insufficientfunds", RejectCode::InsufficientFunds},
      {"insufficientmargin", RejectCode::InsufficientMargin},
      {"insufficientposition", RejectCode::InsufficientPosition},
      {"markpriceunavailable", RejectCode::MarkPriceUnavailable},
      {"ordervaluecalculationfailed", RejectCode::OrderValueCalculationFailed},
      {"accountadjustmentboundsexceeded",
       RejectCode::AccountAdjustmentBoundsExceeded},
  };
  return table;
}

// `strings.ToLower(strings.TrimSpace(name))`.
[[nodiscard]] std::string ToLowerTrimmed(const std::string &s) {
  const auto is_space = [](unsigned char c) { return std::isspace(c) != 0; };
  std::size_t begin = 0;
  std::size_t end = s.size();
  while (begin < end && is_space(static_cast<unsigned char>(s[begin]))) {
    ++begin;
  }
  while (end > begin && is_space(static_cast<unsigned char>(s[end - 1]))) {
    --end;
  }
  std::string out;
  out.reserve(end - begin);
  for (std::size_t i = begin; i < end; ++i) {
    out.push_back(
        static_cast<char>(std::tolower(static_cast<unsigned char>(s[i]))));
  }
  return out;
}

[[nodiscard]] std::optional<RejectCode> ResolveCode(const std::string &name) {
  const auto it = CodeNames().find(ToLowerTrimmed(name));
  if (it == CodeNames().end()) {
    return std::nullopt;
  }
  return it->second;
}

[[nodiscard]] bool ContainsCode(const std::vector<Reject> &rejects,
                                RejectCode want) {
  for (const Reject &r : rejects) {
    if (r.code == want) {
      return true;
    }
  }
  return false;
}

[[nodiscard]] std::string DescribeRejects(const std::vector<Reject> &rejects) {
  if (rejects.empty()) {
    return "";
  }
  std::vector<std::string> names;
  names.reserve(rejects.size());
  for (const Reject &r : rejects) {
    names.push_back(CodeName(r.code));
  }
  std::sort(names.begin(), names.end());
  std::ostringstream out;
  for (std::size_t i = 0; i < names.size(); ++i) {
    if (i != 0) {
      out << ",";
    }
    out << names[i];
  }
  return out.str();
}

//------------------------------------------------------------------------------
// `checkOrderVerdict` / `seedFillVerdictError`.

// Reports an expectation SEED/FILL cannot honor. ACCEPT is an ORDER-only
[[nodiscard]] std::optional<Failure> SeedFillVerdictError(const Row &row) {
  if (row.expect == "ACCEPT") {
    return Failure{row, row.action +
                            " row cannot use ACCEPT (ORDER-only); use OK or "
                            "REJECT"};
  }
  return Failure{row,
                 row.action + " row must use OK/REJECT, got " + row.expect};
}

// Compares a SEED outcome against the row's expected verdict. `rejected` is
[[nodiscard]] std::optional<Failure> CheckSeedVerdict(const Row &row,
                                                      bool rejected) {
  if (row.expect == "OK") {
    if (rejected) {
      return Failure{row, "expected OK, SEED rejected"};
    }
  } else if (row.expect == "REJECT") {
    if (!rejected) {
      return Failure{row, "expected REJECT, SEED accepted"};
    }
  } else {
    return SeedFillVerdictError(row);
  }
  return std::nullopt;
}

// Compares a FILL outcome against the row's expected verdict. `blocked` is
[[nodiscard]] std::optional<Failure> CheckFillVerdict(const Row &row,
                                                      bool blocked) {
  if (row.expect == "OK") {
    if (blocked) {
      return Failure{row, "expected OK, got account block"};
    }
  } else if (row.expect == "REJECT") {
    if (!blocked) {
      return Failure{row, "expected REJECT, FILL produced no block"};
    }
  } else {
    return SeedFillVerdictError(row);
  }
  return std::nullopt;
}

// `checkOrderVerdict`.
[[nodiscard]] std::optional<Failure>
CheckOrderVerdict(const Row &row, const std::vector<Reject> &rejects,
                  bool passed) {
  if (row.expect == "ACCEPT") {
    if (!passed) {
      return Failure{row, "expected ACCEPT, got REJECT(" +
                              DescribeRejects(rejects) + ")"};
    }
  } else if (row.expect == "REJECT") {
    if (passed) {
      return Failure{row, "expected REJECT, got ACCEPT"};
    }
    if (!row.reject.empty()) {
      const std::optional<RejectCode> wantCode = ResolveCode(row.reject);
      if (!wantCode.has_value()) {
        return Failure{row,
                       "unknown reject code \"" + row.reject + "\" in table"};
      }
      if (!ContainsCode(rejects, *wantCode)) {
        return Failure{row, "expected REJECT(" + row.reject + "), got REJECT(" +
                                DescribeRejects(rejects) + ")"};
      }
    }
  } else {
    return Failure{row, "ORDER row must use ACCEPT/REJECT, got " + row.expect};
  }
  return std::nullopt;
}

//------------------------------------------------------------------------------

// Aggregates every GROUP row into the set of accounts to register per group,
// `groupMembership`.
struct GroupMembership {
  std::vector<std::string> order; // group labels in first-seen order
  std::map<std::string, std::vector<param::AccountId>> members;
  std::vector<Row> rows; // every GROUP row, in table order

  [[nodiscard]] Row FirstRow(const std::string &label) const {
    for (const Row &row : rows) {
      if (row.group == label) {
        return row;
      }
    }
    return Row{};
  }

  // `countInReport`.
  void CountInReport(Report &report) const {
    for (const Row &row : rows) {
      report.total++;
      report.accounts[row.account]++;
    }
  }
};

// `collectGroups`.
[[nodiscard]] std::pair<GroupMembership, std::optional<Failure>>
CollectGroups(const std::vector<Row> &rows) {
  GroupMembership g;
  for (const Row &row : rows) {
    if (row.action != "GROUP") {
      continue;
    }
    param::AccountId acc;
    try {
      acc = AccountIdOf(row.account);
    } catch (const std::exception &err) {
      return {std::move(g), Failure{row, err.what()}};
    }
    if (g.members.find(row.group) == g.members.end()) {
      g.order.push_back(row.group);
    }
    g.members[row.group].push_back(acc);
    g.rows.push_back(row);
  }
  return {std::move(g), std::nullopt};
}

//------------------------------------------------------------------------------

// Replays one TICK row: a global push when neither account nor group is set,
// `pushTick`.
void PushTick(MarketFeed &feed, const Row &row) {
  if (row.account.empty() && row.group.empty()) {
    feed.Push(row.instrument, row.price);
    return;
  }
  std::vector<param::AccountId> accounts;
  if (!row.account.empty()) {
    accounts.push_back(AccountIdOf(row.account));
  }
  std::vector<param::AccountGroupId> groups;
  if (!row.group.empty()) {
    groups.push_back(AccountGroupIdOf(row.group));
  }
  feed.PushFor(row.instrument, row.price, accounts, groups);
}

//------------------------------------------------------------------------------
// Engine wiring.

// Owns one sync run's engine and market-data service. The feed (held by the
// runner) borrows the service; the engine must outlive the feed.
struct SyncEngineSet {
  md::Service service;
  openpit::Engine engine;
};

// Builds the Mode A engine: single-thread NoSync with the spot funds policy
[[nodiscard]] SyncEngineSet BuildSpotEngineSync(const Frontmatter &fm) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  md::Service service = md::Builder::FromEngineSyncPolicy(
                            md::QuoteTtl::Infinite(), openpit::SyncPolicy::None)
                            .Build();
  policies::SpotFundsPolicy policy;
  policy.WithMarketOrders(service.Get(), fm.slippageBps)
      .PricingSource(policies::SpotFundsPricingSource::Mark);
  builder.Add(policy);
  openpit::Engine engine = builder.Build();
  return SyncEngineSet{std::move(service), std::move(engine)};
}

//------------------------------------------------------------------------------

[[nodiscard]] std::optional<Failure>
RegisterGroupsSync(const openpit::Engine &engine, const GroupMembership &groups,
                   Report &report) {
  groups.CountInReport(report);
  const openpit::accounts::Accounts accountsView = engine.Accounts();
  for (const std::string &label : groups.order) {
    param::AccountGroupId groupId;
    try {
      groupId = AccountGroupIdOf(label);
    } catch (const std::exception &err) {
      return Failure{groups.FirstRow(label), err.what()};
    }
    const std::optional<openpit::accounts::AccountGroupError> groupErr =
        accountsView.RegisterGroup(groups.members.at(label), groupId);
    if (groupErr.has_value()) {
      return Failure{groups.FirstRow(label),
                     "register group: " + groupErr->message};
    }
  }
  return std::nullopt;
}

[[nodiscard]] std::optional<Failure> RunSyncTick(MarketFeed &feed,
                                                 const Row &row) {
  try {
    PushTick(feed, row);
  } catch (const std::exception &err) {
    return Failure{row, err.what()};
  }
  return std::nullopt;
}

[[nodiscard]] std::optional<Failure>
RunSyncSeed(openpit::Engine &engine, param::AccountId acc, const Row &row) {
  adj::AccountAdjustment adjustment;
  try {
    adjustment = BuildSeedAdjustment(row);
  } catch (const std::exception &err) {
    return Failure{row, err.what()};
  }
  openpit::AdjustmentResult result;
  try {
    std::vector<adj::AccountAdjustment> batch{std::move(adjustment)};
    result = engine.ApplyAccountAdjustment(acc, batch);
  } catch (const openpit::Error &err) {
    return Failure{row, std::string("engine: ") + err.what()};
  }
  return CheckSeedVerdict(row, !result.Passed());
}

[[nodiscard]] std::pair<std::optional<Failure>, std::chrono::nanoseconds>
RunSyncOrder(openpit::Engine &engine, param::AccountId acc, const Row &row) {
  model::Order order;
  try {
    order = BuildOrder(row, acc);
  } catch (const std::exception &err) {
    return {Failure{row, err.what()}, std::chrono::nanoseconds(0)};
  }
  const auto start = std::chrono::steady_clock::now();
  openpit::pretrade::ExecuteResult result;
  try {
    result = engine.ExecutePreTrade(order);
  } catch (const openpit::Error &err) {
    const auto dur = std::chrono::steady_clock::now() - start;
    return {Failure{row, std::string("engine: ") + err.what()}, dur};
  }
  const auto dur = std::chrono::steady_clock::now() - start;
  std::optional<Failure> fail =
      CheckOrderVerdict(row, result.rejects, result.Passed());
  if (result.reservation.has_value()) {
    if (!fail.has_value()) {
      result.reservation->Commit();
    } else {
      result.reservation->Rollback();
    }
  }
  return {fail, dur};
}

[[nodiscard]] std::pair<std::optional<Failure>, std::chrono::nanoseconds>
RunSyncFill(openpit::Engine &engine, param::AccountId acc, const Row &row,
            const MarketFeed &feed) {
  std::optional<FillReport> report;
  try {
    report.emplace(BuildFillReport(row, acc, feed));
  } catch (const std::exception &err) {
    return {Failure{row, err.what()}, std::chrono::nanoseconds(0)};
  }
  const auto start = std::chrono::steady_clock::now();
  openpit::PostTradeResult result;
  try {
    result = engine.ApplyExecutionReport(report->Report(), report->Lock());
  } catch (const openpit::Error &err) {
    const auto dur = std::chrono::steady_clock::now() - start;
    return {Failure{row, std::string("engine: ") + err.what()}, dur};
  }
  const auto dur = std::chrono::steady_clock::now() - start;
  return {CheckFillVerdict(row, !result.accountBlocks.empty()), dur};
}

} // namespace

//------------------------------------------------------------------------------
// LatencyStats / CodeName (public).

void LatencyStats::Observe(std::chrono::nanoseconds d) {
  count++;
  total += d;
  if (count == 1 || d < min) {
    min = d;
  }
  if (d > max) {
    max = d;
  }
}

std::chrono::nanoseconds LatencyStats::Avg() const {
  if (count == 0) {
    return std::chrono::nanoseconds(0);
  }
  return total / static_cast<std::chrono::nanoseconds::rep>(count);
}

void LatencyStats::Merge(const LatencyStats &o) {
  if (o.count == 0) {
    return;
  }
  if (count == 0 || o.min < min) {
    min = o.min;
  }
  if (o.max > max) {
    max = o.max;
  }
  count += o.count;
  total += o.total;
}

std::string CodeName(RejectCode code) {
  for (const auto &[name, value] : CodeNames()) {
    if (value == code) {
      return name;
    }
  }
  return "Code(" + std::to_string(static_cast<int>(code)) + ")";
}

//------------------------------------------------------------------------------
// RunSync.

Report RunSync(Deadline deadline, const Frontmatter &fm,
               const std::vector<Row> &rows) {
  SyncEngineSet set = BuildSpotEngineSync(fm);
  MarketFeed feed(set.service);
  feed.RegisterInstruments(rows);

  Report report;
  report.mode = Mode::Sync;

  auto [groups, collectFail] = CollectGroups(rows);
  if (collectFail.has_value()) {
    report.firstFail = collectFail;
    return report;
  }
  if (std::optional<Failure> fail =
          RegisterGroupsSync(set.engine, groups, report);
      fail.has_value()) {
    report.firstFail = fail;
    return report;
  }

  const auto start = std::chrono::steady_clock::now();
  for (const Row &row : rows) {
    if (Expired(deadline)) {
      break;
    }
    if (row.action == "GROUP") {
      // Registered up front in RegisterGroupsSync.
      continue;
    }
    if (row.action == "TICK") {
      if (std::optional<Failure> fail = RunSyncTick(feed, row);
          fail.has_value()) {
        report.firstFail = fail;
        break;
      }
      continue;
    }
    param::AccountId acc;
    try {
      acc = AccountIdOf(row.account);
    } catch (const std::exception &err) {
      report.firstFail = Failure{row, err.what()};
      break;
    }
    report.total++;
    report.accounts[row.account]++;

    if (row.action == "SEED") {
      if (std::optional<Failure> fail = RunSyncSeed(set.engine, acc, row);
          fail.has_value()) {
        report.firstFail = fail;
        break;
      }
    } else if (row.action == "ORDER") {
      auto [fail, d] = RunSyncOrder(set.engine, acc, row);
      report.order.Observe(d);
      if (fail.has_value()) {
        report.firstFail = fail;
        break;
      }
    } else if (row.action == "FILL") {
      auto [fail, d] = RunSyncFill(set.engine, acc, row, feed);
      report.fill.Observe(d);
      if (fail.has_value()) {
        report.firstFail = fail;
        break;
      }
    }
  }
  report.wallClock = std::chrono::steady_clock::now() - start;
  return report;
}

//------------------------------------------------------------------------------
// RunAsync.

namespace {

using AsyncEngine = ae::TypedAsyncEngine<ae::EngineAdapter>;

// Returns whichever failure sits on the earlier table row; a nullopt is "no
[[nodiscard]] std::optional<Failure> EarlierFailure(std::optional<Failure> a,
                                                    std::optional<Failure> b) {
  if (!a.has_value()) {
    return b;
  }
  if (!b.has_value()) {
    return a;
  }
  if (b->row.line < a->row.line) {
    return b;
  }
  return a;
}

// A row submitted to the async engine, paired with the future-await logic
//
// `await` resolves the step's verdict and finalizes any reservation; `wait`
// blocks until the engine call ran without scoring (used by a TICK barrier);
// `release` finalizes a reservation the verdict loop never reached. Each is an
// empty `std::function` when not applicable.
struct AsyncStep {
  Row row;
  std::function<std::optional<Failure>()> await;
  std::function<void()> wait;
  std::function<void()> release;
};

// Threads the per-account barrier bookkeeping and the per-operation latency
struct AsyncSubmission {
  MarketFeed *feed = nullptr;
  GroupMembership *groups = nullptr;
  Report *report = nullptr;
  Deadline deadline{};
  std::vector<AsyncStep> steps;
  // Fill reports outlive the worker closures that borrow them.
  std::vector<std::shared_ptr<FillReport>> fillReports;
  std::map<OpenPitParamAccountId, std::vector<std::function<void()>>> waiters;
  std::mutex statsMu;
  // report is read.
  std::vector<std::thread> timers;

  // Records one operation's submit-to-resolve latency as soon as its future
  template <typename FutureT>
  void ObserveOnResolve(FutureT future,
                        std::chrono::steady_clock::time_point start,
                        LatencyStats *stat) {
    AsyncSubmission *self = this;
    timers.emplace_back([self, future, start, stat]() mutable {
      try {
        // Block until resolved; ignore the value/exception here.
        static_cast<void>(future.Await());
      } catch (...) {
        // The verdict loop surfaces the failure; timing still records the
        // round-trip below.
      }
      const auto elapsed = std::chrono::steady_clock::now() - start;
      const std::lock_guard<std::mutex> lock(self->statsMu);
      stat->Observe(elapsed);
    });
  }

  void JoinTimers() {
    for (std::thread &t : timers) {
      if (t.joinable()) {
        t.join();
      }
    }
    timers.clear();
  }
};

// without a deadline (the engine resolves these registrations promptly), so the
// argument is presently unused.
[[nodiscard]] std::optional<Failure>
RegisterGroupsAsync([[maybe_unused]] Deadline deadline, AsyncEngine &engine,
                    const GroupMembership &groups, Report &report) {
  groups.CountInReport(report);
  ae::AsyncAccounts<ae::EngineAdapter> accountsView = engine.Accounts();
  for (const std::string &label : groups.order) {
    param::AccountGroupId groupId;
    try {
      groupId = AccountGroupIdOf(label);
    } catch (const std::exception &err) {
      return Failure{groups.FirstRow(label), err.what()};
    }
    try {
      const std::optional<openpit::accounts::AccountGroupError> groupErr =
          accountsView.RegisterGroup(groups.members.at(label), groupId).Await();
      if (groupErr.has_value()) {
        return Failure{groups.FirstRow(label),
                       "register group: " + groupErr->message};
      }
    } catch (const ae::Error &err) {
      return Failure{groups.FirstRow(label),
                     std::string("register group: ") + err.what()};
    }
  }
  return std::nullopt;
}

// Finalizes a passing reservation (commit) or any other (rollback), then closes
// `finalizeReservation`.
void FinalizeReservation(
    [[maybe_unused]] Deadline deadline,
    const std::shared_ptr<ae::AsyncReservation<ae::EngineAdapter>> &res,
    bool commit) {
  if (!res) {
    return;
  }
  try {
    if (commit) {
      static_cast<void>(res->CommitAndClose().Await());
    } else {
      static_cast<void>(res->RollbackAndClose().Await());
    }
  } catch (const ae::Error &) {
    // A stop/abort during finalization leaves the reservation released by its
    // wrapper's destruction; nothing more to do.
  }
}

[[nodiscard]] AsyncStep SubmitAsyncSeed([[maybe_unused]] Deadline deadline,
                                        AsyncEngine &engine,
                                        param::AccountId acc, const Row &row) {
  adj::AccountAdjustment adjustment = BuildSeedAdjustment(row);
  std::vector<adj::AccountAdjustment> batch{std::move(adjustment)};
  auto future = std::make_shared<ae::Future<ae::AdjustmentOutcome>>(
      engine.ApplyAccountAdjustment(acc, std::move(batch)));
  AsyncStep step;
  step.row = row;
  step.await = [future, row]() -> std::optional<Failure> {
    try {
      const ae::AdjustmentOutcome out = future->Await();
      return CheckSeedVerdict(row, !out.Passed());
    } catch (const ae::Error &err) {
      return Failure{row, std::string("engine: ") + err.what()};
    }
  };
  step.wait = [future]() {
    try {
      (void)future->Await();
    } catch (...) {
    }
  };
  return step;
}

// `submitOrder`.
[[nodiscard]] AsyncStep SubmitAsyncOrder(AsyncSubmission &s,
                                         AsyncEngine &engine,
                                         param::AccountId acc, const Row &row) {
  model::Order order = BuildOrder(row, acc);
  const auto start = std::chrono::steady_clock::now();
  auto future =
      std::make_shared<ae::Future<ae::ExecuteOutcome<ae::EngineAdapter>>>(
          engine.ExecutePreTrade(std::move(order)));
  s.ObserveOnResolve(*future, start, &s.report->order);

  // A shared once-guard so await / release resolve the reservation exactly
  // once.
  auto once = std::make_shared<std::once_flag>();
  const Deadline deadline = s.deadline;

  AsyncStep step;
  step.row = row;
  step.await = [future, once, deadline, row]() -> std::optional<Failure> {
    std::optional<Failure> fail;
    std::call_once(*once, [&]() {
      try {
        ae::ExecuteOutcome<ae::EngineAdapter> outcome = future->Await();
        fail = CheckOrderVerdict(row, outcome.rejects, outcome.Passed());
        FinalizeReservation(deadline, outcome.reservation, !fail.has_value());
      } catch (const ae::Error &err) {
        fail = Failure{row, std::string("engine: ") + err.what()};
      }
    });
    return fail;
  };
  step.wait = [future]() {
    try {
      (void)future->Await();
    } catch (...) {
    }
  };
  // release covers steps the verdict loop never awaited: roll back any
  // reservation the worker already resolved so it is closed exactly once.
  step.release = [future, once, deadline]() {
    std::call_once(*once, [&]() {
      try {
        ae::ExecuteOutcome<ae::EngineAdapter> outcome = future->Await();
        FinalizeReservation(deadline, outcome.reservation, false);
      } catch (const ae::Error &) {
      }
    });
  };
  return step;
}

[[nodiscard]] AsyncStep SubmitAsyncFill(AsyncSubmission &s, AsyncEngine &engine,
                                        param::AccountId acc, const Row &row) {
  auto fillReport =
      std::make_shared<FillReport>(BuildFillReport(row, acc, *s.feed));
  s.fillReports.push_back(fillReport);
  const auto start = std::chrono::steady_clock::now();
  auto future = std::make_shared<ae::Future<openpit::PostTradeResult>>(
      engine.Generic().Call(acc, [fillReport](ae::EngineAdapter &driver) {
        return driver.ApplyExecutionReport(fillReport->Report(),
                                           fillReport->Lock());
      }));
  s.ObserveOnResolve(*future, start, &s.report->fill);

  AsyncStep step;
  step.row = row;
  step.await = [future, row]() -> std::optional<Failure> {
    try {
      const openpit::PostTradeResult result = future->Await();
      return CheckFillVerdict(row, !result.accountBlocks.empty());
    } catch (const ae::Error &err) {
      return Failure{row, std::string("engine: ") + err.what()};
    }
  };
  step.wait = [future]() {
    try {
      (void)future->Await();
    } catch (...) {
    }
  };
  return step;
}

// The accounts whose outstanding operations a TICK must fence: the addressed
// `barrierAccounts`.
[[nodiscard]] std::vector<param::AccountId>
BarrierAccounts(const AsyncSubmission &s, const Row &row) {
  std::vector<param::AccountId> accounts;
  if (!row.account.empty()) {
    try {
      accounts.push_back(AccountIdOf(row.account));
    } catch (const std::exception &) {
    }
  }
  if (!row.group.empty()) {
    const auto it = s.groups->members.find(row.group);
    if (it != s.groups->members.end()) {
      for (const param::AccountId &member : it->second) {
        accounts.push_back(member);
      }
    }
  }
  return accounts;
}

// Fences the TICK's target accounts then publishes the quote. A global push has
// no fence (the determinism contract restricts it to the pre-order setup
void ReplayTick(AsyncSubmission &s, const Row &row) {
  if (row.account.empty() && row.group.empty()) {
    s.feed->Push(row.instrument, row.price);
    return;
  }
  for (const param::AccountId &acc : BarrierAccounts(s, row)) {
    const auto it = s.waiters.find(acc.Raw());
    if (it == s.waiters.end()) {
      continue;
    }
    for (const std::function<void()> &wait : it->second) {
      wait();
    }
  }
  PushTick(*s.feed, row);
}

// Submits every non-TICK row and replays addressed TICKs in order. A TICK
// `submitAsyncSteps`.
void SubmitAsyncSteps(Deadline deadline, AsyncEngine &engine,
                      AsyncSubmission &s, const std::vector<Row> &rows) {
  for (const Row &row : rows) {
    if (Expired(deadline)) {
      break;
    }
    if (row.action == "GROUP") {
      // Registered up front in RegisterGroupsAsync.
      continue;
    }
    if (row.action == "TICK") {
      try {
        ReplayTick(s, row);
      } catch (const std::exception &err) {
        s.report->firstFail = Failure{row, err.what()};
        return;
      }
      continue;
    }
    param::AccountId acc;
    try {
      acc = AccountIdOf(row.account);
    } catch (const std::exception &err) {
      s.report->firstFail = Failure{row, err.what()};
      break;
    }
    s.report->total++;
    s.report->accounts[row.account]++;

    AsyncStep step;
    if (row.action == "SEED") {
      step = SubmitAsyncSeed(deadline, engine, acc, row);
    } else if (row.action == "ORDER") {
      step = SubmitAsyncOrder(s, engine, acc, row);
    } else if (row.action == "FILL") {
      step = SubmitAsyncFill(s, engine, acc, row);
    }
    s.waiters[acc.Raw()].push_back(step.wait);
    s.steps.push_back(std::move(step));
  }
}

} // namespace

Report RunAsync(Deadline deadline, const Frontmatter &fm,
                const std::vector<Row> &rows) {
  // Build the AccountSync engine + FullSync market-data service. Mirrors
  openpit::EngineBuilder builder(openpit::SyncPolicy::Account);
  md::Service service =
      md::Builder::FromEngineSyncPolicy(md::QuoteTtl::Infinite(),
                                        openpit::SyncPolicy::Account)
          .Build(); // FromEngineSyncPolicy(Account) starts FullSync.
  MarketFeed feed(service);
  feed.RegisterInstruments(rows);

  policies::SpotFundsPolicy policy;
  policy.WithMarketOrders(service.Get(), fm.slippageBps)
      .PricingSource(policies::SpotFundsPricingSource::Mark);
  builder.Add(policy);
  openpit::Engine engine = builder.Build();
  // Wrap the engine in the typed async engine, Dynamic strategy (one serial
  // queue per account). The driver borrows the engine; both must outlive the
  // async engine.
  ae::EngineAdapter driver(engine);
  AsyncEngine asyncEngine =
      ae::TypedBuilder<ae::EngineAdapter>(driver).Dynamic().Build();

  Report report;
  report.mode = Mode::Async;

  auto [groups, collectFail] = CollectGroups(rows);
  if (collectFail.has_value()) {
    report.firstFail = collectFail;
    (void)asyncEngine.StopGraceful(kAsyncStopTimeout);
    return report;
  }
  if (std::optional<Failure> fail =
          RegisterGroupsAsync(deadline, asyncEngine, groups, report);
      fail.has_value()) {
    report.firstFail = fail;
    (void)asyncEngine.StopGraceful(kAsyncStopTimeout);
    return report;
  }

  AsyncSubmission s;
  s.feed = &feed;
  s.groups = &groups;
  s.report = &report;
  s.deadline = deadline;
  SubmitAsyncSteps(deadline, asyncEngine, s, rows);

  // A TICK replay failure during submission is recorded in report.firstFail;
  // the verdict loop below still finalizes the steps submitted before it. Keep
  // whichever failure sits on the earlier table row.
  const std::optional<Failure> submitFail = report.firstFail;
  const auto start = std::chrono::steady_clock::now();
  std::size_t awaited = 0;
  for (std::size_t i = 0; i < s.steps.size(); ++i) {
    if (Expired(deadline)) {
      break;
    }
    awaited = i + 1;
    if (std::optional<Failure> fail = s.steps[i].await(); fail.has_value()) {
      report.firstFail = fail;
      break;
    }
  }
  report.wallClock = std::chrono::steady_clock::now() - start;
  report.firstFail = EarlierFailure(report.firstFail, submitFail);

  // Drain steps the verdict loop never reached so each reservation is finalized
  for (std::size_t i = awaited; i < s.steps.size(); ++i) {
    if (s.steps[i].release) {
      s.steps[i].release();
    }
  }

  // Every operation has now resolved; wait for the latency timers to finish
  s.JoinTimers();

  // Graceful shutdown, then the service is released as the locals unwind
  // deferred StopGraceful + service.Close().
  (void)asyncEngine.StopGraceful(kAsyncStopTimeout);
  return report;
}

} // namespace spot_table
