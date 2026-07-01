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

#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/marketdata/instrument_id.hpp"
#include "openpit/marketdata/service.hpp"
#include "openpit/param.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

// Built-in pre-trade policy configurations.
//
// Each policy is configured by a small value object carrying its barriers, then
// registered on an `openpit::EngineBuilder` via `AddTo(builder)` (or, from the
// a config is assembled, then `Build(builder)` is called. Registration crosses
// the C boundary; failures (no barrier configured, already-consumed builder,
// argument parsing) surface as a thrown `openpit::Error`.
//
// Financial limits use `openpit::param` value types and assets are owned
// `std::string`; the raw C barrier arrays are materialized only inside `AddTo`,
// so every borrowed string view stays valid for the duration of the call.

namespace openpit::pretrade::policies {

// Base price the spot-funds policy uses to size market-order reservations.
// Mirrors the `pricing_source` byte of
// `openpit_engine_builder_add_builtin_spot_funds_policy`.
enum class SpotFundsPricingSource : std::uint8_t {
  Mark = 0,
  BookTop = 1,
};

// Runtime limit mode for spot-funds reservations.
enum class SpotFundsLimitMode : std::uint8_t {
  Enforce = OpenPitPretradePoliciesSpotFundsLimitMode_Enforce,
  TrackOnly = OpenPitPretradePoliciesSpotFundsLimitMode_TrackOnly,
};

inline constexpr std::string_view RateLimitPolicyName = "RateLimitPolicy";
inline constexpr std::string_view OrderSizeLimitPolicyName =
    "OrderSizeLimitPolicy";
inline constexpr std::string_view PnlBoundsKillSwitchPolicyName =
    "PnlBoundsKillSwitchPolicy";
inline constexpr std::string_view SpotFundsPolicyName = "SpotFundsPolicy";

namespace detail {

[[nodiscard]] inline std::int64_t ToRateLimitWindowNanoseconds(
    std::uint64_t value) {
  if (value >
      static_cast<std::uint64_t>(std::numeric_limits<std::int64_t>::max())) {
    throw ::openpit::Error("rate-limit window exceeds int64 nanoseconds");
  }
  return static_cast<std::int64_t>(value);
}

[[nodiscard]] inline ::openpit::param::PnlOptional PnlOptional(
    const std::optional<::openpit::param::Pnl>& value) noexcept {
  ::openpit::param::PnlOptional raw{};
  if (value) {
    raw.value = value->Raw();
    raw.is_set = true;
  }
  return raw;
}

}  // namespace detail

//------------------------------------------------------------------------------
// OrderSizeLimit

// Maximum quantity and notional for a single order.
struct OrderSizeLimit {
  ::openpit::param::Quantity maxQuantity;
  ::openpit::param::Volume maxNotional;

  OrderSizeLimit(::openpit::param::Quantity quantity,
                 ::openpit::param::Volume notional)
      : maxQuantity(quantity), maxNotional(notional) {}

  [[nodiscard]] OpenPitPretradePoliciesOrderSizeLimit Raw() const noexcept {
    OpenPitPretradePoliciesOrderSizeLimit raw{};
    raw.max_quantity = maxQuantity.Raw();
    raw.max_notional = maxNotional.Raw();
    return raw;
  }
};

// Broker-wide order-size barrier.
struct OrderSizeBrokerBarrier {
  OrderSizeLimit limit;

  explicit OrderSizeBrokerBarrier(OrderSizeLimit barrierLimit)
      : limit(barrierLimit) {}
};

// Per-settlement-asset order-size barrier.
struct OrderSizeAssetBarrier {
  OrderSizeLimit limit;
  std::string settlementAsset;

  OrderSizeAssetBarrier(OrderSizeLimit barrierLimit, std::string asset)
      : limit(barrierLimit), settlementAsset(std::move(asset)) {}
};

// Per-(account, settlement-asset) order-size barrier.
struct OrderSizeAccountAssetBarrier {
  OrderSizeLimit limit;
  ::openpit::param::AccountId accountId;
  std::string settlementAsset;

  OrderSizeAccountAssetBarrier(OrderSizeLimit barrierLimit,
                               ::openpit::param::AccountId account,
                               std::string asset)
      : limit(barrierLimit),
        accountId(account),
        settlementAsset(std::move(asset)) {}
};

// Built-in order-size-limit policy. At least one barrier axis must be
// configured before registration. Mirrors
// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
class OrderSizeLimitPolicy {
 public:
  OrderSizeLimitPolicy& PolicyGroupId(std::uint16_t policyGroupId) {
    m_policyGroupId = policyGroupId;
    return *this;
  }

  OrderSizeLimitPolicy& BrokerBarrier(OrderSizeBrokerBarrier barrier) {
    m_broker = barrier;
    return *this;
  }

  OrderSizeLimitPolicy& AssetBarrier(OrderSizeAssetBarrier barrier) {
    m_assetBarriers.push_back(std::move(barrier));
    return *this;
  }

  OrderSizeLimitPolicy& AccountAssetBarrier(
      OrderSizeAccountAssetBarrier barrier) {
    m_accountAssetBarriers.push_back(std::move(barrier));
    return *this;
  }

