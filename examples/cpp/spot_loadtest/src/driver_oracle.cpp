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

#include "driver_oracle.hpp"

#include "spot_loadtest/decimal.hpp"
#include "spot_loadtest/generator/event.hpp"

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/reject.hpp"

#include <map>
#include <mutex>
#include <optional>
#include <sstream>
#include <string>
#include <vector>

namespace spot_loadtest::driver::detail {
namespace {

// Maps a generator order-check reject reason to the engine reject code the
// oracle requires. On the v1 order path the only predicted reason is
// InsufficientFunds.
[[nodiscard]] bool OrderRejectCodeFor(generator::RejectReason reason,
                                      ::openpit::reject::RejectCode &outCode) {
  switch (reason) {
  case generator::RejectReason::InsufficientFunds:
    outCode = ::openpit::reject::RejectCode::InsufficientFunds;
    return true;
  default:
    return false;
  }
}

[[nodiscard]] bool
ContainsCode(const std::vector<::openpit::reject::Reject> &rejects,
             ::openpit::reject::RejectCode want) {
  for (const auto &r : rejects) {
    if (r.code == want) {
      return true;
    }
  }
  return false;
}

[[nodiscard]] std::string
DescribeRejects(const std::vector<::openpit::reject::Reject> &rejects) {
  if (rejects.empty()) {
    return "<none>";
  }
  std::string out;
  for (std::size_t i = 0; i < rejects.size(); ++i) {
    if (i > 0) {
      out += ",";
    }
    out += rejects[i].reason;
  }
  return out;
}

// Indexes the engine outcomes by asset for an exact per-leg balance compare.
[[nodiscard]] std::map<std::string,
                       ::openpit::accountadjustment::AccountOutcomeEntry>
OutcomesByAsset(
    const std::vector<::openpit::accountadjustment::Outcome> &outcomes) {
  std::map<std::string, ::openpit::accountadjustment::AccountOutcomeEntry> m;
  for (const auto &o : outcomes) {
    m[o.entry.asset] = o.entry;
  }
  return m;
}

// Asserts the engine's volunteered post-op available/held for one leg equal the
// predicted balance exactly (the SpotFundsPolicy sets the absolute fields to
// the true post-op available()/held()). Returns an error string on a mismatch.
[[nodiscard]] std::optional<std::string>
CompareLeg(const std::string &kind, const generator::Event &ev,
           const generator::Balance &want,
           const ::openpit::accountadjustment::AccountOutcomeEntry &got) {
  if (got.balance) {
    const Decimal engine =
        Decimal::FromString(got.balance->absolute.ToString());
    if (engine != want.available) {
      std::ostringstream os;
      os << "oracle: " << kind << " available mismatch: account " << ev.account
         << " seq " << ev.seq << " corr " << ev.correlationId << " asset "
         << want.asset << ": predicted " << want.available.ToString()
         << ", engine " << engine.ToString();
      return os.str();
    }
  }
  if (got.held) {
    const Decimal engine = Decimal::FromString(got.held->absolute.ToString());
    if (engine != want.held) {
      std::ostringstream os;
      os << "oracle: " << kind << " held mismatch: account " << ev.account
         << " seq " << ev.seq << " corr " << ev.correlationId << " asset "
         << want.asset << ": predicted " << want.held.ToString() << ", engine "
         << engine.ToString();
      return os.str();
    }
  }
  return std::nullopt;
}

} // namespace

void Oracle::CheckOrder(const generator::Event &ev,
                        const OrderObservation &obs) {
  std::lock_guard<std::mutex> lock(m_mutex);
  ++m_checked;

  if (ev.accept != obs.accepted) {
    std::ostringstream os;
    os << "oracle: order-check mismatch: account " << ev.account << " seq "
       << ev.seq << " corr " << ev.correlationId
       << ": predicted accept=" << (ev.accept ? "true" : "false")
       << ", engine accept=" << (obs.accepted ? "true" : "false")
       << " (engine rejects: " << DescribeRejects(obs.rejects) << ")";
    Fail(os.str());
    return;
  }

  if (!ev.accept) {
    auto wantCode = ::openpit::reject::RejectCode::Other;
    if (!OrderRejectCodeFor(ev.reason, wantCode)) {
      std::ostringstream os;
      os << "oracle: order-check reject with unmappable predicted reason: "
         << "account " << ev.account << " seq " << ev.seq << " corr "
         << ev.correlationId;
      Fail(os.str());
      return;
    }
    if (!ContainsCode(obs.rejects, wantCode)) {
      std::ostringstream os;
      os << "oracle: order-check reject-code mismatch: account " << ev.account
         << " seq " << ev.seq << " corr " << ev.correlationId
         << ": engine rejects: " << DescribeRejects(obs.rejects);
      Fail(os.str());
      return;
    }
  }
}

void Oracle::CheckSettlement(const generator::Event &ev,
                             const SettleObservation &obs) {
  std::lock_guard<std::mutex> lock(m_mutex);
  ++m_checked;

  if (obs.blocked) {
    std::ostringstream os;
    os << "oracle: settlement produced an account block: account " << ev.account
       << " seq " << ev.seq << " corr " << ev.correlationId
       << " (predicted clean settle)";
    Fail(os.str());
    return;
  }

  const auto got = OutcomesByAsset(obs.outcomes);
  for (const generator::Balance &want : ev.post) {
    auto it = got.find(want.asset);
    if (it == got.end()) {
      std::ostringstream os;
      os << "oracle: settlement missing engine outcome for asset " << want.asset
         << ": account " << ev.account << " seq " << ev.seq << " corr "
         << ev.correlationId;
      Fail(os.str());
      return;
    }
    if (auto err = CompareLeg("settlement", ev, want, it->second)) {
      Fail(*err);
      return;
    }
  }
}

void Oracle::CheckFunding(const generator::Event &ev,
                          const FundingObservation &obs) {
  std::lock_guard<std::mutex> lock(m_mutex);
  ++m_checked;

  const bool predictedReject = !ev.accept;
  if (predictedReject != obs.rejected) {
    std::ostringstream os;
    os << "oracle: funding decision mismatch: account " << ev.account << " seq "
       << ev.seq << " asset " << ev.fundingAsset
       << ": predicted reject=" << (predictedReject ? "true" : "false")
       << ", engine reject=" << (obs.rejected ? "true" : "false");
    Fail(os.str());
    return;
  }

  if (obs.rejected) {
    return;
  }

  const auto got = OutcomesByAsset(obs.outcomes);
  for (const generator::Balance &want : ev.post) {
    auto it = got.find(want.asset);
    if (it == got.end()) {
      std::ostringstream os;
      os << "oracle: funding missing engine outcome for asset " << want.asset
         << ": account " << ev.account << " seq " << ev.seq;
      Fail(os.str());
      return;
    }
    if (auto err = CompareLeg("funding", ev, want, it->second)) {
      Fail(*err);
      return;
    }
  }
}

std::optional<std::string>
Oracle::CheckInvariants(const std::vector<generator::Event> &events) {
  if (auto err = Err()) {
    return err;
  }

  std::map<AssetKey, Holdings> final;
  std::map<std::string, Decimal> expected;

  auto addExpected = [&](const std::string &asset, const Decimal &delta) {
    expected[asset] = expected[asset] + delta;
  };

  for (const generator::Event &ev : events) {
    switch (ev.kind) {
    case generator::EventKind::OrderCheck:
      for (const generator::Balance &b : ev.post) {
        final[AssetKey{ev.account, b.asset}] = Holdings{b.available, b.held};
      }
      break;
    case generator::EventKind::Settlement: {
      // q*p via a full scale-2 decimal multiply (the analogue of Go's
      // ev.Quantity.Mul(ev.Price)): exact at the pinned scales, and — unlike
      // MulInt(ToWholeInt()) never truncates a fractional quantity first.
      const Decimal notional = ev.quantity.Mul(ev.price);
      if (ev.side == generator::Side::Buy) {
        addExpected(ev.settlement, -notional);
        addExpected(ev.underlying, ev.quantity);
      } else {
        addExpected(ev.underlying, -ev.quantity);
        addExpected(ev.settlement, notional);
      }
      for (const generator::Balance &b : ev.post) {
        final[AssetKey{ev.account, b.asset}] = Holdings{b.available, b.held};
      }
      break;
    }
    case generator::EventKind::Funding:
      for (const generator::Balance &b : ev.post) {
        const AssetKey key{ev.account, b.asset};
        const Holdings prior = final.count(key) ? final[key] : Holdings{};
        if (ev.accept) {
          addExpected(b.asset, b.available - prior.available);
        }
        final[key] = Holdings{b.available, b.held};
      }
      break;
    }
  }

  // No-oversell + conservation over the predicted end state.
  std::map<std::string, Decimal> totals;
  for (const auto &[key, h] : final) {
    if (h.available.IsNegative()) {
      std::ostringstream os;
      os << "oracle: oversell invariant: account " << key.account << " asset "
         << key.asset << " available=" << h.available.ToString()
         << " is negative";
      return os.str();
    }
    if (h.held.IsNegative()) {
      std::ostringstream os;
      os << "oracle: oversell invariant: account " << key.account << " asset "
         << key.asset << " held=" << h.held.ToString() << " is negative";
      return os.str();
    }
    totals[key.asset] = totals[key.asset] + h.available + h.held;
  }

  std::map<std::string, bool> assets;
  for (const auto &[a, _] : totals) {
    assets[a] = true;
  }
  for (const auto &[a, _] : expected) {
    assets[a] = true;
  }
  for (const auto &[asset, _] : assets) {
    const Decimal total = totals.count(asset) ? totals[asset] : Decimal{};
    const Decimal exp = expected.count(asset) ? expected[asset] : Decimal{};
    if (total != exp) {
      std::ostringstream os;
      os << "oracle: conservation invariant: asset " << asset
         << " total available+held=" << total.ToString()
         << " != expected (funding+trade flow)=" << exp.ToString();
      return os.str();
    }
  }
  return std::nullopt;
}

} // namespace spot_loadtest::driver::detail
