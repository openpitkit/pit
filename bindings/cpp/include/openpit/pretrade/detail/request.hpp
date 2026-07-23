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

#include "openpit/detail/callback_error.hpp"
#include "openpit/detail/handle.hpp"

#include <openpit.h>

#include <memory>

namespace openpit {
class Order;
}  // namespace openpit

namespace openpit::pretrade::detail {

using RawRequest = ::OpenPitPretradePreTradeRequest;
using RawReservation = ::OpenPitPretradePreTradeReservation;
using RawDryRunReport = ::OpenPitPretradePreTradeDryRunReport;

struct RequestInit {
  RawRequest* handle;
  std::shared_ptr<const ::openpit::Order> order;
};

struct PreTradeRequestDeleter {
  void operator()(RawRequest* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_request(handle);
  }
};

struct PreTradeReservationDeleter {
  void operator()(RawReservation* handle) const noexcept {
    // An unresolved reservation rolls back during destruction. Destructors
    // cannot report user callback failures, so suppress only that rollback's
    // captured exception; explicit Rollback() reports it to the caller.
    ::openpit::detail::ClearPendingCallbackException();
    openpit_destroy_pretrade_pre_trade_reservation(handle);
    ::openpit::detail::ClearPendingCallbackException();
  }
};

struct PreTradeDryRunReportDeleter {
  void operator()(RawDryRunReport* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_dry_run_report(handle);
  }
};

}  // namespace openpit::pretrade::detail
