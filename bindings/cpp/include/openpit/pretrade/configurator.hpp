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

#include "openpit/pretrade/policies.hpp"
#include "openpit/string.hpp"

namespace openpit {

// Runtime policy-settings updater bound to an engine. Every operation throws
// `openpit::ConfigureError` on domain or configuration failures.
//
// The SpotFunds retune methods - `SpotFunds`, `SpotFundsPnlBoundsKillSwitch`,
// `SpotFundsGlobalLimitMode`, `SpotFundsAccountLimitMode`,
// `SpotFundsPositionLimit`, and `SpotFundsAccountGroupLimitMode` - are
// exclusive operations in every mode, including `SyncPolicy::Full`: run each
// with no other engine call in flight, as `asyncengine::AsyncEngine::Exclusive`
// does.
class Configurator {
 public:
  explicit Configurator(const ::openpit::Engine& engine) noexcept
      : m_engine(::openpit::detail::Native(engine)) {}

  /// A set axis replaces every mode on it; an empty vector clears the axis.
  /// An omitted axis is unchanged. A surviving (key, mode) keeps its live
  /// counter. A repeated (key, mode) in one call throws ConfigureError
  /// (Validation) and leaves the settings unchanged.
  /// Clearing an account or account+asset barrier keeps its sliding-window log;
  /// re-adding the (key, mode) resumes counting within that window.
  void RateLimit(
      std::string_view name,
      std::optional<
          std::vector<::openpit::pretrade::policies::RateLimitBrokerBarrier>>
          brokers = std::nullopt,
      std::optional<
          std::vector<::openpit::pretrade::policies::RateLimitAssetBarrier>>
          assets = std::nullopt,
      std::optional<
          std::vector<::openpit::pretrade::policies::RateLimitAccountBarrier>>
          accounts = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::RateLimitAccountAssetBarrier>>
          accountAssets = std::nullopt) const {
    std::vector<OpenPitPretradePoliciesRateLimitBrokerBarrier> brokerRaw;
    if (brokers) {
      brokerRaw.reserve(brokers->size());
      for (const auto& barrier : *brokers) {
        OpenPitPretradePoliciesRateLimitBrokerBarrier raw{};
        raw.max_orders = barrier.limit.maxOrders;
        raw.window_nanoseconds = barrier.limit.windowNanoseconds;
        raw.mode = static_cast<OpenPitPretradeRuleMode>(barrier.mode);
        brokerRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesRateLimitAssetBarrier> assetRaw;
    if (assets) {
      assetRaw.reserve(assets->size());
      for (const auto& barrier : *assets) {
        OpenPitPretradePoliciesRateLimitAssetBarrier raw{};
        raw.settlement_asset =
            ::openpit::detail::Native(barrier.settlementAsset);
        raw.max_orders = barrier.limit.maxOrders;
        raw.window_nanoseconds = barrier.limit.windowNanoseconds;
        raw.mode = static_cast<OpenPitPretradeRuleMode>(barrier.mode);
        assetRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountBarrier> accountRaw;
    if (accounts) {
      accountRaw.reserve(accounts->size());
      for (const auto& barrier : *accounts) {
        OpenPitPretradePoliciesRateLimitAccountBarrier raw{};
        raw.account_id = ::openpit::detail::Native(barrier.accountId);
        raw.max_orders = barrier.limit.maxOrders;
        raw.window_nanoseconds = barrier.limit.windowNanoseconds;
        raw.mode = static_cast<OpenPitPretradeRuleMode>(barrier.mode);
        accountRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesRateLimitAccountAssetBarrier>
        accountAssetRaw;
    if (accountAssets) {
      accountAssetRaw.reserve(accountAssets->size());
      for (const auto& barrier : *accountAssets) {
        OpenPitPretradePoliciesRateLimitAccountAssetBarrier raw{};
        raw.account_id = ::openpit::detail::Native(barrier.accountId);
        raw.settlement_asset =
            ::openpit::detail::Native(barrier.settlementAsset);
        raw.max_orders = barrier.limit.maxOrders;
        raw.window_nanoseconds = barrier.limit.windowNanoseconds;
        raw.mode = static_cast<OpenPitPretradeRuleMode>(barrier.mode);
        accountAssetRaw.push_back(raw);
      }
    }

    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_rate_limit(
            m_engine, ::openpit::detail::MakeStringView(name), brokerRaw.data(),
            brokerRaw.size(), brokers.has_value(), assetRaw.data(),
            assetRaw.size(), assets.has_value(), accountRaw.data(),
            accountRaw.size(), accounts.has_value(), accountAssetRaw.data(),
            accountAssetRaw.size(), accountAssets.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_rate_limit failed");
    }
  }

  void OrderSizeLimit(
      std::string_view name,
      ::openpit::pretrade::policies::OrderSizeBrokerBarrierUpdate broker =
          ::openpit::pretrade::policies::OrderSizeBrokerBarrierUpdate::
              Unchanged(),
      std::optional<
          std::vector<::openpit::pretrade::policies::OrderSizeAssetBarrier>>
          assets = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::OrderSizeAccountAssetBarrier>>
          accountAssets = std::nullopt) const {
    OpenPitPretradePoliciesOrderSizeBrokerBarrier brokerRaw{};
    const OpenPitPretradePoliciesOrderSizeBrokerBarrier* brokerPtr = nullptr;
    if (broker.Barrier()) {
      brokerRaw.limit.max_quantity = ::openpit::pretrade::policies::detail::
          OrderSizeOptionalAccess::Native(broker.Barrier()->limit.maxQuantity);
      brokerRaw.limit.max_notional = ::openpit::pretrade::policies::detail::
          OrderSizeOptionalAccess::Native(broker.Barrier()->limit.maxNotional);
      brokerPtr = &brokerRaw;
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAssetBarrier> assetRaw;
    if (assets) {
      assetRaw.reserve(assets->size());
      for (const auto& barrier : *assets) {
        OpenPitPretradePoliciesOrderSizeAssetBarrier raw{};
        raw.limit.max_quantity = ::openpit::pretrade::policies::detail::
            OrderSizeOptionalAccess::Native(barrier.limit.maxQuantity);
        raw.limit.max_notional = ::openpit::pretrade::policies::detail::
            OrderSizeOptionalAccess::Native(barrier.limit.maxNotional);
        raw.asset = ::openpit::detail::Native(barrier.asset);
        assetRaw.push_back(raw);
      }
    }

    std::vector<OpenPitPretradePoliciesOrderSizeAccountAssetBarrier>
        accountAssetRaw;
    if (accountAssets) {
      accountAssetRaw.reserve(accountAssets->size());
      for (const auto& barrier : *accountAssets) {
        OpenPitPretradePoliciesOrderSizeAccountAssetBarrier raw{};
        raw.limit.max_quantity = ::openpit::pretrade::policies::detail::
            OrderSizeOptionalAccess::Native(barrier.limit.maxQuantity);
        raw.limit.max_notional = ::openpit::pretrade::policies::detail::
            OrderSizeOptionalAccess::Native(barrier.limit.maxNotional);
        raw.account_id = ::openpit::detail::Native(barrier.accountId);
        raw.asset = ::openpit::detail::Native(barrier.asset);
        accountAssetRaw.push_back(raw);
      }
    }

    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_order_size_limit(
            m_engine, ::openpit::detail::MakeStringView(name), brokerPtr,
            broker.HasUpdate(), assetRaw.data(), assetRaw.size(),
            assets.has_value(), accountAssetRaw.data(), accountAssetRaw.size(),
            accountAssets.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_order_size_limit failed");
    }
  }

  /// Replaces supplied review-rule levels. Nullopt keeps an array level;
  /// an empty vector clears it. The global update can keep, clear or set its
  /// rule. Invalid updates throw ConfigureError and leave all levels intact.
  void ReviewRule(
      std::string_view name,
      ::openpit::pretrade::policies::ReviewRuleGlobalRuleUpdate global = ::
          openpit::pretrade::policies::ReviewRuleGlobalRuleUpdate::Unchanged(),
      std::optional<std::vector<
          ::openpit::pretrade::policies::ReviewRuleAccountGroupRule>>
          groups = std::nullopt,
      std::optional<
          std::vector<::openpit::pretrade::policies::ReviewRuleAccountRule>>
          accounts = std::nullopt) const {
    const ::openpit::pretrade::policies::detail::ReviewRuleArguments raw(
        global.Rule(), groups ? &*groups : nullptr,
        accounts ? &*accounts : nullptr);
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_review_rule(
            m_engine, ::openpit::detail::MakeStringView(name),
            raw.m_global ? &*raw.m_global : nullptr, global.HasUpdate(),
            raw.m_groups.data(), raw.m_groups.size(), groups.has_value(),
            raw.m_accounts.data(), raw.m_accounts.size(), accounts.has_value(),
            &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_review_rule failed");
    }
  }

  /// Replaces the supplied barrier lists; nullopt leaves an axis unchanged.
  /// Keys include mode: (asset, mode) for brokers, (account, asset, mode) for
  /// accounts. Reject and review barriers may coexist.
  /// See ::openpit::pretrade::policies::PnlBoundsKillSwitchPolicy for how
  /// barrier breaches are handled.
  /// Throws ConfigureError with kind Validation for invalid modes, duplicate
  /// keys or invalid bounds; the previous barriers remain in force.
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
            ::openpit::detail::Native(barrier.settlementAsset);
        raw.mode = static_cast<OpenPitPretradeRuleMode>(barrier.mode);
        raw.lower_bound =
            ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
                barrier.lowerBound);
        raw.upper_bound =
            ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
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
        raw.account_id = ::openpit::detail::Native(barrier.accountId);
        raw.settlement_asset =
            ::openpit::detail::Native(barrier.settlementAsset);
        raw.mode = static_cast<OpenPitPretradeRuleMode>(barrier.mode);
        raw.lower_bound =
            ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
                barrier.lowerBound);
        raw.upper_bound =
            ::openpit::pretrade::policies::detail::PnlOptionalAccess::Native(
                barrier.upperBound);
        accountRaw.push_back(raw);
      }
    }

    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_pnl_bounds_killswitch(
            m_engine, ::openpit::detail::MakeStringView(name), brokerRaw.data(),
            brokerRaw.size(), brokers.has_value(), accountRaw.data(),
            accountRaw.size(), accounts.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_pnl_bounds_killswitch failed");
    }
  }

  void SetAccountPnl(std::string_view name,
                     ::openpit::param::AccountId accountId,
                     const ::openpit::param::Asset& settlementAsset,
                     ::openpit::param::Pnl pnl) const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(
            m_engine, ::openpit::detail::MakeStringView(name),
            ::openpit::detail::Native(accountId),
            ::openpit::detail::Native(settlementAsset),
            ::openpit::detail::Native(pnl), &error)) {
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
        overridesRaw.push_back(::openpit::detail::Native(override));
      }
    }

