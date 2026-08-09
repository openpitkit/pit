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

#include "spot_loadtest/generator/generator.hpp"

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/decimal.hpp"
#include "spot_loadtest/generator/cohort.hpp"
#include "spot_loadtest/generator/controller.hpp"
#include "spot_loadtest/generator/event.hpp"
#include "spot_loadtest/generator/ledger.hpp"
#include "spot_loadtest/generator/lifecycle.hpp"
#include "spot_loadtest/generator/money.hpp"
#include "spot_loadtest/generator/rng.hpp"

#include <array>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <limits>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <vector>

namespace spot_loadtest::generator {
namespace {

using std::chrono::nanoseconds;

// Deterministic per-instrument limit-price table, in integer cents (classic
// two-decimal equity ticks). The same config yields the same prices.
constexpr std::array<std::uint64_t, 10> kPriceCentsGrid = {
    5000, 7500, 10000, 12500, 15000, 20000, 25000, 30000, 42500, 99900};

// Bounds for how many wakes an admitted account stays active before it idles.
constexpr int kDwellWakesMin = 2;
constexpr int kDwellWakesSpread = 6;

// Small fixed spacing between an event and a causally-dependent successor on
// the same account on the virtual timeline.
constexpr nanoseconds kCausalGap = std::chrono::microseconds(1);

[[nodiscard]] Decimal PriceFromGrid(std::size_t symbolIndex) {
  return PriceFromCents(kPriceCentsGrid[symbolIndex % kPriceCentsGrid.size()]);
}

[[nodiscard]] std::map<std::string, Decimal>
AssignPrices(const std::vector<std::string> &symbols) {
  std::map<std::string, Decimal> prices;
  for (std::size_t i = 0; i < symbols.size(); ++i) {
    prices[symbols[i]] = PriceFromGrid(i);
  }
  return prices;
}

[[nodiscard]] nanoseconds MaxDuration(nanoseconds a, nanoseconds b) {
  return a > b ? a : b;
}

[[nodiscard]] nanoseconds CheckedAdd(nanoseconds left, nanoseconds right,
                                     const char *context) {
  const std::int64_t leftCount = left.count();
  const std::int64_t rightCount = right.count();
  constexpr std::int64_t kMin = std::numeric_limits<std::int64_t>::min();
  constexpr std::int64_t kMax = std::numeric_limits<std::int64_t>::max();
  if ((rightCount > 0 && leftCount > kMax - rightCount) ||
      (rightCount < 0 && leftCount < kMin - rightCount)) {
    throw std::overflow_error(std::string(context) +
                              " exceeds nanoseconds range");
  }
  return nanoseconds(leftCount + rightCount);
}

// One inter-arrival of the offered order-check process (exponential). When the
// offered rate is 0 the process is unpaced: return 0.
[[nodiscard]] nanoseconds InterArrival(Rng &vg, double rate) {
  if (rate <= 0) {
    return nanoseconds(0);
  }
  return nanoseconds(
      static_cast<std::int64_t>(vg.ExpFloat(rate) * 1e9)); // seconds -> ns.
}

struct ReportDelayParams {
  nanoseconds mean{0};
  double sigma = 0.0;
  config::ReportDelayDistribution dist = config::ReportDelayDistribution::None;
};

[[nodiscard]] ReportDelayParams
ResolveReportDelay(const config::ReportDelay &rd) {
  switch (rd.distribution) {
  case config::ReportDelayDistribution::None:
  case config::ReportDelayDistribution::Fixed:
  case config::ReportDelayDistribution::Lognormal:
    break;
  default:
    throw std::invalid_argument(
        "report_delay.distribution has unknown value " +
        std::to_string(static_cast<int>(rd.distribution)));
  }
  if (rd.mean.count() < 0) {
    throw std::invalid_argument("report_delay.mean must be non-negative");
  }
  if (!std::isfinite(rd.sigma) || rd.sigma < 0) {
    throw std::invalid_argument(
        "report_delay.sigma must be finite and non-negative");
  }
  if (rd.distribution == config::ReportDelayDistribution::None &&
      (rd.mean.count() != 0 || rd.sigma != 0.0)) {
    throw std::invalid_argument(
        "report_delay.mean and sigma must be zero when distribution is none");
  }
  if (rd.distribution == config::ReportDelayDistribution::Fixed &&
      rd.sigma != 0.0) {
    throw std::invalid_argument(
        "report_delay.sigma must be zero when distribution is fixed");
  }
  if (rd.distribution == config::ReportDelayDistribution::Lognormal &&
      rd.mean.count() == 0 && rd.sigma != 0.0) {
    throw std::invalid_argument(
        "report_delay.sigma must be zero when lognormal mean is zero");
  }
  ReportDelayParams out;
  out.mean = rd.mean;
  out.sigma = rd.sigma;
  out.dist = rd.distribution;
  return out;
}

// Samples the simulated report-return delay added to a settlement's virtual
// time. Zero mean -> 0. Fixed -> the mean. Lognormal -> a lognormal whose
// median is the mean: mean * exp(sigma * Z), Z ~ N(0,1).
[[nodiscard]] nanoseconds ReportDelaySample(Rng &vg,
                                            const ReportDelayParams &p) {
  switch (p.dist) {
  case config::ReportDelayDistribution::None:
    return nanoseconds(0);
  case config::ReportDelayDistribution::Fixed:
    return p.mean.count() <= 0 ? nanoseconds(0) : p.mean;
  case config::ReportDelayDistribution::Lognormal:
    if (p.mean.count() <= 0) {
      return nanoseconds(0);
    }
    break;
  default:
    throw std::invalid_argument("report_delay.distribution has unknown value " +
                                std::to_string(static_cast<int>(p.dist)));
  }
  const double z = vg.NormFloat();
  const double factor = std::exp(p.sigma * z);
  if (!std::isfinite(factor)) {
    throw std::overflow_error("report_delay sample exceeds nanoseconds range");
  }
  if (factor == 1.0) {
    return p.mean;
  }
  // The random factor is already double. Keeping the range decision in that
  // type avoids changing the result with the platform's long double ABI.
  const double delayNs = static_cast<double>(p.mean.count()) * factor;
  const double maxExclusiveNs = std::ldexp(1.0, 63);
  if (!std::isfinite(delayNs) || delayNs >= maxExclusiveNs) {
    throw std::overflow_error("report_delay sample exceeds nanoseconds range");
  }
  return nanoseconds(static_cast<std::int64_t>(delayNs));
}

// One slot of the bounded active working set.
struct ActiveSlot {
  int account = 0;
  int dwell = 0;
};

class GeneratorImpl {
public:
  explicit GeneratorImpl(const config::Config &cfg)
      : m_cfg(cfg), m_pop(Population::Build(cfg)), m_lifecycle(cfg.lifecycle),
        m_rng(Rng::NewContent(cfg.run.seed)),
        m_prices(AssignPrices(m_pop->symbols())),
        m_ctrl(cfg.reject.targetRate) {}

