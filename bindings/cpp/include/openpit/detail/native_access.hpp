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

#include "openpit/fwd.hpp"

#include <utility>

namespace openpit::detail {

// Central attorney for the binding's private native representations. The
// callable objects published below keep their call operators private; only
// explicitly trusted SDK types may cross the native boundary.
class NativeAccess final {
 private:
  template <typename Wrapper>
  [[nodiscard]] static constexpr auto GetImpl(
      const Wrapper& wrapper, int) noexcept(noexcept(wrapper.Native()))
      -> decltype(wrapper.Native()) {
    return wrapper.Native();
  }

  template <typename Wrapper, typename Native>
  [[nodiscard]] static constexpr auto MakeImpl(Native&& native, int)
      -> decltype(Wrapper(std::forward<Native>(native))) {
    return Wrapper(std::forward<Native>(native));
  }

  template <typename Wrapper, typename Native>
  [[nodiscard]] static constexpr auto MakeImpl(Native&& native, long)
      -> decltype(Wrapper::FromRaw(std::forward<Native>(native))) {
    return Wrapper::FromRaw(std::forward<Native>(native));
  }

  template <typename Wrapper>
  [[nodiscard]] static constexpr decltype(auto) Get(
      const Wrapper& wrapper) noexcept(noexcept(NativeAccess::GetImpl(wrapper,
                                                                      0))) {
    return NativeAccess::GetImpl(wrapper, 0);
  }

  template <typename Wrapper, typename Native>
  [[nodiscard]] static constexpr decltype(auto) Make(Native&& native) {
    return NativeAccess::MakeImpl<Wrapper>(std::forward<Native>(native), 0);
  }

  struct GetOperation {
    template <typename Wrapper>
    [[nodiscard]] static constexpr decltype(auto) Invoke(
        const Wrapper& wrapper) noexcept(noexcept(NativeAccess::Get(wrapper))) {
      return NativeAccess::Get(wrapper);
    }
  };

  template <typename Wrapper>
  struct MakeOperation {
    template <typename Native>
    [[nodiscard]] static constexpr decltype(auto) Invoke(Native&& native) {
      return NativeAccess::Make<Wrapper>(std::forward<Native>(native));
    }
  };

  template <typename Operation>
  class Function final {
   public:
    constexpr Function() noexcept = default;

   private:
    template <typename... Arguments>
    [[nodiscard]] constexpr decltype(auto) operator()(Arguments&&... arguments)
        const noexcept(noexcept(
            Operation::Invoke(std::forward<Arguments>(arguments)...))) {
      return Operation::Invoke(std::forward<Arguments>(arguments)...);
    }

    friend class ::openpit::BytesView;
    friend class ::openpit::Configurator;
    friend class ::openpit::Engine;
    friend class ::openpit::EngineBuilder;
    friend class ::openpit::ExecutionReport;
    friend class ::openpit::InstrumentId;
    friend class ::openpit::Order;
    friend class ::openpit::ReferenceBook;
    friend class ::openpit::SharedBytes;
    friend class ::openpit::SharedString;
    friend class ::openpit::StringView;
    friend class ::openpit::detail::ErrorAccess;
    friend struct ::openpit::AdjustmentResult;
    friend struct ::openpit::PolicyConfigurationResult;
    friend struct ::openpit::PostTradeResult;
    friend struct ::openpit::SettlementLag;
    friend struct ::openpit::SettlementScheme;

    friend class ::openpit::param::AccountGroupId;
    friend class ::openpit::param::AccountId;
    friend class ::openpit::param::AdjustmentAmount;
    friend class ::openpit::param::Asset;
    friend class ::openpit::param::CashFlow;
    friend class ::openpit::param::Fee;
    friend class ::openpit::param::GroupId;
    friend class ::openpit::param::Leverage;
    friend class ::openpit::param::MonetaryAmount;
    friend class ::openpit::param::Notional;
    friend class ::openpit::param::Pnl;
    friend class ::openpit::param::PositionSize;
    friend class ::openpit::param::Price;
    friend class ::openpit::param::Quantity;
    friend class ::openpit::param::Volume;
    template <typename Derived, typename Traits>
    friend class ::openpit::param::detail::ExactValue;
    friend class ::openpit::param::detail::AdjustmentAmountAccess;
    friend class ::openpit::param::detail::LeverageAccess;
    friend class ::openpit::param::detail::MonetaryAmountAccess;
    friend class ::openpit::param::detail::ValueOperations;

