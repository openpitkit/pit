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

#include "spot_loadtest/reporter/reporter.hpp"

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/env/env.hpp"
#include "spot_loadtest/generator/event.hpp"
#include "spot_loadtest/measurement/snapshot.hpp"

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <ostream>
#include <string>
#include <vector>

namespace spot_loadtest::reporter {
namespace {

constexpr int kSeparatorWidth = 72;
constexpr double kPctScale = 100.0;
constexpr int kTrajectoryRuleWidthOC = 67;
constexpr int kTrajectoryRuleWidthSet = 58;

using nanoseconds = std::chrono::nanoseconds;

void P(std::ostream &out, const std::string &s) { out << s << '\n'; }

template <typename... Args>
void Pf(std::ostream &out, const char *fmt, Args... args) {
  char buf[1024];
  std::snprintf(buf, sizeof(buf), fmt, args...);
  out << buf << '\n';
}

[[nodiscard]] std::string Repeat(char c, int n) {
  return std::string(static_cast<std::size_t>(n), c);
}

// Renders a compact duration for the report table at seconds, ms, µs, and
// ns boundaries.
[[nodiscard]] std::string FmtDur(nanoseconds d) {
  const std::int64_t ns = d.count();
  char buf[32];
  if (ns <= 0) {
    return "0";
  }
  if (ns >= 1'000'000'000LL) {
    std::snprintf(buf, sizeof(buf), "%.3fs", static_cast<double>(ns) / 1e9);
  } else if (ns >= 1'000'000LL) {
    std::snprintf(buf, sizeof(buf), "%.3fms", static_cast<double>(ns) / 1e6);
  } else if (ns >= 1'000LL) {
    std::snprintf(buf, sizeof(buf), "%.1f\xC2\xB5s",
                  static_cast<double>(ns) / 1e3);
  } else {
    std::snprintf(buf, sizeof(buf), "%lldns", static_cast<long long>(ns));
  }
  return buf;
}

[[nodiscard]] std::string FmtDurSigned(nanoseconds d) {
  if (d.count() < 0) {
    return "-" + FmtDur(nanoseconds(-d.count()));
  }
  return FmtDur(d);
}

// Renders configuration durations in their compact canonical form. It differs
// from FmtDur, the report-table latency renderer: 2500ms becomes "2.5s" and a
// minute becomes "1m0s". Below one second it uses one fractional unit
// (ns/µs/ms); from one second it composes h/m/s with a fractional-second part.
[[nodiscard]] std::string FmtConfigDuration(nanoseconds dur) {
  std::int64_t d = dur.count();
  if (d == 0) {
    return "0s";
  }
  const bool neg = d < 0;
  std::uint64_t u = neg ? static_cast<std::uint64_t>(-(d + 1)) + 1ULL
                        : static_cast<std::uint64_t>(d);

  std::string buf;
  auto prepend = [&buf](char c) { buf.insert(buf.begin(), c); };
  auto prependStr = [&buf](const std::string &s) { buf.insert(0, s); };

  // Emits fractional digits in the chosen unit, trims trailing zeros, and
  // returns the remaining integer-unit value.
  auto fmtFrac = [&](std::uint64_t v, int prec) -> std::uint64_t {
    bool print = false;
    std::string frac;
    for (int i = 0; i < prec; ++i) {
      const int digit = static_cast<int>(v % 10);
      print = print || digit != 0;
      if (print) {
        frac.insert(frac.begin(), static_cast<char>('0' + digit));
      }
      v /= 10;
    }
    if (print) {
      frac.insert(frac.begin(), '.');
    }
    prependStr(frac);
    return v;
  };
  auto fmtInt = [&](std::uint64_t v) {
    if (v == 0) {
      prepend('0');
      return;
    }
    while (v > 0) {
      prepend(static_cast<char>('0' + static_cast<int>(v % 10)));
      v /= 10;
    }
  };

  if (u < 1'000'000'000ULL) {
    // Less than one second: format as ns / µs / ms with a fractional part. The
    // buffer is built right-to-left, so 's' (rightmost) is prepended first,
    // then the unit char(s) to its left, then the fraction, then the integer
    // part.
    int prec = 0;
    prepend('s');
    if (u < 1'000ULL) {
      prec = 0;
      prepend('n');
    } else if (u < 1'000'000ULL) {
      prec = 3;
      prependStr("\xC2\xB5"); // "µ"
    } else {
      prec = 6;
      prepend('m');
    }
    u = fmtFrac(u, prec);
    fmtInt(u);
  } else {
    prepend('s');
    u = fmtFrac(u, 9); // fractional seconds (nanosecond precision)
    fmtInt(u % 60);    // seconds (always printed)
    u /= 60;
    if (u > 0) {
      prepend('m');
      fmtInt(u % 60); // minutes
      u /= 60;
      if (u > 0) {
        prepend('h');
        fmtInt(u); // hours
      }
    }
  }

  if (neg) {
    prepend('-');
  }
  return buf;
}

// Renders a window interval as elapsed seconds from the monotonic run start,
// preserving the clock used for latency measurement.
[[nodiscard]] std::string FmtWallRange(measurement::Clock::time_point runStart,
                                       const measurement::WindowSnapshot &win) {
  if (win.wallStart == measurement::Clock::time_point{}) {
    return "";
  }
  const auto s = std::chrono::duration_cast<std::chrono::milliseconds>(
                     win.wallStart - runStart)
                     .count();
  const auto e = std::chrono::duration_cast<std::chrono::milliseconds>(
                     win.wallEnd - runStart)
                     .count();
  char buf[64];
  std::snprintf(buf, sizeof(buf), "+%.3fs-+%.3fs",
                static_cast<double>(s) / 1000.0,
                static_cast<double>(e) / 1000.0);
  return buf;
}

[[nodiscard]] bool ZeroChecksumInvalid(const measurement::Snapshot &snap) {
  const std::uint64_t resolved =
      snap.totalOrderChecks + snap.totalSettlements + snap.totalFundings;
  return resolved > 0 && snap.checksum == 0;
}

[[nodiscard]] std::vector<std::string>
InvalidReasons(const measurement::Snapshot &snap) {
  std::vector<std::string> reasons;
  if (snap.submitLagBreaches > 0) {
    char buf[96];
    std::snprintf(buf, sizeof(buf),
                  "submit scheduling lag limit exceeded x %llu",
                  static_cast<unsigned long long>(snap.submitLagBreaches));
    reasons.emplace_back(buf);
  }
  if (snap.backpressure > 0) {
    char buf[96];
    std::snprintf(buf, sizeof(buf), "dispatch backpressure (QueueLimit x %llu)",
                  static_cast<unsigned long long>(snap.backpressure));
    reasons.emplace_back(buf);
  }
  if (ZeroChecksumInvalid(snap)) {
    reasons.emplace_back("zero anti-DCE checksum on a non-empty run");
  }
  return reasons;
}

[[nodiscard]] std::string Join(const std::vector<std::string> &parts,
                               const std::string &sep) {
  std::string out;
  for (std::size_t i = 0; i < parts.size(); ++i) {
    if (i > 0) {
      out += sep;
    }
    out += parts[i];
  }
  return out;
}

[[nodiscard]] std::string SteadyStateLabel(const measurement::Snapshot &snap) {
  if (snap.warmupWindows == 0) {
    return "all windows (single window - no warmup exclusion possible)";
  }
  char buf[128];
  std::snprintf(buf, sizeof(buf),
                "windows 2-%zu (window 1 excluded as warmup: JIT + cache + "
                "engine ramp-up)",
                snap.windows.size());
  return buf;
}

void WritePercentiles(std::ostream &out, const measurement::Percentiles &p) {
  Pf(out, "    samples: %lld", static_cast<long long>(p.count));
  Pf(out, "    p50    : %s", FmtDur(p.p50).c_str());
  Pf(out, "    p90    : %s", FmtDur(p.p90).c_str());
  Pf(out, "    p99    : %s", FmtDur(p.p99).c_str());
  Pf(out, "    p99.9  : %s", FmtDur(p.p999).c_str());
  Pf(out, "    max    : %s", FmtDur(p.max).c_str());
}

void WriteHeadline(std::ostream &out, const measurement::Snapshot &snap) {
  P(out, "=== Headline: Open-Loop Order-Check Latency (intended arrival -> "
         "decision) ===");
  P(out, "");
  P(out, "  Open-loop latency-under-load: t0 is the event's intended arrival "
         "on the");
  P(out, "  virtual causal timeline (NOT the actual submit instant), so "
         "queueing and");
  P(out, "  stalls under load are counted, not omitted (coordinated-omission "
         "defence).");
  P(out, "");

  const measurement::Percentiles &oc = snap.steadyStateOrderCheck;
  Pf(out, "  Steady-state definition : %s", SteadyStateLabel(snap).c_str());
  P(out, "");
  Pf(out, "  Order-check p50 (steady-state, open-loop): %s",
     FmtDur(oc.p50).c_str());
  Pf(out, "  Order-check p99 (steady-state, open-loop): %s",
     FmtDur(oc.p99).c_str());
  P(out, "");
  P(out, "  Full tail (steady-state, so no warmup spike is hidden):");
  Pf(out, "    p99   : %s", FmtDur(oc.p99).c_str());
  Pf(out, "    p99.9 : %s", FmtDur(oc.p999).c_str());
  Pf(out, "    max   : %s", FmtDur(oc.max).c_str());
  P(out, "");
  P(out, "  All-run merged (includes warmup window - full picture):");
  Pf(out, "    p50   : %s", FmtDur(snap.orderCheck.p50).c_str());
  Pf(out, "    p99   : %s", FmtDur(snap.orderCheck.p99).c_str());
  Pf(out, "    p99.9 : %s", FmtDur(snap.orderCheck.p999).c_str());
  Pf(out, "    max   : %s", FmtDur(snap.orderCheck.max).c_str());
  P(out, "");
  Pf(out,
     "  Throughput (decided ops/s, separate saturation metric): %.0f ops/s",
     snap.throughput);
  Pf(out, "  Max in-flight (open-loop depth witness)               : %lld",
     static_cast<long long>(snap.maxInFlight));
  P(out, "");
}

void WriteEnvironment(std::ostream &out, const env::Env &e,
                      const config::Config &cfg) {
  P(out, "=== Environment ===");
  P(out, "");
  P(out, "Host:");
  Pf(out, "  cpu model  : %s", e.host.cpuModel.c_str());
  Pf(out, "  cores      : %d", e.host.cores);
  Pf(out, "  ram        : %s", e.host.ram.c_str());
  Pf(out, "  os         : %s", e.host.os.c_str());
  Pf(out, "  kernel     : %s", e.host.kernel.c_str());
  P(out, "");
  P(out, "C++ toolchain:");
  Pf(out, "  compiler   : %s", e.toolchain.compiler.c_str());
  Pf(out, "  std        : %s", e.toolchain.cppStd.c_str());
  Pf(out, "  arch       : %s", e.toolchain.targetArch.c_str());
  P(out, "");
  P(out, "Pit repository:");
  Pf(out, "  commit     : %s (%s)", e.pit.commit.c_str(),
     e.pit.DirtyStatus().c_str());
  P(out, "");
  P(out, "Core (native runtime):");
  Pf(out, "  version    : %s", e.core.version.c_str());
  Pf(out, "  profile    : %s", e.core.profile.c_str());
  Pf(out, "  opt_level  : %s", e.core.optLevel.c_str());
  Pf(out, "  debug_assertions : %s", e.core.debugAssertions ? "true" : "false");
  Pf(out, "  target     : %s", e.core.target.c_str());
  Pf(out, "  target_cpu : %s", e.core.targetCpu.c_str());
  Pf(out, "  lto        : %s", e.core.lto.c_str());
  Pf(out, "  build_profile_raw : %s", e.core.raw.c_str());
  P(out, "");
  P(out, "Run config:");
  Pf(out, "  config path: %s", cfg.path.c_str());
  Pf(out, "  config hash: %s (SHA-256)", cfg.hash.c_str());
  Pf(out, "  seed       : 0x%llX",
     static_cast<unsigned long long>(cfg.run.seed));
  Pf(out, "  total_ops  : %llu",
     static_cast<unsigned long long>(cfg.run.totalOps));
  Pf(out, "  window     : %llu ops",
     static_cast<unsigned long long>(cfg.run.window));
  Pf(out, "  observer   : %s", cfg.run.observer ? "true" : "false");
  P(out, "");
}

void WriteConcurrency(std::ostream &out, const measurement::Snapshot &snap,
                      const config::Config &cfg,
                      std::size_t actualSubmitterThreads) {
  P(out, "  Concurrency model (bounded active working set):");
  Pf(out, "    population (total accounts)   : %llu",
     static_cast<unsigned long long>(cfg.accounts.count));
  Pf(out, "    active working set (max hot)  : %llu",
     static_cast<unsigned long long>(cfg.concurrency.activeAccounts));
  Pf(out, "    configured submitter cap      : %llu",
     static_cast<unsigned long long>(cfg.concurrency.submitterWorkers));
  Pf(out, "    actual submitter threads      : %llu",
     static_cast<unsigned long long>(actualSubmitterThreads));
  Pf(out, "    maximum submit scheduling lag : %s",
     FmtConfigDuration(cfg.concurrency.maxSubmitLag).c_str());
  Pf(out, "    submit-lag limit breaches     : %llu",
     static_cast<unsigned long long>(snap.submitLagBreaches));
  if (cfg.accounts.count > 0) {
    Pf(out, "    active fraction of population : %.1f%%",
       static_cast<double>(cfg.concurrency.activeAccounts) /
           static_cast<double>(cfg.accounts.count) * kPctScale);
  }
  P(out, "");

  const config::AsyncEngine &engineCfg = cfg.asyncEngine;
  P(out, "  Engine dispatch sizing (resource knob, NOT sync semantics):");
  Pf(out, "    strategy                      : %s",
     config::ToString(engineCfg.strategy).c_str());
  if (engineCfg.strategy == config::AsyncEngineStrategy::Sharded) {
    Pf(out, "    sharded_workers               : %d", engineCfg.shardedWorkers);
  } else {
    if (engineCfg.maxQueues == 0) {
      P(out, "    max_queues (Dynamic capacity) : unlimited (0)");
    } else {
      Pf(out, "    max_queues (Dynamic capacity) : %llu",
         static_cast<unsigned long long>(engineCfg.maxQueues));
    }
    if (engineCfg.idleCleanup.count() == 0) {
      P(out, "    idle_cleanup (queue retire)   : disabled (0)");
    } else {
      Pf(out, "    idle_cleanup (queue retire)   : %s",
         FmtConfigDuration(engineCfg.idleCleanup).c_str());
    }
  }
  if (engineCfg.queueCapacity == 0) {
    P(out, "    queue_capacity (per-queue buf): default (1024)");
  } else {
    Pf(out, "    queue_capacity (per-queue buf): %d", engineCfg.queueCapacity);
  }
  if (engineCfg.slowSubmitThreshold.count() == 0) {
    P(out, "    slow_submit_threshold         : default (1m)");
  } else {
    Pf(out, "    slow_submit_threshold         : %s",
       FmtConfigDuration(engineCfg.slowSubmitThreshold).c_str());
  }
  P(out, "");

  if (snap.backpressure == 0) {
    P(out,
      "  Backpressure (QueueLimit submits): 0 (healthy - dispatch held the "
      "load)");
  } else {
    Pf(out,
       "  Backpressure (QueueLimit submits): %llu (dispatch capacity was "
       "exceeded; the run is degraded - raise max_queues or lower "
       "active_accounts)",
       static_cast<unsigned long long>(snap.backpressure));
  }

  if (snap.handoffStalls == 0) {
    P(out, "  Handoff stalls: 0 (finalizer pool kept up with the collector)");
  } else {
    Pf(out,
       "  Handoff stalls: %llu (DIAGNOSTIC - finalizer pool transiently lagged "
       "the collector and spilled to the off-path overflow; does NOT throttle "
       "the submit schedule or invalidate the run - raise the finalizer pool "
       "size if persistent)",
       static_cast<unsigned long long>(snap.handoffStalls));
  }

  if (snap.maxWorkOverflow == 0) {
    P(out,
      "  Submit->collector overflow (max depth): 0 (collectors kept up with "
      "submission)");
  } else {
    Pf(out,
       "  Submit->collector overflow (max depth): %d (DIAGNOSTIC - collectors "
       "lagged submission at peak; usually engine slowness correctly in the "
       "headline, but under host CPU starvation can fold collector-dispatch "
       "delay into the tail - cross-check throughput and engine-compute)",
       snap.maxWorkOverflow);
  }
  P(out, "");
}

void WriteWorkload(std::ostream &out, const measurement::Snapshot &snap,
                   const generator::StreamStats &streamStats,
                   const config::Config &cfg,
                   std::size_t actualSubmitterThreads) {
  P(out, "=== Workload ===");
  P(out, "");
  const std::uint64_t total = snap.totalOrderChecks + snap.totalSettlements;
  Pf(out, "  Total resolved ops      : %llu",
     static_cast<unsigned long long>(total));
  Pf(out, "  Order-checks            : %llu",
     static_cast<unsigned long long>(snap.totalOrderChecks));
  Pf(out, "    accepts               : %llu",
     static_cast<unsigned long long>(snap.totalAccepts));
  Pf(out, "    rejects               : %llu",
     static_cast<unsigned long long>(snap.totalRejects));
  Pf(out, "  Settlements             : %llu",
     static_cast<unsigned long long>(snap.totalSettlements));
  Pf(out, "    accepts               : %llu",
     static_cast<unsigned long long>(snap.totalSettlementAccepts));
  Pf(out, "    blocked               : %llu",
     static_cast<unsigned long long>(snap.totalSettlementBlocks));
  if (snap.totalFundings > 0) {
    Pf(out, "  Funding adjustments     : %llu (accepted %llu / rejected %llu)",
       static_cast<unsigned long long>(snap.totalFundings),
       static_cast<unsigned long long>(snap.totalFundingAccepts),
       static_cast<unsigned long long>(snap.totalFundingRejects));
  }
  P(out, "");

  WriteConcurrency(out, snap, cfg, actualSubmitterThreads);

  Pf(out, "  Achieved reject rate    : %.4f (%.2f%%)", snap.achievedRejectRate,
     snap.achievedRejectRate * kPctScale);
  Pf(out, "  Target reject rate      : %.4f (%.2f%%)", cfg.reject.targetRate,
     cfg.reject.targetRate * kPctScale);
  double delta = snap.achievedRejectRate - cfg.reject.targetRate;
  if (delta < 0) {
    delta = -delta;
  }
  Pf(out, "  Rate deviation          : %.4f (tolerance ±%.4f)", delta,
     cfg.reject.tolerance);
  P(out, "");

  P(out, "  Generator stream summary (pre-run predictions):");
  Pf(out, "    order-checks  : %llu",
     static_cast<unsigned long long>(streamStats.orderChecks));
  Pf(out, "    accepts       : %llu",
     static_cast<unsigned long long>(streamStats.accepts));
  Pf(out, "    rejects       : %llu",
     static_cast<unsigned long long>(streamStats.rejects));
  Pf(out, "    settlements   : %llu",
     static_cast<unsigned long long>(streamStats.settlements));
  Pf(out, "    fundings      : %llu",
     static_cast<unsigned long long>(streamStats.fundings));
  Pf(out, "    forced rejects: %llu",
     static_cast<unsigned long long>(streamStats.forcedRejects));
  Pf(out, "    natural rej.  : %llu",
     static_cast<unsigned long long>(streamStats.naturalRejects));
  if (streamStats.orderChecks > 0) {
    Pf(out, "    predicted rej rate: %.4f", streamStats.PredictedRejectRate());
  }
  P(out, "");

  P(out, "  Cohorts:");
  double totalWeight = 0.0;
  for (const config::Cohort &c : cfg.cohorts) {
    totalWeight += c.weight;
  }
  for (const config::Cohort &c : cfg.cohorts) {
    double share = 0.0;
    if (totalWeight > 0) {
      share = c.weight / totalWeight * kPctScale;
    }
    Pf(out,
       "    [%s] weight=%.2f (%.1f%%), activity=%.2f, reject_propensity=%.2f, "
       "burst_len=%llu",
       c.name.c_str(), c.weight, share, c.activity, c.rejectPropensity,
       static_cast<unsigned long long>(c.burstLen));
  }
  P(out, "");

  if (snap.wallStartSet && snap.wallEnd > snap.wallStart) {
    const auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
                        snap.wallEnd - snap.wallStart)
                        .count();
    Pf(out, "  Wall time (first submit to last resolve): %.3fs",
       static_cast<double>(ms) / 1000.0);
  }
  P(out, "");
}

void WriteTrajectory(std::ostream &out, const measurement::Snapshot &snap) {
  P(out, "=== Trajectory (per-window percentiles) ===");
  P(out, "");
  if (snap.windows.empty()) {
    P(out, "  No windows recorded.");
    P(out, "");
    return;
  }

  const measurement::Clock::time_point runStart =
      snap.windows.front().wallStart;

  P(out, "  Order-check (open-loop: intended arrival -> decision, stage "
         "1->2):");
  P(out, "  win  | ops   | p50        | p99        | p99.9      | wall");
  P(out, "  " + Repeat('-', kTrajectoryRuleWidthOC));
  for (std::size_t i = 0; i < snap.windows.size(); ++i) {
    const measurement::WindowSnapshot &win = snap.windows[i];
    char label[8];
    std::snprintf(label, sizeof(label), "%4zu", i + 1);
    std::string labelStr = label;
    if (i == 0 && snap.windows.size() > 1) {
      labelStr += 'w';
    }
    Pf(out, "  %-5s| %-6lld| %-11s| %-11s| %-11s| %s", labelStr.c_str(),
       static_cast<long long>(win.orderCheck.count),
       FmtDur(win.orderCheck.p50).c_str(), FmtDur(win.orderCheck.p99).c_str(),
       FmtDur(win.orderCheck.p999).c_str(),
       FmtWallRange(runStart, win).c_str());
  }
  if (snap.windows.size() > 1) {
    P(out, "  (w = warmup window, excluded from steady-state headline)");
  }
  P(out, "");

  P(out, "  Settlement (open-loop: intended arrival -> decision, stage "
         "3->4):");
  P(out, "  win  | ops   | p50        | p99        | p99.9");
  P(out, "  " + Repeat('-', kTrajectoryRuleWidthSet));
  for (std::size_t i = 0; i < snap.windows.size(); ++i) {
    const measurement::WindowSnapshot &win = snap.windows[i];
    if (win.settlement.count == 0) {
      continue;
    }
    char label[8];
    std::snprintf(label, sizeof(label), "%4zu", i + 1);
    std::string labelStr = label;
    if (i == 0 && snap.windows.size() > 1) {
      labelStr += 'w';
    }
    Pf(out, "  %-5s| %-6lld| %-11s| %-11s| %s", labelStr.c_str(),
       static_cast<long long>(win.settlement.count),
       FmtDur(win.settlement.p50).c_str(), FmtDur(win.settlement.p99).c_str(),
       FmtDur(win.settlement.p999).c_str());
  }
  P(out, "");
}

void WriteDistribution(std::ostream &out, const measurement::Snapshot &snap) {
  P(out, "=== Distribution (final merged, all windows) ===");
  P(out, "");
  P(out, "  Order-check latency (stage 1->2, open-loop intended arrival -> "
         "decision, incl. queue):");
  WritePercentiles(out, snap.orderCheck);
  P(out, "");
  P(out, "  Settlement latency (stage 3->4, open-loop intended arrival -> "
         "decision):");
  WritePercentiles(out, snap.settlement);
  P(out, "");
  P(out, "  Harness self-overhead (adjustment-path FFI+queue floor, quiescent "
         "engine):");
  P(out, "    NOTE: probed via ApplyAccountAdjustment, NOT ExecutePreTrade; "
         "read it");
  P(out, "    as the bare FFI+queue floor, not the order-check overhead.");
  if (snap.overhead.probes == 0) {
    P(out, "    overhead probe disabled or not probed");
  } else {
    Pf(out, "    probes : %d", snap.overhead.probes);
    Pf(out, "    p50    : %s", FmtDur(snap.overhead.distribution.p50).c_str());
    Pf(out, "    p99    : %s", FmtDur(snap.overhead.distribution.p99).c_str());
    Pf(out, "    p99.9  : %s", FmtDur(snap.overhead.distribution.p999).c_str());
    Pf(out, "    max    : %s", FmtDur(snap.overhead.distribution.max).c_str());
  }
  P(out, "");
  Pf(out, "  Clamped samples (> hist max): %lld",
     static_cast<long long>(snap.clampedSamples));
  if (snap.clampedSamples > 0) {
    P(out,
      "    NOTE: the upper tail saturated at the histogram ceiling; those");
    P(out, "    samples are counted at the ceiling, so tail percentiles "
           "at/above");
    P(out, "    the ceiling are a LOWER BOUND on the true latency.");
  }
  P(out, "");
  Pf(out, "  Anti-DCE checksum: 0x%016llX  (proof every decision was consumed)",
     static_cast<unsigned long long>(snap.checksum));
  P(out, "");
}

void WriteServiceTime(std::ostream &out, const measurement::Snapshot &snap) {
  const measurement::Percentiles &st = snap.serviceTime;
  P(out, "  Service-time (resolve - ACTUAL submit), order-check:");
  P(out, "    DIAGNOSTIC, NOT the headline. It discounts queue wait before the "
         "actual");
  P(out, "    submit, so it hides the saturation tail; the headline is the "
         "open-loop");
  P(out, "    latency-under-load (intended arrival -> decision).");
  if (st.count == 0) {
    P(out, "    no samples");
  } else {
    Pf(out, "    samples: %lld", static_cast<long long>(st.count));
    Pf(out, "    p50    : %s", FmtDur(st.p50).c_str());
    Pf(out, "    p99    : %s", FmtDur(st.p99).c_str());
    Pf(out, "    p99.9  : %s", FmtDur(st.p999).c_str());
    Pf(out, "    max    : %s", FmtDur(st.max).c_str());
  }
  P(out, "");
}

void WriteSubmitLag(std::ostream &out, const measurement::Snapshot &snap) {
  const measurement::Percentiles &lag = snap.submitLag;
  P(out, "  Submit scheduling lag (ACTUAL submit - intended arrival):");
  P(out,
    "    HARNESS VALIDITY metric. A limit breach invalidates the run because");
  P(out, "    the configured open-loop arrival schedule was not sustained.");
  if (lag.count == 0) {
    P(out, "    no samples");
  } else {
    Pf(out, "    samples : %lld", static_cast<long long>(lag.count));
    Pf(out, "    p50     : %s", FmtDur(lag.p50).c_str());
    Pf(out, "    p99     : %s", FmtDur(lag.p99).c_str());
    Pf(out, "    p99.9   : %s", FmtDur(lag.p999).c_str());
    Pf(out, "    max     : %s", FmtDur(lag.max).c_str());
    Pf(out, "    breaches: %llu",
       static_cast<unsigned long long>(snap.submitLagBreaches));
  }
  P(out, "");
}

void WriteDiagnostics(std::ostream &out, const measurement::Snapshot &snap,
                      const config::Config &cfg) {
  P(out, "=== Diagnostics (decomposition, NOT the headline) ===");
  P(out, "");
  WriteSubmitLag(out, snap);
  WriteServiceTime(out, snap);

  if (!cfg.run.observer) {
    P(out, "  Observer disabled (observer = off in config). No inner metrics.");
    P(out, "  To enable: set observer = on in [run] section of the config.");
    P(out, "");
    return;
  }

  const measurement::InnerMetrics &im = snap.innerMetrics;
  P(out, "  NOTE: these are per-account AGGREGATE distributions, not "
         "per-order.");
  P(out, "  The residual is an approximation (aggregate, not per-op "
         "subtraction).");
  P(out, "");

  P(out, "  Queue wait (time a task spent in the async engine queue):");
  if (im.queueWait.count == 0) {
    P(out, "    no samples (observer may not have fired)");
  } else {
    Pf(out, "    callbacks: %lld", static_cast<long long>(im.dequeues));
    Pf(out, "    p50      : %s", FmtDur(im.queueWait.p50).c_str());
    Pf(out, "    p99      : %s", FmtDur(im.queueWait.p99).c_str());
    Pf(out, "    p99.9    : %s", FmtDur(im.queueWait.p999).c_str());
    Pf(out, "    max      : %s", FmtDur(im.queueWait.max).c_str());
  }
  P(out, "");

  P(out, "  Engine compute (wall time of the engine call inside the queue):");
  if (im.engineCompute.count == 0) {
    P(out, "    no samples");
  } else {
    Pf(out, "    callbacks: %lld", static_cast<long long>(im.completes));
    Pf(out, "    p50      : %s", FmtDur(im.engineCompute.p50).c_str());
    Pf(out, "    p99      : %s", FmtDur(im.engineCompute.p99).c_str());
    Pf(out, "    p99.9    : %s", FmtDur(im.engineCompute.p999).c_str());
    Pf(out, "    max      : %s", FmtDur(im.engineCompute.max).c_str());
  }
  P(out, "");

  if (im.queueWait.count > 0 && im.engineCompute.count > 0) {
    const nanoseconds residualP50 =
        snap.orderCheck.p50 - (im.queueWait.p50 + im.engineCompute.p50);
    const nanoseconds residualP99 =
        snap.orderCheck.p99 - (im.queueWait.p99 + im.engineCompute.p99);
    P(out, "  Aggregate FFI+handoff residual (APPROXIMATE - aggregate "
           "arithmetic,");
    P(out, "  NOT per-op subtraction; interpret with care):");
    P(out, "    residual p50 = order_check.p50 - (queue_wait.p50 + "
           "engine_compute.p50)");
    Pf(out, "               = %s - (%s + %s) = %s",
       FmtDur(snap.orderCheck.p50).c_str(), FmtDur(im.queueWait.p50).c_str(),
       FmtDur(im.engineCompute.p50).c_str(), FmtDurSigned(residualP50).c_str());
    Pf(out, "    residual p99 = %s - (%s + %s) = %s",
       FmtDur(snap.orderCheck.p99).c_str(), FmtDur(im.queueWait.p99).c_str(),
       FmtDur(im.engineCompute.p99).c_str(), FmtDurSigned(residualP99).c_str());
  }
  P(out, "");

  P(out, "  Queue lifecycle:");
  Pf(out, "    queues created: %lld", static_cast<long long>(im.queuesCreated));
  Pf(out, "    queues removed: %lld", static_cast<long long>(im.queuesRemoved));
  P(out, "");
}

void WriteDisclaimer(std::ostream &out, const std::string &configFlag) {
  P(out, "=== Disclaimer ===");
  P(out, "");
  P(out, "What IS measured (HEADLINE = open-loop latency-under-load):");
  P(out,
    "  intended-arrival -> decision latency for the pre-trade order-check");
  P(out, "  (ExecutePreTrade), including the per-account async queue wait, "
         "through the");
  P(out, "  C++ FFI boundary. Both order-check (stage 1->2) and "
         "report-settlement");
  P(out, "  (stage 3->4) are measured this way.");
  P(out, "");
  P(out, "  The harness is TRUE OPEN-LOOP. The generator assigns every event a "
         "virtual");
  P(out, "  arrival time on an offline CAUSAL timeline: order-check arrivals "
         "follow the");
  P(out, "  offered process; a settlement is its order's arrival plus a "
         "report-return");
  P(out, "  delay; a causally-dependent order follows its dependency's "
         "hold/fill. The");
  P(out, "  driver paces each event to its virtual arrival and submits without "
         "ever");
  P(out, "  blocking on a decision, so submissions pipeline (many ops in "
         "flight per");
  P(out, "  account). t0 is that virtual arrival, stamped independently of the "
         "actual");
  P(out, "  submit, so any queueing or stall under load is COUNTED, not "
         "omitted");
  P(out, "  (coordinated-omission defence). The per-op oracle stays strict: "
         "the engine");
  P(out, "  is FIFO-per-account with in-place holds, so the single live run "
         "reproduces");
  P(out, "  the shadow's offline-ordered decisions exactly.");
  P(out, "");
  P(out,
    "  The Diagnostics section also reports a SERVICE-TIME figure (resolve "
    "-");
  P(out, "  ACTUAL submit). That is a diagnostic only and is NEVER the "
         "headline: it");
  P(out, "  discounts the pre-submit queue wait and so hides the saturation "
         "tail.");
  P(out, "");
  P(out, "What is NOT measured:");
  P(out, "  client or TS network latency, serialization beyond the C++ binding "
         "boundary,");
  P(out, "  OS scheduling jitter beyond what the monotonic clock already "
         "captures,");
  P(out, "  and any TS-side processing other than the pit core.");
  P(out, "");
  P(out, "Reproduction recipe:");
  P(out, "  cd <repo>");
  P(out, "  cargo build --release");
  P(out, "  export OPENPIT_RUNTIME_LIBRARY=$(pwd)/target/release/"
         "libopenpit_ffi.<dylib|so>");
  P(out, "  cmake -S examples/cpp/spot_loadtest -B build "
         "-DCMAKE_BUILD_TYPE=Release");
  P(out, "  cmake --build build");
  Pf(out, "  ./build/spot_loadtest --config %s", configFlag.c_str());
  P(out, "");
  P(out, Repeat('-', kSeparatorWidth));
}

void WriteInvalidBanner(std::ostream &out, const measurement::Snapshot &snap) {
  const auto reasons = InvalidReasons(snap);
  Pf(out,
     "*** RUN INVALID: %s; latency numbers suppressed - this is not a valid "
     "measurement. ***",
     Join(reasons, "; ").c_str());
  P(out, "");
  if (snap.submitLagBreaches > 0) {
    P(out, "  Submit lag: the bounded submitter pool missed the configured");
    P(out,
      "  open-loop schedule. The observed latency no longer represents the");
    P(out, "  requested arrival rate, so the headline is suppressed.");
    P(out, "");
  }
  if (snap.backpressure > 0) {
    P(out,
      "  Backpressure: the engine refused one or more submits because the");
    P(out, "  live-queue cap was reached. Those ops never produced a decision, "
           "so the");
    P(out, "  latency sample is incomplete and skewed. Fix the dispatch sizing "
           "(raise");
    P(out, "  max_queues or lower active_accounts) and re-run.");
    P(out, "");
  }
  if (ZeroChecksumInvalid(snap)) {
    P(out, "  Zero checksum: the run resolved a non-empty set of decisions but "
           "the");
    P(out, "  anti-DCE checksum is zero, so the decisions were not provably "
           "consumed");
    P(out, "  (the measurement loop may have been optimized away). The numbers "
           "cannot be");
    P(out, "  trusted and the headline is suppressed.");
    P(out, "");
  }
  Pf(out, "  Submit-lag limit breaches            : %llu",
     static_cast<unsigned long long>(snap.submitLagBreaches));
  Pf(out, "  Maximum submit scheduling lag        : %s",
     FmtDur(snap.submitLag.max).c_str());
  Pf(out, "  Backpressure (QueueLimit submits)     : %llu",
     static_cast<unsigned long long>(snap.backpressure));
  Pf(out, "  Handoff stalls (harness starvation)   : %llu",
     static_cast<unsigned long long>(snap.handoffStalls));
  Pf(out, "  Anti-DCE checksum                     : 0x%016llX",
     static_cast<unsigned long long>(snap.checksum));
  P(out, "");
}

void WriteInvalidFooter(std::ostream &out, const measurement::Snapshot &snap,
                        const std::string &configFlag) {
  P(out, "=== Reproduction (after fixing the cause above) ===");
  P(out, "");
  P(out, "  cd <repo>");
  P(out, "  cargo build --release");
  P(out, "  export OPENPIT_RUNTIME_LIBRARY=$(pwd)/target/release/"
         "libopenpit_ffi.<dylib|so>");
  P(out, "  cmake -S examples/cpp/spot_loadtest -B build "
         "-DCMAKE_BUILD_TYPE=Release");
  P(out, "  cmake --build build");
  Pf(out, "  ./build/spot_loadtest --config %s", configFlag.c_str());
  P(out, "");
  Pf(out, "*** RUN INVALID: latency numbers suppressed (%s). ***",
     Join(InvalidReasons(snap), "; ").c_str());
  P(out, Repeat('-', kSeparatorWidth));
}

} // namespace

void Write(std::ostream &out, const env::Env &e, const config::Config &cfg,
           const std::string &configFlag, const measurement::Snapshot &snap,
           std::size_t actualSubmitterThreads,
           const generator::StreamStats &streamStats) {
  WriteHeadline(out, snap);
  WriteEnvironment(out, e, cfg);
  WriteWorkload(out, snap, streamStats, cfg, actualSubmitterThreads);
  WriteTrajectory(out, snap);
  WriteDistribution(out, snap);
  WriteDiagnostics(out, snap, cfg);
  WriteDisclaimer(out, configFlag);
}

void WriteInvalid(std::ostream &out, const env::Env &e,
                  const config::Config &cfg, const std::string &configFlag,
                  const measurement::Snapshot &snap,
                  std::size_t actualSubmitterThreads,
                  const generator::StreamStats &streamStats) {
  WriteInvalidBanner(out, snap);
  WriteEnvironment(out, e, cfg);
  WriteWorkload(out, snap, streamStats, cfg, actualSubmitterThreads);
  WriteSubmitLag(out, snap);
  WriteInvalidFooter(out, snap, configFlag);
}

} // namespace spot_loadtest::reporter