  [[nodiscard]] std::unique_ptr<Stream> Run() {
    SeedAccounts();
    RunLoop();
    AssignVirtualTimes();
    auto stream = std::make_unique<Stream>();
    stream->events = std::move(m_events);
    stream->stats = m_stats;
    return stream;
  }

private:
  const config::Config &m_cfg;
  std::unique_ptr<Population> m_pop;
  Ledger m_ledger;
  Lifecycle m_lifecycle;
  Rng m_rng;
  std::map<std::string, Decimal> m_prices;
  RejectController m_ctrl;

  std::vector<ActiveSlot> m_active;
  std::map<int, bool> m_activeIdx;

  std::vector<Event> m_events;
  StreamStats m_stats;
  std::uint64_t m_seq = 0;
  std::uint64_t m_corr = 0;

  [[nodiscard]] std::uint64_t NextSeq() { return m_seq++; }
  [[nodiscard]] std::uint64_t NextCorr() { return ++m_corr; }
  [[nodiscard]] bool IsActive(int i) const {
    auto it = m_activeIdx.find(i);
    return it != m_activeIdx.end() && it->second;
  }
  [[nodiscard]] int NextDwell() {
    return kDwellWakesMin + m_rng.IntN(kDwellWakesSpread);
  }

  void SeedAccounts() {
    const std::string &settle = m_cfg.instruments.settlement;
    const Decimal &seed = m_cfg.funding.seed;
    for (const Account &acc : m_pop->accounts()) {
      const FundingResult res =
          m_ledger.ApplyFunding(acc.id, settle, FundingKind::Absolute, seed);
      EmitFundingSeed(acc.id, settle, FundingKind::Absolute, seed, res, true);
    }
  }

