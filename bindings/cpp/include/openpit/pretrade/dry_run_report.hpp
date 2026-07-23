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
#include "openpit/detail/native_access.hpp"
#include "openpit/error.hpp"
#include "openpit/pretrade/detail/lists.hpp"
#include "openpit/pretrade/detail/request.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"

#include <openpit.h>

#include <optional>
#include <vector>

namespace openpit::pretrade {

// Owning dry-run report. A dry-run never mutates engine state; the report is a
// detached snapshot of the verdict and any would-be lock, account adjustments,
// and account blocks.
class DryRunReport {
 public:
  DryRunReport() = default;

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  [[nodiscard]] bool Passed() const {
    return openpit_pretrade_pre_trade_dry_run_report_is_pass(RequireHandle());
  }

  [[nodiscard]] std::vector<::openpit::pretrade::Reject> Rejects() const {
    return detail::ListAccess::DrainRejects(
        openpit_pretrade_pre_trade_dry_run_report_get_rejects(RequireHandle()));
  }

  [[nodiscard]] ::openpit::pretrade::PreTradeLock Lock() const {
    return ::openpit::detail::FromNative<::openpit::pretrade::PreTradeLock>(
        openpit_pretrade_pre_trade_dry_run_report_get_lock(RequireHandle()));
  }

  [[nodiscard]] std::vector<::openpit::accountadjustment::Outcome>
  AccountAdjustments() const {
    const auto outcomes = ::openpit::detail::FromNative<
        ::openpit::accountadjustment::OutcomeList>(
        openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(
            RequireHandle()));
    return outcomes.ToVector();
  }

  /// Returns the winning account block the dry-run would latch.
  [[nodiscard]] std::optional<::openpit::accounts::AccountBlock> AccountBlock()
      const {
    OpenPitPretradeAccountBlockList* blocks =
        openpit_pretrade_pre_trade_dry_run_report_get_account_block(
            RequireHandle());
    ::openpit::detail::Handle<OpenPitPretradeAccountBlockList,
                              detail::AccountBlockListDeleter>
        owner(blocks);
    if (blocks == nullptr) {
      return std::nullopt;
    }
    OpenPitPretradeAccountBlock raw{};
    if (!openpit_pretrade_account_block_list_get(owner.Get(), 0, &raw)) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<::openpit::accounts::AccountBlock>(
        raw);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit DryRunReport(detail::RawDryRunReport* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] detail::RawDryRunReport* Native() const noexcept {
    return m_handle.Get();
  }

  [[nodiscard]] detail::RawDryRunReport* RequireHandle() const {
    if (!m_handle) {
      throw ::openpit::Error("pre-trade dry-run report is empty");
    }
    return m_handle.Get();
  }

  ::openpit::detail::Handle<detail::RawDryRunReport,
                            detail::PreTradeDryRunReportDeleter>
      m_handle;
};

}  // namespace openpit::pretrade
