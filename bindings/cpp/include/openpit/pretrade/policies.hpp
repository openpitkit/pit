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
#include "openpit/param/param.hpp"
#include "openpit/pretrade/detail/lists.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

// Built-in pre-trade policy configurations.
//
// Each policy is configured by a small value object carrying its barriers, then
// registered on an `openpit::EngineBuilder` via `AddTo(builder)`. Failures such
// as missing barriers, a consumed builder, or invalid arguments surface as a
// thrown `openpit::Error`.
//
// Financial limits use `openpit::param` value types and assets own their
// storage.

namespace openpit::pretrade::policies {

// Base price the spot-funds policy uses to size market-order reservations.
enum class SpotFundsPricingSource : std::uint8_t {
  Mark = 0,
  BookTop = 1,
};

// Runtime limit mode for spot-funds reservations.
enum class SpotFundsLimitMode : std::uint8_t {
  Enforce = 0,
  TrackOnly = 1,
};

inline constexpr std::string_view RateLimitPolicyName = "RateLimitPolicy";
inline constexpr std::string_view OrderSizeLimitPolicyName =
    "OrderSizeLimitPolicy";
inline constexpr std::string_view PnlBoundsKillSwitchPolicyName =
    "PnlBoundsKillSwitchPolicy";
inline constexpr std::string_view SpotFundsPolicyName = "SpotFundsPolicy";

namespace detail {

class PnlOptionalAccess final {
 private:
  [[nodiscard]] static ::openpit::param::detail::RawPnlOptional Native(
      const std::optional<::openpit::param::Pnl>& value) noexcept {
    ::openpit::param::detail::RawPnlOptional raw{};
    if (value) {
      raw.value = ::openpit::detail::Native(*value);
      raw.is_set = true;
    }
    return raw;
  }

  friend class ::openpit::pretrade::policies::PnlBoundsKillSwitchPolicy;
  friend class ::openpit::pretrade::policies::
      SpotFundsPnlBoundsGlobalBarrierUpdate;
  friend class ::openpit::pretrade::policies::
      SpotFundsPnlBoundsKillSwitchPolicy;
  friend class ::openpit::pretrade::policies::SpotFundsPolicy;
  friend class ::openpit::Configurator;
  friend struct ::openpit::pretrade::policies::PnlBoundsAccountBarrier;
  friend struct ::openpit::pretrade::policies::PnlBoundsAccountBarrierUpdate;
  friend struct ::openpit::pretrade::policies::PnlBoundsBrokerBarrier;
  friend struct ::openpit::pretrade::policies::SpotFundsPnlBoundsAccountBarrier;
  friend struct ::openpit::pretrade::policies::
      SpotFundsPnlBoundsAccountGroupBarrier;
  friend struct ::openpit::pretrade::policies::SpotFundsPnlBoundsBarrier;
};

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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] OpenPitPretradePoliciesOrderSizeLimit Native() const noexcept {
    OpenPitPretradePoliciesOrderSizeLimit raw{};
    raw.max_quantity = ::openpit::detail::Native(maxQuantity);
    raw.max_notional = ::openpit::detail::Native(maxNotional);
    return raw;
  }
};

// Broker-wide order-size barrier.
struct OrderSizeBrokerBarrier {
  OrderSizeLimit limit;

  explicit OrderSizeBrokerBarrier(OrderSizeLimit barrierLimit)
      : limit(barrierLimit) {}
};

/// Tri-state runtime update for the singular order-size broker barrier.
class OrderSizeBrokerBarrierUpdate {
 public:
  [[nodiscard]] static OrderSizeBrokerBarrierUpdate Unchanged() noexcept {
    return OrderSizeBrokerBarrierUpdate(false, std::nullopt);
  }

  [[nodiscard]] static OrderSizeBrokerBarrierUpdate Clear() noexcept {
    return OrderSizeBrokerBarrierUpdate(true, std::nullopt);
  }

  [[nodiscard]] static OrderSizeBrokerBarrierUpdate Set(
      OrderSizeBrokerBarrier barrier) {
    return OrderSizeBrokerBarrierUpdate(true, barrier);
  }

  [[nodiscard]] bool HasUpdate() const noexcept { return m_hasUpdate; }

  [[nodiscard]] const std::optional<OrderSizeBrokerBarrier>& Barrier()
      const noexcept {
    return m_barrier;
  }