  void InitActiveSet() {
    const int n = static_cast<int>(m_pop->accounts().size());
    int size = static_cast<int>(m_cfg.concurrency.activeAccounts);
    if (size <= 0 || size > n) {
      size = n;
    }
    m_active.resize(static_cast<std::size_t>(size));
    for (auto &slot : m_active) {
      const int acc =
          m_pop->AdmitAccount(m_rng, [this](int i) { return IsActive(i); });
      slot = ActiveSlot{acc, NextDwell()};
      m_activeIdx[acc] = true;
    }
  }

  void AdvanceSlot(int slot) {
    m_active[static_cast<std::size_t>(slot)].dwell--;
    if (m_active[static_cast<std::size_t>(slot)].dwell > 0) {
      return;
    }
    m_activeIdx.erase(m_active[static_cast<std::size_t>(slot)].account);
    int acc = m_pop->AdmitAccount(m_rng, [this](int i) { return IsActive(i); });
    if (acc < 0) {
      acc = m_active[static_cast<std::size_t>(slot)].account;
    }
    m_active[static_cast<std::size_t>(slot)] = ActiveSlot{acc, NextDwell()};
    m_activeIdx[acc] = true;
  }

  void RunLoop() {
    if (m_cfg.run.totalOps == 0) {
      throw std::invalid_argument("run.total_ops must be positive");
    }
    const std::uint64_t target = m_cfg.run.totalOps;
    InitActiveSet();
    while (m_stats.orderChecks < target) {
      const int slot = m_rng.IntN(static_cast<int>(m_active.size()));
      const Account &acc = m_pop->accounts()[static_cast<std::size_t>(
          m_active[static_cast<std::size_t>(slot)].account)];
      Wake(acc, target);
      AdvanceSlot(slot);
    }
  }

  void Wake(const Account &acc, std::uint64_t target) {
    const Cohort &co = m_pop->cohorts()[static_cast<std::size_t>(acc.cohort)];
    if (!m_rng.Bernoulli(co.cfg.activity)) {
      return;
    }
    for (std::uint64_t b = 0;
         b < co.cfg.burstLen && m_stats.orderChecks < target; ++b) {
      OneOrder(acc);
    }
  }

  void OneOrder(const Account &acc) {
    const Cohort &co = m_pop->cohorts()[static_cast<std::size_t>(acc.cohort)];

    const int symIdx = m_pop->PickSymbol(m_rng, acc.cohort);
    const std::string &underlying =
        m_pop->symbols()[static_cast<std::size_t>(symIdx)];
    const std::string &settle = m_cfg.instruments.settlement;
    const Decimal price = m_prices.at(underlying);

    MaybeTopUp(acc.id, settle);

    const Action act = m_lifecycle.Decide(m_rng, acc.id, underlying);
    if (act == Action::Idle) {
      return;
    }

    auto [side, lots] = SizeOrder(acc, underlying, settle, price, act);
    if (lots == 0) {
      return;
    }
    Decimal quantity = QuantityDecimal(lots);

    bool forced = false;
    if (side == Side::Buy &&
        m_ctrl.ShouldForce(m_rng, co.cfg.rejectPropensity)) {
      auto [lots2, ok] = OversizeBuy(acc.id, settle, price);
      if (ok) {
        lots = lots2;
        quantity = QuantityDecimal(lots);
        forced = true;
      }
    }

    const std::uint64_t corr = NextCorr();
    const PreTradeResult res =
        m_ledger.PreTrade(acc.id, side, underlying, settle, quantity, price);
    EmitOrderCheck(acc.id, underlying, settle, side, quantity, price, corr,
                   res);
    m_ctrl.Observe(res.Accepted());

    if (forced) {
      ++m_stats.forcedRejects;
    } else if (!res.Accepted()) {
      ++m_stats.naturalRejects;
    }

    if (!res.Accepted()) {
      return;
    }

    if (side == Side::Buy) {
      m_lifecycle.ApplyOpenOrAdd(acc.id, underlying, lots);
    } else {
      m_lifecycle.ApplyClose(acc.id, underlying, lots);
    }

    const SettlementResult settleRes = m_ledger.SettleFullFill(
        acc.id, side, underlying, settle, quantity, price);
    if (settleRes.error) {
      throw std::runtime_error("generator: settlement underflow for account " +
                               acc.id);
    }
    EmitSettlement(acc.id, underlying, settle, side, quantity, price, corr,
                   settleRes);
  }

