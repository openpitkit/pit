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

#pragma once

#include "spot_loadtest/decimal.hpp"

#include <chrono>
#include <cstdint>
#include <string>
#include <vector>

// Abstract, typed, serialisable load-test events plus their predictions.
//
// The predictions (accept/reason/post) ARE the oracle: the driver asserts the
// engine reproduces them exactly, per account, in account event order.

namespace spot_loadtest::generator {

// The order side. Defined here (not imported from the binding) so the generator
// stays engine-independent.
enum class Side : std::uint8_t { Buy, Sell };

[[nodiscard]] std::string ToString(Side side);

// The generator's own reject taxonomy (a local enum, not the binding's
// reject::Code) so the generator never imports the engine.
enum class RejectReason : std::uint8_t {
  None,
  InsufficientFunds,
  AccountAssetNotConfigured,
};

[[nodiscard]] std::string ToString(RejectReason reason);

// Selects the adjustment semantics for a funding event.
enum class FundingKind : std::uint8_t { Absolute, Delta };

[[nodiscard]] std::string ToString(FundingKind kind);

// Discriminates the abstract event types in the pre-materialised stream.
enum class EventKind : std::uint8_t { OrderCheck, Settlement, Funding };

[[nodiscard]] std::string ToString(EventKind kind);

// A predicted (available, held) pair for one asset after an op: the oracle's
// expectation for the engine's post-op state.
struct Balance {
  std::string asset;
  Decimal available;
  Decimal held;
};

// One abstract, typed load-test event. A single struct carries every kind;
// unused fields stay at their zero value.
struct Event {
  std::uint64_t seq = 0;
  EventKind kind = EventKind::OrderCheck;

  // The event's intended arrival on the offline virtual causal timeline,
  // measured from run start. What the driver paces to (open-loop) and stamps as
  // the measured t0.
  std::chrono::nanoseconds virtualT0{0};

  std::string account;

  // Order / settlement fields (zero for funding).
  std::string underlying;
  std::string settlement;
  Side side = Side::Buy;
  Decimal quantity;
  Decimal price;
  std::uint64_t correlationId = 0;

  // Order-check prediction.
  bool accept = false;
  RejectReason reason = RejectReason::None;

  // Funding fields (zero for order/settlement).
  FundingKind fundingKind = FundingKind::Absolute;
  std::string fundingAsset;
  Decimal fundingAmount;
  bool fundingIsSeed = false;

  // The predicted post-op balances (charge/held leg first for a stable order).
  std::vector<Balance> post;

  [[nodiscard]] bool FundingIsDelta() const noexcept {
    return fundingKind == FundingKind::Delta;
  }
};

// Aggregate counts over the generated stream.
struct StreamStats {
  std::uint64_t orderChecks = 0;
  std::uint64_t accepts = 0;
  std::uint64_t rejects = 0;
  std::uint64_t settlements = 0;
  std::uint64_t fundings = 0;
  std::uint64_t forcedRejects = 0;
  std::uint64_t naturalRejects = 0;
  std::uint64_t seeds = 0;

  [[nodiscard]] double PredictedRejectRate() const {
    if (orderChecks == 0) {
      return 0.0;
    }
    return static_cast<double>(rejects) / static_cast<double>(orderChecks);
  }
};

// The pre-materialised, deterministic sequence of events plus summary metadata.
struct Stream {
  std::vector<Event> events;
  StreamStats stats;

  // A deterministic, line-oriented text encoding (the determinism artifact the
  // property test hashes and compares).
  [[nodiscard]] std::string Serialize() const;
};

} // namespace spot_loadtest::generator