    friend class ::openpit::model::ExecutionReport;
    friend class ::openpit::model::Order;
    friend class ::openpit::model::TradeAmount;
    friend struct ::openpit::model::ExecutionReportOperation;
    friend struct ::openpit::model::Fill;
    friend struct ::openpit::model::FinancialImpact;
    friend struct ::openpit::model::Instrument;
    friend struct ::openpit::model::OrderMargin;
    friend struct ::openpit::model::OrderOperation;
    friend struct ::openpit::model::OrderPosition;
    friend struct ::openpit::model::PositionImpact;
    friend struct ::openpit::model::Trade;

    friend class ::openpit::pretrade::AccountOutcomes;
    friend class ::openpit::pretrade::Context;
    friend class ::openpit::pretrade::DryRunReport;
    friend class ::openpit::pretrade::DropCopyOperation;
    friend class ::openpit::pretrade::PostTradeAdjustments;
    friend class ::openpit::pretrade::PostTradeContext;
    friend class ::openpit::pretrade::PostTradePnls;
    friend class ::openpit::pretrade::PreTradeLock;
    friend class ::openpit::pretrade::Request;
    friend class ::openpit::pretrade::Reservation;
    friend class ::openpit::pretrade::Result;
    template <typename Handler>
    friend class ::openpit::pretrade::CustomPolicy;
    friend struct ::openpit::pretrade::DropCopyResult;
    friend struct ::openpit::pretrade::ExecuteResult;
    friend struct ::openpit::pretrade::LockEntry;
    friend struct ::openpit::pretrade::PolicyAccountAdjustmentResult;
    friend struct ::openpit::pretrade::PolicyDecision;
    friend struct ::openpit::pretrade::StartResult;
    friend class ::openpit::pretrade::detail::CustomPolicyAccess;
    friend class ::openpit::pretrade::detail::ListAccess;

    friend struct ::openpit::reject::Reject;
    friend class ::openpit::tx::Mutations;

    friend class ::openpit::pretrade::policies::OrderSizeLimitPolicy;
    friend class ::openpit::pretrade::policies::OrderValidationPolicy;
    friend class ::openpit::pretrade::policies::PnlBoundsKillSwitchPolicy;
    friend class ::openpit::pretrade::policies::RateLimitPolicy;
    friend class ::openpit::pretrade::policies::
        SpotFundsPnlBoundsGlobalBarrierUpdate;
    friend class ::openpit::pretrade::policies::
        SpotFundsPnlBoundsKillSwitchPolicy;
    friend class ::openpit::pretrade::policies::SpotFundsPolicy;
    friend struct ::openpit::pretrade::policies::OrderSizeAccountAssetBarrier;
    friend struct ::openpit::pretrade::policies::OrderSizeAssetBarrier;
    friend struct ::openpit::pretrade::policies::OrderSizeBrokerBarrier;
    friend struct ::openpit::pretrade::policies::PnlBoundsAccountBarrier;
    friend struct ::openpit::pretrade::policies::PnlBoundsAccountBarrierUpdate;
    friend struct ::openpit::pretrade::policies::PnlBoundsBrokerBarrier;
    friend struct ::openpit::pretrade::policies::RateLimit;
    friend struct ::openpit::pretrade::policies::RateLimitAccountAssetBarrier;
    friend struct ::openpit::pretrade::policies::RateLimitAccountBarrier;
    friend struct ::openpit::pretrade::policies::RateLimitAssetBarrier;
    friend struct ::openpit::pretrade::policies::RateLimitBrokerBarrier;
    friend struct ::openpit::pretrade::policies::SpotFundsOverride;
    friend struct ::openpit::pretrade::policies::
        SpotFundsPnlBoundsAccountBarrier;
    friend struct ::openpit::pretrade::policies::
        SpotFundsPnlBoundsAccountGroupBarrier;
    friend struct ::openpit::pretrade::policies::SpotFundsPnlBoundsBarrier;
    friend class ::openpit::pretrade::policies::detail::OrderSizeOptionalAccess;
    friend class ::openpit::pretrade::policies::detail::PnlOptionalAccess;