    OpenPitConfigureError* error = nullptr;
    const std::uint8_t source =
        pricingSource
            ? static_cast<std::uint8_t>(*pricingSource)
            : static_cast<std::uint8_t>(
                  ::openpit::pretrade::policies::SpotFundsPricingSource::Mark);
    if (!openpit_engine_configure_spot_funds(
            m_engine, ::openpit::detail::MakeStringView(name),
            globalSlippageBps.value_or(0), globalSlippageBps.has_value(),
            source, pricingSource.has_value(), overridesRaw.data(),
            overridesRaw.size(), overrides.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_spot_funds failed");
    }
  }

  /// Retunes the SpotFunds self-computed P&L bounds axis.
  ///
  /// Optional vectors are PATCH tiers: `std::nullopt` leaves a tier unchanged,
  /// an engaged empty vector clears it, and a list replaces every mode.
  /// Repeated (key, mode) entries throw ConfigureError without changes.
  /// Account updates preserve each live accumulated P&L value.
  /// With a known effective account currency, an account-tier mismatch fails
  /// closed and blocks the account; account-group and global mismatches are
  /// skipped. A known-currency account with no account barrier and no matching
  /// fallback has no effective barrier, but its P&L keeps accumulating and
  /// publishing. Without an effective currency, the first in-scope barrier
  /// applies. Bounds are compared as stored and are never FX-converted.
  ///
  /// An account whose effective barrier changed is evaluated against its
  /// stored account P&L in the same call: an already halted account, or one
  /// beyond a new Reject-mode barrier, is blocked before this call returns and
  /// its block is reported here as already recorded by the engine. Each
  /// returned `accounts::AccountBlockOutcome` names the account that owns its
  /// newly inserted block. Removing the last effective barrier reports no
  /// block and does not release an existing block. Clearing an override can
  /// expose a fallback barrier; the fallback is evaluated normally and may
  /// record and report a block.
  /// Review-mode breaches yield review marks during evaluation without
  /// blocking. Evaluation failures block in every mode.
  [[nodiscard]] ::openpit::AccountBlockOutcomes SpotFundsPnlBoundsKillSwitch(
      std::string_view name,
      std::optional<
          std::vector<::openpit::pretrade::policies::SpotFundsPnlBoundsBarrier>>
          global = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::SpotFundsPnlBoundsAccountGroupBarrier>>
          accountGroups = std::nullopt,
      std::optional<std::vector<
          ::openpit::pretrade::policies::SpotFundsPnlBoundsAccountBarrier>>
          accounts = std::nullopt) const {
    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier> globalRaw;
    if (global) {
      globalRaw.reserve(global->size());
      for (const auto& barrier : *global) {
        globalRaw.push_back(::openpit::detail::Native(barrier));
      }
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier>
        accountGroupRaw;
    if (accountGroups) {
      accountGroupRaw.reserve(accountGroups->size());
      for (const auto& barrier : *accountGroups) {
        accountGroupRaw.push_back(::openpit::detail::Native(barrier));
      }
    }

    std::vector<OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier>
        accountRaw;
    if (accounts) {
      accountRaw.reserve(accounts->size());
      for (const auto& barrier : *accounts) {
        accountRaw.push_back(::openpit::detail::Native(barrier));
      }
    }

    OpenPitConfigureError* error = nullptr;
    OpenPitPretradeAccountBlockOutcomeList* blocks =
        openpit_engine_configure_spot_funds_pnl_bounds_killswitch(
            m_engine, ::openpit::detail::MakeStringView(name), globalRaw.data(),
            globalRaw.size(), global.has_value(), accountGroupRaw.data(),
            accountGroupRaw.size(), accountGroups.has_value(),
            accountRaw.data(), accountRaw.size(), accounts.has_value(), &error);
    if (blocks == nullptr) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_pnl_bounds_killswitch failed");
    }
    ::openpit::AccountBlockOutcomes result;
    result.accountBlocks =
        ::openpit::pretrade::detail::ListAccess::DrainAccountBlockOutcomes(
            blocks);
    return result;
  }

