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
using RawDropCopyOperation = ::OpenPitPretradeDropCopyOperation;

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
    // captured exception; explicit Rollback() reports it to the caller. The
    // engine kill switch a failed finalizer arms is not suppressed: it still
    // blocks every account, and only that block reports the failure here.
    ::openpit::detail::CallbackExceptionScope callbackExceptions;
    openpit_destroy_pretrade_pre_trade_reservation(handle);
  }
};

struct PreTradeDryRunReportDeleter {
  void operator()(RawDryRunReport* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_dry_run_report(handle);
  }
};

struct DropCopyOperationDeleter {
  void operator()(RawDropCopyOperation* handle) const noexcept {
    // An unresolved operation rolls back during destruction. Destructors cannot
    // report user callback failures, so suppress only that rollback's captured
    // exception; explicit Rollback() reports it to the caller. The engine kill
    // switch a failed finalizer arms is not suppressed: it still blocks every
    // account, and only that block reports the failure here.
    ::openpit::detail::CallbackExceptionScope callbackExceptions;
    openpit_destroy_pretrade_drop_copy_operation(handle);
  }
};

}  // namespace openpit::pretrade::detail