 private:
  OrderSizeBrokerBarrierUpdate(
      bool hasUpdate, std::optional<OrderSizeBrokerBarrier> barrier) noexcept
      : m_hasUpdate(hasUpdate), m_barrier(barrier) {}

  bool m_hasUpdate = false;
  std::optional<OrderSizeBrokerBarrier> m_barrier;
};

// Per-settlement-asset order-size barrier.
struct OrderSizeAssetBarrier {
  OrderSizeLimit limit;
  ::openpit::param::Asset settlementAsset;

  OrderSizeAssetBarrier(OrderSizeLimit barrierLimit,
                        ::openpit::param::Asset asset)
      : limit(barrierLimit), settlementAsset(std::move(asset)) {}
};

// Per-(account, settlement-asset) order-size barrier.
struct OrderSizeAccountAssetBarrier {
  OrderSizeLimit limit;
  ::openpit::param::AccountId accountId;
  ::openpit::param::Asset settlementAsset;

  OrderSizeAccountAssetBarrier(OrderSizeLimit barrierLimit,
                               ::openpit::param::AccountId account,
                               ::openpit::param::Asset asset)
      : limit(barrierLimit),
        accountId(account),
        settlementAsset(std::move(asset)) {}
};

// Built-in order-size-limit policy. At least one barrier axis must be
// configured before registration.
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
      brokerRaw.limit = ::openpit::detail::Native(m_broker->limit);
      brokerPtr = &brokerRaw;
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAssetBarrier> assetRaw;
    assetRaw.reserve(m_assetBarriers.size());
    for (const OrderSizeAssetBarrier& barrier : m_assetBarriers) {
      OpenPitPretradePoliciesOrderSizeAssetBarrier raw{};
      raw.limit = ::openpit::detail::Native(barrier.limit);
      raw.settlement_asset = ::openpit::detail::Native(barrier.settlementAsset);
      assetRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAccountAssetBarrier>
        accountAssetRaw;
    accountAssetRaw.reserve(m_accountAssetBarriers.size());
    for (const OrderSizeAccountAssetBarrier& barrier : m_accountAssetBarriers) {
      OpenPitPretradePoliciesOrderSizeAccountAssetBarrier raw{};
      raw.limit = ::openpit::detail::Native(barrier.limit);
      raw.account_id = ::openpit::detail::Native(barrier.accountId);
      raw.settlement_asset = ::openpit::detail::Native(barrier.settlementAsset);
      accountAssetRaw.push_back(raw);
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_order_size_limit_policy(
            ::openpit::detail::Native(builder), m_policyGroupId, brokerPtr,
            assetRaw.data(), assetRaw.size(), accountAssetRaw.data(),
            accountAssetRaw.size(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_order_size_limit_policy failed");
    }
  }

 private:
  std::optional<OrderSizeBrokerBarrier> m_broker;
  std::vector<OrderSizeAssetBarrier> m_assetBarriers;
  std::vector<OrderSizeAccountAssetBarrier> m_accountAssetBarriers;
  std::uint16_t m_policyGroupId = ::openpit::param::DefaultPolicyGroupId;
};

//------------------------------------------------------------------------------
// OrderValidation

// Built-in order-validation policy. Requires no barriers.
class OrderValidationPolicy {
 public:
  OrderValidationPolicy& PolicyGroupId(std::uint16_t policyGroupId) {
    m_policyGroupId = policyGroupId;
    return *this;
  }

  void AddTo(::openpit::EngineBuilder& builder) const {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_order_validation_policy(
            ::openpit::detail::Native(builder), m_policyGroupId, &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_order_validation_policy failed");
    }
  }

 private:
  std::uint16_t m_policyGroupId = ::openpit::param::DefaultPolicyGroupId;
};

//------------------------------------------------------------------------------
// PnlBoundsKillSwitch

// Broker-level P&L bounds for one settlement asset, applied across all
// accounts. Lower bound is typically the negative loss limit; upper bound the
// positive profit-take limit. Either may be absent.
struct PnlBoundsBrokerBarrier {
  ::openpit::param::Asset settlementAsset;
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;

  explicit PnlBoundsBrokerBarrier(::openpit::param::Asset asset)
      : settlementAsset(std::move(asset)) {}
};

// Per-(account, settlement-asset) P&L bounds with an initial accumulated-P&L
// seed.
struct PnlBoundsAccountBarrier {
  ::openpit::param::AccountId accountId;
  ::openpit::param::Asset settlementAsset;
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;
  ::openpit::param::Pnl initialPnl;

  PnlBoundsAccountBarrier(::openpit::param::AccountId account,
                          ::openpit::param::Asset asset,
                          ::openpit::param::Pnl initial)
      : accountId(account),
        settlementAsset(std::move(asset)),
        initialPnl(initial) {}
};

// Runtime replacement for a per-(account, settlement-asset) P&L barrier.
// Unlike `PnlBoundsAccountBarrier`, it intentionally carries no initial P&L:
// runtime replacement preserves the live accumulator.
struct PnlBoundsAccountBarrierUpdate {
  ::openpit::param::AccountId accountId;
  ::openpit::param::Asset settlementAsset;
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;

  PnlBoundsAccountBarrierUpdate(::openpit::param::AccountId account,
                                ::openpit::param::Asset asset)
      : accountId(account), settlementAsset(std::move(asset)) {}
};

// Built-in P&L bounds kill-switch policy. At least one barrier (broker or
// account) must be configured.
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
      raw.settlement_asset = ::openpit::detail::Native(barrier.settlementAsset);
      raw.lower_bound =
          ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
              barrier.lowerBound);
      raw.upper_bound =
          ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
              barrier.upperBound);
      brokerRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesPnlBoundsAccountBarrier> accountRaw;
    accountRaw.reserve(m_accountBarriers.size());
    for (const PnlBoundsAccountBarrier& barrier : m_accountBarriers) {
      OpenPitPretradePoliciesPnlBoundsAccountBarrier raw{};
      raw.account_id = ::openpit::detail::Native(barrier.accountId);
      raw.settlement_asset = ::openpit::detail::Native(barrier.settlementAsset);
      raw.lower_bound =
          ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
              barrier.lowerBound);
      raw.upper_bound =
          ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
              barrier.upperBound);
      raw.initial_pnl = ::openpit::detail::Native(barrier.initialPnl);
      accountRaw.push_back(raw);
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
            ::openpit::detail::Native(builder), m_policyGroupId,
            brokerRaw.data(), brokerRaw.size(), accountRaw.data(),
            accountRaw.size(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy "
          "failed");
    }
  }

 private:
  std::vector<PnlBoundsBrokerBarrier> m_brokerBarriers;
  std::vector<PnlBoundsAccountBarrier> m_accountBarriers;
  std::uint16_t m_policyGroupId = ::openpit::param::DefaultPolicyGroupId;
};

