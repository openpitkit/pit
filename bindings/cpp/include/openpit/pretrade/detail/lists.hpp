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

#include "openpit/accounts/accounts.hpp"
#include "openpit/detail/handle.hpp"
#include "openpit/detail/native_access.hpp"
#include "openpit/pretrade/decision.hpp"

#include <openpit.h>

#include <cstddef>
#include <vector>

namespace openpit::pretrade::detail {

struct RejectListDeleter {
  void operator()(OpenPitPretradeRejectList* list) const noexcept {
    openpit_pretrade_destroy_reject_list(list);
  }
};

struct AccountBlockListDeleter {
  void operator()(OpenPitPretradeAccountBlockList* list) const noexcept {
    openpit_pretrade_destroy_account_block_list(list);
  }
};

// These ABI lists are transient caller-owned snapshots. Materialize them at
// the boundary so the public result has ordinary C++ value semantics. Outcome
// lists remain lazy because their native handle is itself a public owned value.
class ListAccess final {
 private:
  [[nodiscard]] static std::vector<::openpit::pretrade::Reject> DrainRejects(
      OpenPitPretradeRejectList* list) {
    ::openpit::detail::Handle<OpenPitPretradeRejectList, RejectListDeleter>
        owner(list);
    std::vector<::openpit::pretrade::Reject> rejects;
    const std::size_t count = openpit_pretrade_reject_list_len(owner.Get());
    rejects.reserve(count);
    for (std::size_t index = 0; index < count; ++index) {
      OpenPitPretradeReject raw{};
      if (openpit_pretrade_reject_list_get(owner.Get(), index, &raw)) {
        rejects.push_back(
            ::openpit::detail::FromNative<::openpit::pretrade::Reject>(raw));
      }
    }
    return rejects;
  }

  [[nodiscard]] static std::vector<::openpit::accounts::AccountBlock>
  DrainAccountBlocks(OpenPitPretradeAccountBlockList* list) {
    ::openpit::detail::Handle<OpenPitPretradeAccountBlockList,
                              AccountBlockListDeleter>
        owner(list);
    std::vector<::openpit::accounts::AccountBlock> blocks;
    const std::size_t count =
        openpit_pretrade_account_block_list_len(owner.Get());
    blocks.reserve(count);
    for (std::size_t index = 0; index < count; ++index) {
      OpenPitPretradeAccountBlock raw{};
      if (openpit_pretrade_account_block_list_get(owner.Get(), index, &raw)) {
        blocks.push_back(
            ::openpit::detail::FromNative<::openpit::accounts::AccountBlock>(
                raw));
      }
    }
    return blocks;
  }

  friend class ::openpit::Engine;
  friend class ::openpit::Configurator;
  friend class ::openpit::pretrade::DryRunReport;
  friend class ::openpit::pretrade::Request;
  friend class ::openpit::pretrade::policies::
      SpotFundsPnlBoundsGlobalBarrierUpdate;
  friend class ::openpit::pretrade::policies::
      SpotFundsPnlBoundsKillSwitchPolicy;
  friend class ::openpit::pretrade::policies::SpotFundsPolicy;
};

}  // namespace openpit::pretrade::detail
