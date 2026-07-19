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

#include "openpit/detail/handle.hpp"
#include "openpit/reject.hpp"

#include <openpit.h>

#include <cstddef>
#include <vector>

namespace openpit::accountadjustment {

//------------------------------------------------------------------------------
// BatchError

namespace detail {

struct BatchErrorDeleter {
  void operator()(OpenPitAccountAdjustmentBatchError* handle) const noexcept {
    openpit_destroy_account_adjustment_batch_error(handle);
  }
};

}  // namespace detail

// Owning RAII wrapper over a caller-owned `OpenPitAccountAdjustmentBatchError`
// returned by an apply call when a policy rejects the batch. A rejected batch
// is an expected business outcome, so this is a value type, never thrown.
// `FailedAdjustmentIndex()` is the position of the offending adjustment in the
// applied array; `Rejects()` copies the policy rejects that caused it.
// Move-only.
class BatchError {
 public:
  BatchError() noexcept = default;

  explicit BatchError(OpenPitAccountAdjustmentBatchError* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  [[nodiscard]] OpenPitAccountAdjustmentBatchError* Get() const noexcept {
    return m_handle.Get();
  }

  // Index of the failing adjustment within the applied batch.
  [[nodiscard]] std::size_t FailedAdjustmentIndex() const noexcept {
    if (!m_handle) {
      return 0;
    }
    return openpit_account_adjustment_batch_error_get_failed_adjustment_index(
        m_handle.Get());
  }

  // Copies the policy rejects carried by this batch error. The rejects borrow
  // string memory from the batch error only during the copy; the returned
  // values own their strings.
  [[nodiscard]] std::vector<::openpit::reject::Reject> Rejects() const {
    std::vector<::openpit::reject::Reject> out;
    if (!m_handle) {
      return out;
    }
    const OpenPitPretradeRejectList* list =
        openpit_account_adjustment_batch_error_get_rejects(m_handle.Get());
    if (list == nullptr) {
      return out;
    }
    const std::size_t count = openpit_pretrade_reject_list_len(list);
    out.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
      OpenPitPretradeReject raw{};
      if (openpit_pretrade_reject_list_get(list, i, &raw)) {
        out.push_back(::openpit::reject::Reject::FromRaw(raw));
      }
    }
    return out;
  }

 private:
  ::openpit::detail::Handle<OpenPitAccountAdjustmentBatchError,
                            detail::BatchErrorDeleter>
      m_handle;
};

}  // namespace openpit::accountadjustment