  /// Replaces one SpotFunds live account P&L accumulator with a numeric value.
  ///
  /// This is separate from barrier retuning and re-arms the accumulator after
  /// a calculation halt. It does not affect any position-level accumulator.
  /// A value outside an effective Reject-mode bound returns the policy-reported
  /// block, even when the account already has a block. A breach of a
  /// review-mode bound returns no block and no review mark: configuration does
  /// not issue review marks. The engine processes the block request before
  /// returning and preserves the existing first cause. Otherwise the returned
  /// block list is empty.
  [[nodiscard]] ::openpit::PolicyConfigurationResult SetSpotFundsAccountPnl(
      std::string_view name, ::openpit::param::AccountId accountId,
      ::openpit::param::Pnl pnl) const {
    return SetSpotFundsAccountPnlRaw(
        name, accountId,
        ::openpit::accountadjustment::detail::PnlStateAccess::Native(pnl));
  }

  /// Replaces one SpotFunds live account P&L accumulator with a halt reason.
  ///
  /// This is separate from barrier retuning and does not affect any
  /// position-level accumulator. When an effective account P&L barrier of any
  /// mode is configured, the result reports the policy block even when the
  /// account is already blocked. The engine processes the block request before
  /// returning and preserves the existing first cause.
  [[nodiscard]] ::openpit::PolicyConfigurationResult SetSpotFundsAccountPnl(
      std::string_view name, ::openpit::param::AccountId accountId,
      ::openpit::accountadjustment::PnlHaltReason reason) const {
    return SetSpotFundsAccountPnlRaw(
        name, accountId,
        ::openpit::accountadjustment::detail::PnlStateAccess::Native(reason));
  }