//------------------------------------------------------------------------------
// RateLimit

// Maximum number of orders accepted within a sliding window expressed in
// nanoseconds.
struct RateLimit {
  std::size_t maxOrders = 0;
  std::int64_t windowNanoseconds = 0;

  RateLimit(std::size_t orders, std::int64_t windowNanos)
      : maxOrders(orders), windowNanoseconds(windowNanos) {}
};

// Broker-wide rate-limit barrier.
struct RateLimitBrokerBarrier {
  RateLimit limit;

  explicit RateLimitBrokerBarrier(RateLimit barrierLimit)
      : limit(barrierLimit) {}
};

/// Tri-state runtime update for the singular rate-limit broker barrier.
class RateLimitBrokerBarrierUpdate {
 public:
  [[nodiscard]] static RateLimitBrokerBarrierUpdate Unchanged() noexcept {
    return RateLimitBrokerBarrierUpdate(false, std::nullopt);
  }

  [[nodiscard]] static RateLimitBrokerBarrierUpdate Clear() noexcept {
    return RateLimitBrokerBarrierUpdate(true, std::nullopt);
  }

  [[nodiscard]] static RateLimitBrokerBarrierUpdate Set(
      RateLimitBrokerBarrier barrier) {
    return RateLimitBrokerBarrierUpdate(true, barrier);
  }

  [[nodiscard]] bool HasUpdate() const noexcept { return m_hasUpdate; }

  [[nodiscard]] const std::optional<RateLimitBrokerBarrier>& Barrier()
      const noexcept {
    return m_barrier;
  }

 private:
  RateLimitBrokerBarrierUpdate(
      bool hasUpdate, std::optional<RateLimitBrokerBarrier> barrier) noexcept
      : m_hasUpdate(hasUpdate), m_barrier(barrier) {}