  // Returns the side and lot count for an action.
  [[nodiscard]] std::pair<Side, std::uint64_t>
  SizeOrder(const Account &acc, const std::string &underlying,
            const std::string &settle, const Decimal &price, Action act) {
    switch (act) {
    case Action::Open:
    case Action::Add: {
      const std::uint64_t lots = m_pop->PickSize(m_rng, acc.cohort);
      return {Side::Buy, CapBuyToAvailable(acc.id, settle, price, lots)};
    }
    case Action::PartialClose:
      return {Side::Sell,
              m_lifecycle.CloseLots(m_rng, acc.id, underlying, false)};
    case Action::FullClose:
      return {Side::Sell,
              m_lifecycle.CloseLots(m_rng, acc.id, underlying, true)};
    default:
      return {Side::Buy, 0};
    }
  }

  // Reduces a desired Buy lot count so that q*p <= available; returns 0 when
  // even one lot does not fit.
  [[nodiscard]] std::uint64_t CapBuyToAvailable(const std::string &account,
                                                const std::string &settle,
                                                const Decimal &price,
                                                std::uint64_t lots) {
    const Decimal avail = m_ledger.Available(account, settle);
    if (price.IsZero()) {
      return lots;
    }
    const std::int64_t maxLots = avail.FloorDivToInt(price);
    if (maxLots <= 0) {
      return 0;
    }
    const auto maxU = static_cast<std::uint64_t>(maxLots);
    return lots > maxU ? maxU : lots;
  }

  // Returns a Buy lot count whose charge strictly exceeds available, so the
  // shadow pre-trade predicts InsufficientFunds.
  [[nodiscard]] std::pair<std::uint64_t, bool>
  OversizeBuy(const std::string &account, const std::string &settle,
              const Decimal &price) {
    if (price.IsZero()) {
      return {0, false};
    }
    const Decimal avail = m_ledger.Available(account, settle);
    const std::int64_t lots = avail.FloorDivToInt(price) + 1;
    if (lots <= 0) {
      return {0, false};
    }
    return {static_cast<std::uint64_t>(lots), true};
  }

  void MaybeTopUp(const std::string &account, const std::string &settle) {
    if (m_cfg.funding.trigger != config::FundingTrigger::BalanceBelow) {
      return;
    }
    auto [cur, exists] = m_ledger.Get(account, settle);
    if (exists && cur.available > m_cfg.funding.threshold) {
      return;
    }
    if (!exists) {
      const Decimal amount = m_cfg.funding.seed;
      const FundingResult res =
          m_ledger.ApplyFunding(account, settle, FundingKind::Absolute, amount);
      EmitFunding(account, settle, FundingKind::Absolute, amount, res);
      return;
    }
    const Decimal amount = m_cfg.funding.topUp;
    const FundingResult res =
        m_ledger.ApplyFunding(account, settle, FundingKind::Delta, amount);
    EmitFunding(account, settle, FundingKind::Delta, amount, res);
  }

  void EmitOrderCheck(const std::string &account, const std::string &underlying,
                      const std::string &settle, Side side,
                      const Decimal &quantity, const Decimal &price,
                      std::uint64_t corr, const PreTradeResult &res) {
    ++m_stats.orderChecks;
    if (res.Accepted()) {
      ++m_stats.accepts;
    } else {
      ++m_stats.rejects;
    }
    Event ev;
    ev.seq = NextSeq();
    ev.kind = EventKind::OrderCheck;
    ev.account = account;
    ev.underlying = underlying;
    ev.settlement = settle;
    ev.side = side;
    ev.quantity = quantity;
    ev.price = price;
    ev.correlationId = corr;
    ev.accept = res.Accepted();
    ev.reason = res.reason;
    ev.post.push_back(
        Balance{res.chargeAsset, res.postAvailable, res.postHeld});
    m_events.push_back(std::move(ev));
  }