  /// Sets the global insufficient-funds mode: Enforce, TrackOnly,
  /// ReviewWithHold, or ReviewWithoutHold. Unknown modes throw ConfigureError
  /// with ConfigureErrorKind::Validation, leaving the previous mode unchanged.
  void SpotFundsGlobalLimitMode(
      std::string_view name,
      ::openpit::pretrade::policies::SpotFundsLimitMode mode) const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_global_limit_mode(
            m_engine, ::openpit::detail::MakeStringView(name),
            static_cast<std::uint8_t>(mode), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_global_limit_mode failed");
    }
  }

  /// Pins Enforce, TrackOnly, ReviewWithHold, or ReviewWithoutHold for an
  /// account; std::nullopt clears the pin and restores the cascade.
  /// Unknown modes throw ConfigureError with ConfigureErrorKind::Validation,
  /// leaving the previous mode unchanged.
  void SpotFundsAccountLimitMode(
      std::string_view name, ::openpit::param::AccountId accountId,
      std::optional<::openpit::pretrade::policies::SpotFundsLimitMode> mode)
      const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_account_limit_mode(
            m_engine, ::openpit::detail::MakeStringView(name),
            ::openpit::detail::Native(accountId),
            mode ? static_cast<std::uint8_t>(*mode) : 0, mode.has_value(),
            &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_account_limit_mode "
          "failed");
    }
  }

  /// Pins, replaces, or clears the inclusive long and short position magnitude
  /// limit for an account, asset and mode in the named SpotFunds policy. The
  /// checked position starts with recorded available + held, including a
  /// negative held residual. A long projection adds open positive incoming; a
  /// short projection subtracts open positive held. Only the side the order
  /// moves toward is checked, so a position-reducing order passes while that
  /// projection stays within the limit. A Reject breach rejects with
  /// PositionLimitExceeded at Order scope. A review breach adds a
  /// PositionLimitExceeded review mark with the limit's mode. A projection that
  /// cannot be computed exactly rejects with ArithmeticOverflow at Order scope
  /// in every mode.
  /// A change applies from the next order and does not re-evaluate open
  /// reservations.
  /// `std::nullopt` clears only this mode's limit; other modes are unchanged
  /// by either upsert or clear. Modes are Reject, ReviewWithHold and
  /// ReviewWithoutHold. Zero means Reject; unknown modes throw ConfigureError
  /// with ConfigureErrorKind::Validation.
  void SpotFundsPositionLimit(
      std::string_view name, ::openpit::param::AccountId accountId,
      const ::openpit::param::Asset& asset, ::openpit::reject::RuleMode mode,
      std::optional<::openpit::param::Quantity> limit) const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_position_limit(
            m_engine, ::openpit::detail::MakeStringView(name),
            ::openpit::detail::Native(accountId),
            ::openpit::detail::Native(asset),
            static_cast<OpenPitPretradeRuleMode>(mode),
            limit ? ::openpit::detail::Native(*limit) : OpenPitParamQuantity{},
            limit.has_value(), &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_spot_funds_position_limit failed");
    }
  }

  /// Pins Enforce, TrackOnly, ReviewWithHold, or ReviewWithoutHold for a group;
  /// std::nullopt clears the pin and restores the global mode.
  /// Unknown modes throw ConfigureError with ConfigureErrorKind::Validation,
  /// leaving the previous mode unchanged.
  void SpotFundsAccountGroupLimitMode(
      std::string_view name, ::openpit::param::AccountGroupId accountGroupId,
      std::optional<::openpit::pretrade::policies::SpotFundsLimitMode> mode)
      const {
    OpenPitConfigureError* error = nullptr;
    if (!openpit_engine_configure_spot_funds_account_group_limit_mode(
            m_engine, ::openpit::detail::MakeStringView(name),
            ::openpit::detail::Native(accountGroupId),
            mode ? static_cast<std::uint8_t>(*mode) : 0, mode.has_value(),
            &error)) {
      ::openpit::detail::ThrowFromConfigureError(
          error,
          "openpit_engine_configure_spot_funds_account_group_limit_mode "
          "failed");
    }
  }

 private:
  [[nodiscard]] ::openpit::PolicyConfigurationResult SetSpotFundsAccountPnlRaw(
      std::string_view name, ::openpit::param::AccountId accountId,
      OpenPitPnlState state) const {
    OpenPitConfigureError* error = nullptr;
    OpenPitPretradeAccountBlockList* blocks =
        openpit_engine_configure_spot_funds_set_account_pnl(
            m_engine, ::openpit::detail::MakeStringView(name),
            ::openpit::detail::Native(accountId), state, &error);
    if (blocks == nullptr) {
      ::openpit::detail::ThrowFromConfigureError(
          error, "openpit_engine_configure_spot_funds_set_account_pnl failed");
    }
    ::openpit::PolicyConfigurationResult result;
    result.accountBlocks =
        ::openpit::pretrade::detail::ListAccess::DrainAccountBlocks(blocks);
    return result;
  }

  OpenPitEngine* m_engine = nullptr;
};

}  // namespace openpit

namespace openpit {

[[nodiscard]] inline ::openpit::Configurator Engine::Configure()
    const noexcept {
  return ::openpit::Configurator(*this);
}

}  // namespace openpit
