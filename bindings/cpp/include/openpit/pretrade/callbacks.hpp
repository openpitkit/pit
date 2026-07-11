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

#include "openpit/account_adjustment.hpp"
#include "openpit/accounts.hpp"
#include "openpit/error.hpp"
#include "openpit/param.hpp"

#include <openpit.h>

#include <optional>

// Non-owning collectors and contexts passed to custom-policy callbacks. Every
// wrapper is valid only for the duration of the callback that created it.

namespace openpit::accountadjustment {

class Context {
 public:
  explicit Context(const OpenPitAccountAdjustmentContext* native) noexcept
      : m_native(native) {}

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
    return ::openpit::accounts::AccountControl(control);
  }

  [[nodiscard]] std::optional<::openpit::param::AccountGroupId> AccountGroup()
      const {
    OpenPitParamAccountGroupId group = 0;
    if (!openpit_account_adjustment_context_get_account_group(m_native,
                                                              &group)) {
      return std::nullopt;
    }
    return ::openpit::param::AccountGroupId::FromRaw(group);
  }

  [[nodiscard]] const OpenPitAccountAdjustmentContext* Native() const noexcept {
    return m_native;
  }

 private:
  const OpenPitAccountAdjustmentContext* m_native = nullptr;
};

}  // namespace openpit::accountadjustment

namespace openpit::pretrade {

class Result {
 public:
  explicit Result(OpenPitPretradePreTradeResult* native) noexcept
      : m_native(native) {}

  Result(const Result&) = delete;
  Result& operator=(const Result&) = delete;
  Result(Result&&) = delete;
  Result& operator=(Result&&) = delete;

  void PushLockPrice(const ::openpit::param::Price& price) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_pre_trade_result_push_lock_price(
            m_native, price.Raw(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_pre_trade_result_push_lock_price failed");
    }
  }

  void PushAccountAdjustment(
      const ::openpit::accountadjustment::AccountOutcomeEntry& entry) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_pre_trade_result_push_account_adjustment(
            m_native, entry.Raw(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          "openpit_pretrade_pre_trade_result_push_account_adjustment failed");
    }
  }

  [[nodiscard]] OpenPitPretradePreTradeResult* Native() const noexcept {
    return m_native;
  }

 private:
  OpenPitPretradePreTradeResult* m_native = nullptr;
};

class PostTradeContext {
 public:
  explicit PostTradeContext(const OpenPitPostTradeContext* native) noexcept
      : m_native(native) {}

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
    return ::openpit::param::AccountGroupId::FromRaw(group);
  }

  [[nodiscard]] const OpenPitPostTradeContext* Native() const noexcept {
    return m_native;
  }

 private:
  const OpenPitPostTradeContext* m_native = nullptr;
};

class PostTradeAdjustments {
 public:
  explicit PostTradeAdjustments(OpenPitPostTradeAdjustmentList* native) noexcept
      : m_native(native) {}

  PostTradeAdjustments(const PostTradeAdjustments&) = delete;
  PostTradeAdjustments& operator=(const PostTradeAdjustments&) = delete;
  PostTradeAdjustments(PostTradeAdjustments&&) = delete;
  PostTradeAdjustments& operator=(PostTradeAdjustments&&) = delete;

  void Push(::openpit::param::GroupId policyGroupId,
            const ::openpit::accountadjustment::AccountOutcomeEntry& entry) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_pretrade_post_trade_adjustment_list_push(
            m_native, policyGroupId.Raw(), entry.Raw(), &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_pretrade_post_trade_adjustment_list_push failed");
    }
  }

  [[nodiscard]] OpenPitPostTradeAdjustmentList* Native() const noexcept {
    return m_native;
  }

 private:
  OpenPitPostTradeAdjustmentList* m_native = nullptr;
};

class AccountOutcomes {
 public:
  explicit AccountOutcomes(OpenPitAccountOutcomeEntryList* native) noexcept
      : m_native(native) {}

  AccountOutcomes(const AccountOutcomes&) = delete;
  AccountOutcomes& operator=(const AccountOutcomes&) = delete;
  AccountOutcomes(AccountOutcomes&&) = delete;
  AccountOutcomes& operator=(AccountOutcomes&&) = delete;

  void Push(const ::openpit::accountadjustment::AccountOutcomeEntry& entry) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_account_outcome_entry_list_push(m_native, entry.Raw(),
                                                 &error)) {
      ::openpit::detail::ThrowFromSharedString(
          error, "openpit_account_outcome_entry_list_push failed");
    }
  }

  [[nodiscard]] OpenPitAccountOutcomeEntryList* Native() const noexcept {
    return m_native;
  }

 private:
  OpenPitAccountOutcomeEntryList* m_native = nullptr;
};

}  // namespace openpit::pretrade