  void EmitSettlement(const std::string &account, const std::string &underlying,
                      const std::string &settle, Side side,
                      const Decimal &quantity, const Decimal &price,
                      std::uint64_t corr, const SettlementResult &res) {
    ++m_stats.settlements;
    Event ev;
    ev.seq = NextSeq();
    ev.kind = EventKind::Settlement;
    ev.account = account;
    ev.underlying = underlying;
    ev.settlement = settle;
    ev.side = side;
    ev.quantity = quantity;
    ev.price = price;
    ev.correlationId = corr;
    ev.post.push_back(
        Balance{res.heldAsset, res.heldPost.available, res.heldPost.held});
    ev.post.push_back(Balance{res.creditAsset, res.creditPost.available,
                              res.creditPost.held});
    m_events.push_back(std::move(ev));
  }

  void EmitFunding(const std::string &account, const std::string &asset,
                   FundingKind kind, const Decimal &amount,
                   const FundingResult &res) {
    EmitFundingSeed(account, asset, kind, amount, res, false);
  }

  void EmitFundingSeed(const std::string &account, const std::string &asset,
                       FundingKind kind, const Decimal &amount,
                       const FundingResult &res, bool seed) {
    ++m_stats.fundings;
    if (seed) {
      ++m_stats.seeds;
    }
    Event ev;
    ev.seq = NextSeq();
    ev.kind = EventKind::Funding;
    ev.account = account;
    ev.fundingKind = kind;
    ev.fundingAsset = asset;
    ev.fundingAmount = amount;
    ev.fundingIsSeed = seed;
    ev.accept = !res.rejected;
    ev.reason = res.reason;
    ev.post.push_back(Balance{asset, res.post.available, res.post.held});
    m_events.push_back(std::move(ev));
  }

  // Stamps each event with a VirtualT0 on the offline
  // virtual causal timeline using a dedicated schedule RNG (decorrelated from
  // the content RNG), so the emitted content stays unchanged and only the
  // virtual times are added.
  void AssignVirtualTimes() {
    Rng vg = Rng::NewSchedule(m_cfg.run.seed);
    const double rate = static_cast<double>(m_cfg.arrival.offeredRate);
    const ReportDelayParams report = ResolveReportDelay(m_cfg.reportDelay);

    nanoseconds globalClock(0);
    std::unordered_map<std::string, nanoseconds> acctClock;
    std::unordered_map<std::uint64_t, nanoseconds> ocVirtualByCorr;

    for (Event &ev : m_events) {
      switch (ev.kind) {
      case EventKind::Funding: {
        if (ev.fundingIsSeed) {
          ev.virtualT0 = nanoseconds(0);
          continue;
        }
        const nanoseconds arrival =
            MaxDuration(globalClock, acctClock[ev.account]);
        ev.virtualT0 = arrival;
        acctClock[ev.account] =
            CheckedAdd(arrival, kCausalGap, "account causal deadline");
        break;
      }
      case EventKind::OrderCheck: {
        globalClock = CheckedAdd(globalClock, InterArrival(vg, rate),
                                 "virtual arrival timeline");
        const nanoseconds arrival =
            MaxDuration(globalClock, acctClock[ev.account]);
        ev.virtualT0 = arrival;
        ocVirtualByCorr[ev.correlationId] = arrival;
        acctClock[ev.account] =
            CheckedAdd(arrival, kCausalGap, "account causal deadline");
        break;
      }
      case EventKind::Settlement: {
        const nanoseconds oc = ocVirtualByCorr[ev.correlationId];
        const nanoseconds delay = ReportDelaySample(vg, report);
        const nanoseconds settle =
            CheckedAdd(oc, delay, "report settlement time");
        ev.virtualT0 = settle;
        acctClock[ev.account] = MaxDuration(
            acctClock[ev.account],
            CheckedAdd(settle, kCausalGap, "account causal deadline"));
        break;
      }
      }
    }
  }
};

} // namespace

std::unique_ptr<Stream> Generate(const config::Config &cfg) {
  try {
    GeneratorImpl gen(cfg);
    return gen.Run();
  } catch (const std::exception &ex) {
    throw std::runtime_error(std::string("generator: ") + ex.what());
  }
}

} // namespace spot_loadtest::generator