  bool m_hasUpdate = false;
  std::optional<RateLimitBrokerBarrier> m_barrier;
};

// Per-settlement-asset rate-limit barrier.
struct RateLimitAssetBarrier {
  RateLimit limit;
  ::openpit::param::Asset settlementAsset;

  RateLimitAssetBarrier(RateLimit barrierLimit, ::openpit::param::Asset asset)
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
  ::openpit::param::Asset settlementAsset;

  RateLimitAccountAssetBarrier(RateLimit barrierLimit,
                               ::openpit::param::AccountId account,
                               ::openpit::param::Asset asset)
      : limit(barrierLimit),
        accountId(account),
        settlementAsset(std::move(asset)) {}
};

// Built-in rate-limit policy. At least one barrier axis must be configured.
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
      brokerRaw.window_nanoseconds = m_broker->limit.windowNanoseconds;
      brokerPtr = &brokerRaw;
    }

    std::vector<OpenPitPretradePoliciesRateLimitAssetBarrier> assetRaw;
    assetRaw.reserve(m_assetBarriers.size());
    for (const RateLimitAssetBarrier& barrier : m_assetBarriers) {
      OpenPitPretradePoliciesRateLimitAssetBarrier raw{};
      raw.settlement_asset = ::openpit::detail::Native(barrier.settlementAsset);
      raw.max_orders = barrier.limit.maxOrders;
      raw.window_nanoseconds = barrier.limit.windowNanoseconds;
      assetRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountBarrier> accountRaw;
    accountRaw.reserve(m_accountBarriers.size());
    for (const RateLimitAccountBarrier& barrier : m_accountBarriers) {
      OpenPitPretradePoliciesRateLimitAccountBarrier raw{};
      raw.account_id = ::openpit::detail::Native(barrier.accountId);
      raw.max_orders = barrier.limit.maxOrders;
      raw.window_nanoseconds = barrier.limit.windowNanoseconds;
      accountRaw.push_back(raw);
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountAssetBarrier>
        accountAssetRaw;
    accountAssetRaw.reserve(m_accountAssetBarriers.size());
    for (const RateLimitAccountAssetBarrier& barrier : m_accountAssetBarriers) {
      OpenPitPretradePoliciesRateLimitAccountAssetBarrier raw{};
      raw.account_id = ::openpit::detail::Native(barrier.accountId);
      raw.settlement_asset = ::openpit::detail::Native(barrier.settlementAsset);
      raw.max_orders = barrier.limit.maxOrders;
      raw.window_nanoseconds = barrier.limit.windowNanoseconds;
      accountAssetRaw.push_back(raw);
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_rate_limit_policy(
            ::openpit::detail::Native(builder), m_policyGroupId, brokerPtr,
            assetRaw.data(), assetRaw.size(), accountRaw.data(),
            accountRaw.size(), accountAssetRaw.data(), accountAssetRaw.size(),
            &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_engine_builder_add_builtin_rate_limit_policy failed");
    }
  }

 private:
  std::optional<RateLimitBrokerBarrier> m_broker;
  std::vector<RateLimitAssetBarrier> m_assetBarriers;
  std::vector<RateLimitAccountBarrier> m_accountBarriers;
  std::vector<RateLimitAccountAssetBarrier> m_accountAssetBarriers;
  std::uint16_t m_policyGroupId = ::openpit::param::DefaultPolicyGroupId;
};

//------------------------------------------------------------------------------
// SpotFunds

// Per-instrument slippage override for the spot-funds policy. The target is a
// tagged union: instrument default, instrument+account, or
// instrument+account-group. When `slippageBps` is absent the entry is ignored
// during construction and clears the selected override during
// reconfiguration.
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] OpenPitPretradePoliciesSpotFundsOverride Native()
      const noexcept {
    OpenPitPretradePoliciesSpotFundsOverride raw{};
    raw.target = m_target;
    if (slippageBps) {
      raw.slippage_bps = *slippageBps;
      raw.has_slippage_bps = true;
    }
    return raw;
  }

  [[nodiscard]] static OpenPitPretradePoliciesSpotFundsOverrideTarget
  InstrumentTarget(::openpit::marketdata::InstrumentId instrument) noexcept {
    OpenPitPretradePoliciesSpotFundsOverrideTarget result{};
    result.tag =
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT;
    result.payload.instrument.instrument_id =
        ::openpit::detail::Native(instrument);
    return result;
  }

  [[nodiscard]] static OpenPitPretradePoliciesSpotFundsOverrideTarget
  InstrumentAccountTarget(::openpit::marketdata::InstrumentId instrument,
                          ::openpit::param::AccountId accountId) noexcept {
    OpenPitPretradePoliciesSpotFundsOverrideTarget result{};
    result.tag =
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT;
    result.payload.instrument_account.instrument_id =
        ::openpit::detail::Native(instrument);
    result.payload.instrument_account.account_id =
        ::openpit::detail::Native(accountId);
    return result;
  }

  [[nodiscard]] static OpenPitPretradePoliciesSpotFundsOverrideTarget
  InstrumentAccountGroupTarget(
      ::openpit::marketdata::InstrumentId instrument,
      ::openpit::param::AccountGroupId accountGroupId) noexcept {
    OpenPitPretradePoliciesSpotFundsOverrideTarget result{};
    result.tag =
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT_GROUP;
    result.payload.instrument_account_group.instrument_id =
        ::openpit::detail::Native(instrument);
    result.payload.instrument_account_group.account_group_id =
        ::openpit::detail::Native(accountGroupId);
    return result;
  }

  OpenPitPretradePoliciesSpotFundsOverrideTarget m_target{};
};

