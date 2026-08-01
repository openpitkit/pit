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

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts/accounts.hpp"
#include "openpit/error.hpp"
#include "openpit/param/param.hpp"
#include "openpit/pretrade/decision.hpp"

#include <openpit.h>

#include <optional>
#include <vector>

// Non-owning collectors and contexts passed to custom-policy callbacks. Every
// wrapper is valid only for the duration of the callback that created it.

namespace openpit::accountadjustment {

class Context {
 public:
  Context(const Context&) = delete;
  Context& operator=(const Context&) = delete;
  Context(Context&&) = delete;
  Context& operator=(Context&&) = delete;

  [[nodiscard]] ::openpit::accounts::AccountControl AccountControl() const {
    OpenPitAccountControl* control =
        openpit_account_adjustment_context_get_account_control(m_native);
    if (control == nullptr) {
      throw ::openpit::Error(
          "openpit_account_adjustment_context_get_account_control failed");
    }
    return ::openpit::detail::FromNative<::openpit::accounts::AccountControl>(
        control);
  }

  [[nodiscard]] std::optional<::openpit::param::AccountGroupId> AccountGroup()
      const {
    OpenPitParamAccountGroupId group = 0;
    if (!openpit_account_adjustment_context_get_account_group(m_native,
                                                              &group)) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<::openpit::param::AccountGroupId>(
        group);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit Context(const OpenPitAccountAdjustmentContext* native) noexcept
      : m_native(native) {}

  [[nodiscard]] const OpenPitAccountAdjustmentContext* Native() const noexcept {
    return m_native;
  }

  const OpenPitAccountAdjustmentContext* m_native = nullptr;
};

}  // namespace openpit::accountadjustment

namespace openpit::pretrade {

class Result {
 public:
  // Ordinary pre-trade keeps these contributions only on acceptance.
  // Drop-copy also keeps them with ordinary, non-enforcing rejects;
  // evaluation-failure rejects abort drop-copy and discard them.
  Result(const Result&) = delete;
  Result& operator=(const Result&) = delete;
  Result(Result&&) = delete;
  Result& operator=(Result&&) = delete;

  void PushLockPrice(const ::openpit::param::Price& price) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_pre_trade_result_push_lock_price(
            m_native, ::openpit::detail::Native(price), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_pre_trade_result_push_lock_price failed");
    }
  }

  void PushAccountAdjustment(
      const ::openpit::accountadjustment::AccountOutcomeEntry& entry) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_pre_trade_result_push_account_adjustment(
            m_native, ::openpit::detail::Native(entry), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_pretrade_pre_trade_result_push_account_adjustment failed");
    }
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit Result(OpenPitPretradePreTradeResult* native) noexcept
      : m_native(native) {}

  [[nodiscard]] OpenPitPretradePreTradeResult* Native() const noexcept {
    return m_native;
  }

  OpenPitPretradePreTradeResult* m_native = nullptr;
};

class PostTradeContext {
 public:
  PostTradeContext(const PostTradeContext&) = delete;
  PostTradeContext& operator=(const PostTradeContext&) = delete;
  PostTradeContext(PostTradeContext&&) = delete;
  PostTradeContext& operator=(PostTradeContext&&) = delete;

  [[nodiscard]] std::optional<::openpit::param::AccountGroupId> AccountGroup()
      const {
    OpenPitParamAccountGroupId group = 0;
    if (!openpit_post_trade_context_get_account_group(m_native, &group)) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<::openpit::param::AccountGroupId>(
        group);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit PostTradeContext(const OpenPitPostTradeContext* native) noexcept
      : m_native(native) {}

  [[nodiscard]] const OpenPitPostTradeContext* Native() const noexcept {
    return m_native;
  }

  const OpenPitPostTradeContext* m_native = nullptr;
};

class PostTradeAdjustments {
 public:
  PostTradeAdjustments(const PostTradeAdjustments&) = delete;
  PostTradeAdjustments& operator=(const PostTradeAdjustments&) = delete;
  PostTradeAdjustments(PostTradeAdjustments&&) = delete;
  PostTradeAdjustments& operator=(PostTradeAdjustments&&) = delete;

  void Push(::openpit::param::GroupId policyGroupId,
            const ::openpit::accountadjustment::AccountOutcomeEntry& entry) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_post_trade_adjustment_list_push(
            m_native, ::openpit::detail::Native(policyGroupId),
            ::openpit::detail::Native(entry), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_post_trade_adjustment_list_push failed");
    }
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit PostTradeAdjustments(OpenPitPostTradeAdjustmentList* native) noexcept
      : m_native(native) {}

  [[nodiscard]] OpenPitPostTradeAdjustmentList* Native() const noexcept {
    return m_native;
  }

  OpenPitPostTradeAdjustmentList* m_native = nullptr;
};

class PostTradePnls {
 public:
  PostTradePnls(const PostTradePnls&) = delete;
  PostTradePnls& operator=(const PostTradePnls&) = delete;
  PostTradePnls(PostTradePnls&&) = delete;
  PostTradePnls& operator=(PostTradePnls&&) = delete;

  void Push(const ::openpit::accountadjustment::AccountPnlOutcome& outcome) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_post_trade_account_pnl_list_push(
            m_native, ::openpit::detail::Native(outcome), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_post_trade_account_pnl_list_push failed");
    }
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit PostTradePnls(OpenPitPostTradeAccountPnlList* native) noexcept
      : m_native(native) {}

  [[nodiscard]] OpenPitPostTradeAccountPnlList* Native() const noexcept {
    return m_native;
  }

  OpenPitPostTradeAccountPnlList* m_native = nullptr;
};

class AccountOutcomes {
 public:
  AccountOutcomes(const AccountOutcomes&) = delete;
  AccountOutcomes& operator=(const AccountOutcomes&) = delete;
  AccountOutcomes(AccountOutcomes&&) = delete;
  AccountOutcomes& operator=(AccountOutcomes&&) = delete;

  void Push(const ::openpit::accountadjustment::AccountOutcomeEntry& entry) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_account_adjustment_result_push_account_outcome(
            m_native, ::openpit::detail::Native(entry), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_pretrade_account_adjustment_result_push_account_outcome "
          "failed");
    }
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit AccountOutcomes(
      OpenPitPretradeAccountAdjustmentResult* native) noexcept
      : m_native(native) {}

  [[nodiscard]] OpenPitPretradeAccountAdjustmentResult* Native()
      const noexcept {
    return m_native;
  }

  OpenPitPretradeAccountAdjustmentResult* m_native = nullptr;
};

// Accepted result of a custom account-adjustment callback. The engine commits
// its mutations before recording every account block in `accountBlocks`.
struct PolicyAccountAdjustmentResult {
  PolicyDecision decision;
  std::vector<::openpit::accounts::AccountBlock> accountBlocks;
};

}  // namespace openpit::pretrade