    friend class ::openpit::accounts::AccountControl;
    friend class ::openpit::accounts::Accounts;
    friend struct ::openpit::accounts::AccountBlock;
    friend struct ::openpit::accounts::AccountBlockError;
    friend struct ::openpit::accounts::AccountGroupError;

    friend class ::openpit::accountadjustment::AccountPnlOperation;
    friend class ::openpit::accountadjustment::BatchError;
    friend class ::openpit::accountadjustment::Context;
    friend class ::openpit::accountadjustment::OutcomeList;
    friend struct ::openpit::accountadjustment::AccountAdjustment;
    friend struct ::openpit::accountadjustment::AccountOutcomeEntry;
    friend class ::openpit::accountadjustment::AccountPnlOutcome;
    friend struct ::openpit::accountadjustment::Amount;
    friend struct ::openpit::accountadjustment::BalanceOperation;
    friend struct ::openpit::accountadjustment::Bounds;
    friend class ::openpit::accountadjustment::Operation;
    friend struct ::openpit::accountadjustment::Outcome;
    friend struct ::openpit::accountadjustment::OutcomeAmount;
    friend class ::openpit::accountadjustment::PnlOutcome;
    friend struct ::openpit::accountadjustment::PnlOutcomeAmount;
    friend struct ::openpit::accountadjustment::PositionOperation;
    friend class ::openpit::accountadjustment::detail::PnlStateAccess;

    friend class ::openpit::marketdata::Builder;
    friend class ::openpit::marketdata::Quote;
    friend class ::openpit::marketdata::QuoteTtl;
    friend class ::openpit::marketdata::Service;
    friend struct ::openpit::marketdata::GetResult;
    friend struct ::openpit::marketdata::RegisterResult;

    friend class ::openpit::asyncengine::EngineAdapter;
    friend class ::openpit::asyncengine::NoopObserver;
    friend class ::openpit::asyncengine::Observer;
    friend class ::openpit::asyncengine::OwnedTypedAsyncEngine;
    template <typename Value>
    friend class ::openpit::asyncengine::Future;
    template <typename First, typename Second>
    friend class ::openpit::asyncengine::PairFuture;
    template <typename Value>
    friend class ::openpit::asyncengine::Promise;
    template <typename First, typename Second>
    friend class ::openpit::asyncengine::PairPromise;
    template <typename Value>
    friend class ::openpit::asyncengine::Result;
    template <typename Driver>
    friend class ::openpit::asyncengine::AsyncAccounts;
    template <typename Driver>
    friend class ::openpit::asyncengine::AsyncDropCopyOperation;
    template <typename Driver>
    friend class ::openpit::asyncengine::AsyncRequest;
    template <typename Driver>
    friend class ::openpit::asyncengine::AsyncReservation;
    template <typename Driver>
    friend class ::openpit::asyncengine::Builder;
    template <typename Driver>
    friend class ::openpit::asyncengine::DynamicBuilder;
    template <typename Driver>
    friend class ::openpit::asyncengine::ShardedBuilder;
    template <typename Driver>
    friend class ::openpit::asyncengine::TypedAsyncEngine;
    template <typename Driver>
    friend class ::openpit::asyncengine::TypedBuilder;
    template <typename Driver>
    friend class ::openpit::asyncengine::TypedDynamicBuilder;
    template <typename Driver>
    friend class ::openpit::asyncengine::TypedShardedBuilder;
    friend struct ::openpit::asyncengine::AdjustmentOutcome;
    friend class ::openpit::asyncengine::detail::Base;
    template <typename Driver>
    friend struct ::openpit::asyncengine::DropCopyOutcome;
    template <typename Driver>
    friend struct ::openpit::asyncengine::ExecuteOutcome;
    template <typename Driver>
    friend struct ::openpit::asyncengine::StartOutcome;
  };

 public:
  [[nodiscard]] static constexpr auto GetFunction() noexcept {
    return Function<GetOperation>{};
  }

  template <typename Wrapper>
  [[nodiscard]] static constexpr auto MakeFunction() noexcept {
    return Function<MakeOperation<Wrapper>>{};
  }
};

inline constexpr auto Native = NativeAccess::GetFunction();

template <typename Wrapper>
inline constexpr auto FromNative = NativeAccess::MakeFunction<Wrapper>();

}  // namespace openpit::detail