/// Account-wide P&L bounds computed by the spot-funds ledger.
///
/// Lower and upper bounds are optional; lower is typically a negative loss
/// limit and upper is typically a positive profit-taking limit. At least one
/// bound must be set whenever a barrier is installed.
struct SpotFundsPnlBoundsBarrier {
  std::optional<::openpit::param::Pnl> lowerBound;
  std::optional<::openpit::param::Pnl> upperBound;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier Native()
      const noexcept {
    OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier raw{};
    raw.lower_bound =
        ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
            lowerBound);
    raw.upper_bound =
        ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
            upperBound);
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
      : barrier(groupBarrier), accountGroupId(groupId) {}

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
  Native() const noexcept {
    OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier raw{};
    raw.account_group_id = ::openpit::detail::Native(accountGroupId);
    raw.barrier = ::openpit::detail::Native(barrier);
    return raw;
  }
};

/// Per-account spot-funds P&L bounds refinement.
struct SpotFundsPnlBoundsAccountBarrier {
  SpotFundsPnlBoundsBarrier barrier;
  ::openpit::param::AccountId accountId;

  /// Creates an account P&L barrier.
  SpotFundsPnlBoundsAccountBarrier(::openpit::param::AccountId account,
                                   SpotFundsPnlBoundsBarrier accountBarrier)
      : barrier(accountBarrier), accountId(account) {}

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier Native()
      const noexcept {
    OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier raw{};
    raw.account_id = ::openpit::detail::Native(accountId);
    raw.barrier = ::openpit::detail::Native(barrier);
    return raw;
  }
};

/// Tri-state runtime update for the singular SpotFunds global P&L barrier.
///
/// Use the explicitly named `Unchanged`, `Clear`, and `Set` factories. Direct
/// construction is intentionally unavailable so legacy `std::nullopt` and
/// barrier arguments fail to compile instead of silently changing meaning. A
/// collection state is not representable because this axis is singular.
class SpotFundsPnlBoundsGlobalBarrierUpdate {
 public:
  /// Leaves the current global barrier unchanged.
  [[nodiscard]] static SpotFundsPnlBoundsGlobalBarrierUpdate
  Unchanged() noexcept {
    return SpotFundsPnlBoundsGlobalBarrierUpdate(false, std::nullopt);
  }

  /// Clears the current global barrier.
  [[nodiscard]] static SpotFundsPnlBoundsGlobalBarrierUpdate Clear() noexcept {
    return SpotFundsPnlBoundsGlobalBarrierUpdate(true, std::nullopt);
  }

  /// Replaces the current global barrier.
  [[nodiscard]] static SpotFundsPnlBoundsGlobalBarrierUpdate Set(
      SpotFundsPnlBoundsBarrier barrier) {
    return SpotFundsPnlBoundsGlobalBarrierUpdate(true, barrier);
  }

  [[nodiscard]] bool HasUpdate() const noexcept { return m_hasUpdate; }

  [[nodiscard]] const std::optional<SpotFundsPnlBoundsBarrier>& Barrier()
      const noexcept {
    return m_barrier;
  }