  // Registers the policy on `builder`. Throws `openpit::Error` on failure.
  void AddTo(::openpit::EngineBuilder& builder) const {
    OpenPitPretradePoliciesOrderSizeBrokerBarrier brokerRaw{};
    const OpenPitPretradePoliciesOrderSizeBrokerBarrier* brokerPtr = nullptr;
    if (m_broker) {
      brokerRaw.limit = m_broker->limit.Raw();
      brokerPtr = &brokerRaw;
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAssetBarrier> assetRaw;
    assetRaw.reserve(m_assetBarriers.size());
    for (const OrderSizeAssetBarrier& barrier : m_assetBarriers) {
      OpenPitPretradePoliciesOrderSizeAssetBarrier raw{};
      raw.limit = barrier.limit.Raw();
      raw.settlement_asset = ::openpit::MakeStringView(barrier.settlementAsset);
      assetRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAccountAssetBarrier>
        accountAssetRaw;
    accountAssetRaw.reserve(m_accountAssetBarriers.size());
    for (const OrderSizeAccountAssetBarrier& barrier : m_accountAssetBarriers) {
      OpenPitPretradePoliciesOrderSizeAccountAssetBarrier raw{};
      raw.limit = barrier.limit.Raw();
      raw.account_id = barrier.accountId.Raw();
      raw.settlement_asset = ::openpit::MakeStringView(barrier.settlementAsset);
      accountAssetRaw.push_back(raw);
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_order_size_limit_policy(
            builder.Get(), m_policyGroupId, brokerPtr, assetRaw.data(),
            assetRaw.size(), accountAssetRaw.data(), accountAssetRaw.size(),
            &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_order_size_limit_policy failed");
    }
  }

 private:
  std::optional<OrderSizeBrokerBarrier> m_broker;
  std::vector<OrderSizeAssetBarrier> m_assetBarriers;
  std::vector<OrderSizeAccountAssetBarrier> m_accountAssetBarriers;
  std::uint16_t m_policyGroupId = OPENPIT_DEFAULT_POLICY_GROUP_ID;
};

//------------------------------------------------------------------------------
// OrderValidation

// Built-in order-validation policy. Requires no barriers. Mirrors
// `openpit_engine_builder_add_builtin_order_validation_policy`.
class OrderValidationPolicy {
 public:
  OrderValidationPolicy& PolicyGroupId(std::uint16_t policyGroupId) {
    m_policyGroupId = policyGroupId;
    return *this;
  }

  void AddTo(::openpit::EngineBuilder& builder) const {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_order_validation_policy(
            builder.Get(), m_policyGroupId, &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_order_validation_policy failed");
    }
  }

 private:
  std::uint16_t m_policyGroupId = OPENPIT_DEFAULT_POLICY_GROUP_ID;
};

//------------------------------------------------------------------------------
// PnlBoundsKillSwitch

// Broker-level P&L bounds for one settlement asset, applied across all
// accounts. Lower bound is typically the negative loss limit; upper bound the
// positive profit-take limit. Either may be absent.
struct PnlBoundsBrokerBarrier {
  std::string settlementAsset;
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;

  explicit PnlBoundsBrokerBarrier(std::string asset)
      : settlementAsset(std::move(asset)) {}
};

// Per-(account, settlement-asset) P&L bounds with an initial accumulated-P&L
// seed.
struct PnlBoundsAccountBarrier {
  ::openpit::param::AccountId accountId;
  std::string settlementAsset;
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;
  ::openpit::param::Pnl initialPnl;

  PnlBoundsAccountBarrier(::openpit::param::AccountId account,
                          std::string asset, ::openpit::param::Pnl initial)
      : accountId(account),
        settlementAsset(std::move(asset)),
        initialPnl(initial) {}
};

// Runtime replacement for a per-(account, settlement-asset) P&L barrier.
// Unlike `PnlBoundsAccountBarrier`, it intentionally carries no initial P&L:
// runtime replacement preserves the live accumulator.
struct PnlBoundsAccountBarrierUpdate {
  ::openpit::param::AccountId accountId;
  std::string settlementAsset;
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;

  PnlBoundsAccountBarrierUpdate(::openpit::param::AccountId account,
                                std::string asset)
      : accountId(account), settlementAsset(std::move(asset)) {}
};

// Built-in P&L bounds kill-switch policy. At least one barrier (broker or
// account) must be configured. Mirrors
// `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`.
class PnlBoundsKillSwitchPolicy {
 public:
  PnlBoundsKillSwitchPolicy& PolicyGroupId(std::uint16_t policyGroupId) {
    m_policyGroupId = policyGroupId;
    return *this;
  }

  PnlBoundsKillSwitchPolicy& BrokerBarrier(PnlBoundsBrokerBarrier barrier) {
    m_brokerBarriers.push_back(std::move(barrier));
    return *this;
  }

  PnlBoundsKillSwitchPolicy& AccountBarrier(PnlBoundsAccountBarrier barrier) {
    m_accountBarriers.push_back(std::move(barrier));
    return *this;
  }

  void AddTo(::openpit::EngineBuilder& builder) const {
    std::vector<OpenPitPretradePoliciesPnlBoundsBarrier> brokerRaw;
    brokerRaw.reserve(m_brokerBarriers.size());
    for (const PnlBoundsBrokerBarrier& barrier : m_brokerBarriers) {
      OpenPitPretradePoliciesPnlBoundsBarrier raw{};
      raw.settlement_asset = ::openpit::MakeStringView(barrier.settlementAsset);
      raw.lower_bound = ::openpit::pretrade::policies::detail::PnlOptional(
          barrier.lowerBound);
      raw.upper_bound = ::openpit::pretrade::policies::detail::PnlOptional(
          barrier.upperBound);
      brokerRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesPnlBoundsAccountBarrier> accountRaw;
    accountRaw.reserve(m_accountBarriers.size());
    for (const PnlBoundsAccountBarrier& barrier : m_accountBarriers) {
      OpenPitPretradePoliciesPnlBoundsAccountBarrier raw{};
      raw.account_id = barrier.accountId.Raw();
      raw.settlement_asset = ::openpit::MakeStringView(barrier.settlementAsset);
      raw.lower_bound = ::openpit::pretrade::policies::detail::PnlOptional(
          barrier.lowerBound);
      raw.upper_bound = ::openpit::pretrade::policies::detail::PnlOptional(
          barrier.upperBound);
      raw.initial_pnl = barrier.initialPnl.Raw();
      accountRaw.push_back(raw);
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
            builder.Get(), m_policyGroupId, brokerRaw.data(), brokerRaw.size(),
            accountRaw.data(), accountRaw.size(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy "
          "failed");
    }
  }

 private:
  std::vector<PnlBoundsBrokerBarrier> m_brokerBarriers;
  std::vector<PnlBoundsAccountBarrier> m_accountBarriers;
  std::uint16_t m_policyGroupId = OPENPIT_DEFAULT_POLICY_GROUP_ID;
};

//------------------------------------------------------------------------------
// RateLimit

// Maximum number of orders accepted within a sliding window. The window is
// expressed in nanoseconds to match the native runtime.
struct RateLimit {
  std::size_t maxOrders = 0;
  std::uint64_t windowNanoseconds = 0;

  RateLimit(std::size_t orders, std::uint64_t windowNanos)
      : maxOrders(orders), windowNanoseconds(windowNanos) {}
};

// Broker-wide rate-limit barrier.
struct RateLimitBrokerBarrier {
  RateLimit limit;

  explicit RateLimitBrokerBarrier(RateLimit barrierLimit)
      : limit(barrierLimit) {}
};

// Per-settlement-asset rate-limit barrier.
struct RateLimitAssetBarrier {
  RateLimit limit;
  std::string settlementAsset;

  RateLimitAssetBarrier(RateLimit barrierLimit, std::string asset)
      : limit(barrierLimit), settlementAsset(std::move(asset)) {}
};

// Per-account rate-limit barrier.
struct RateLimitAccountBarrier {
  RateLimit limit;
  ::openpit::param::AccountId accountId;

  RateLimitAccountBarrier(RateLimit barrierLimit,
                          ::openpit::param::AccountId account)
      : limit(barrierLimit), accountId(account) {}
};

// Per-(account, settlement-asset) rate-limit barrier.
struct RateLimitAccountAssetBarrier {
  RateLimit limit;
  ::openpit::param::AccountId accountId;
  std::string settlementAsset;

  RateLimitAccountAssetBarrier(RateLimit barrierLimit,
                               ::openpit::param::AccountId account,
                               std::string asset)
      : limit(barrierLimit),
        accountId(account),
        settlementAsset(std::move(asset)) {}
};

// Built-in rate-limit policy. At least one barrier axis must be configured.
// Mirrors `openpit_engine_builder_add_builtin_rate_limit_policy`.
class RateLimitPolicy {
 public:
  RateLimitPolicy& PolicyGroupId(std::uint16_t policyGroupId) {
    m_policyGroupId = policyGroupId;
    return *this;
  }

  RateLimitPolicy& BrokerBarrier(RateLimitBrokerBarrier barrier) {
    m_broker = barrier;
    return *this;
  }

  RateLimitPolicy& AssetBarrier(RateLimitAssetBarrier barrier) {
    m_assetBarriers.push_back(std::move(barrier));
    return *this;
  }

  RateLimitPolicy& AccountBarrier(RateLimitAccountBarrier barrier) {
    m_accountBarriers.push_back(barrier);
    return *this;
  }

  RateLimitPolicy& AccountAssetBarrier(RateLimitAccountAssetBarrier barrier) {
    m_accountAssetBarriers.push_back(std::move(barrier));
    return *this;
  }

  void AddTo(::openpit::EngineBuilder& builder) const {
    OpenPitPretradePoliciesRateLimitBrokerBarrier brokerRaw{};
    const OpenPitPretradePoliciesRateLimitBrokerBarrier* brokerPtr = nullptr;
    if (m_broker) {
      brokerRaw.max_orders = m_broker->limit.maxOrders;
      brokerRaw.window_nanoseconds =
          ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
              m_broker->limit.windowNanoseconds);
      brokerPtr = &brokerRaw;
    }

    std::vector<OpenPitPretradePoliciesRateLimitAssetBarrier> assetRaw;
    assetRaw.reserve(m_assetBarriers.size());
    for (const RateLimitAssetBarrier& barrier : m_assetBarriers) {
      OpenPitPretradePoliciesRateLimitAssetBarrier raw{};
      raw.settlement_asset = ::openpit::MakeStringView(barrier.settlementAsset);
      raw.max_orders = barrier.limit.maxOrders;
      raw.window_nanoseconds =
          ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
              barrier.limit.windowNanoseconds);
      assetRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountBarrier> accountRaw;
    accountRaw.reserve(m_accountBarriers.size());
    for (const RateLimitAccountBarrier& barrier : m_accountBarriers) {
      OpenPitPretradePoliciesRateLimitAccountBarrier raw{};
      raw.account_id = barrier.accountId.Raw();
      raw.max_orders = barrier.limit.maxOrders;
      raw.window_nanoseconds =
          ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
              barrier.limit.windowNanoseconds);
      accountRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountAssetBarrier>
        accountAssetRaw;
    accountAssetRaw.reserve(m_accountAssetBarriers.size());
    for (const RateLimitAccountAssetBarrier& barrier : m_accountAssetBarriers) {
      OpenPitPretradePoliciesRateLimitAccountAssetBarrier raw{};
      raw.account_id = barrier.accountId.Raw();
      raw.settlement_asset = ::openpit::MakeStringView(barrier.settlementAsset);
      raw.max_orders = barrier.limit.maxOrders;
      raw.window_nanoseconds =
          ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
              barrier.limit.windowNanoseconds);
      accountAssetRaw.push_back(raw);
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_rate_limit_policy(
            builder.Get(), m_policyGroupId, brokerPtr, assetRaw.data(),
            assetRaw.size(), accountRaw.data(), accountRaw.size(),
            accountAssetRaw.data(), accountAssetRaw.size(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_engine_builder_add_builtin_rate_limit_policy failed");
    }
  }

 private:
  std::optional<RateLimitBrokerBarrier> m_broker;
  std::vector<RateLimitAssetBarrier> m_assetBarriers;
  std::vector<RateLimitAccountBarrier> m_accountBarriers;
  std::vector<RateLimitAccountAssetBarrier> m_accountAssetBarriers;
  std::uint16_t m_policyGroupId = OPENPIT_DEFAULT_POLICY_GROUP_ID;
};

//------------------------------------------------------------------------------
// SpotFunds

// Per-instrument slippage override for the spot-funds policy. The target is a
// tagged union: instrument default, instrument+account, or
// instrument+account-group. When `slippageBps` is absent the entry is ignored
// during construction and clears the selected runtime override during
// reconfiguration. Mirrors `OpenPitPretradePoliciesSpotFundsOverride`.
struct SpotFundsOverride {
  std::optional<std::uint16_t> slippageBps;

  /// Creates an instrument-level slippage override.
  explicit SpotFundsOverride(::openpit::marketdata::InstrumentId instrument)
      : m_target(InstrumentTarget(instrument)) {}

  /// Creates an instrument+account slippage override.
  SpotFundsOverride(::openpit::marketdata::InstrumentId instrument,
                    ::openpit::param::AccountId accountId)
      : m_target(InstrumentAccountTarget(instrument, accountId)) {}

  /// Creates an instrument+account-group slippage override.
  SpotFundsOverride(::openpit::marketdata::InstrumentId instrument,
                    ::openpit::param::AccountGroupId accountGroupId)
      : m_target(InstrumentAccountGroupTarget(instrument, accountGroupId)) {}

  /// Lowers the override to the native C payload.
  [[nodiscard]] OpenPitPretradePoliciesSpotFundsOverride Raw() const noexcept {
    OpenPitPretradePoliciesSpotFundsOverride raw{};
    raw.target = m_target;
    if (slippageBps) {
      raw.slippage_bps = *slippageBps;
      raw.has_slippage_bps = true;
    }
    return raw;
  }

 private:
  [[nodiscard]] static OpenPitPretradePoliciesSpotFundsOverrideTarget
  InstrumentTarget(::openpit::marketdata::InstrumentId instrument) noexcept {
    OpenPitPretradePoliciesSpotFundsOverrideTarget result{};
    result.tag = OpenPitPretradePoliciesSpotFundsOverrideTargetTag_Instrument;
    result.payload.instrument.instrument_id = instrument.Raw();
    return result;
  }

  [[nodiscard]] static OpenPitPretradePoliciesSpotFundsOverrideTarget
  InstrumentAccountTarget(::openpit::marketdata::InstrumentId instrument,
                          ::openpit::param::AccountId accountId) noexcept {
    OpenPitPretradePoliciesSpotFundsOverrideTarget result{};
    result.tag =
        OpenPitPretradePoliciesSpotFundsOverrideTargetTag_InstrumentAccount;
    result.payload.instrument_account.instrument_id = instrument.Raw();
    result.payload.instrument_account.account_id = accountId.Raw();
    return result;
  }

  [[nodiscard]] static OpenPitPretradePoliciesSpotFundsOverrideTarget
  InstrumentAccountGroupTarget(
      ::openpit::marketdata::InstrumentId instrument,
      ::openpit::param::AccountGroupId accountGroupId) noexcept {
    OpenPitPretradePoliciesSpotFundsOverrideTarget result{};
    result.tag =
        OpenPitPretradePoliciesSpotFundsOverrideTargetTag_InstrumentAccountGroup;
    result.payload.instrument_account_group.instrument_id = instrument.Raw();
    result.payload.instrument_account_group.account_group_id =
        accountGroupId.Raw();
    return result;
  }

  OpenPitPretradePoliciesSpotFundsOverrideTarget m_target{};
};

/// Account-currency P&L bounds computed by the spot-funds ledger.
///
/// Lower and upper bounds are optional; lower is typically a negative loss
/// limit and upper is typically a positive profit-taking limit.
struct SpotFundsPnlBoundsBarrier {
  std::string accountCurrency;
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;

  /// Creates a barrier for one account currency.
  explicit SpotFundsPnlBoundsBarrier(std::string currency)
      : accountCurrency(std::move(currency)) {}

  /// Lowers the barrier to the native C payload.
  [[nodiscard]] OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier Raw()
      const noexcept {
    OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier raw{};
    raw.account_currency = ::openpit::MakeStringView(accountCurrency);
    raw.lower_bound =
        ::openpit::pretrade::policies::detail::PnlOptional(lowerBound);
    raw.upper_bound =
        ::openpit::pretrade::policies::detail::PnlOptional(upperBound);
    return raw;
  }
};

/// Per-account-group spot-funds P&L bounds refinement.
struct SpotFundsPnlBoundsAccountGroupBarrier {
  SpotFundsPnlBoundsBarrier barrier;
  ::openpit::param::AccountGroupId accountGroupId;

  /// Creates an account-group P&L barrier.
  SpotFundsPnlBoundsAccountGroupBarrier(
      ::openpit::param::AccountGroupId groupId,
      SpotFundsPnlBoundsBarrier groupBarrier)
      : barrier(std::move(groupBarrier)), accountGroupId(groupId) {}

  /// Lowers the account-group refinement to the native C payload.
  [[nodiscard]] OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
  Raw() const noexcept {
    OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier raw{};
    raw.account_group_id = accountGroupId.Raw();
    raw.barrier = barrier.Raw();
    return raw;
  }
};

/// Per-account spot-funds P&L bounds with a construction-time P&L seed.
struct SpotFundsPnlBoundsAccountBarrier {
  SpotFundsPnlBoundsBarrier barrier;
  ::openpit::param::AccountId accountId;
  ::openpit::param::Pnl initialPnl;

  /// Creates an account P&L barrier with an initial accumulated P&L.
  SpotFundsPnlBoundsAccountBarrier(::openpit::param::AccountId account,
                                   SpotFundsPnlBoundsBarrier accountBarrier,
                                   ::openpit::param::Pnl initial)
      : barrier(std::move(accountBarrier)),
        accountId(account),
        initialPnl(initial) {}

  /// Lowers the account barrier to the native C payload.
  [[nodiscard]] OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier Raw()
      const noexcept {
    OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier raw{};
    raw.account_id = accountId.Raw();
    raw.barrier = barrier.Raw();
    raw.initial_pnl = initialPnl.Raw();
    return raw;
  }
};

/// Runtime replacement for per-account spot-funds P&L bounds.
///
/// This update intentionally carries no `initialPnl`; runtime reconfiguration
/// preserves the live accumulator.
struct SpotFundsPnlBoundsAccountBarrierUpdate {
  SpotFundsPnlBoundsBarrier barrier;
  ::openpit::param::AccountId accountId;

  /// Creates an account P&L barrier update.
  SpotFundsPnlBoundsAccountBarrierUpdate(
      ::openpit::param::AccountId account,
      SpotFundsPnlBoundsBarrier accountBarrier)
      : barrier(std::move(accountBarrier)), accountId(account) {}

  /// Lowers the account update to the native C payload.
  [[nodiscard]]
  OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrierUpdate Raw()
      const noexcept {
    OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrierUpdate raw{};
    raw.account_id = accountId.Raw();
    raw.barrier = barrier.Raw();
    return raw;
  }
};

/// Built-in spot-funds self-computed P&L bounds kill switch.
///
/// This registers the regular `SpotFundsPolicy` name and configures its
/// account-currency P&L-bounds axis. The policy computes realized P&L from
/// reconciled fills instead of trusting an externally supplied P&L figure.
class SpotFundsPnlBoundsKillSwitchPolicy {
 public:
  /// Assigns the policy to a pricing group.
  SpotFundsPnlBoundsKillSwitchPolicy& PolicyGroupId(
      std::uint16_t policyGroupId) {
    m_policyGroupId = policyGroupId;
    return *this;
  }

  /// Sets the market-data service used for FX conversion.
  SpotFundsPnlBoundsKillSwitchPolicy& WithMarketData(
      const ::openpit::marketdata::Service& marketData) noexcept {
    return WithMarketData(marketData.Get());
  }

  /// Sets the borrowed raw market-data service handle used for FX conversion.
  SpotFundsPnlBoundsKillSwitchPolicy& WithMarketData(
      const OpenPitMarketDataService* marketData) noexcept {
    m_marketData = marketData;
    return *this;
  }

  /// Adds a global account-currency P&L barrier.
  SpotFundsPnlBoundsKillSwitchPolicy& GlobalBarrier(
      SpotFundsPnlBoundsBarrier barrier) {
    m_globalBarriers.push_back(std::move(barrier));
    return *this;
  }

  /// Adds an account-group account-currency P&L barrier.
  SpotFundsPnlBoundsKillSwitchPolicy& AccountGroupBarrier(
      SpotFundsPnlBoundsAccountGroupBarrier barrier) {
    m_accountGroupBarriers.push_back(std::move(barrier));
    return *this;
  }

  /// Adds an account account-currency P&L barrier.
  SpotFundsPnlBoundsKillSwitchPolicy& AccountBarrier(
      SpotFundsPnlBoundsAccountBarrier barrier) {
    m_accountBarriers.push_back(std::move(barrier));
    return *this;
  }

  /// Registers the policy on `builder`.
  void AddTo(::openpit::EngineBuilder& builder) const {
    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier> globalRaw;
    globalRaw.reserve(m_globalBarriers.size());
    for (const SpotFundsPnlBoundsBarrier& barrier : m_globalBarriers) {
      globalRaw.push_back(barrier.Raw());
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier>
        accountGroupRaw;
    accountGroupRaw.reserve(m_accountGroupBarriers.size());
    for (const SpotFundsPnlBoundsAccountGroupBarrier& barrier :
         m_accountGroupBarriers) {
      accountGroupRaw.push_back(barrier.Raw());
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier>
        accountRaw;
    accountRaw.reserve(m_accountBarriers.size());
    for (const SpotFundsPnlBoundsAccountBarrier& barrier : m_accountBarriers) {
      accountRaw.push_back(barrier.Raw());
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(
            builder.Get(), m_marketData, m_policyGroupId, globalRaw.data(),
            globalRaw.size(), accountGroupRaw.data(), accountGroupRaw.size(),
            accountRaw.data(), accountRaw.size(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_"
          "killswitch_policy failed");
    }
  }

 private:
  const OpenPitMarketDataService* m_marketData = nullptr;
  std::vector<SpotFundsPnlBoundsBarrier> m_globalBarriers;
  std::vector<SpotFundsPnlBoundsAccountGroupBarrier> m_accountGroupBarriers;
  std::vector<SpotFundsPnlBoundsAccountBarrier> m_accountBarriers;
  std::uint16_t m_policyGroupId = OPENPIT_DEFAULT_POLICY_GROUP_ID;
};

// Built-in spot-funds policy, configured inline (no separate accessors).
//
// By default market orders are rejected (limit-only mode). Call
// `WithMarketOrders` to enable them, supplying the borrowed market-data service
// handle and the worst-case global slippage in basis points. The market-data
// handle is owned by the caller (the market-data binding slice); it must
// outlive registration. Mirrors
// `openpit_engine_builder_add_builtin_spot_funds_policy`.
class SpotFundsPolicy {
 public:
  SpotFundsPolicy& PolicyGroupId(std::uint16_t policyGroupId) {
    m_policyGroupId = policyGroupId;
    return *this;
  }

  /// Enables market orders from a C++ market-data service.
  ///
  /// `marketData` must outlive registration; `slippageBps` is the worst-case
  /// global slippage (1 bps = 0.01%).
  SpotFundsPolicy& WithMarketOrders(
      const ::openpit::marketdata::Service& marketData,
      std::uint16_t slippageBps) {
    return WithMarketOrders(marketData.Get(), slippageBps);
  }

  /// Enables market orders from a borrowed raw market-data service handle.
  SpotFundsPolicy& WithMarketOrders(const OpenPitMarketDataService* marketData,
                                    std::uint16_t slippageBps) {
    m_marketData = marketData;
    m_marketSlippageBps = slippageBps;
    return *this;
  }

  SpotFundsPolicy& PricingSource(SpotFundsPricingSource source) {
    m_pricingSource = source;
    return *this;
  }

  SpotFundsPolicy& Override(SpotFundsOverride override) {
    m_overrides.push_back(override);
    return *this;
  }

  void AddTo(::openpit::EngineBuilder& builder) const {
    std::vector<OpenPitPretradePoliciesSpotFundsOverride> overridesRaw;
    overridesRaw.reserve(m_overrides.size());
    for (const SpotFundsOverride& override : m_overrides) {
      overridesRaw.push_back(override.Raw());
    }

    const std::uint16_t slippage = m_marketSlippageBps.value_or(0);
    const std::uint16_t* slippagePtr =
        m_marketSlippageBps ? &slippage : nullptr;

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_spot_funds_policy(
            builder.Get(), m_marketData, slippagePtr,
            static_cast<std::uint8_t>(m_pricingSource), overridesRaw.data(),
            overridesRaw.size(), m_policyGroupId, &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_engine_builder_add_builtin_spot_funds_policy failed");
    }
  }

 private:
  const OpenPitMarketDataService* m_marketData = nullptr;
  std::optional<std::uint16_t> m_marketSlippageBps;
  SpotFundsPricingSource m_pricingSource = SpotFundsPricingSource::Mark;
  std::vector<SpotFundsOverride> m_overrides;
  std::uint16_t m_policyGroupId = OPENPIT_DEFAULT_POLICY_GROUP_ID;
};

}  // namespace openpit::pretrade::policies

namespace openpit {

// Runtime policy-settings updater bound to an engine. Every call forwards to
// the native runtime and throws `openpit::ConfigureError` on
// domain/configuration failures.
class Configurator {
 public:
  explicit Configurator(const ::openpit::Engine& engine) noexcept
      : m_engine(engine.Get()) {}

  explicit Configurator(OpenPitEngine* engine) noexcept : m_engine(engine) {}

  void RateLimit(
      std::string_view name,
      std::optional<::openpit::pretrade::policies::RateLimitBrokerBarrier>
          broker,
      std::optional<
          std::vector<::openpit::pretrade::policies::RateLimitAssetBarrier>>
          assets = std::nullopt,
      std::optional<
          std::vector<::openpit::pretrade::policies::RateLimitAccountBarrier>>
          accounts = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::RateLimitAccountAssetBarrier>>
          accountAssets = std::nullopt) const {
    OpenPitPretradePoliciesRateLimitBrokerBarrier brokerRaw{};
    const OpenPitPretradePoliciesRateLimitBrokerBarrier* brokerPtr = nullptr;
    if (broker) {
      brokerRaw.max_orders = broker->limit.maxOrders;
      brokerRaw.window_nanoseconds =
          ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
              broker->limit.windowNanoseconds);
      brokerPtr = &brokerRaw;
    }

    std::vector<OpenPitPretradePoliciesRateLimitAssetBarrier> assetRaw;
    if (assets) {
      assetRaw.reserve(assets->size());
      for (const auto& barrier : *assets) {
        OpenPitPretradePoliciesRateLimitAssetBarrier raw{};
        raw.settlement_asset =
            ::openpit::MakeStringView(barrier.settlementAsset);
        raw.max_orders = barrier.limit.maxOrders;
        raw.window_nanoseconds =
            ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
                barrier.limit.windowNanoseconds);
        assetRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountBarrier> accountRaw;
    if (accounts) {
      accountRaw.reserve(accounts->size());
      for (const auto& barrier : *accounts) {
        OpenPitPretradePoliciesRateLimitAccountBarrier raw{};
        raw.account_id = barrier.accountId.Raw();
        raw.max_orders = barrier.limit.maxOrders;
        raw.window_nanoseconds =
            ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
                barrier.limit.windowNanoseconds);
        accountRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountAssetBarrier>
        accountAssetRaw;
    if (accountAssets) {
      accountAssetRaw.reserve(accountAssets->size());
      for (const auto& barrier : *accountAssets) {
        OpenPitPretradePoliciesRateLimitAccountAssetBarrier raw{};
        raw.account_id = barrier.accountId.Raw();
        raw.settlement_asset =
            ::openpit::MakeStringView(barrier.settlementAsset);
        raw.max_orders = barrier.limit.maxOrders;
        raw.window_nanoseconds =
            ::openpit::pretrade::policies::detail::ToRateLimitWindowNanoseconds(
                barrier.limit.windowNanoseconds);
        accountAssetRaw.push_back(raw);
      }
    }

    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_rate_limit(
            m_engine, ::openpit::MakeStringView(name), brokerPtr,
            broker.has_value(), assetRaw.data(), assetRaw.size(),
            assets.has_value(), accountRaw.data(), accountRaw.size(),
            accounts.has_value(), accountAssetRaw.data(),
            accountAssetRaw.size(), accountAssets.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_rate_limit failed");
    }
  }

  void OrderSizeLimit(
      std::string_view name,
      std::optional<::openpit::pretrade::policies::OrderSizeBrokerBarrier>
          broker,
      std::optional<
          std::vector<::openpit::pretrade::policies::OrderSizeAssetBarrier>>
          assets = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::OrderSizeAccountAssetBarrier>>
          accountAssets = std::nullopt) const {
    OpenPitPretradePoliciesOrderSizeBrokerBarrier brokerRaw{};
    const OpenPitPretradePoliciesOrderSizeBrokerBarrier* brokerPtr = nullptr;
    if (broker) {
      brokerRaw.limit = broker->limit.Raw();
      brokerPtr = &brokerRaw;
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAssetBarrier> assetRaw;
    if (assets) {
      assetRaw.reserve(assets->size());
      for (const auto& barrier : *assets) {
        OpenPitPretradePoliciesOrderSizeAssetBarrier raw{};
        raw.limit = barrier.limit.Raw();
        raw.settlement_asset =
            ::openpit::MakeStringView(barrier.settlementAsset);
        assetRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAccountAssetBarrier>
        accountAssetRaw;
    if (accountAssets) {
      accountAssetRaw.reserve(accountAssets->size());
      for (const auto& barrier : *accountAssets) {
        OpenPitPretradePoliciesOrderSizeAccountAssetBarrier raw{};
        raw.limit = barrier.limit.Raw();
        raw.account_id = barrier.accountId.Raw();
        raw.settlement_asset =
            ::openpit::MakeStringView(barrier.settlementAsset);
        accountAssetRaw.push_back(raw);
      }
    }

    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_order_size_limit(
            m_engine, ::openpit::MakeStringView(name), brokerPtr,
            broker.has_value(), assetRaw.data(), assetRaw.size(),
            assets.has_value(), accountAssetRaw.data(), accountAssetRaw.size(),
            accountAssets.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_order_size_limit failed");
    }
  }

  void PnlBoundsKillSwitch(
      std::string_view name,
      std::optional<
          std::vector<::openpit::pretrade::policies::PnlBoundsBrokerBarrier>>
          brokers = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::PnlBoundsAccountBarrierUpdate>>
          accounts = std::nullopt) const {
    std::vector<OpenPitPretradePoliciesPnlBoundsBarrier> brokerRaw;
    if (brokers) {
      brokerRaw.reserve(brokers->size());
      for (const auto& barrier : *brokers) {
        OpenPitPretradePoliciesPnlBoundsBarrier raw{};
        raw.settlement_asset =
            ::openpit::MakeStringView(barrier.settlementAsset);
        raw.lower_bound = ::openpit::pretrade::policies::detail::PnlOptional(
            barrier.lowerBound);
        raw.upper_bound = ::openpit::pretrade::policies::detail::PnlOptional(
            barrier.upperBound);
        brokerRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate>
        accountRaw;
    if (accounts) {
      accountRaw.reserve(accounts->size());
      for (const auto& barrier : *accounts) {
        OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate raw{};
        raw.account_id = barrier.accountId.Raw();
        raw.settlement_asset =
            ::openpit::MakeStringView(barrier.settlementAsset);
        raw.lower_bound = ::openpit::pretrade::policies::detail::PnlOptional(
            barrier.lowerBound);
        raw.upper_bound = ::openpit::pretrade::policies::detail::PnlOptional(
            barrier.upperBound);
        accountRaw.push_back(raw);
      }
    }

    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_pnl_bounds_killswitch(
            m_engine, ::openpit::MakeStringView(name), brokerRaw.data(),
            brokerRaw.size(), brokers.has_value(), accountRaw.data(),
            accountRaw.size(), accounts.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_pnl_bounds_killswitch failed");
    }
  }

  void SetAccountPnl(std::string_view name,
                     ::openpit::param::AccountId accountId,
                     std::string_view settlementAsset,
                     ::openpit::param::Pnl pnl) const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(
            m_engine, ::openpit::MakeStringView(name), accountId.Raw(),
            ::openpit::MakeStringView(settlementAsset), pnl.Raw(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl "
          "failed");
    }
  }

  void SpotFunds(
      std::string_view name,
      std::optional<std::uint16_t> globalSlippageBps = std::nullopt,
      std::optional<::openpit::pretrade::policies::SpotFundsPricingSource>
          pricingSource = std::nullopt,
      std::optional<
          std::vector<::openpit::pretrade::policies::SpotFundsOverride>>
          overrides = std::nullopt) const {
    std::vector<OpenPitPretradePoliciesSpotFundsOverride> overridesRaw;
    if (overrides) {
      overridesRaw.reserve(overrides->size());
      for (const auto& override : *overrides) {
        overridesRaw.push_back(override.Raw());
      }
    }

    OpenPitConfigureError* error = nullptr;
    const std::uint8_t source =
        pricingSource
            ? static_cast<std::uint8_t>(*pricingSource)
            : static_cast<std::uint8_t>(
                  ::openpit::pretrade::policies::SpotFundsPricingSource::Mark);
    if (!openpit_engine_configure_spot_funds(
            m_engine, ::openpit::MakeStringView(name),
            globalSlippageBps.value_or(0), globalSlippageBps.has_value(),
            source, pricingSource.has_value(), overridesRaw.data(),
            overridesRaw.size(), overrides.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_spot_funds failed");
    }
  }

  /// Retunes the SpotFunds self-computed P&L bounds axis.
  ///
  /// Each optional vector is a PATCH axis: `std::nullopt` leaves that axis
  /// unchanged; an engaged empty vector clears it. Account updates preserve
  /// each live accumulated P&L value.
  void SpotFundsPnlBoundsKillSwitch(
      std::string_view name,
      std::optional<
          std::vector<::openpit::pretrade::policies::SpotFundsPnlBoundsBarrier>>
          global = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::SpotFundsPnlBoundsAccountGroupBarrier>>
          accountGroups = std::nullopt,
      std::optional<std::vector<::openpit::pretrade::policies::
                                    SpotFundsPnlBoundsAccountBarrierUpdate>>
          accounts = std::nullopt) const {
    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier> globalRaw;
    if (global) {
      globalRaw.reserve(global->size());
      for (const auto& barrier : *global) {
        globalRaw.push_back(barrier.Raw());
      }
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier>
        accountGroupRaw;
    if (accountGroups) {
      accountGroupRaw.reserve(accountGroups->size());
      for (const auto& barrier : *accountGroups) {
        accountGroupRaw.push_back(barrier.Raw());
      }
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrierUpdate>
        accountRaw;
    if (accounts) {
      accountRaw.reserve(accounts->size());
      for (const auto& barrier : *accounts) {
        accountRaw.push_back(barrier.Raw());
      }
    }

    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_pnl_bounds_killswitch(
            m_engine, ::openpit::MakeStringView(name), globalRaw.data(),
            globalRaw.size(), global.has_value(), accountGroupRaw.data(),
            accountGroupRaw.size(), accountGroups.has_value(),
            accountRaw.data(), accountRaw.size(), accounts.has_value(),
            &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_pnl_bounds_killswitch failed");
    }
  }

  /// Force-sets one SpotFunds live account-currency P&L accumulator.
  ///
  /// This is an absolute assignment and is separate from barrier retuning.
  void SetSpotFundsAccountPnl(std::string_view name,
                              ::openpit::param::AccountId accountId,
                              std::string_view accountCurrency,
                              ::openpit::param::Pnl pnl) const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_set_account_pnl(
            m_engine, ::openpit::MakeStringView(name), accountId.Raw(),
            ::openpit::MakeStringView(accountCurrency), pnl.Raw(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_spot_funds_set_account_pnl failed");
    }
  }

  void SpotFundsGlobalLimitMode(
      std::string_view name,
      ::openpit::pretrade::policies::SpotFundsLimitMode mode) const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_global_limit_mode(
            m_engine, ::openpit::MakeStringView(name),
            static_cast<std::uint8_t>(mode), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_global_limit_mode failed");
    }
  }

  void SpotFundsAccountLimitMode(
      std::string_view name, ::openpit::param::AccountId accountId,
      std::optional<::openpit::pretrade::policies::SpotFundsLimitMode> mode)
      const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_account_limit_mode(
            m_engine, ::openpit::MakeStringView(name), accountId.Raw(),
            mode ? static_cast<std::uint8_t>(*mode) : 0, mode.has_value(),
            &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_account_limit_mode "
          "failed");
    }
  }

  void SpotFundsAccountGroupLimitMode(
      std::string_view name, ::openpit::param::AccountGroupId accountGroupId,
      std::optional<::openpit::pretrade::policies::SpotFundsLimitMode> mode)
      const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_account_group_limit_mode(
            m_engine, ::openpit::MakeStringView(name), accountGroupId.Raw(),
            mode ? static_cast<std::uint8_t>(*mode) : 0, mode.has_value(),
            &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_account_group_limit_mode "
          "failed");
    }
  }

 private:
  OpenPitEngine* m_engine = nullptr;
};

}  // namespace openpit

namespace openpit {

[[nodiscard]] inline ::openpit::Configurator Engine::Configure()
    const noexcept {
  return ::openpit::Configurator(*this);
}

}  // namespace openpit