 private:
  SpotFundsPnlBoundsGlobalBarrierUpdate(
      bool hasUpdate, std::optional<SpotFundsPnlBoundsBarrier> barrier) noexcept
      : m_hasUpdate(hasUpdate), m_barrier(barrier) {}

  bool m_hasUpdate{false};
  std::optional<SpotFundsPnlBoundsBarrier> m_barrier;
};

/// Built-in spot-funds self-computed P&L bounds kill switch.
///
/// This registers the regular `SpotFundsPolicy` name and configures its
/// account P&L-bounds axis. The policy computes realized P&L from
/// reconciled fills instead of trusting an externally supplied P&L figure.
/// Its SpotFunds limit mode is `TrackOnly`: holdings and P&L are updated, but
/// insufficient-funds gating is disabled. Mark pricing is used for market
/// orders.
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
    m_marketData = ::openpit::detail::Native(marketData);
    return *this;
  }

  /// Sets the global account P&L barrier.
  SpotFundsPnlBoundsKillSwitchPolicy& GlobalBarrier(
      SpotFundsPnlBoundsBarrier barrier) {
    m_globalBarrier = barrier;
    return *this;
  }

  /// Adds an account-group account P&L barrier.
  SpotFundsPnlBoundsKillSwitchPolicy& AccountGroupBarrier(
      SpotFundsPnlBoundsAccountGroupBarrier barrier) {
    m_accountGroupBarriers.push_back(barrier);
    return *this;
  }

  /// Adds an account P&L barrier.
  SpotFundsPnlBoundsKillSwitchPolicy& AccountBarrier(
      SpotFundsPnlBoundsAccountBarrier barrier) {
    m_accountBarriers.push_back(barrier);
    return *this;
  }

  /// Registers the policy on `builder`.
  void AddTo(::openpit::EngineBuilder& builder) const {
    OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier globalRaw{};
    const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier* globalPtr = nullptr;
    if (m_globalBarrier) {
      globalRaw = ::openpit::detail::Native(*m_globalBarrier);
      globalPtr = &globalRaw;
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier>
        accountGroupRaw;
    accountGroupRaw.reserve(m_accountGroupBarriers.size());
    for (const SpotFundsPnlBoundsAccountGroupBarrier& barrier :
         m_accountGroupBarriers) {
      accountGroupRaw.push_back(::openpit::detail::Native(barrier));
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier>
        accountRaw;
    accountRaw.reserve(m_accountBarriers.size());
    for (const SpotFundsPnlBoundsAccountBarrier& barrier : m_accountBarriers) {
      accountRaw.push_back(::openpit::detail::Native(barrier));
    }

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(
            ::openpit::detail::Native(builder), m_marketData, m_policyGroupId,
            globalPtr, accountGroupRaw.data(), accountGroupRaw.size(),
            accountRaw.data(), accountRaw.size(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_"
          "killswitch_policy failed");
    }
  }

 private:
  const OpenPitMarketDataService* m_marketData = nullptr;
  std::optional<SpotFundsPnlBoundsBarrier> m_globalBarrier;
  std::vector<SpotFundsPnlBoundsAccountGroupBarrier> m_accountGroupBarriers;
  std::vector<SpotFundsPnlBoundsAccountBarrier> m_accountBarriers;
  std::uint16_t m_policyGroupId = ::openpit::param::DefaultPolicyGroupId;
};

// Built-in spot-funds policy, configured inline (no separate accessors).
//
// By default market orders are rejected (limit-only mode). Call
// `WithMarketOrders` to enable them, supplying the borrowed market-data service
// handle and the worst-case global slippage in basis points. The market-data
// handle is owned by the caller and must outlive registration.
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
    m_marketData = ::openpit::detail::Native(marketData);
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
      overridesRaw.push_back(::openpit::detail::Native(override));
    }

    const std::uint16_t slippage = m_marketSlippageBps.value_or(0);
    const std::uint16_t* slippagePtr =
        m_marketSlippageBps ? &slippage : nullptr;

    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_builtin_spot_funds_policy(
            ::openpit::detail::Native(builder), m_marketData, slippagePtr,
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
  std::uint16_t m_policyGroupId = ::openpit::param::DefaultPolicyGroupId;
};

}  // namespace openpit::pretrade::policies

#include "openpit/pretrade/configurator.hpp"
